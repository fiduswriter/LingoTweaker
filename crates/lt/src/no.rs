//! Norwegian Bokmål (`no`) pipeline parts.
//!
//! Hand-authored language: upstream LanguageTool never had a Norwegian
//! module (only a spell-check-only dynamic language). This module grows in
//! the same stages as the ported languages: XML rules (`data/no/rules/`),
//! the Hunspell speller (`nb_NO.dic`/`.aff`) and word-list rules; the
//! `simple_replace` instances live in [`crate::no::rules`] and ride the
//! shared list-rule loop.

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod context;
pub mod priorities;
pub mod rules;
pub mod spelling;

/// Norwegian Bokmål pipeline state.
pub struct NorwegianPipeline {
    /// `XmlRuleDisambiguator` over `no/disambiguation.xml` (+ global rules)
    /// when the file exists, empty otherwise.
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `NB_SPELLER` over the vendored `nb_NO` Hunspell dictionary.
    pub spelling: Arc<crate::no::spelling::NorwegianSpellingRule>,
    /// `NB_WORD_REPETITION`.
    pub repetition: crate::word_repetition::WordRepetitionRule,
    /// Curated gender overrides for the heuristics in `context.rs`.
    pub gender_overrides: crate::no::context::GenderOverrides,
}

impl NorwegianPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}
