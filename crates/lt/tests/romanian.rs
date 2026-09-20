//! Romanian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Offsets are UTF-8 bytes (the engine
//! format); the Java probes print UTF-16 code units, converted in the comments
//! where a test exercises diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

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

/// `CommaWhitespaceRule` (1), Java probe: `Bună , lume!` 4..6 message.
#[test]
fn romanian_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("Bună , lume!", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 5);
    assert_eq!(matches[0].range.end, 7);
    assert_eq!(
        matches[0].message,
        "Pune un spațiu după virgulă, dar nu înainte de virgulă"
    );
    assert_eq!(matches[0].suggestions[0].value, ",");
}

/// `DoublePunctuationRule` (2), Java probe: `propozitie..` 25..27.
#[test]
fn romanian_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Aceasta este o propozitie..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 25);
    assert_eq!(matches[0].range.end, 27);
    assert_eq!(matches[0].message, "Două puncte consecutive");
}

/// `UppercaseSentenceStartRule` (3), Java probe: the second sentence starts
/// lowercase, 27..34.
#[test]
fn romanian_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one(
        "Aceasta este o propozitie. aceasta continua.",
        "UPPERCASE_SENTENCE_START",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 27);
    assert_eq!(matches[0].range.end, 34);
    assert_eq!(matches[0].message, "Propoziția nu începe cu literă mare");
    assert_eq!(matches[0].suggestions[0].value, "Aceasta");
}

/// `GenericUnpairedBracketsRule` (5) with the Romanian symbol lists,
/// `(un exemplu.` 12..13.
#[test]
fn romanian_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("Acesta este (un exemplu.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 13);
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: ')' seems to be missing"
    );
}

/// `WordRepeatRule` (6), the generic base class: `este este` 8..17.
#[test]
fn romanian_word_repeat() {
    let _guard = engine_guard();
    let matches = one("Aceasta este este o propozitie.", "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 8);
    assert_eq!(matches[0].range.end, 17);
    assert_eq!(matches[0].suggestions[0].value, "este");
}

/// `MorfologikRomanianSpellerRule` (7), Java probe: `propozitie` 15..25 with
/// the Morfologik ranking.
#[test]
fn romanian_spelling_suggestions() {
    let _guard = engine_guard();
    let matches = one(
        "Aceasta este o propozitie fara diacritice.",
        "MORFOLOGIK_RULE_RO_RO",
    );
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert_eq!(matches[0].range.start, 15);
    assert_eq!(matches[0].range.end, 25);
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
    assert_eq!(matches[1].range.start, 26);
    assert_eq!(matches[1].range.end, 30);
}

/// `SimpleReplaceRule` (9) over `/ro/replace.txt`, Java-probed offsets and
/// case folding.
#[test]
fn romanian_simple_replace() {
    let _guard = engine_guard();

    let matches = one("Patrusprezece case.", "RO_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 13));
    assert_eq!(matches[0].suggestions[0].value, "Paisprezece");
    assert_eq!(
        matches[0].message,
        "'Patrusprezece' este incorect sau ieșit din uz, folosiți <suggestion>paisprezece</suggestion>"
    );

    let matches = one("Satul are patrusprezece case.", "RO_SIMPLE_REPLACE");
    assert_eq!((matches[0].range.start, matches[0].range.end), (10, 23));
    assert_eq!(matches[0].suggestions[0].value, "paisprezece");

    // multi-word
    let matches = one("aqua forte", "RO_SIMPLE_REPLACE");
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 10));
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
    let matches = one("câte și trei", "RO_COMPOUND");
    assert_eq!(matches.len(), 1, "{matches:?}");
    // Java UTF-16 0..12; `â`/`ș` are one char but two UTF-8 bytes.
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 14));
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
    let matches = one(
        "Ion are mere. Ion are pere. Ion are prune.",
        "ROMANIAN_WORD_REPEAT_BEGINNING_RULE",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 28);
    assert_eq!(matches[0].range.end, 31);
    assert_eq!(
        matches[0].message,
        "Trei propoziții succesive încep cu același cuvânt. Consider rewording the sentence or use a thesaurus to find a synonym."
    );
    assert!(matches[0].suggestions.is_empty());
}
