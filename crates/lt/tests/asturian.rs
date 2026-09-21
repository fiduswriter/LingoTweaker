//! Asturian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule and speller values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/ast/check-diff-ast.sh`);
//! the probes use real Asturian orthography and the `l’` apostrophe.
//! `Asturian` has no disambiguator/synthesizer; the `AsturianTagger` reads
//! the old CFSA (`0xc5`) Morfologik dictionary.

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
    Engine::builder(Lang::Ast).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ast)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ast) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ast.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 71 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn asturian_engine_state() {
    let _guard = engine_guard();
    let Some(ast) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(ast.active_rule_count(), 71);
    assert_eq!(ast.skipped_counts().filters, 0);
    assert!(
        ast.compile_failures().is_empty(),
        "{:?}",
        ast.compile_failures()
    );
}

/// `A_CONTR[1]` XML rule -> `al`.
#[test]
fn asturian_xml_a_contr() {
    let _guard = engine_guard();
    let text = "Foi a el cine cola so hermana.";
    let matches = one(text, "A_CONTR");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (4, 8));
    assert_eq!(suggestions(&matches[0]), vec!["al"]);
}

/// `A_CONTR[2]` XML rule -> `al entamu`.
#[test]
fn asturian_xml_a_contr_apostrophe() {
    let _guard = engine_guard();
    let text = "Tienes que lo dicir a l’entamu.";
    let matches = one(text, "A_CONTR");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (20, 30));
    assert_eq!(suggestions(&matches[0]), vec!["al entamu"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`) -> `,`.
#[test]
fn asturian_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Voi , y depués vengo.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (3, 5));
    assert_eq!(
        matches[0].message,
        "Pon un espaciu depués de la coma, pero non enantes"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION` -> `.` with suggestions `.|…`.
#[test]
fn asturian_double_punctuation() {
    let _guard = engine_guard();
    let text = "Eso ye mui bien.. Pero nun vieno.";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (15, 17));
    assert_eq!(matches[0].message, "Dos puntos siguíos");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` -> `Esti`.
#[test]
fn asturian_uppercase_start() {
    let _guard = engine_guard();
    let text = "esti ye un test.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 4));
    assert_eq!(matches[0].message, "Esta frase nun entama con mayúscules");
    assert_eq!(suggestions(&matches[0]), vec!["Esti"]);
}

/// `WHITESPACE_RULE`.
#[test]
fn asturian_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Voi  a la casa.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (3, 5));
    assert_eq!(matches[0].message, "Posible error: repitisti un espaciu");
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `UNPAIRED_BRACKETS` -> `(`.
#[test]
fn asturian_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Esti ye un test.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Símbolu despareyáu: paez que falta \")\""
    );
}

/// `MorfologikAsturianSpellerRule` (`MORFOLOGIK_RULE_AST`):
/// `esti ye un test.` flags `test` and pins the full Java suggestion list
/// (which restores `testu`).
#[test]
fn asturian_speller_suggestions() {
    let _guard = engine_guard();
    let text = "esti ye un test.";
    let matches = one(text, "MORFOLOGIK_RULE_AST");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (11, 15));
    assert_eq!(matches[0].message, "Atopáu un posible fallu d'ortografía");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "tes", "tesa", "tese", "teso", "testu", "testé", "testó", "tesu", "tesé", "tesó",
            "tex", "texa", "texe", "texi", "texo", "texte", "texu", "texí", "texó", "tiesa",
            "tieso", "tiesta", "tieste", "tiesto", "tiestu", "tiesu", "tés"
        ]
    );
}

/// Correct Asturian text is clean.
#[test]
fn asturian_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(ast) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = ast
        .check("La xente ta mui contenta cola fiesta.")
        .expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
