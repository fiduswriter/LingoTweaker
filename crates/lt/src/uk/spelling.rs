//! Ukrainian spelling rule: `MorfologikUkrainianSpellerRule`
//! (`MORFOLOGIK_RULE_UK_UA`, `/uk/hunspell/uk_UA.dict`).
//!
//! Uses the shared `MorfologikSpellingRule` with the Ukrainian overrides:
//! `isLatinScript() = false`, the `UKRAINIAN_LETTERS` `ignoreToken` pattern
//! plus the initial-with-dot guard, `ignoreTaggedWords` (`hasGoodTag`), the
//! trailing-dash `isMisspelled` rule, the "potential spelling error" match when
//! no good tag exists, the 2019 `dash_prefixes.txt` additional suggestions and
//! the spaced-suggestion filter.

use std::path::Path;

use lt_core::Result;

use crate::morfologik_spelling::{MorfologikSpellerConfig, MorfologikSpellingRule};

pub const RULE_ID: &str = "MORFOLOGIK_RULE_UK_UA";

/// `MorfologikUkrainianSpellerRule.UKRAINIAN_LETTERS`: matches a word that
/// contains at least one Ukrainian letter.
const UKRAINIAN_LETTERS: &str = ".*[а-яіїєґА-ЯІЇЄҐ].*";

pub type UkrainianSpellingRule = MorfologikSpellingRule;

pub fn load(data_dir: &Path) -> Result<UkrainianSpellingRule> {
    MorfologikSpellingRule::load(
        data_dir,
        MorfologikSpellerConfig {
            lang_dir: "uk",
            dict_stem: "uk_UA",
            dict_subdir: "hunspell",
            rule_id: RULE_ID,
            description: "Ймовірна орфографічна помилка",
            message: "Знайдено потенційну орфографічну помилку.",
            short_message: "Орфографічна помилка",
            category_id: "TYPOS",
            category_name: "Можлива механічна помилка",
            is_latin_script: false,
            // `MorfologikUkrainianSpellerRule.ignoreToken` ends in
            // `hasGoodTag(tokens[idx])`.
            ignore_tagged_words: true,
            split_on_hyphen: false,
            no_suggest_words: &[],
            ignore_token_pattern: Some(UKRAINIAN_LETTERS),
            ignore_initial_with_dot: true,
            hyphen_end_misspelled: true,
            potential_spelling_error: Some((
                "Потенційна орфографічна помилка",
                "Орфографічна помилка",
            )),
            dash_prefix_suggestions: Some(load_dash_prefixes(data_dir)),
            filter_spaced_suggestions: true,
        },
    )
}

/// `MorfologikUkrainianSpellerRule` static initializer:
/// `ExtraDictionaryLoader.loadMap("/uk/dash_prefixes.txt")` (file order), then
/// remove entries whose value matches `:(bad|alt|slang)` or whose key does not
/// match `[а-яіїєґ]{3,}`.
fn load_dash_prefixes(data_dir: &Path) -> Vec<String> {
    let path = data_dir.join("uk/words/dash_prefixes.txt");
    let Ok(text) = lt_data::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut prefixes = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split(' ');
        let Some(key) = parts.next() else { continue };
        let value = parts.next().unwrap_or("");
        if value.ends_with(":bad") || value.ends_with(":alt") || value.ends_with(":slang") {
            continue;
        }
        if !is_ukrainian_three_letter_key(key) {
            continue;
        }
        prefixes.push(key.to_string());
    }
    prefixes
}

/// `key.matches("[а-яіїєґ]{3,}")`: at least three lowercase Ukrainian letters
/// and no other characters.
fn is_ukrainian_three_letter_key(key: &str) -> bool {
    let mut count = 0usize;
    for c in key.chars() {
        let lowercase = matches!(c, 'а'..='я' | 'і' | 'ї' | 'є' | 'ґ');
        if !lowercase {
            return false;
        }
        count += 1;
    }
    count >= 3
}
