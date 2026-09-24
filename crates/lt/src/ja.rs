//! Japanese (`ja`) pipeline parts.
//!
//! `Japanese` is a plain `Language`: the `JapaneseWordTokenizer`
//! (Lindera Viterbi segmentation over `data/ja/dictionary`) already produces
//! `surface POS basicForm` triples, so the `JapaneseTagger` just splits them.
//! There is no disambiguator (no `ja/disambiguation.xml`), no chunker, no
//! synthesizer and no speller. `Japanese.getRelevantRules` returns only
//! `DoublePunctuationRule` and `MultipleWhitespaceRule`.
//!
//! The Lindera dictionary is read through `lt_data::fs` (patched
//! `lindera-dictionary`, see `vendor/`), so it loads from an in-memory pack on
//! wasm. Unlike net.java.sen, both segmenters drop whitespace tokens, so the
//! analyzer re-inserts whitespace runs by locating each surface in the source
//! text, keeping byte offsets exact.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

/// `Japanese` pipeline parts.
pub struct JapanesePipeline {
    /// Lindera segmenter over `data/ja/dictionary` (see `lt-lindera`).
    pub segmenter: Arc<lt_lindera::CjkSegmenter>,
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

/// Push one whitespace token per character, like LT's per-character
/// whitespace tokenization, so `MultipleWhitespaceRule` sees adjacent tokens.
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

/// Tokenize + tag one sentence like Java's `Japanese`: the
/// `JapaneseWordTokenizer` (Lindera) followed by the `JapaneseTagger`.
pub fn analyze_japanese_sentence(ja: &JapanesePipeline, text: &str) -> AnalyzedSentence {
    let seg = ja.segmenter.tokenize(text).unwrap_or_default();

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(seg.len() + 2);
    tokens.push(sent_start());

    let mut cursor = 0usize;
    let mut prev_was_ws = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for t in &seg {
        let start = match text[cursor..].find(t.surface.as_str()) {
            Some(rel) => cursor + rel,
            // Should not happen: surfaces come from this text. Fall back to the
            // running cursor so we never panic on unexpected input.
            None => cursor,
        };
        if start > cursor {
            push_whitespace(&mut tokens, &text[cursor..start], cursor, &mut prev_was_ws);
        }
        tokens.push(AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(
                t.surface.clone(),
                Some(t.basic_form.clone()),
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
        let lemma = tr.readings.first().and_then(|r| r.stem.clone());
        tr.add_reading(AnalyzedToken::new(
            surface,
            lemma,
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
