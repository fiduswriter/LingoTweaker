//! Request-local text encoding reused by all matches and sentence ranges.

use std::cell::OnceCell;

pub(crate) const CONTEXT_SIZE: i32 = 40;

struct Utf16Text {
    // Correct documents need offsets for sentence ranges but no contexts.
    units: OnceCell<Vec<u16>>,
    /// At each non-ASCII scalar's end: byte offset and cumulative difference
    /// between UTF-8 bytes and UTF-16 units. ASCII needs no offset table.
    offsets: Vec<(usize, usize)>,
}

pub(crate) struct ResponseText<'a> {
    text: &'a str,
    utf16: Option<Utf16Text>,
}

impl<'a> ResponseText<'a> {
    pub(crate) fn new(text: &'a str, utf8_offsets: bool) -> Self {
        let utf16 = (!utf8_offsets).then(|| {
            let mut offsets = Vec::new();
            if !text.is_ascii() {
                let mut difference = 0;
                for (byte, ch) in text.char_indices() {
                    if !ch.is_ascii() {
                        difference += ch.len_utf8() - ch.len_utf16();
                        offsets.push((byte + ch.len_utf8(), difference));
                    }
                }
            }
            Utf16Text {
                units: OnceCell::new(),
                offsets,
            }
        });
        Self { text, utf16 }
    }

    pub(crate) fn as_str(&self) -> &'a str {
        self.text
    }

    fn offset(&self, byte: usize) -> usize {
        let Some(utf16) = &self.utf16 else {
            return byte;
        };
        // Match lt::to_utf16_offset's clamping, including legacy invalid spans.
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        let index = utf16.offsets.partition_point(|&(end, _)| end <= byte);
        byte - index.checked_sub(1).map_or(0, |i| utf16.offsets[i].1)
    }

    pub(crate) fn range(&self, start: usize, end: usize) -> (usize, usize) {
        (self.offset(start), self.offset(end))
    }

    pub(crate) fn context(&self, start: usize, end: usize) -> (String, usize, usize) {
        if let Some(utf16) = &self.utf16 {
            let (from, to) = self.range(start, end);
            let units = utf16
                .units
                .get_or_init(|| self.text.encode_utf16().collect());
            context_window(units, from, to)
        } else {
            context_window_utf8(self.text, start, end)
        }
    }
}

/// LT ContextTools' UTF-16 window: clipping markers are part of the text and
/// its offset, newlines become spaces, and clipped surrogate pairs decode lossily.
fn context_window(units: &[u16], from: usize, to: usize) -> (String, usize, usize) {
    let start = from.saturating_sub(CONTEXT_SIZE as usize);
    let end = (to + CONTEXT_SIZE as usize).min(units.len());
    let prefix = if from < CONTEXT_SIZE as usize {
        ""
    } else {
        "..."
    };
    let postfix = if to + CONTEXT_SIZE as usize > units.len() {
        ""
    } else {
        "..."
    };
    let mut context = String::with_capacity(prefix.len() + end - start + postfix.len());
    context.push_str(prefix);
    context.extend(
        char::decode_utf16(units[start..end].iter().copied()).map(|ch| match ch {
            Ok('\n') => ' ',
            Ok(ch) => ch,
            Err(_) => char::REPLACEMENT_CHARACTER,
        }),
    );
    context.push_str(postfix);
    (context, prefix.len() + from - start, to - from)
}

