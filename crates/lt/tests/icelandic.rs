//! Icelandic engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule and speller values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/is/check-diff-is.sh`) print UTF-16 code units. Every probe
//! sentence below uses real Icelandic orthography (ð/þ/á/í/ö), so the two
//! formats differ and both are stated per case. `Icelandic` has no Java rule
//! classes: `getRelevantRules` is the generic built-ins plus
//! `HunspellNoSuggestionRule`, which emits no suggestions.

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

/// `ARFLEIFÐ` XML rule: `Þessi vandi er arfleið nýlendutímans.`
/// Java UTF-16 15..22 / engine UTF-8 16..24 (`Þ` is one extra byte) ->
/// `arfleifð`.
#[test]
fn icelandic_xml_arfleid() {
    let _guard = engine_guard();
    let matches = one("Þessi vandi er arfleið nýlendutímans.", "ARFLEIFÐ");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 16);
    assert_eq!(matches[0].range.end, 24);
    assert_eq!(suggestions(&matches[0]), vec!["arfleifð"]);
}

/// `ÁNNA` XML rule (context `við|í|…`): `Veitingastaðurinn við ánna var
/// frábær.` Java UTF-16 22..26 / engine UTF-8 24..29 -> `ána`.
#[test]
fn icelandic_xml_anna() {
    let _guard = engine_guard();
    let matches = one("Veitingastaðurinn við ánna var frábær.", "ÁNNA");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 24);
    assert_eq!(matches[0].range.end, 29);
    assert_eq!(suggestions(&matches[0]), vec!["ána"]);
}

/// `FARM_FRAM` XML rule: `Við horfum farm á veginn.` Java UTF-16 11..15 /
/// engine UTF-8 12..16 -> `fram`.
#[test]
fn icelandic_xml_farm_fram() {
    let _guard = engine_guard();
    let matches = one("Við horfum farm á veginn.", "FARM_FRAM");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 16);
    assert_eq!(suggestions(&matches[0]), vec!["fram"]);
}

/// `LEITI` XML rule (`á næsta leyti`): `Skrifa skal leiti í orðasambandinu: á
/// næsta leyti.` Java UTF-16 44..49 / engine UTF-8 48..53 -> `leiti`.
#[test]
fn icelandic_xml_leiti() {
    let _guard = engine_guard();
    let matches = one(
        "Skrifa skal leiti í orðasambandinu: á næsta leyti.",
        "LEITI",
    );
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 48);
    assert_eq!(matches[0].range.end, 53);
    assert_eq!(suggestions(&matches[0]), vec!["leiti"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`):
/// `Ég elska íslensku , en ekki ensku.` Java UTF-16 17..19 / engine UTF-8
/// 19..21 -> `,`.
#[test]
fn icelandic_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one(
        "Ég elska íslensku , en ekki ensku.",
        "COMMA_PARENTHESIS_WHITESPACE",
    );
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 19);
    assert_eq!(matches[0].range.end, 21);
    assert_eq!(
        matches[0].message,
        "Bil skal vera á eftir kommu, ekki á undan henni"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION`: `Þetta er gott.. En hann er ekki hér.` Java UTF-16
/// 13..15 / engine UTF-8 14..16 -> `.` with suggestions `.|…`.
#[test]
fn icelandic_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Þetta er gott.. En hann er ekki hér.", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 14);
    assert_eq!(matches[0].range.end, 16);
    assert_eq!(matches[0].message, "Tveir punktar í röð");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START`: `þetta er lítill setning.` Java UTF-16 0..5 /
/// engine UTF-8 0..6 -> `þetta` -> `Þetta`.
#[test]
fn icelandic_uppercase_start() {
    let _guard = engine_guard();
    let matches = one("þetta er lítill setning.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 6);
    assert_eq!(matches[0].message, "Þessi setning hefst ekki á hástaf");
    assert_eq!(suggestions(&matches[0]), vec!["Þetta"]);
}

/// `WHITESPACE_RULE`: `Þetta  er gott.` Java UTF-16 5..7 / engine UTF-8 6..8.
#[test]
fn icelandic_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Þetta  er gott.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 6);
    assert_eq!(matches[0].range.end, 8);
    assert_eq!(matches[0].message, "Hugsanleg ritvilla: endurtekið bil");
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `WORD_REPEAT_RULE`: `Hann hann fór út.` Java UTF-16 0..9 / engine UTF-8
/// 0..9 (all ASCII) -> `Hann`.
#[test]
fn icelandic_word_repeat() {
    let _guard = engine_guard();
    let matches = one("Hann hann fór út.", "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 9);
    assert_eq!(matches[0].message, "Hugsanleg ritvilla: orð endurtekið");
    assert_eq!(suggestions(&matches[0]), vec!["Hann"]);
}

/// `UNPAIRED_BRACKETS`: `(Þetta er svigi.` Java UTF-16 0..1 / engine UTF-8
/// 0..1 -> `(`.
#[test]
fn icelandic_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("(Þetta er svigi.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 1);
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: ')' seems to be missing"
    );
}

/// `HunspellNoSuggestionRule`: `Þetta er tesst.` Java UTF-16 9..14 / engine
/// UTF-8 10..15; the rule reports no suggestions (`getSuggestions` is empty).
#[test]
fn icelandic_speller_no_suggestions() {
    let _guard = engine_guard();
    let matches = one("Þetta er tesst.", "HUNSPELL_NO_SUGGEST_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range.start, 10);
    assert_eq!(matches[0].range.end, 15);
    assert_eq!(matches[0].message, "Possible spelling mistake found.");
    assert!(suggestions(&matches[0]).is_empty());
}

/// The `is_IS` dictionary keeps the orthography: the misspelling
/// `rittgerninginn` (UTF-16 14..28 / UTF-8 15..29) and `íslands` for
/// `Íslands` (UTF-16 11..18 / UTF-8 13..21) are flagged with no suggestions.
#[test]
fn icelandic_speller_diacritics() {
    let _guard = engine_guard();
    let m = one("Hann skrifaði rittgerninginn.", "HUNSPELL_NO_SUGGEST_RULE");
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].range.start, m[0].range.end), (15, 29));
    assert!(suggestions(&m[0]).is_empty());

    let m = one("Ég fór til íslands í gær.", "HUNSPELL_NO_SUGGEST_RULE");
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].range.start, m[0].range.end), (13, 21));
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
