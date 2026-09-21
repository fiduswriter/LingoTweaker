//! Tagalog spelling rule (`MorfologikTagalogSpellerRule`): the plain base
//! `MorfologikSpellerRule` over `tl/hunspell/tl_PH.dict` with no language
//! override beyond the file name/id.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_TL";

pub type TagalogSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<TagalogSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "tl",
            dict_stem: "tl_PH",
            rule_id: RULE_ID,
            description: "Posibleng pagkakamali sa ispeling",
            message: "Posibleng may nahanap na mali sa ispeling",
            short_message: "Pagkakamali sa ispeling",
            category_id: "TYPOS",
            category_name: "Posibleng Typo",
            is_latin_script: true,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: &[],
            ignore_token_pattern: None,
        },
    )
}
