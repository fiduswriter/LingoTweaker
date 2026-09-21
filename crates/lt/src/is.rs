//! Icelandic pipeline parts: the `HunspellNoSuggestionRule` speller and the
//! generic core rules. `Icelandic` has no default tagger (`Language`'s
//! `DemoTagger`), no synthesis and no disambiguator (`DemoDisambiguator` is a
//! no-op), so the analyzed sentence is the surface tokenization only.

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod spelling;

/// `Icelandic` pipeline parts.
pub struct IcelandicPipeline {
    /// `HunspellNoSuggestionRule` (`HUNSPELL_NO_SUGGEST_RULE`): flags
    /// misspellings without suggestions.
    pub spelling: Option<Arc<crate::is::spelling::IcelandicSpellingRule>>,
    /// `WordRepeatRule` (`WORD_REPEAT_RULE`), the generic built-in.
    pub word_repeat: crate::word_repeat::WordRepeatRule,
}

impl IcelandicPipeline {
    /// `Icelandic.createDefaultDisambiguator` is the base `DemoDisambiguator`
    /// (a no-op).
    pub fn disambiguate(&self, _sentence: &mut AnalyzedSentence) {}
}

/// `WordRepeatRule` (`WORD_REPEAT_RULE`) with the `MessagesBundle_is` strings.
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Endurtekið orð (t.d. 'mun mun')",
        message: "Hugsanleg ritvilla: orð endurtekið",
        short_message: "Endurtekið orð",
        category_name: "Ýmislegt",
    })
}
