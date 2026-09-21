//! Nordum spelling rule (`NDM_SPELLER`) over the generated `nrd` Hunspell
//! dictionary. Suggestions come from the small authoritative core list
//! (`nrd_core.dic`), not from the large generated dictionary.

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result};

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};

pub const RULE_ID: &str = "NDM_SPELLER";

pub struct NordumSpellingRule(InnerRule);

impl NordumSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Nordum spelling",
                message: "Possible spelling mistake.",
                short_message: "Spelling",
                category_id: "TYPOS",
                category_name: "Possible Typo",
                lang_dir: "nrd",
                aff: "nrd.aff",
                dic: "nrd.dic",
                suggestion_file: Some("nrd_core.dic"),
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: false,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    /// Dictionary membership for the context rules (possessive agreement).
    pub fn is_known(&self, word: &str) -> bool {
        self.0.is_known(word)
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        self.0.check_sentence(tokens, sentence_offset)
    }
}
