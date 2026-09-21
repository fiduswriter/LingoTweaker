//! Slovenian pipeline parts: the `MorfologikSlovenianSpellerRule` and the
//! generic core rules. Slovenian has no default tagger (`Language`'s
//! `DemoTagger`), no synthesis and no disambiguator (`DemoDisambiguator` is a
//! no-op), so the analyzed sentence is the surface tokenization only.

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod rules;
pub mod spelling;

/// `Slovenian` pipeline parts.
pub struct SlovenianPipeline {
    /// `MorfologikSlovenianSpellerRule` (`MORFOLOGIK_RULE_SL_SI`).
    pub spelling: Option<Arc<crate::sl::spelling::SlovenianSpellingRule>>,
    /// `WordRepeatRule` (`WORD_REPEAT_RULE`), the generic built-in.
    pub word_repeat: crate::word_repeat::WordRepeatRule,
}

impl SlovenianPipeline {
    /// `Slovenian.createDefaultDisambiguator` is the base `DemoDisambiguator`
    /// (a no-op).
    pub fn disambiguate(&self, _sentence: &mut AnalyzedSentence) {}
}
