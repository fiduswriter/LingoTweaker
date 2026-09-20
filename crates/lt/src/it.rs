//! Italian pipeline parts: tokenizer/tagger, the `ItalianRuleDisambiguator`
//! order and (incrementally) the Java-coded Italian built-in rules.
//!
//! `Italian.java` uses the default `WordTokenizer` (no Italian subclass), the
//! plain `BaseTagger`/`BaseSynthesizer` and the XML rule disambiguator over
//! `it/disambiguation.xml`.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;
use lt_tokenize::wordtokenizer::{
    base_tokenizing_characters, join_emails_and_urls, string_tokenize,
};

pub mod filters;
pub mod priorities;
pub mod rules;
pub mod spelling;

/// `ItalianRuleDisambiguator` (an `XmlRuleDisambiguator` over the Italian
/// XML + the global rules) with the Italian tagger/synthesizer.
pub struct ItalianPipeline {
    pub tagger: Arc<lt_tagger::ItalianTagger>,
    pub synthesizer: Arc<lt_tagger::ItalianSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<ItalianSynthesizerAdapter>,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikItalianSpellerRule` (`MORFOLOGIK_RULE_IT_IT`, rule 5)
    pub spelling: Arc<crate::it::spelling::ItalianSpellingRule>,
}

impl ItalianPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}

/// Adapter exposing the Italian synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct ItalianSynthesizerAdapter {
    pub synth: Arc<lt_tagger::ItalianSynthesizer>,
    pub tagger: Arc<lt_tagger::ItalianTagger>,
}

impl ItalianSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::ItalianSynthesizer {
        &self.synth
    }
}

impl Synthesizer for ItalianSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        self.synth.target_pos_tag(pos_tags, fallback)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`
        !self
            .tagger
            .tag_word(word)
            .into_iter()
            .next()
            .is_some_and(|r| r.stem.is_none() && r.pos_tag.is_none())
    }
}

/// Italian sentence tokenization + tagger. `Italian.java` does not override
/// `createDefaultWordTokenizer`, so the base `WordTokenizer` is used
/// (`stringTokenize` + e-mail/URL joining).
pub fn analyze_italian_sentence(italian: &ItalianPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = join_emails_and_urls(string_tokenize(text, &base_tokenizing_characters()));
    let tagged = italian.tagger.tag(&raw_tokens);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(tagged.len() + 1);
    // synthetic sentence-start entry (LT: AnalyzedToken("", "SENT_START", null))
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
        // Java `AnalyzedTokenReadings.isTagged()` counts SENT_START as a real
        // POS tag (only SENT_END/PARA_END/null are "no real tag")
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        // `BaseTagger` never sets the typographic-apostrophe flag (only the
        // en/es/ca taggers do)
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
