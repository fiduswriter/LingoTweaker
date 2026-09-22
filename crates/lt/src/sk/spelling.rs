//! Slovak spelling rule (`MorfologikSlovakSpellerRule`): the plain base
//! `MorfologikSpellerRule` over `sk/hunspell/sk_SK.dict` with no language
//! override beyond the file name/id.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_SK_SK";

pub type SlovakSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<SlovakSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "sk",
            dict_stem: "sk_SK",
            rule_id: RULE_ID,
            description: "Pravdepodobne preklep",
            message: "Nájdený pravdepodobný preklep",
            short_message: "Preklep",
            category_id: "TYPOS",
            category_name: "Možný preklep",
            is_latin_script: true,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: &[],
            ignore_token_pattern: None,
            ..Default::default()
        },
    )
}
