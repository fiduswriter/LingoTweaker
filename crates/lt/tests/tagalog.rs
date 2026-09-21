//! Tagalog engine tests: stage-1 XML wiring state plus Java-probed built-in
//! rule and speller values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/tl/check-diff-tl.sh`,
//! `scripts/oracle/tl/probe-rule.sh`).
//! `Tagalog` has no disambiguator/synthesizer; the `TagalogTagger` reads the
//! ISO-8859-1 `tagalog.dict` and the `TagalogWordTokenizer` splits hyphens.

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
    Engine::builder(Lang::Tl).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Tl)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(tl) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    tl.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 44 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn tagalog_engine_state() {
    let _guard = engine_guard();
    let Some(tl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(tl.active_rule_count(), 44);
    assert_eq!(tl.skipped_counts().filters, 0);
    assert!(
        tl.compile_failures().is_empty(),
        "{:?}",
        tl.compile_failures()
    );
}

/// `NG_NG` XML rule -> `ng mga`.
#[test]
fn tagalog_xml_ng_ng() {
    let _guard = engine_guard();
    let text = "Pinalakad ng ng abogado si Maria.";
    let matches = one(text, "NG_NG");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (10, 15));
    assert_eq!(
        matches[0].message,
        "Do you mean <suggestion>ng mga</suggestion>? \"ng\" can not be followed by another \"ng\"."
    );
    assert_eq!(suggestions(&matches[0]), vec!["ng mga"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (Tagalog `no_space_before_dot`).
#[test]
fn tagalog_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Kumain ako ng kanin .";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (19, 21));
    assert_eq!(
        matches[0].message,
        "Huwag lagyan ng espasyo bago ang tuldok"
    );
    assert_eq!(suggestions(&matches[0]), vec!["."]);
}

/// `DOUBLE_PUNCTUATION` (Tagalog `two_dots`).
#[test]
fn tagalog_double_punctuation() {
    let _guard = engine_guard();
    let text = "Kumain ako ..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (11, 13));
    assert_eq!(matches[0].message, "Dalawang magkasunod na tuldok");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Dalawang magkasunod na tuldok")
    );
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` (`incorrect_case`/`category_case`).
#[test]
fn tagalog_uppercase_start() {
    let _guard = engine_guard();
    let text = "kumain ako ng kanin.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 6));
    assert_eq!(
        matches[0].message,
        "Hindi nagsisimula ang pangungusap na ito sa malaking letra"
    );
    assert_eq!(matches[0].short_message.as_deref(), Some("Kapitalisasiyon"));
    assert_eq!(suggestions(&matches[0]), vec!["Kumain"]);
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`, Tagalog strings).
#[test]
fn tagalog_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Kumain  ako ng kanin.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (6, 8));
    assert_eq!(
        matches[0].message,
        "Posibleng typo: naulit mo ang whitespace"
    );
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `UNPAIRED_BRACKETS` (`GenericUnpairedBracketsRule`).
#[test]
fn tagalog_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Kumain ako ng kanin.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: ')' seems to be missing"
    );
}

/// `MorfologikTagalogSpellerRule` (`MORFOLOGIK_RULE_TL`).
#[test]
fn tagalog_speller_suggestions() {
    let _guard = engine_guard();
    let text = "Kumain ako ng zzqqx.";
    let matches = one(text, "MORFOLOGIK_RULE_TL");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (14, 19));
    assert_eq!(
        matches[0].message,
        "Posibleng may nahanap na mali sa ispeling"
    );
    assert_eq!(suggestions(&matches[0]), vec!["ZPAQ"]);
}

/// Correct Tagalog text is clean.
#[test]
fn tagalog_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(tl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = tl.check("Kumain ako ng kanin.").expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
