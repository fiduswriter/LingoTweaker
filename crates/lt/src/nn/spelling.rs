//! Nynorsk spelling rule (`NN_SPELLER`) over the vendored `nn_NO` Hunspell
//! dictionary (LibreOffice `no/` package; word-list data CC BY 4.0 +
//! CLARIN PUB+BY, affixes GPL-2.0). Hunspell remains the spelling authority;
//! suggestions come from the ported native hunspell `suggest()` (capped at
//! five, like the other hand-authored languages).

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result};

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};

pub const RULE_ID: &str = "NN_SPELLER";

pub struct NynorskSpellingRule(InnerRule);

impl NynorskSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Stavekontroll for nynorsk",
                message: "Mogeleg skrivefeil.",
                short_message: "Skrivefeil",
                category_id: "TYPOS",
                category_name: "Mogeleg skrivefeil",
                lang_dir: "nn",
                aff: "nn_NO.aff",
                dic: "nn_NO.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: true,
                cap_native_suggestions: true,
                latin_script: true,
                strip_tashkeel: false,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    /// Dictionary membership for rules that need dictionary lookups.
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
