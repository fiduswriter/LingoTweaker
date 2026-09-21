//! Guaraní spelling rule (`GN_SPELLER`) over the vendored LibreOffice `gug`
//! Hunspell dictionary (4,216 words, GFDL-1.2-or-later, unmaintained since
//! 2016). Suggestions come from the same word list with a bounded
//! edit-distance search; the dictionary has no affix morphology, so
//! inflected/agglutinated forms are a known coverage gap.

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result};

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};

pub const RULE_ID: &str = "GN_SPELLER";

pub struct GuaraniSpellingRule(InnerRule);

impl GuaraniSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Guaraní spelling",
                message: "Possible spelling mistake.",
                short_message: "Spelling",
                category_id: "TYPOS",
                category_name: "Possible Typo",
                lang_dir: "gn",
                aff: "gug.aff",
                dic: "gug.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: false,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        self.0.check_sentence(tokens, sentence_offset)
    }
}
