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
    assert_eq!(fa.grammar_rule_count(), 283);
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

/// `PersianCommaWhitespaceRule` (`PERSIAN_COMMA_PARENTHESIS_WHITESPACE`,
/// default off, comma `،`): a space before the Persian comma.
#[test]
fn persian_comma_whitespace() {
    let _guard = engine_guard();
    let text = "چرا ، بله";
    let matches = one(text, "PERSIAN_COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "پس از کاما فاصله بگذارید، ولی نه قبل از آن"
    );
    assert_utf16(text, &matches[0], (3, 5));
}

/// `PersianDoublePunctuationRule` (`PERSIAN_DOUBLE_PUNCTUATION`, comma `،`).
#[test]
fn persian_double_punctuation() {
    let _guard = engine_guard();
    let text = "الف،، ب";
    let matches = one(text, "PERSIAN_DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "دو کامای پشت سر هم");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("دو کامای پشت‌سرهم")
    );
    assert_utf16(text, &matches[0], (3, 4));
}

/// `PersianSpaceBeforeRule` (`FA_SPACE_BEFORE_CONJUNCTION`, default off):
/// mirrors `PersianSpaceBeforeRuleTest`.
#[test]
fn persian_space_before_conjunction() {
    let _guard = engine_guard();
    assert_eq!(one("به اینجا", "FA_SPACE_BEFORE_CONJUNCTION").len(), 1);
    assert_eq!(one("من به اینجا", "FA_SPACE_BEFORE_CONJUNCTION").len(), 0);
    assert_eq!(one("(به اینجا", "FA_SPACE_BEFORE_CONJUNCTION").len(), 0);
}

/// `SimpleReplaceRule` (`FA_SIMPLE_REPLACE`) over `fa/rules/replace.txt`.
#[test]
fn persian_simple_replace() {
    let _guard = engine_guard();
    let text = "وی حاظر به همکاری شد.";
    let matches = one(text, "FA_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "اشتباه محتمل املائی پیداشده: حاضر.");
    assert_eq!(matches[0].suggestions[0].value, "حاضر");
    assert_utf16(text, &matches[0], (3, 7));
}

/// `PersianWordRepeatRule` (`PERSIAN_WORD_REPEAT_RULE`).
#[test]
fn persian_word_repeat() {
    let _guard = engine_guard();
    let text = "این کار برای برای تو بود.";
    let matches = one(text, "PERSIAN_WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (8, 17));
    // `ignore` list: "لی لی" must not match.
    assert_eq!(
        one("من لی لی را دیدم.", "PERSIAN_WORD_REPEAT_RULE").len(),
        0
    );
}

/// `PersianWordRepeatBeginningRule` (`PERSIAN_WORD_REPEAT_BEGINNING_RULE`).
#[test]
fn persian_word_repeat_beginning() {
    let _guard = engine_guard();
    let text = "همچنین، خیابان تقریباً مسکونی است. همچنین، به افتخار یک شاعر نامگذاری شده‌است.";
    let matches = one(text, "PERSIAN_WORD_REPEAT_BEGINNING_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (35, 41));
}

/// `WordCoherencyRule` (`FA_WORD_COHERENCY`): mirrors `WordCoherencyRuleTest`.
#[test]
fn persian_word_coherency() {
    let _guard = engine_guard();
    let text = "این یک اتاق است. این یک اطاق است.";
    let matches = one(text, "FA_WORD_COHERENCY");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "'اطاق' و 'اتاق' نباید در یک جا استفاده شوند"
    );
    assert_eq!(matches[0].suggestions[0].value, "اتاق");
    assert_utf16(text, &matches[0], (24, 28));
}

/// `Bad_ZWNJ` sub-rule 1 uses Java's ASCII `\w`, so a ZWNJ after a joining
/// Persian letter (e.g. `ی` in `می‌ایستادند`) is correct and must not match;
/// the explicit non-joining letters (`و`) and punctuation still match.
/// Regression: the Rust `regex` crate's Unicode `\w` flagged correct ZWNJ.
#[test]
fn persian_bad_zwnj_is_ascii_word_only() {
    let _guard = engine_guard();
    // incorrect: ZWNJ after و (an explicit non-joining letter)
    assert_eq!(one("و‌ارد", "Bad_ZWNJ").len(), 1);
    // correct: ZWNJ after ی / س (joining letters, not in the rule's class)
    assert_eq!(one("می‌ایستادند", "Bad_ZWNJ").len(), 0);
    assert_eq!(one("سرشناس‌تر", "Bad_ZWNJ").len(), 0);
    // Latin/digit/punctuation still match through `\w`
    assert_eq!(one("test‌x", "Bad_ZWNJ").len(), 1);
}
