//! Chinese (`zh`) pipeline parts.
//!
//! `Chinese` is a plain `Language`: the `ChineseWordTokenizer` (Lindera jieba
//! segmentation over `data/zh/dictionary`) produces tokens whose POS is the
//! jieba detail field, and the `ChineseTagger` turns each into an
//! `AnalyzedToken(word, pos, null)`. There is no disambiguator, chunker,
//! synthesizer or speller. `Chinese.getRelevantRules` returns only
//! `DoublePunctuationRule` and `MultipleWhitespaceRule`.
//!
//! Sentence splitting does **not** use SRX: Java's `ChineseSentenceTokenizer`
//! wraps HanLP's `SentencesUtil.toSentenceList(text)` (shortest units, so it
//! also breaks at `，,;；` and spaces). [`split_sentences`] ports that scan.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

/// `Chinese` pipeline parts.
pub struct ChinesePipeline {
    /// Lindera segmenter over `data/zh/dictionary` (see `lt-lindera`).
    pub segmenter: Arc<lt_lindera::CjkSegmenter>,
}

/// `SentencesUtil.toSentenceList(text)` (HanLP 1.7.8, `shortest = true`),
/// returned as byte spans into `text`. The trailing break character is kept
/// (except whitespace, which is trimmed); empty segments are dropped.
pub fn split_sentences(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    let flush = |start: &mut Option<usize>, end: usize, spans: &mut Vec<(usize, usize)>| {
        if let Some(s) = start.take() {
            let raw = &text[s..end];
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let off = s + (raw.len() - raw.trim_start().len());
                spans.push((off, off + trimmed.len()));
            }
        }
    };
    let mut i = 0usize;
    while i < n {
        let (b, c) = chars[i];
        if start.is_none() {
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            start = Some(b);
        }
        end = b + c.len_utf8();
        let mut do_flush = false;
        match c {
            '.' => {
                if i + 1 < n && (chars[i + 1].1 as u32) > 128 {
                    do_flush = true;
                }
            }
            '…' => {
                if i + 1 < n && chars[i + 1].1 == '…' {
                    i += 1;
                    end = chars[i].0 + chars[i].1.len_utf8();
                    do_flush = true;
                }
            }
            '，' | ',' | ';' | '；' | ' ' | '\t' | '\u{3000}' | '。' | '!' | '！' | '?' | '？'
            | '\n' | '\r' => do_flush = true,
            _ => {}
        }
        if do_flush {
            flush(&mut start, end, &mut spans);
        }
        i += 1;
    }
    flush(&mut start, end, &mut spans);
    spans
}

fn sent_start() -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(
            "",
            None,
            Some(crate::pipeline::sentence_start_tag().to_string()),
        )],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: true,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    }
}

fn whitespace_token(surface: &str, start: usize, prev_was_ws: bool) -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(surface, None, None)],
        chunk_tags: Vec::new(),
        whitespace_before: prev_was_ws,
        start_pos: start,
        raw_byte_len: surface.len(),
        is_whitespace: true,
        is_sentence_start: false,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: false,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    }
}

fn push_whitespace(
    tokens: &mut Vec<AnalyzedTokenReadings>,
    gap: &str,
    gap_start: usize,
    prev_was_ws: &mut bool,
) {
    let mut pos = gap_start;
    for ch in gap.chars() {
        let len = ch.len_utf8();
        tokens.push(whitespace_token(
            &gap[pos - gap_start..pos - gap_start + len],
            pos,
            *prev_was_ws,
        ));
        *prev_was_ws = true;
        pos += len;
    }
}

/// Tokenize + tag one Chinese sentence like Java's `Chinese`: the
/// `ChineseWordTokenizer` (Lindera) followed by the `ChineseTagger`.
/// `ChineseTagger` sets the lemma to `null`, so the readings carry no stem.
pub fn analyze_chinese_sentence(zh: &ChinesePipeline, text: &str) -> AnalyzedSentence {
    let seg = zh.segmenter.tokenize_zh(text).unwrap_or_default();

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(seg.len() + 2);
    tokens.push(sent_start());

    let mut cursor = 0usize;
    let mut prev_was_ws = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for t in &seg {
        let start = match text[cursor..].find(t.surface.as_str()) {
            Some(rel) => cursor + rel,
            None => cursor,
        };
        if start > cursor {
            push_whitespace(&mut tokens, &text[cursor..start], cursor, &mut prev_was_ws);
        }
        tokens.push(AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(
                t.surface.clone(),
                None,
                Some(t.pos.clone()),
            )],
            chunk_tags: Vec::new(),
            whitespace_before: prev_was_ws,
            start_pos: start,
            raw_byte_len: t.surface.len(),
            is_whitespace: false,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: true,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        });
        last_non_ws_idx = Some(tokens.len() - 1);
        cursor = start + t.surface.len();
        prev_was_ws = false;
    }
    if cursor < text.len() {
        push_whitespace(&mut tokens, &text[cursor..], cursor, &mut prev_was_ws);
    }

    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        let surface = tr
            .readings
            .first()
            .map(|r| r.token.clone())
            .unwrap_or_default();
        tr.add_reading(AnalyzedToken::new(
            surface,
            None,
            Some(crate::pipeline::sentence_end_tag().to_string()),
        ));
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        tr.is_sentence_end = true;
        if tr.is_linebreak() {
            tr.set_paragraph_end();
        }
    }

    AnalyzedSentence {
        text: text.to_string(),
        offset: 0,
        tokens,
        pre_disambig_tokens: Vec::new(),
        pre_disambig_detached: Vec::new(),
    }
}
