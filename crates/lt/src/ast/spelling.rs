//! Asturian spelling rule (`MorfologikAsturianSpellerRule`): the plain base
//! `MorfologikSpellerRule` over `ast/hunspell/ast_ES.dict` with no language
//! override beyond the file name/id.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_AST";

pub type AsturianSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<AsturianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "ast",
            dict_stem: "ast_ES",
            rule_id: RULE_ID,
            description: "Posible fallu d'ortografía",
            message: "Atopáu un posible fallu d'ortografía",
            short_message: "Fallu d'ortografía",
            category_id: "TYPOS",
            category_name: "Posible error",
            is_latin_script: true,
            ignore_tagged_words: false,
            split_on_hyphen: false,
        },
    )
}
