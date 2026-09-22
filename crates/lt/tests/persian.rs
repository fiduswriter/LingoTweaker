//! Persian engine tests: stage-1 XML wiring state plus the generic core
//! built-ins with the `MessagesBundle_fa` strings.
//!
//! The pinned `fa` module is a small, unmaintained module with no tagger,
//! disambiguator, synthesizer or speller; the Java oracle is wired in a later
//! stage. Expectations come from the shared engine foundations plus the
//! `MessagesBundle_fa` strings, and UTF-16 offsets are asserted with the
//! `common::assert_utf16` helper.

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
    Engine::builder(Lang::Fa).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Fa)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(fa) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    fa.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: the `grammar.xml` rule units compile (`compile_failures()` = 0)
/// and the inventory-reported active count.
#[test]
fn persian_engine_state() {
    let _guard = engine_guard();
    let Some(fa) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert!(
        fa.compile_failures().is_empty(),
        "{:?}",
        fa.compile_failures()
    );
    // `grammar.xml` parses to 283 rule units; three are `default="off"`
    // (`Complex_Past_Verbs`, `Complex_Present_Verbs`, `PluralFix`), so 280 are
    // active by default. `lt-cli inventory` reports the parsed 283.
    assert_eq!(fa.grammar().expect("grammar").rules.len(), 283);
    assert_eq!(fa.active_rule_count(), 280);
    assert_eq!(fa.skipped_counts().filters, 0);
    assert_eq!(fa.skipped_counts().off_by_default, 3);
}

/// The engine resolves `fa` / `fa-IR` / `fa-AF` to the same language.
#[test]
fn persian_language_metadata() {
    assert_eq!(Lang::from_long_code("fa"), Some(Lang::Fa));
    assert_eq!(Lang::from_long_code("fa-IR"), Some(Lang::Fa));
    assert_eq!(Lang::from_long_code("fa-AF"), Some(Lang::Fa));
    assert_eq!(Lang::Fa.base_code(), "fa");
    assert_eq!(Lang::Fa.info().long_code, "fa-IR");
    assert_eq!(Lang::Fa.info().name, "Persian");
}

/// Generic `CommaWhitespaceRule` (`COMMA_PARENTHESIS_WHITESPACE`) with the
/// Persian strings: a space before the full stop.
#[test]
fn persian_generic_comma_whitespace() {
    let _guard = engine_guard();
    let text = "این یک آزمایش است .";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "قبل از نقطه فاصله نگذارید");
    assert_utf16(text, &matches[0], (17, 19));
}

/// Generic `DoublePunctuationRule` (`DOUBLE_PUNCTUATION`) with the Persian
/// strings: two consecutive dots.
#[test]
fn persian_generic_double_punctuation() {
    let _guard = engine_guard();
    let text = "این یک آزمایش است..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "دو نقطهٔ پشت سر هم");
    assert_eq!(matches[0].short_message.as_deref(), Some("دو نقطهٔ پشت‌سرهم"));
}

/// Generic `MultipleWhitespaceRule` (`WHITESPACE_RULE`) with the Persian
/// strings.
#[test]
fn persian_generic_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "این  یک آزمایش است";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "اشتباه تایپی محتمل: شما فاصله را تکرار کرده‌اید"
    );
}
