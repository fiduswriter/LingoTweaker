//! Dutch pipeline parts: tokenizer/tagger, the `DutchHybridDisambiguator`
//! chunker order and (incrementally) the Java-coded Dutch built-in rules.
//!
//! Stage 1 (XML rules) wires the plain `BaseTagger`/`BaseSynthesizer`
//! foundations over the vendored `dutch*.dict` dictionaries and the hybrid
//! chunker-disambiguator (`core/spelling_global.txt` → `nl/multiwords.txt` →
//! XML rules). `DutchTagger`'s heuristics, `DutchWordTokenizer`, the speller
//! and the compound family follow in stages 2/3.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod compound_acceptor;
pub mod date_filters;
pub mod filters;
pub mod priorities;
pub mod rules;
pub mod spelling;
pub mod tools;

/// `DutchHybridDisambiguator`: global chunker → `nl/multiwords.txt` chunker →
/// XML rules (+ global rules). Both chunkers use `tagForNotAddingTags`
/// (`_NONE_`) and `setIgnoreSpelling(true)`.
pub struct DutchPipeline {
    /// Long code of the requested variant (`nl-NL` default, `nl-BE`);
    /// selects the variant rule directory.
    pub variant: String,
    pub tagger: Arc<lt_tagger::DutchTagger>,
    pub synthesizer: Arc<lt_tagger::DutchSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<DutchSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/spelling_global.txt", false, true,
    /// false, tagForNotAddingTags)` with `setIgnoreSpelling(true)`
    pub global_chunker: lt_disambig::MultiWordChunker,
    /// `MultiWordChunker.getInstance("/nl/multiwords.txt", true, true, false,
    /// tagForNotAddingTags)` with `setIgnoreSpelling(true)`
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikDutchSpellerRule` (`MORFOLOGIK_RULE_NL_NL`)
    pub spelling: Arc<crate::nl::spelling::DutchSpellingRule>,
    /// `DutchMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// `Dutch.getCompoundAcceptor()` (`CompoundFilter`, speller
    /// `ignorePotentiallyMisspelledWord`)
    pub compound_acceptor: Arc<crate::nl::compound_acceptor::CompoundAcceptor>,
    /// `CompoundRule` (`NL_COMPOUNDS`)
    pub compound: crate::compound::CompoundRule,
    /// `DutchWrongWordInContextRule`
    pub wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule,
    /// `WordCoherencyRule` (`NL_WORD_COHERENCY`, text level)
    pub word_coherency: crate::word_coherency::WordCoherencyRule,
    /// `SimpleReplaceRule` (`NL_SIMPLE_REPLACE`)
    pub simple_replace: crate::simple_replace::SimpleReplaceRule,
    /// `CheckCaseRule` (`NL_CHECKCASE`)
    pub check_case: crate::simple_replace::SimpleReplaceRule,
    /// `PreferredWordRule` (`NL_PREFERRED_WORD_RULE_INTERNAL` matches)
    pub preferred_word: crate::nl::rules::PreferredWordRule,
    /// `SpaceInCompoundRule` (`NL_SPACE_IN_COMPOUND_*`)
    pub space_in_compound: crate::nl::rules::SpaceInCompoundRule,
}

impl DutchPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.global_chunker.apply(sentence);
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// Adapter exposing the Dutch synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct DutchSynthesizerAdapter {
    pub synth: Arc<lt_tagger::DutchSynthesizer>,
    pub tagger: Arc<lt_tagger::DutchTagger>,
}

impl DutchSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::DutchSynthesizer {
        &self.synth
    }
}

impl Synthesizer for DutchSynthesizerAdapter {
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

/// Dutch sentence tokenization + tagger (`DutchWordTokenizer`, which keeps
/// apostrophes inside words like `oma's`).
pub fn analyze_dutch_sentence(dutch: &DutchPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::DutchWordTokenizer.tokenize(text);
    let tagged = dutch.tagger.tag(&raw_tokens);

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
    }
}
