//! Crimean Tatar spelling rule (`MorfologikCrimeanTatarSpellerRule`): the
//! base `MorfologikSpellerRule` over `crh/hunspell/crh_UA.dict` with
//! `isLatinScript() = false` and no other override. The module has no
//! `MessagesBundle_crh`, so the core English strings apply.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_CRH_UA";

pub type CrimeanTatarSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<CrimeanTatarSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "crh",
            dict_stem: "crh_UA",
            rule_id: RULE_ID,
            description: "Possible spelling mistake",
            message: "Possible spelling mistake found.",
            short_message: "Spelling mistake",
            category_id: "TYPOS",
            category_name: "Possible Typo",
            is_latin_script: false,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: &[],
            ignore_token_pattern: None,
            ..Default::default()
        },
    )
}
