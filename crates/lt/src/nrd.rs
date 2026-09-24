//! Nordum (`nrd`) pipeline parts.
//!
//! Hand-authored constructed language (<https://www.nordum.org>): the legacy
//! engine has no Nordum module. The speller is a Hunspell dictionary
//! generated from the authoritative Nordum word list plus the Norwegian,
//! Danish and Swedish dictionaries with the Nordum orthographic rules
//! applied (see `tools/nordum-dict/`); the word-list rules live in
//! [`crate::nrd::rules`].

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

pub mod context;
pub mod priorities;
pub mod rules;
pub mod spelling;

/// Nordum pipeline state.
pub struct NordumPipeline {
    /// Tagger over the generated `nrd/dictionaries/nrd.dict` Morfologik
    /// dictionary (`tools/nordum-dict/build-nordum-tagger.py`).
    pub tagger: Arc<lt_tagger::NrdTagger>,
    /// `XmlRuleDisambiguator` over `nrd/disambiguation.xml` (+ global rules)
    /// when the file exists, empty otherwise.
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `NDM_SPELLER` over the generated `nrd` Hunspell dictionary.
    pub spelling: Arc<crate::nrd::spelling::NordumSpellingRule>,
    /// `NDM_WORD_REPETITION`.
    pub repetition: crate::word_repetition::WordRepetitionRule,
}

impl NordumPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}

/// Nordum sentence tokenization + tagger. The word tokenizer is the base
/// `WordTokenizer` (Nordum overrides no word tokenizer), so the token stream
/// concatenates back to the original text exactly and the byte offsets
/// accumulate. Same shape as `analyze_danish_sentence`.
pub fn analyze_nordum_sentence(nordum: &NordumPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(
            text,
            &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
        ),
    );
    let tagged = nordum.tagger.tag(&raw_tokens);

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

    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for (raw, mut reading) in raw_tokens.iter().zip(tagged) {
        let is_whitespace = lt_core::is_whitespace(raw);
        reading.whitespace_before = prev_was_whitespace;
        reading.start_pos = byte_pos;
        reading.raw_byte_len = raw.len();
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
