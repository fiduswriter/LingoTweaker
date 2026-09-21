//! Icelandic engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule and speller values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/is/check-diff-is.sh`); the
//! probes use real Icelandic orthography (ð/þ/á/í/ö). `Icelandic` has no Java
//! rule classes: `getRelevantRules` is the generic built-ins plus
//! `HunspellNoSuggestionRule`, which emits no suggestions.

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
    Engine::builder(Lang::Is).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Is)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(is) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    is.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 39 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn icelandic_engine_state() {
    let _guard = engine_guard();
    let Some(is) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(is.active_rule_count(), 39);
    assert_eq!(is.skipped_counts().filters, 0);
    assert!(
        is.compile_failures().is_empty(),
        "{:?}",
        is.compile_failures()
    );
}

/// `ARFLEIFÐ` XML rule -> `arfleifð`.
#[test]
fn icelandic_xml_arfleid() {
    let _guard = engine_guard();
    let text = "Þessi vandi er arfleið nýlendutímans.";
    let matches = one(text, "ARFLEIFÐ");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (15, 22));
    assert_eq!(suggestions(&matches[0]), vec!["arfleifð"]);
}

/// `ÁNNA` XML rule (context `við|í|…`) -> `ána`.
#[test]
fn icelandic_xml_anna() {
    let _guard = engine_guard();
    let text = "Veitingastaðurinn við ánna var frábær.";
    let matches = one(text, "ÁNNA");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (22, 26));
    assert_eq!(suggestions(&matches[0]), vec!["ána"]);
}

/// `FARM_FRAM` XML rule -> `fram`.
#[test]
fn icelandic_xml_farm_fram() {
    let _guard = engine_guard();
    let text = "Við horfum farm á veginn.";
    let matches = one(text, "FARM_FRAM");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (11, 15));
    assert_eq!(suggestions(&matches[0]), vec!["fram"]);
}

/// `LEITI` XML rule (`á næsta leyti`) -> `leiti`.
#[test]
fn icelandic_xml_leiti() {
    let _guard = engine_guard();
    let text = "Skrifa skal leiti í orðasambandinu: á næsta leyti.";
    let matches = one(text, "LEITI");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (44, 49));
    assert_eq!(suggestions(&matches[0]), vec!["leiti"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`) -> `,`.
#[test]
fn icelandic_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Ég elska íslensku , en ekki ensku.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (17, 19));
    assert_eq!(
        matches[0].message,
        "Bil skal vera á eftir kommu, ekki á undan henni"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION` -> `.` with suggestions `.|…`.
#[test]
fn icelandic_double_punctuation() {
    let _guard = engine_guard();
    let text = "Þetta er gott.. En hann er ekki hér.";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (13, 15));
    assert_eq!(matches[0].message, "Tveir punktar í röð");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` -> `Þetta`.
#[test]
fn icelandic_uppercase_start() {
    let _guard = engine_guard();
    let text = "þetta er lítill setning.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 5));
    assert_eq!(matches[0].message, "Þessi setning hefst ekki á hástaf");
    assert_eq!(suggestions(&matches[0]), vec!["Þetta"]);
}

/// `WHITESPACE_RULE`.
#[test]
fn icelandic_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Þetta  er gott.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (5, 7));
    assert_eq!(matches[0].message, "Hugsanleg ritvilla: endurtekið bil");
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `WORD_REPEAT_RULE` -> `Hann`.
#[test]
fn icelandic_word_repeat() {
    let _guard = engine_guard();
    let text = "Hann hann fór út.";
    let matches = one(text, "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 9));
    assert_eq!(matches[0].message, "Hugsanleg ritvilla: orð endurtekið");
    assert_eq!(suggestions(&matches[0]), vec!["Hann"]);
}

/// `UNPAIRED_BRACKETS` -> `(`.
#[test]
fn icelandic_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Þetta er svigi.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: ')' seems to be missing"
    );
}

/// `HunspellNoSuggestionRule`: the rule reports no suggestions
/// (`getSuggestions` is empty).
#[test]
fn icelandic_speller_no_suggestions() {
    let _guard = engine_guard();
    let text = "Þetta er tesst.";
    let matches = one(text, "HUNSPELL_NO_SUGGEST_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (9, 14));
    assert_eq!(matches[0].message, "Possible spelling mistake found.");
    assert!(suggestions(&matches[0]).is_empty());
}

/// The `is_IS` dictionary keeps the orthography: the misspelling
/// `rittgerninginn` and `íslands` for `Íslands` are flagged with no
/// suggestions.
#[test]
fn icelandic_speller_diacritics() {
    let _guard = engine_guard();
    let text = "Hann skrifaði rittgerninginn.";
    let m = one(text, "HUNSPELL_NO_SUGGEST_RULE");
    assert_eq!(m.len(), 1);
    assert_utf16(text, &m[0], (14, 28));
    assert!(suggestions(&m[0]).is_empty());

    let text = "Ég fór til íslands í gær.";
    let m = one(text, "HUNSPELL_NO_SUGGEST_RULE");
    assert_eq!(m.len(), 1);
    assert_utf16(text, &m[0], (11, 18));
    assert!(suggestions(&m[0]).is_empty());
}

/// Correct Icelandic text (ð/þ/á/ö) is clean.
#[test]
fn icelandic_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(is) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = is
        .check("Þetta er góð setning. Ég elska íslensku og Ísland.")
        .expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
