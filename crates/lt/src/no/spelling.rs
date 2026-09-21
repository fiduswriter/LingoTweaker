//! Norwegian Bokmål spelling rule (`NB_SPELLER`) over the vendored
//! `nb_NO` Hunspell dictionary (LibreOffice `no/`, data CC BY 4.0,
//! `.aff` GPL-2.0). Hunspell remains the spelling authority; suggestions
//! come from the one-off Morfologik build
//! `data/no/dictionaries/no.dict` (see `data/no/README.md`), because the
//! dictionary has ~700k entries, above the bounded edit-distance limit.

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result};

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};

pub const RULE_ID: &str = "NB_SPELLER";

pub struct NorwegianSpellingRule(InnerRule);

impl NorwegianSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Stavekontroll for norsk bokmål",
                message: "Mulig stavefeil.",
                short_message: "Stavefeil",
                category_id: "TYPOS",
                category_name: "Mulig skrivefeil",
                lang_dir: "no",
                aff: "nb_NO.aff",
                dic: "nb_NO.dic",
                suggestion_file: None,
                morfologik_dict: Some(("no/dictionaries/no.dict", "no/dictionaries/no.info")),
                max_suggestions: 5,
                native_suggestions: false,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    /// Dictionary membership for the context rules (gender, compounds).
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
