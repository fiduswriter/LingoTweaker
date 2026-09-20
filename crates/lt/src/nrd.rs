//! Nordum (`nrd`) pipeline parts.
//!
//! Hand-authored constructed language (<https://www.nordum.org>): upstream
//! LanguageTool has no Nordum module. The speller is a Hunspell dictionary
//! generated from the authoritative Nordum word list plus the Norwegian,
//! Danish and Swedish dictionaries with the Nordum orthographic rules
//! applied (see `tools/nordum-dict/`); the word-list rules live in
//! [`crate::nrd::rules`].

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod context;
pub mod priorities;
pub mod rules;
pub mod spelling;

/// Nordum pipeline state.
pub struct NordumPipeline {
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
