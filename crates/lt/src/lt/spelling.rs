//! Lithuanian spelling rule (`MorfologikLithuanianSpellerRule`): the base
//! `MorfologikSpellerRule` over `lt/hunspell/lt_LT.dict` with no language
//! override.
//!
//! The pinned upstream module references `/lt/hunspell/lt_LT.dict` but the
//! dictionary is **not shipped** in the LanguageTool checkout or in any pinned
//! Maven artifact (the language is deprecated upstream since 3.6), so the
//! legacy engine throws on every check. The engine treats the missing
//! dictionary as a disabled rule; see `attic/docs/parity/lt-rule-port.md`
//! and `docs/differences.md` #12.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_LT_LT";

pub type LithuanianSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<LithuanianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "lt",
            dict_stem: "lt_LT",
            rule_id: RULE_ID,
            description: "Galima rašybos klaida",
            message: "Rasta galima rašybos klaida.",
            short_message: "Rašybos klaida",
            category_id: "TYPOS",
            category_name: "Galima rinkimo klaida",
            is_latin_script: true,
            ignore_tagged_words: false,
            split_on_hyphen: false,
        },
    )
}
