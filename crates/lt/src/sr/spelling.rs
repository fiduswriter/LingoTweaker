//! Serbian spelling rule (`MorfologikEkavianSpellerRule`): the plain base
//! `MorfologikSpellerRule` over `sr/dictionaries/ekavian/serbian.dict` (the
//! same CFSA2, frequency-included dictionary the tagger reads) with the
//! `spelling.txt`/`ignored.txt` lists from the same directory. `isLatinScript`
//! is not overridden (default `true`) and there is no custom `ignoreToken`.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_SR_EKAVIAN";

pub type SerbianSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<SerbianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "sr",
            dict_stem: "serbian",
            dict_subdir: "dictionaries/ekavian",
            rule_id: RULE_ID,
            description: "Могућа грешка спеловања",
            message: "Пронађена вероватна грешка спеловања",
            short_message: "Грешка спеловања",
            category_id: "TYPOS",
            category_name: "Могућа грешка",
            is_latin_script: true,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: &[],
            ignore_token_pattern: None,
            ..Default::default()
        },
    )
}
