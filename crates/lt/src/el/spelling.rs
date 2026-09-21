//! Greek spelling rule (`MorfologikGreekSpellerRule`): the base
//! `MorfologikSpellerRule` over `el/hunspell/el_GR.dict` (ISO-8859-7) with
//! `isLatinScript() = false` (any token without a Unicode letter is ignored)
//! and no other language override.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_EL_GR";

pub type GreekSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<GreekSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "el",
            dict_stem: "el_GR",
            rule_id: RULE_ID,
            description: "Πιθανό ορθογραφικό λάθος",
            message: "Βρέθηκε πιθανό ορθογραφικό λάθος",
            short_message: "Ορθογραφικό λάθος",
            category_id: "TYPOS",
            category_name: "Πιθανό λάθος",
            is_latin_script: false,
        },
    )
}
