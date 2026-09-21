//! Esperanto engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule, `DateCheckFilter` and speller values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/eo/check-diff-eo.sh`) print UTF-16 code units. Every probe
//! sentence uses real Esperanto orthography (ĉ/ĝ/ĥ/ĵ/ŝ/ŭ), so the two formats
//! differ and both are stated per case. `Esperanto.getRelevantRules` is the
//! generic built-ins (including `SentenceWhitespaceRule`) plus the
//! `EsperantoTagger` and the XML `DateCheckFilter`.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

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

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`):
/// `Mi amas ĉokoladon , kaj kafon.` Java UTF-16 17..19 / engine UTF-8
/// 18..20 (`ĉ` is one extra byte) -> `,`.
#[test]
fn esperanto_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one(
        "Mi amas ĉokoladon , kaj kafon.",
        "COMMA_PARENTHESIS_WHITESPACE",
    );
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (18, 20));
    assert_eq!(
        matches[0].message,
        "Enmeti spaceton post la komo, sed ne antaŭ la komo"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION`: `Tio estas bona.. Sed li ne venis.` Java UTF-16
/// 14..16 / engine UTF-8 14..16 (all ASCII) -> `.` with suggestions `.|…`.
#[test]
fn esperanto_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Tio estas bona.. Sed li ne venis.", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (14, 16));
    assert_eq!(matches[0].message, "Du sinsekvaj punktoj");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START`: `tio estas bela tago.` Java UTF-16 0..3 /
/// engine UTF-8 0..3 -> `tio` -> `Tio`.
#[test]
fn esperanto_uppercase_start() {
    let _guard = engine_guard();
    let matches = one("tio estas bela tago.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 3));
    assert_eq!(
        matches[0].message,
        "Tiu frazo ne komenciĝas per majuskla litero"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Tio"]);
}

/// `WHITESPACE_RULE`: `Mi  amas vin.` Java UTF-16 2..4 / engine UTF-8 2..4.
#[test]
fn esperanto_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Mi  amas vin.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (2, 4));
    assert_eq!(matches[0].message, "Ebla mistajpaĵo: vi ripetis spaceton");
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `WORD_REPEAT_RULE`: `Li li iris hejmen.` Java UTF-16 0..5 / engine UTF-8
/// 0..5 -> `Li`.
#[test]
fn esperanto_word_repeat() {
    let _guard = engine_guard();
    let matches = one("Li li iris hejmen.", "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 5));
    assert_eq!(matches[0].message, "Ebla mistajpaĵo: vi ripetis vorton");
    assert_eq!(suggestions(&matches[0]), vec!["Li"]);
}

/// `SENTENCE_WHITESPACE`: `Tio estas bona.Sed li ne venis.` Java UTF-16
/// 15..18 / engine UTF-8 15..18 -> ` Sed`.
#[test]
fn esperanto_sentence_whitespace() {
    let _guard = engine_guard();
    let matches = one("Tio estas bona.Sed li ne venis.", "SENTENCE_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (15, 18));
    assert_eq!(matches[0].message, "Aldoni spaceton inter frazoj");
    assert_eq!(suggestions(&matches[0]), vec![" Sed"]);
}

/// `UNPAIRED_BRACKETS`: `(Tio estas bona.` Java UTF-16 0..1 / engine UTF-8
/// 0..1 -> `(`.
#[test]
fn esperanto_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("(Tio estas bona.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 1));
    assert_eq!(
        matches[0].message,
        "Nekongruaj simboloj: ŝajnas, ke \")\" mankas"
    );
}

/// `DATO_TAGO` + `org.languagetool.rules.eo.DateCheckFilter`: the date
/// `28-an de Aŭgusto 2014` is a Thursday, so `Vendredon` is wrong (Java
/// UTF-16 0..34 / engine UTF-8 0..35, `ŭ` is one extra byte); `Ĵaŭdon` is
/// correct and the filter rejects the match.
#[test]
fn esperanto_date_filter() {
    let _guard = engine_guard();
    let matches = one("Vendredon la 28-an de Aŭgusto 2014.", "DATO_TAGO");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 35));
    assert_eq!(
        matches[0].message,
        "La tago de la dato «Vendredon la 28-an de Aŭgusto 2014» ne estas vendredo, sed jaŭdo."
    );
    assert!(one("Ĵaŭdon la 28-an de Aŭgusto 2014.", "DATO_TAGO").is_empty());
}

/// `HunspellRule`: `Ĵaŭdon la 28-an de Aŭgusto 2014.` flags the `-an` suffix
/// without the leading dash (Java UTF-16 13..15 / engine UTF-8 15..17) and
/// pins the full native suggestion list.
#[test]
fn esperanto_speller_suggestions() {
    let _guard = engine_guard();
    let matches = one("Ĵaŭdon la 28-an de Aŭgusto 2014.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (15, 17));
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
