//! Slovenian spelling rule (`MorfologikSlovenianSpellerRule`): the plain base
//! `MorfologikSpellerRule` over `sl/hunspell/sl_SI.dict` with no language
//! override beyond the file name/id.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_SL_SI";

pub type SlovenianSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<SlovenianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "sl",
            dict_stem: "sl_SI",
            rule_id: RULE_ID,
            description: "Morebitna napaka pri črkovanju",
            message: "Najdena morebitna napaka pri črkovanju.",
            short_message: "Napaka pri črkovanju",
            category_id: "TYPOS",
            category_name: "Možna tipkarska napaka",
        },
    )
}
