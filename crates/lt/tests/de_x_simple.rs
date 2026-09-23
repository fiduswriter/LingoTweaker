//! Simple German (`de-DE-x-simple-language`) engine tests.
//!
//! `SimpleGerman extends GermanyGerman` but `getRelevantRules` returns only
//! `new de.LongSentenceRule(messages, userConfig, 12)` and `getRuleFileNames`
//! loads only the variant's `grammar.xml`; the German rule classes and the
//! German speller never run. The Rust engine models this as the `Lang::De`
//! variant string `de-DE-x-simple-language` (`new_german_simple`), reusing the
//! German tagger/synthesizer/disambiguator/chunker. Every expectation below is
//! probed against the pinned Java build
//! (`scripts/oracle/de-x-simple/check-diff-de-x-simple.sh`); UTF-16 offsets are
//! asserted with the `common::assert_utf16` helper.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::assert_utf16;

const VARIANT: &str = "de-DE-x-simple-language";

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
    Engine::builder(Lang::De)
        .ok()?
        .data_dir(data)
        .variant(VARIANT)
        .build()
        .ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::De)
        .ok()?
        .data_dir(data)
        .variant(VARIANT)
        .options(options)
        .build()
        .ok()
}

fn engine_with_options(options: EngineOptions) -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::De)
        .ok()?
        .data_dir(data)
        .variant(VARIANT)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(engine) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    engine
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: the variant's `grammar.xml` compiles (`compile_failures()` = 0)
/// with 92 loaded rules (17 rulegroups expand to their 34 nested rules), two
/// `default="off"` rules (90 active), and no `<filter>`.
#[test]
fn simple_german_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert!(
        engine.compile_failures().is_empty(),
        "{:?}",
        engine.compile_failures()
    );
    assert_eq!(engine.grammar_rule_count(), 92);
    assert_eq!(engine.active_rule_count(), 90);
    assert_eq!(engine.skipped_counts().filters, 0);
    assert_eq!(engine.skipped_counts().off_by_default, 2);
}

/// The private-use long code resolves to `Lang::De` (the variant is selected
/// separately), while the German default code stays distinct.
#[test]
fn simple_german_language_metadata() {
    assert_eq!(Lang::from_long_code(VARIANT), Some(Lang::De));
    assert_eq!(Lang::from_long_code("de-DE"), Some(Lang::De));
    assert_eq!(Lang::De.base_code(), "de");
}

/// The `SimpleGermanTest` demo text: the six expected rules fire and nothing
/// else does.
#[test]
fn simple_german_demo_text_rules() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let text = "Fügen Sie hier Ihren Text ein oder benutzen Sie diesen Text als Beispiel. \
        Dieser Text wurde nur zum Testen geschrieben. \
        Die Donaudampfschifffahrt darf da nicht fehlen. \
        Und die Nutzung des Genitivs auch nicht.";
    let ids: Vec<String> = engine
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .map(|m| m.rule_id)
        .collect();
    for expected in [
        "ZWEI_INFORMATIONSEINHEITEN_PRO_SATZ",
        "PASSIV",
        "LANGES_WORT",
        "VERNEINUNG",
        "ABSTRAKTE_WOERTER",
        "GENITIV",
    ] {
        assert!(
            ids.iter().any(|id| id == expected),
            "{expected} not in {ids:?}"
        );
    }
}

/// The German rule classes do not run: a lowercase sentence start, a double
/// space and a misspelling produce no German generic or speller match.
#[test]
fn simple_german_no_german_rules() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    for (text, forbidden) in [
        ("das ist ein test.", "UPPERCASE_SENTENCE_START"),
        ("ein  test mit  doppeltem leerzeichen.", "WHITESPACE_RULE"),
        ("Das ist ein Tasstaturfehler.", "GERMAN_SPELLER_RULE"),
    ] {
        let ids: Vec<String> = engine
            .check(text)
            .expect("check")
            .matches
            .into_iter()
            .map(|m| m.rule_id)
            .collect();
        assert!(!ids.iter().any(|id| id == forbidden), "{text}: {ids:?}");
    }
    // the misspelled word still triggers the length rule
    let text = "Das ist ein Tasstaturfehler.";
    let matches = one(text, "LANGES_WORT");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "Dieses Wort hat mehr als dreizehn Buchstaben. Benutzen Sie kurze Wörter."
    );
    assert_eq!(matches[0].category_name, "Schwierige Wörter und Wendungen");
    assert_utf16(text, &matches[0], (12, 27));
}

/// XML rule `GENITIV`: `Die Durchführung der Untersuchung` (Java probe:
/// 17..33 UTF-16).
#[test]
fn simple_german_genitiv() {
    let _guard = engine_guard();
    let text = "Die Durchführung der Untersuchung war erfolgreich.";
    let matches = one(text, "GENITIV");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Vermeiden Sie den Genitiv.");
    assert_eq!(matches[0].category_name, "Leichte Sprache");
    assert_utf16(text, &matches[0], (17, 33));
}

/// XML rulegroup `RELATIVSAETZE` (sub id 1): `das groß ist`.
#[test]
fn simple_german_relativsaetze() {
    let _guard = engine_guard();
    let text = "Das ist ein Haus, das groß ist.";
    let matches = one(text, "RELATIVSAETZE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Verzichten Sie auf Relativsätze.");
    assert_utf16(text, &matches[0], (16, 21));
}

/// `SimpleGerman.getRelevantRules`' only Java rule: the 12-word
/// `TOO_LONG_SENTENCE_DE` (German messages, `tags="picky"`), which does not run
/// at the default level.
#[test]
fn simple_german_long_sentence_is_picky_12_words() {
    let _guard = engine_guard();
    let text = vec!["Wort"; 13].join(" ") + ".";
    // default level: absent
    let Some(default_engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let default_ids: Vec<String> = default_engine
        .check(&text)
        .expect("check")
        .matches
        .into_iter()
        .map(|m| m.rule_id)
        .collect();
    assert!(!default_ids.iter().any(|id| id == "TOO_LONG_SENTENCE_DE"));
    // picky level: present
    let Some(picky) = engine_with_options(EngineOptions {
        picky: true,
        ..Default::default()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let matches: Vec<lt::Match> = picky
        .check(&text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == "TOO_LONG_SENTENCE_DE")
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].description,
        "Lesbarkeit: Satz mit mehr als 12 Wörtern"
    );
    assert_eq!(matches[0].category_name, "Stil");
}
