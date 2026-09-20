//! Dictionary charset support for the Morfologik `.info`
//! `fsa.dict.encoding` property.
//!
//! All current LT dictionaries are UTF-8 except the Italian tagger and
//! speller dictionaries, which are ISO-8859-15 (Latin-9). Morfologik's
//! `DictionaryLookup` encodes the query word with the metadata charset and
//! decodes hits the same way, so the tagger/speller must not assume UTF-8
//! bytes (D-100).

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    #[default]
    Utf8,
    /// ISO-8859-15 (Latin-9): Latin-1 with 8 replaced code points.
    Iso885915,
    /// ISO-8859-1 (Latin-1).
    Iso88591,
}

/// The 8 code points where ISO-8859-15 differs from ISO-8859-1.
const LATIN9_OVERRIDES: [(u8, char); 8] = [
    (0xA4, '\u{20AC}'), // €
    (0xA6, '\u{0160}'), // Š
    (0xA8, '\u{0161}'), // š
    (0xB4, '\u{017D}'), // Ž
    (0xB8, '\u{017E}'), // ž
    (0xBC, '\u{0152}'), // Œ
    (0xBD, '\u{0153}'), // œ
    (0xBE, '\u{0178}'), // Ÿ
];

impl Charset {
    pub fn from_info_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "utf-8" | "utf8" => Some(Charset::Utf8),
            "iso-8859-15" | "iso8859-15" | "latin9" | "l9" => Some(Charset::Iso885915),
            "iso-8859-1" | "iso8859-1" | "latin1" | "l1" => Some(Charset::Iso88591),
            _ => None,
        }
    }

    /// Decode FSA bytes into a string. Invalid UTF-8 falls back to a lossy
    /// decode like the previous UTF-8-only code.
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        match self {
            Charset::Utf8 => String::from_utf8_lossy(bytes),
            Charset::Iso885915 | Charset::Iso88591 => {
                let overrides = if self == Charset::Iso885915 {
                    &LATIN9_OVERRIDES[..]
                } else {
                    &[][..]
                };
                Cow::Owned(
                    bytes
                        .iter()
                        .map(|&b| match overrides.iter().find(|(byte, _)| *byte == b) {
                            Some((_, c)) => *c,
                            None => b as char,
                        })
                        .collect(),
                )
            }
        }
    }
    /// Decode the first character of a (possibly partial) byte sequence.
    /// UTF-8 returns `None` for an incomplete sequence so the caller can keep
    /// accumulating continuation bytes; single-byte charsets always decode the
    /// last byte.
    pub fn decode_one(self, bytes: &[u8]) -> Option<char> {
        match self {
            Charset::Utf8 => std::str::from_utf8(bytes)
                .ok()
                .and_then(|s| s.chars().next()),
            Charset::Iso885915 | Charset::Iso88591 => bytes
                .last()
                .map(|&b| self.decode(&[b]).chars().next().unwrap_or('?')),
        }
    }

    /// Encode a string into FSA bytes. `None` when a character cannot be
    /// mapped to the charset: morfologik's `DictionaryLookup` treats that as
    /// "the dictionary cannot contain this word" and returns no hits, so the
    /// caller must do the same.
    pub fn encode(self, text: &str) -> Option<Vec<u8>> {
        match self {
            Charset::Utf8 => Some(text.as_bytes().to_vec()),
            Charset::Iso885915 | Charset::Iso88591 => {
                let overrides = if self == Charset::Iso885915 {
                    &LATIN9_OVERRIDES[..]
                } else {
                    &[][..]
                };
                let mut out = Vec::with_capacity(text.len());
                for c in text.chars() {
                    if (c as u32) < 0x80 || (0xA0..=0xFF).contains(&(c as u32)) {
                        out.push(c as u8);
                    } else if let Some((byte, _)) = overrides.iter().find(|(_, ch)| *ch == c) {
                        out.push(*byte);
                    } else {
                        return None;
                    }
                }
                Some(out)
            }
        }
    }
}
