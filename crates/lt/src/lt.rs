//! Lithuanian pipeline parts: the `DemoTagger` (every token is untagged, i.e.
//! the surface-only tokenization) and the (missing-dictionary)
//! `MorfologikLithuanianSpellerRule`. `Lithuanian` does not override
//! `createDefaultDisambiguator`, so the base no-op `DemoDisambiguator`
//! applies; it has no synthesizer and no tokenizer override.
//!
//! The upstream speller dictionary `/lt/hunspell/lt_LT.dict` is not shipped
//! (see `crate::lt::spelling`), so `spelling` is normally `None`; the XML
//! rules and generic built-ins still run, and the language is gated by the
//! tests-only path.

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod spelling;

/// Lithuanian pipeline parts (base no-op disambiguator, `DemoTagger`).
pub struct LithuanianPipeline {
    /// `MorfologikLithuanianSpellerRule` (`MORFOLOGIK_RULE_LT_LT`); `None`
    /// because the upstream dictionary is not shipped.
    pub spelling: Option<Arc<crate::lt::spelling::LithuanianSpellingRule>>,
}

impl LithuanianPipeline {
    pub fn disambiguate(&self, _sentence: &mut AnalyzedSentence) {}
}
