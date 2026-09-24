//! Lithuanian spelling rule (`MorfologikLithuanianSpellerRule`).
//!
//! The legacy module keeps the id `MORFOLOGIK_RULE_LT_LT` and points at
//! `/lt/hunspell/lt_LT.dict`, but no such dictionary is shipped in the pinned
//! checkout or in any pinned Maven artifact, so the legacy engine throws on
//! every check. By owner request we vendor a
//! third-party ispell-lt dictionary instead (`data/lt/hunspell/`, BSD-3-Clause)
//! and run the rule with the legacy id over it. The dictionary ships as a
//! Hunspell `.aff`/`.dic` pair, so the ported native Hunspell speller is the
//! spelling authority: `SET UTF-8`, the default single-character `FLAG` mode,
//! only `PFX`/`SFX` plus `REP`/`MAP` (no compound directives).
//!
//! Suggestions come from the ported native hunspell `suggest()` and are capped
//! at five (the hand-authored product behaviour).

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result};

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_LT_LT";

/// Load the rule (free function matching the other language modules).
pub fn load(data_dir: &Path) -> Result<LithuanianSpellingRule> {
    LithuanianSpellingRule::load(data_dir)
}

pub struct LithuanianSpellingRule(InnerRule);

impl LithuanianSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Galima rašybos klaida",
                message: "Rasta galima rašybos klaida.",
                short_message: "Rašybos klaida",
                category_id: "TYPOS",
                category_name: "Galima rinkimo klaida",
                lang_dir: "lt",
                aff: "lt_LT.aff",
                dic: "lt_LT.dic",
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

    /// Dictionary membership, for tests and future context rules.
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
