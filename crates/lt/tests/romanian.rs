//! Romanian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Probe offsets are the Java UTF-16
//! code units and are asserted with the `common::assert_utf16` helper.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::assert_utf16;

/// One engine at a time: the Romanian engines hold the tagger dictionary.
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
    Engine::builder(Lang::Ro).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ro)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ro) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ro.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: active XML rule count, disambiguation rules and
/// `compile_failures()` = 0. Updated by each stage.
#[test]
fn romanian_engine_state() {
    let _guard = engine_guard();
    let Some(ro) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(ro.active_rule_count(), 454);
    assert_eq!(ro.disambig_rule_count(), 68);
    assert_eq!(ro.skipped_counts().filters, 0);
    assert_eq!(ro.skipped_counts().off_by_default, 3);
    assert!(
        ro.compile_failures().is_empty(),
        "{:?}",
        ro.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe.
#[test]
fn romanian_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Bună , lume!";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (4, 6));
    assert_eq!(
        matches[0].message,
        "Pune un spațiu după virgulă, dar nu înainte de virgulă"
    );
    assert_eq!(matches[0].suggestions[0].value, ",");
}

/// `DoublePunctuationRule` (2), Java probe.
#[test]
fn romanian_double_punctuation() {
    let _guard = engine_guard();
    let text = "Aceasta este o propozitie..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (25, 27));
    assert_eq!(matches[0].message, "Două puncte consecutive");
}

/// `UppercaseSentenceStartRule` (3), Java probe: the second sentence starts
/// lowercase.
#[test]
fn romanian_uppercase_sentence_start() {
    let _guard = engine_guard();
    let text = "Aceasta este o propozitie. aceasta continua.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (27, 34));
    assert_eq!(matches[0].message, "Propoziția nu începe cu literă mare");
    assert_eq!(matches[0].suggestions[0].value, "Aceasta");
}

/// `GenericUnpairedBracketsRule` (5) with the Romanian symbol lists.
#[test]
fn romanian_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "Acesta este (un exemplu.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (12, 13));
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: ')' seems to be missing"
    );
}

/// `WordRepeatRule` (6), the generic base class.
#[test]
fn romanian_word_repeat() {
    let _guard = engine_guard();
    let text = "Aceasta este este o propozitie.";
    let matches = one(text, "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (8, 17));
    assert_eq!(matches[0].suggestions[0].value, "este");
}

/// `MorfologikRomanianSpellerRule` (7), Java probe: `propozitie` with the
/// Morfologik ranking.
#[test]
fn romanian_spelling_suggestions() {
    let _guard = engine_guard();
    let text = "Aceasta este o propozitie fara diacritice.";
    let matches = one(text, "MORFOLOGIK_RULE_RO_RO");
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert_utf16(text, &matches[0], (15, 25));
    assert_eq!(
        matches[0].message,
        "S-a găsit o posibilă greșeală de ortografie"
    );
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(
        suggestions,
        vec![
            "propoziție",
            "prepoziție",
            "propoziția",
            "propoziției",
            "propoziții",
            "propozițio",
        ]
    );
    assert_utf16(text, &matches[1], (26, 30));
}

/// `SimpleReplaceRule` (9) over `/ro/replace.txt`, Java-probed offsets and
/// case folding.
#[test]
fn romanian_simple_replace() {
    let _guard = engine_guard();

    let text = "Patrusprezece case.";
    let matches = one(text, "RO_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (0, 13));
    assert_eq!(matches[0].suggestions[0].value, "Paisprezece");
    assert_eq!(
        matches[0].message,
        "'Patrusprezece' este incorect sau ieșit din uz, folosiți <suggestion>paisprezece</suggestion>"
    );

    let text = "Satul are patrusprezece case.";
    let matches = one(text, "RO_SIMPLE_REPLACE");
    assert_utf16(text, &matches[0], (10, 23));
    assert_eq!(matches[0].suggestions[0].value, "paisprezece");

    // multi-word
    let text = "aqua forte";
    let matches = one(text, "RO_SIMPLE_REPLACE");
    assert_utf16(text, &matches[0], (0, 10));
    assert_eq!(matches[0].suggestions[0].value, "Acvaforte");

    // dash-delimited + two suggestions
    let matches = one("Iată un cau-boi.", "RO_SIMPLE_REPLACE");
    assert_eq!(matches[0].suggestions[0].value, "cowboy");
    let matches = one("A fost adăogită o altă regulă.", "RO_SIMPLE_REPLACE");
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, vec!["adăugită", "adăugată"]);
}

/// `CompoundRule` (10) over `/ro/compounds.txt`, Java probe.
#[test]
fn romanian_compound() {
    let _guard = engine_guard();
    let text = "câte și trei";
    let matches = one(text, "RO_COMPOUND");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (0, 12));
    assert_eq!(matches[0].message, "Cuvântul se scrie legat.");
    assert_eq!(matches[0].suggestions[0].value, "câteșitrei");

    let matches = one("câte-și-trei", "RO_COMPOUND");
    assert_eq!(matches[0].suggestions[0].value, "câteșitrei");

    // already correct: no match
    let matches = one("Au plecat câteșitrei.", "RO_COMPOUND");
    assert!(matches.is_empty(), "{matches:?}");
}

/// `RomanianWordRepeatBeginningRule` (8), text level: three successive
/// sentences starting with the same word.
#[test]
fn romanian_word_repeat_beginning() {
    let _guard = engine_guard();
    let text = "Ion are mere. Ion are pere. Ion are prune.";
    let matches = one(text, "ROMANIAN_WORD_REPEAT_BEGINNING_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (28, 31));
    assert_eq!(
        matches[0].message,
        "Trei propoziții succesive încep cu același cuvânt. Consider rewording the sentence or use a thesaurus to find a synonym."
    );
    assert!(matches[0].suggestions.is_empty());
}