/// Native v3 offsets and context use UTF-8 bytes and code-point boundaries.
fn context_window_utf8(text: &str, from: usize, to: usize) -> (String, usize, usize) {
    let char_boundary = |mut pos: usize, backwards: bool| -> usize {
        let mut remaining = CONTEXT_SIZE;
        while remaining > 0 {
            if backwards {
                if pos == 0 {
                    break;
                }
                pos -= 1;
                while !text.is_char_boundary(pos) {
                    pos -= 1;
                }
            } else {
                if pos >= text.len() {
                    break;
                }
                pos += 1;
                while pos < text.len() && !text.is_char_boundary(pos) {
                    pos += 1;
                }
            }
            remaining -= 1;
        }
        pos
    };
    let start = char_boundary(from, true);
    let end = char_boundary(to, false);
    let prefix = if start > 0 { "..." } else { "" };
    let postfix = if end < text.len() { "..." } else { "" };
    let mut context = String::with_capacity(prefix.len() + postfix.len() + end - start);
    context.push_str(prefix);
    context.push_str(&text[start..end].replace('\n', " "));
    context.push_str(postfix);
    (context, prefix.len() + from - start, to - from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn old_offset(text: &str, byte: usize) -> usize {
        let mut byte = byte.min(text.len());
        while !text.is_char_boundary(byte) {
            byte -= 1;
        }
        text[..byte].encode_utf16().count()
    }

    // ContextTools.getContext + RuleMatchesAsJsonSerializer.writeContext:
    // Java's substring is indexed in UTF-16, and both ellipses are retained
    // after the serializer removes the error marker.
    fn java_context(text: &str, from: usize, to: usize) -> (String, usize, usize) {
        let units: Vec<u16> = text.encode_utf16().collect();
        let len = units.len() as i32;
        let mut start = from as i32 - CONTEXT_SIZE;
        let mut prefix = "...";
        if start < 0 {
            prefix = "";
            start = 0;
        }
        let mut end = to as i32 + CONTEXT_SIZE;
        let mut postfix = "...";
        if end > len {
            postfix = "";
            end = len;
        }
        let context_units: Vec<u16> = units[start as usize..end as usize]
            .iter()
            .map(|&u| if u == b'\n' as u16 { b' ' as u16 } else { u })
            .collect();
        (
            format!(
                "{prefix}{}{postfix}",
                String::from_utf16_lossy(&context_units)
            ),
            prefix.len() + from - start as usize,
            to - from,
        )
    }

    #[test]
    fn offsets_match_original_at_every_byte_including_invalid_boundaries() {
        for text in ["", "ASCII\ntext", "école Ελληνικά", "😀a𐐀e\u{301}🇩🇪\r\n終"]
        {
            let view = ResponseText::new(text, false);
            assert_eq!(view.as_str(), text);
            for byte in 0..text.len() + 5 {
                assert_eq!(view.offset(byte), old_offset(text, byte), "{text:?}:{byte}");
            }
        }
    }

    #[test]
    fn every_context_matches_java_including_clipped_surrogates() {
        for text in [
            "".to_owned(),
            "a".repeat(100),
            "a\n😀é𐐀e\u{301}終\r\n".repeat(10),
        ] {
            let view = ResponseText::new(&text, false);
            let boundaries: Vec<_> = text
                .char_indices()
                .map(|(byte, _)| byte)
                .chain(std::iter::once(text.len()))
                .collect();
            for (i, &start) in boundaries.iter().enumerate() {
                for &end in &boundaries[i..] {
                    let from = old_offset(&text, start);
                    let to = old_offset(&text, end);
                    assert_eq!(view.range(start, end), (from, to));
                    assert_eq!(view.context(start, end), java_context(&text, from, to));
                }
            }
        }
    }

    #[test]
    fn native_surface_allocates_no_utf16_view() {
        let text = "😀\n".repeat(100);
        let view = ResponseText::new(&text, true);
        assert!(view.utf16.is_none());
        assert_eq!(view.range(100, 104), (100, 104));
        assert_eq!(view.context(100, 104), context_window_utf8(&text, 100, 104));
    }

    #[test]
    fn ascii_needs_no_offset_table() {
        let view = ResponseText::new("ASCII text", false);
        assert!(view.utf16.as_ref().unwrap().offsets.is_empty());
        assert_eq!(view.range(0, 10), (0, 10));
        assert!(view.utf16.as_ref().unwrap().units.get().is_none());
    }

    #[test]
    fn contexts_share_one_encoding() {
        let view = ResponseText::new("😀é a misspeling", false);
        assert!(view.utf16.as_ref().unwrap().units.get().is_none());
        view.context(0, 4);
        let first = view.utf16.as_ref().unwrap().units.get().unwrap().as_ptr();
        view.context(7, 8);
        assert_eq!(
            view.utf16.as_ref().unwrap().units.get().unwrap().as_ptr(),
            first
        );
    }

    #[test]
    fn clipping_markers_and_offsets_follow_java_at_exact_boundaries() {
        let text = "a".repeat(100);
        let view = ResponseText::new(&text, false);
        assert_eq!(view.context(0, 1), (format!("{}...", "a".repeat(41)), 0, 1));
        assert_eq!(
            view.context(40, 41),
            (format!("...{}...", "a".repeat(81)), 43, 1)
        );
        assert_eq!(
            view.context(59, 60),
            (format!("...{}...", "a".repeat(81)), 43, 1)
        );
        assert_eq!(
            view.context(99, 100),
            (format!("...{}", "a".repeat(41)), 43, 1)
        );
    }
}
