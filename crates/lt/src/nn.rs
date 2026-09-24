//! Norwegian Nynorsk (`nn`) pipeline parts.
//!
//! Hand-authored language (ISO 639-1 `nn`, Nynorsk Norwegian): the legacy
//! engine has no Nynorsk module (only a spell-check-only dynamic language
//! for Norwegian). The speller is the vendored LibreOffice `nn_NO` Hunspell
//! dictionary (Ordbanken full forms, CC BY 4.0; affixes GPL-2.0 — see
//! `data/nn/README.md` and `THIRD_PARTY_NOTICES.md`); the word-list rules
//! live in [`crate::nn::rules`]. The closest relative is the Norwegian
//! Bokmål module ([`crate::no`]), which this module mirrors.

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod priorities;
pub mod rules;
pub mod spelling;

/// Nynorsk pipeline state.
pub struct NynorskPipeline {
    /// `XmlRuleDisambiguator` over `nn/disambiguation.xml` (+ global rules)
    /// when the file exists, empty otherwise.
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `NN_SPELLER` over the vendored `nn_NO` Hunspell dictionary.
    pub spelling: Arc<crate::nn::spelling::NynorskSpellingRule>,
    /// `NN_WORD_REPETITION`.
    pub repetition: crate::word_repetition::WordRepetitionRule,
}

impl NynorskPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}
