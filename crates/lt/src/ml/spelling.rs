//! Malayalam spelling rule (`MorfologikMalayalamSpellerRule`): the plain base
//! `MorfologikSpellerRule` over `ml/hunspell/ml_IN.dict` with no language
//! override beyond the file name/id. `isLatinScript()` is not overridden, so
//! it stays `true` and Malayalam-script words are ignored by the spell checker
//! (only Latin-script tokens are checked), exactly like Java. There is no
//! `MessagesBundle_ml`, so the base English strings are used.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_ML_IN";

pub type MalayalamSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<MalayalamSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "ml",
            dict_stem: "ml_IN",
            rule_id: RULE_ID,
            description: "Possible spelling mistake",
            message: "Possible spelling mistake found.",
            short_message: "Spelling mistake",
            category_id: "TYPOS",
            category_name: "Possible Typo",
            is_latin_script: true,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: &[],
            ignore_token_pattern: None,
            ..Default::default()
        },
    )
}
