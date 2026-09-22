//! Ukrainian pipeline parts: the `UkrainianTagger`/`UkrainianSynthesizer` over
//! the vendored Morfologik dictionaries and the `XmlRuleDisambiguator` order.
//!
//! Stage 1 (XML rules) wires the tagger/synthesizer foundation and the
//! `uk/disambiguation.xml` rules; the custom `UkrainianWordTokenizer`, the
//! `UkrainianHybridDisambiguator`, the Morfologik speller and the Java rule
//! classes follow in stages 2/3.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod disambig;
pub mod filters;
pub mod spelling;

/// `Ukrainian.createDefaultDisambiguator` is
/// `UkrainianHybridDisambiguator`: `SimpleDisambiguator` + the multiword
/// chunker + the XML rules (the remaining hand-coded hybrid passes follow).
pub struct UkrainianPipeline {
    pub tagger: Arc<lt_tagger::UkrainianTagger>,
    pub synthesizer: Arc<lt_tagger::UkrainianSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<UkrainianSynthesizerAdapter>,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `UkrainianMultiwordChunker` (`uk/words/multiwords.txt`).
    pub chunker: lt_disambig::MultiWordChunker,
    /// `SimpleDisambiguator` (rare-form and duplicate-lemma removal).
    pub simple: crate::uk::disambig::SimpleDisambiguator,
    /// `MorfologikUkrainianSpellerRule` (`MORFOLOGIK_RULE_UK_UA`); `None` only
    /// when the vendored `uk_UA` dictionary cannot be read.
    pub spelling: Option<Arc<crate::uk::spelling::UkrainianSpellingRule>>,
}

impl UkrainianPipeline {
    /// `UkrainianHybridDisambiguator.disambiguate`: `preDisambiguate` (the
    /// `SimpleDisambiguator` part), then the chunker, then the XML rules.
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.simple.remove_rare_forms(sentence);
        self.chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// Adapter exposing the Ukrainian synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct UkrainianSynthesizerAdapter {
    pub synth: Arc<lt_tagger::UkrainianSynthesizer>,
    pub tagger: Arc<lt_tagger::UkrainianTagger>,
}

impl UkrainianSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::UkrainianSynthesizer {
        &self.synth
    }
}

impl Synthesizer for UkrainianSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synth.synthesize_plain(token, pos_tag)
    }

    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        self.synth.target_pos_tag(pos_tags, fallback)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`.
        let tagged = self.tagger.tag(std::slice::from_ref(&word.to_string()));
        tagged
            .first()
            .and_then(|tr| tr.readings.first())
            .is_some_and(|r| r.stem.is_some() || r.pos_tag.is_some())
    }
}

/// Ukrainian sentence tokenization + tagger. Stage 1 used the base word
/// tokenizer; the custom `UkrainianWordTokenizer` is now wired.
pub fn analyze_ukrainian_sentence(ukrainian: &UkrainianPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::ukrainian::tokenize(text);
    let tagged = ukrainian.tagger.tag(&raw_tokens);

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
