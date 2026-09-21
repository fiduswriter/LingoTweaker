//! Belarusian pipeline parts: the `DemoTagger` (every token is untagged, i.e.
//! surface-only tokenization), the `BelarusianWordTokenizer` (apostrophes stay
//! inside the word, `’` normalized to `'`) and the
//! `MorfologikBelarusianSpellerRule`. `Belarusian` does not override
//! `createDefaultDisambiguator`, so the base no-op `DemoDisambiguator` applies;
//! it has no synthesizer. The XML rules reference no `<filter>` class, and the
//! module has **no** `GenericUnpairedBracketsRule`.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

pub mod priorities;
pub mod rules;
pub mod spelling;

/// Belarusian pipeline parts (base no-op disambiguator, `DemoTagger`).
pub struct BelarusianPipeline {
    /// `MorfologikBelarusianSpellerRule` (`MORFOLOGIK_RULE_BE_BY`). `None`
    /// only when the vendored `be_BY` dictionary cannot be read.
    pub spelling: Option<Arc<crate::be::spelling::BelarusianSpellingRule>>,
}

impl BelarusianPipeline {
    pub fn disambiguate(&self, _sentence: &mut AnalyzedSentence) {}
}

/// Belarusian sentence tokenization (`BelarusianWordTokenizer`) with the
/// `DemoTagger` (all tokens untagged).
///
/// The tokenizer normalizes `’` to `'` (one character for one character) but
/// `’` is 3 UTF-8 bytes and `'` is 1, so the tokens are mapped back onto the
/// original text by characters to keep the byte offsets correct (like the
/// Breton/Catalan pipelines).
pub fn analyze_belarusian_sentence(text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::BelarusianWordTokenizer::new().tokenize(text);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(raw_tokens.len() + 1);
    tokens.push(AnalyzedTokenReadings {
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
    });

    let char_bytes: Vec<(usize, usize)> = {
        let mut ranges = Vec::with_capacity(text.len());
        let mut idx = 0usize;
        for ch in text.chars() {
            ranges.push((idx, idx + ch.len_utf8()));
            idx += ch.len_utf8();
        }
        ranges
    };
    let mut char_cursor = 0usize;
    let mut use_cursor = true;
    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for raw in raw_tokens {
        let is_whitespace = lt_core::is_whitespace(&raw);
        let n_chars = raw.chars().count();
        let (start_pos, raw_byte_len) =
            if use_cursor && n_chars > 0 && char_cursor + n_chars <= char_bytes.len() {
                let start = char_bytes[char_cursor].0;
                let end = char_bytes[char_cursor + n_chars - 1].1;
                char_cursor += n_chars;
                (start, end - start)
            } else {
                use_cursor = false;
                (byte_pos, raw.len())
            };
        let has_typographic_apostrophe = n_chars > 1 && raw.contains('’');
        tokens.push(AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(raw.clone(), None, None)],
            chunk_tags: Vec::new(),
            whitespace_before: prev_was_whitespace,
            start_pos,
            raw_byte_len,
            is_whitespace,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: false,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe,
            is_pos_tag_unknown: !is_whitespace,
        });
        if !is_whitespace {
            last_non_ws_idx = Some(tokens.len() - 1);
        }
        byte_pos += raw.len();
        prev_was_whitespace = is_whitespace;
    }

    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("SENT_END"))
        {
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
        }
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        if !tr.is_sentence_end {
            tr.add_reading(AnalyzedToken::new(
                tr.surface().to_string(),
                None,
                Some(crate::pipeline::sentence_end_tag().to_string()),
            ));
            tr.is_sentence_end = true;
        }
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
