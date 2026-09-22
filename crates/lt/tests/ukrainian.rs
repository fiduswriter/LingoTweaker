//! Ukrainian engine tests. Stage 1 pins the XML wiring state (active rules and
//! the single unmapped `<filter>` class) and the generic `MultipleWhitespace`
//! built-in; the custom tokenizer/tagger/disambiguator, the speller and the
//! language's Java rule classes are added in stages 2/3.
//!
//! `Ukrainian` loads `grammar.xml` plus `Ukrainian.RULE_FILES`
//! (`grammar-spelling`, `grammar-grammar`, `grammar-barbarism`,
//! `grammar-style`, `grammar-punctuation`); the only XML-referenced filter is
//! `org.languagetool.rules.uk.DateCheckFilter` (ported in stage 3).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::assert_utf16;

fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Uk).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Uk)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(uk) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    uk.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 1,247 default-active XML rules, one XML-referenced filter
/// (`uk.DateCheckFilter`, ported in stage 3) and `compile_failures()` = 1.
#[test]
fn ukrainian_engine_state() {
    let _guard = engine_guard();
    let Some(uk) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(uk.active_rule_count(), 1247);
    assert_eq!(uk.skipped_counts().filters, 1);
    let failures = uk.compile_failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].0, "DATE_WEEKDAY1");
    assert!(failures[0].1.contains("DateCheckFilter"), "{failures:?}");
}

/// `MultipleWhitespaceRule` with the `MessagesBundle_uk` strings.
#[test]
fn ukrainian_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Це  тест.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Ймовірна помилка: повтор пробілу");
    assert_eq!(matches[0].description, "Повтор пробілу");
    assert_eq!(matches[0].category_id, "TYPOGRAPHY");
    assert_eq!(matches[0].category_name, "Оформлення");
    assert_utf16(text, &matches[0], (2, 4));
}

/// `MORFOLOGIK_RULE_UK_UA` over `uk/hunspell/uk_UA.dict`
/// (`scripts/oracle/uk/probe-speller.sh`): `кампутар` -> `Каптар`/`кампусам`/
/// `кампусах` (Java-probed list and UTF-16 range).
#[test]
fn ukrainian_speller_кампутар() {
    let _guard = engine_guard();
    let text = "кампутар";
    let matches = one(text, "MORFOLOGIK_RULE_UK_UA");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 8));
    assert_eq!(
        matches[0].message,
        "Знайдено потенційну орфографічну помилку."
    );
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Орфографічна помилка")
    );
    assert_eq!(matches[0].description, "Ймовірна орфографічна помилка");
    assert_eq!(matches[0].category_id, "TYPOS");
    assert_eq!(matches[0].category_name, "Можлива механічна помилка");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["Каптар", "кампусам", "кампусах"]
    );
}

/// The 2019 `dash_prefixes.txt` additional suggestions (Java-probed): the
/// dash-split form is appended after the speller suggestions.
#[test]
fn ukrainian_speller_dash_prefix_suggestions() {
    let _guard = engine_guard();
    for (text, expected_last) in [("блогерр", "блог-ерр"), ("бізнесменн", "бізнес-менн")]
    {
        let matches = one(text, "MORFOLOGIK_RULE_UK_UA");
        assert_eq!(matches.len(), 1, "{text}");
        let got = suggestions(&matches[0]);
        assert_eq!(
            got.last().map(String::as_str),
            Some(expected_last),
            "{text}: {got:?}"
        );
    }
}

/// A correctly spelled dictionary word is clean (the speller ignores tagged
/// tokens).
#[test]
fn ukrainian_speller_accepts_known_words() {
    let _guard = engine_guard();
    let Some(uk) = engine_with_rules(&["MORFOLOGIK_RULE_UK_UA"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    for text in ["автомобіль", "тест", "кінопрокат", "їжак"] {
        let result = uk.check(text).expect("check");
        assert!(result.matches.is_empty(), "{text}: {:?}", result.matches);
    }
}
