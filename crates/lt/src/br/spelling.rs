//! Breton spelling rule (`MorfologikBretonSpellerRule`): the base
//! `MorfologikSpellerRule` over `br/hunspell/br_FR.dict` with
//! `setIgnoreTaggedWords()` (tagged words are not spell-checked) and the
//! `tokenizingPattern()` `-` (every hyphen segment is checked separately).

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_BR_FR";

pub type BretonSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<BretonSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "br",
            dict_stem: "br_FR",
            rule_id: RULE_ID,
            description: "Fazi reizhskrivañ posupl",
            message: "Fazi reizhskrivañ posupl kavet.",
            short_message: "Fazi reizhskrivañ",
            category_id: "TYPOS",
            category_name: "Fazi bizskrivañ posupl",
            is_latin_script: true,
            ignore_tagged_words: true,
            split_on_hyphen: true,
            no_suggest_words: &[],
            ignore_token_pattern: None,
            ..Default::default()
        },
    )
}
