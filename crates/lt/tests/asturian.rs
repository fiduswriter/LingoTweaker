//! Asturian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule and speller values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/ast/check-diff-ast.sh`) print UTF-16 code units. The
//! probes use real Asturian orthography and the `l’` apostrophe, so the two
//! formats can differ and both are stated per case. `Asturian` has no
//! disambiguator/synthesizer; the `AsturianTagger` reads the old CFSA
//! (`0xc5`) Morfologik dictionary.

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

/// `A_CONTR[1]` XML rule: `Foi a el cine cola so hermana.` Java UTF-16 4..8 /
/// engine UTF-8 4..8 -> `al`.
#[test]
fn asturian_xml_a_contr() {
    let _guard = engine_guard();
    let matches = one("Foi a el cine cola so hermana.", "A_CONTR");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (4, 8));
    assert_eq!(suggestions(&matches[0]), vec!["al"]);
}

/// `A_CONTR[2]` XML rule: `Tienes que lo dicir a l’entamu.` Java UTF-16
/// 20..30 / engine UTF-8 20..32 (`’` is two extra bytes) -> `al entamu`.
#[test]
fn asturian_xml_a_contr_apostrophe() {
    let _guard = engine_guard();
    let matches = one("Tienes que lo dicir a l’entamu.", "A_CONTR");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (20, 32));
    assert_eq!(suggestions(&matches[0]), vec!["al entamu"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`):
/// `Voi , y depués vengo.` Java UTF-16 3..5 / engine UTF-8 3..5 -> `,`.
#[test]
fn asturian_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("Voi , y depués vengo.", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (3, 5));
    assert_eq!(
        matches[0].message,
        "Pon un espaciu depués de la coma, pero non enantes"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION`: `Eso ye mui bien.. Pero nun vieno.` Java UTF-16
/// 15..17 / engine UTF-8 15..17 -> `.` with suggestions `.|…`.
#[test]
fn asturian_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Eso ye mui bien.. Pero nun vieno.", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (15, 17));
    assert_eq!(matches[0].message, "Dos puntos siguíos");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START`: `esti ye un test.` Java UTF-16 0..4 / engine
/// UTF-8 0..4 -> `Esti`.
#[test]
fn asturian_uppercase_start() {
    let _guard = engine_guard();
    let matches = one("esti ye un test.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 4));
    assert_eq!(matches[0].message, "Esta frase nun entama con mayúscules");
    assert_eq!(suggestions(&matches[0]), vec!["Esti"]);
}

/// `WHITESPACE_RULE`: `Voi  a la casa.` Java UTF-16 3..5 / engine UTF-8 3..5.
#[test]
fn asturian_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Voi  a la casa.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (3, 5));
    assert_eq!(matches[0].message, "Posible error: repitisti un espaciu");
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `UNPAIRED_BRACKETS`: `(Esti ye un test.` Java UTF-16 0..1 / engine UTF-8
/// 0..1 -> `(`.
#[test]
fn asturian_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("(Esti ye un test.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 1));
    assert_eq!(
        matches[0].message,
        "Símbolu despareyáu: paez que falta \")\""
    );
}

/// `MorfologikAsturianSpellerRule` (`MORFOLOGIK_RULE_AST`):
/// `esti ye un test.` flags `test` at Java UTF-16 11..15 / engine UTF-8
/// 11..15 and pins the full Java suggestion list (which restores `testu`).
#[test]
fn asturian_speller_suggestions() {
    let _guard = engine_guard();
    let matches = one("esti ye un test.", "MORFOLOGIK_RULE_AST");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (11, 15));
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
