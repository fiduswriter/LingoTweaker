//! Esperanto engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule, `DateCheckFilter` and speller values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/eo/check-diff-eo.sh`); the
//! probes use real Esperanto orthography (ĉ/ĝ/ĥ/ĵ/ŝ/ŭ).
//! `Esperanto.getRelevantRules` is the generic built-ins (including
//! `SentenceWhitespaceRule`) plus the `EsperantoTagger` and the XML
//! `DateCheckFilter`.

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
    Engine::builder(Lang::Eo).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Eo)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(eo) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    eo.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 422 active XML rules, one XML-referenced filter
/// (`DateCheckFilter`, registered) and `compile_failures()` = 0.
#[test]
fn esperanto_engine_state() {
    let _guard = engine_guard();
    let Some(eo) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(eo.active_rule_count(), 422);
    assert_eq!(eo.skipped_counts().filters, 0);
    assert!(
        eo.compile_failures().is_empty(),
        "{:?}",
        eo.compile_failures()
    );
}

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`) -> `,`.
#[test]
fn esperanto_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Mi amas ĉokoladon , kaj kafon.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (17, 19));
    assert_eq!(
        matches[0].message,
        "Enmeti spaceton post la komo, sed ne antaŭ la komo"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION` -> `.` with suggestions `.|…`.
#[test]
fn esperanto_double_punctuation() {
    let _guard = engine_guard();
    let text = "Tio estas bona.. Sed li ne venis.";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (14, 16));
    assert_eq!(matches[0].message, "Du sinsekvaj punktoj");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` -> `Tio`.
#[test]
fn esperanto_uppercase_start() {
    let _guard = engine_guard();
    let text = "tio estas bela tago.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 3));
    assert_eq!(
        matches[0].message,
        "Tiu frazo ne komenciĝas per majuskla litero"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Tio"]);
}

/// `WHITESPACE_RULE`.
#[test]
fn esperanto_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Mi  amas vin.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (2, 4));
    assert_eq!(matches[0].message, "Ebla mistajpaĵo: vi ripetis spaceton");
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `WORD_REPEAT_RULE` -> `Li`.
#[test]
fn esperanto_word_repeat() {
    let _guard = engine_guard();
    let text = "Li li iris hejmen.";
    let matches = one(text, "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 5));
    assert_eq!(matches[0].message, "Ebla mistajpaĵo: vi ripetis vorton");
    assert_eq!(suggestions(&matches[0]), vec!["Li"]);
}

/// `SENTENCE_WHITESPACE` -> ` Sed`.
#[test]
fn esperanto_sentence_whitespace() {
    let _guard = engine_guard();
    let text = "Tio estas bona.Sed li ne venis.";
    let matches = one(text, "SENTENCE_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (15, 18));
    assert_eq!(matches[0].message, "Aldoni spaceton inter frazoj");
    assert_eq!(suggestions(&matches[0]), vec![" Sed"]);
}

/// `UNPAIRED_BRACKETS` -> `(`.
#[test]
fn esperanto_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Tio estas bona.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Nekongruaj simboloj: ŝajnas, ke \")\" mankas"
    );
}

/// `DATO_TAGO` + `org.languagetool.rules.eo.DateCheckFilter`: the date
/// `28-an de Aŭgusto 2014` is a Thursday, so `Vendredon` is wrong; `Ĵaŭdon`
/// is correct and the filter rejects the match.
#[test]
fn esperanto_date_filter() {
    let _guard = engine_guard();
    let text = "Vendredon la 28-an de Aŭgusto 2014.";
    let matches = one(text, "DATO_TAGO");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 34));
    assert_eq!(
        matches[0].message,
        "La tago de la dato «Vendredon la 28-an de Aŭgusto 2014» ne estas vendredo, sed jaŭdo."
    );
    assert!(one("Ĵaŭdon la 28-an de Aŭgusto 2014.", "DATO_TAGO").is_empty());
}

/// `HunspellRule`: `Ĵaŭdon la 28-an de Aŭgusto 2014.` flags the `-an` suffix
/// without the leading dash and pins the full native suggestion list.
#[test]
fn esperanto_speller_suggestions() {
    let _guard = engine_guard();
    let text = "Ĵaŭdon la 28-an de Aŭgusto 2014.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (13, 15));
    assert_eq!(matches[0].message, "Ebla mistajpaĵo trovita");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["ano", "ana", "ane", "ani", "ian", "anu", "ajn", "en", "al", "tn", "aŭ", "aĥ"]
    );
}

/// Correct Esperanto text (ĉ/ĝ/ŭ) is clean.
#[test]
fn esperanto_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(eo) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = eo
        .check("La ĝardeno estas bela. Mi manĝis pomon kaj trinkis akvon.")
        .expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
