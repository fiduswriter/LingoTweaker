//! Belarusian spelling rule (`MorfologikBelarusianSpellerRule`): the base
//! `MorfologikSpellerRule` over `be/hunspell/be_BY.dict` with
//! `isLatinScript() = false` and no other language override. The dictionary
//! comes from the `io.github.belarus:linguistics.grammardb.spell.languagetool`
//! artifact (CC-BY-SA-4.0, see `THIRD_PARTY_NOTICES.md`).

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_BE_BY";

pub type BelarusianSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<BelarusianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "be",
            dict_stem: "be_BY",
            rule_id: RULE_ID,
            description: "Праверка арфаграфіі (з выпраўленнямі)",
            message: "Знойдзена магчымая памылка.",
            short_message: "Арфаграфічная памылка",
            category_id: "TYPOS",
            category_name: "Магчымыя памылкі набору",
            is_latin_script: false,
            ignore_tagged_words: false,
            split_on_hyphen: false,
        },
    )
}
