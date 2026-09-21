//! Lithuanian pipeline parts: the `DemoTagger` (every token is untagged, i.e.
//! the surface-only tokenization) and the `MorfologikLithuanianSpellerRule`
//! (`MORFOLOGIK_RULE_LT_LT`) over the vendored third-party ispell-lt
//! dictionary. `Lithuanian` does not override `createDefaultDisambiguator`, so
//! the base no-op `DemoDisambiguator` applies; it has no synthesizer and no
//! tokenizer override.

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod spelling;

/// Lithuanian pipeline parts (base no-op disambiguator, `DemoTagger`).
pub struct LithuanianPipeline {
    /// `MorfologikLithuanianSpellerRule` (`MORFOLOGIK_RULE_LT_LT`). `None`
    /// only when the vendored `lt_LT` dictionary cannot be read.
    pub spelling: Option<Arc<crate::lt::spelling::LithuanianSpellingRule>>,
}

impl LithuanianPipeline {
    pub fn disambiguate(&self, _sentence: &mut AnalyzedSentence) {}
}
