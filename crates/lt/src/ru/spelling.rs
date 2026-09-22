//! Russian spelling rules: `MorfologikRussianSpellerRule`
//! (`MORFOLOGIK_RULE_RU_RU`, `/ru/hunspell/ru_RU.dict`) and the experimental
//! `MorfologikRussianYOSpellerRule` (`MORFOLOGIK_RULE_RU_RU_YO`,
//! `/ru/hunspell/ru_RU_yo.dict`, default off).
//!
//! Both use the KOI8-R morfologik dictionaries, `isLatinScript() = false`, the
//! `RUSSIAN_LETTERS` `ignoreToken` override (a token with anything but
//! Russian letters/hyphen/stress marks is ignored) and a
//! `filterNoSuggestWords` list.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_RU_RU";
pub const YO_RULE_ID: &str = "MORFOLOGIK_RULE_RU_RU_YO";

/// `MorfologikRussianSpellerRule.RUSSIAN_LETTERS` (full-match
/// `Matcher.matches()`): hyphen, Russian letters incl. `ё`, the combining
/// acute/grave stress marks, `ѝ`, `ʼ`, and `А-ЯЁ`.
const RUSSIAN_LETTERS: &str = "[-а-яёо\u{301}а\u{301}е\u{301}у\u{301}и\u{301}ы\u{301}э\u{301}ю\u{301}я\u{301}о\u{300}а\u{300}е\u{300}у\u{300}ѝы\u{300}э\u{300}ю\u{300}я\u{300}ʼА-ЯЁ]*";

/// `MorfologikRussianSpellerRule.lcDoNotSuggestWords`.
const RU_NO_SUGGEST: &[&str] = &["блоггер", "дрочим", "анальный", "орочем"];
/// `MorfologikRussianYOSpellerRule.lcDoNotSuggestWords`.
const RU_YO_NO_SUGGEST: &[&str] = &["блоггер", "елка", "дрочим", "анальный", "орочем"];

pub type RussianSpellingRule = MorfologikSpellingRule;
pub type RussianYOSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<RussianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "ru",
            dict_stem: "ru_RU",
            rule_id: RULE_ID,
            description: "Проверка орфографии с исправлениями",
            message: "Возможно найдена орфографическая ошибка.",
            short_message: "Орфографическая ошибка",
            category_id: "TYPOS",
            category_name: "Проверка орфографии",
            is_latin_script: false,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: RU_NO_SUGGEST,
            ignore_token_pattern: Some(RUSSIAN_LETTERS),
            ..Default::default()
        },
    )
}

pub fn load_yo(data_dir: &Path) -> Result<RussianYOSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "ru",
            dict_stem: "ru_RU_yo",
            rule_id: YO_RULE_ID,
            description: "Проверка орфографии. Только «Ё» (экспериментальное правило).",
            message: "Возможно найдена орфографическая ошибка.",
            short_message: "Орфографическая ошибка",
            category_id: "TYPOS",
            category_name: "Проверка орфографии",
            is_latin_script: false,
            ignore_tagged_words: false,
            split_on_hyphen: false,
            no_suggest_words: RU_YO_NO_SUGGEST,
            ignore_token_pattern: Some(RUSSIAN_LETTERS),
            ..Default::default()
        },
    )
}
