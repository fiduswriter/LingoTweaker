//! Breton pipeline parts: the `BretonTagger` + `BretonWordTokenizer`, the
//! plain `XmlRuleDisambiguator` (`br/disambiguation.xml`, no global rules)
//! and the `MorfologikBretonSpellerRule`. Stage 3 adds the `TopoReplaceRule`
//! (`BR_TOPO`) and the XML-referenced `DateCheckFilter`.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

pub mod filters;
pub mod spelling;
pub mod topo;

/// `Breton.createDefaultDisambiguator` is `new XmlRuleDisambiguator(new
/// Breton())`, which does NOT load `disambiguation-global.xml`
/// (`useGlobalDisambiguation = false`). Breton has no chunker.
pub struct BretonPipeline {
    pub tagger: Arc<lt_tagger::BretonTagger>,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikBretonSpellerRule` (`MORFOLOGIK_RULE_BR_FR`). `None` only
    /// when the vendored `br_FR` dictionary cannot be read.
    pub spelling: Option<Arc<crate::br::spelling::BretonSpellingRule>>,
    /// `TopoReplaceRule` (`BR_TOPO`).
    pub topo: crate::br::topo::TopoReplaceRule,
}

impl BretonPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}

/// Breton sentence tokenization (`BretonWordTokenizer`) + tagger.
pub fn analyze_breton_sentence(breton: &BretonPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::BretonWordTokenizer::new().tokenize(text);
    let tagged = breton.tagger.tag(&raw_tokens);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(tagged.len() + 1);
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

    // `BretonWordTokenizer` normalizes the apostrophes (`'` → `’`, the `c’h`
    // trigraph and the `n’` + word split) and inserts/removes synthetic
    // spaces, but every substitution is one character for one character, so
    // the concatenated token surfaces have the same *character* count as the
    // sentence. Map every token back onto the original text by characters to
    // keep the byte offsets correct (`'` is 1 UTF-8 byte, `’` is 3).
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
    for (raw, mut reading) in raw_tokens.iter().zip(tagged) {
        let is_whitespace = lt_core::is_whitespace(raw);
        reading.whitespace_before = prev_was_whitespace;
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
        reading.start_pos = start_pos;
        reading.raw_byte_len = raw_byte_len;
        reading.is_whitespace = is_whitespace;
        reading.is_tagged = reading.readings.iter().any(|r| r.pos_tag.is_some());
        tokens.push(reading);
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
            let lemma = tr.readings.first().and_then(|r| r.stem.clone());
            tr.add_reading(AnalyzedToken::new(
                surface,
                lemma,
                Some(crate::pipeline::sentence_end_tag().to_string()),
            ));
        }
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        if !tr.is_sentence_end {
            tr.add_reading(AnalyzedToken::new(
                tr.surface().to_string(),
                tr.readings.first().and_then(|r| r.stem.clone()),
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
