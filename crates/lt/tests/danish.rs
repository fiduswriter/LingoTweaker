//! Danish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/da/check-diff-da.sh`, `docs/parity/golden/da-full.txt`)
//! print UTF-16 code units. The probe sentences here are ASCII, so both agree.
//! `Danish.getRelevantRules` has no Java rule classes beyond the generic
//! built-ins; `MorfologikSpellerRule`/`HunspellRule` suggestions come from the
//! bounded dictionary search instead of the unported native `hunspell.suggest`
//! (documented divergence), so the speller test pins the match, not the list.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Danish engines hold the speller dictionary.
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
    Engine::builder(Lang::Da).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Da)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(da) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    da.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: active XML rule count, no XML-referenced filters and
/// `compile_failures()` = 0 (Danish has no language-specific Java rules).
#[test]
fn danish_engine_state() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(da.active_rule_count(), 69);
    assert_eq!(da.skipped_counts().filters, 0);
    assert!(
        da.compile_failures().is_empty(),
        "{:?}",
        da.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe: `Det er en fejl , her.` 14..16.
#[test]
fn danish_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("Det er en fejl , her.", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 14);
    assert_eq!(matches[0].range.end, 16);
    assert_eq!(
        matches[0].message,
        "Indsæt ikke et mellemrum før komma, men efter det."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe: `Det er en fejl..` 14..16.
#[test]
fn danish_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Det er en fejl..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 14);
    assert_eq!(matches[0].range.end, 16);
    assert_eq!(matches[0].message, "To på hinanden følgende punktummer");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `GenericUnpairedBracketsRule` (3) with the explicit Danish bracket lists,
/// Java probe: `(Det er en fejl.` 0..1.
#[test]
fn danish_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("(Det er en fejl.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 1);
    assert_eq!(
        matches[0].message,
        "Ikke parret symbol: \")\" ser ud til at mangle"
    );
}

/// `UppercaseSentenceStartRule` (5), Java probe: `det er en fejl.` 0..3.
#[test]
fn danish_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one("det er en fejl.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 3);
    assert_eq!(
        matches[0].message,
        "Denne sætning starter ikke med et stort begyndelsesbogstav"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Det"]);
}

/// `MultipleWhitespaceRule` (6), Java probe: `Det  er en fejl.` 3..5.
#[test]
fn danish_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Det  er en fejl.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 3);
    assert_eq!(matches[0].range.end, 5);
    assert_eq!(
        matches[0].message,
        "Mulig slåfejl: du har gentaget et mellemrum"
    );
}

/// `HunspellRule` (4), Java probe: `Dette er en tset.` 12..16 (the suggestion
/// list is the documented bounded-search divergence).
#[test]
fn danish_speller() {
    let _guard = engine_guard();
    let matches = one("Dette er en tset.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 16);
    assert_eq!(matches[0].message, "Mulig stavefejl fundet");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert!(!suggestions(&matches[0]).is_empty());
}

/// XML rule `grube` (`grammar.xml`), Java-probed via the corpus golden
/// (`da-full.java.tsv`): `faldgruper` 13..23 -> `faldgruber`.
#[test]
fn danish_xml_grube() {
    let _guard = engine_guard();
    let matches = one("Der er mange faldgruper.", "grube");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 13);
    assert_eq!(matches[0].range.end, 23);
    assert_eq!(suggestions(&matches[0]), vec!["faldgruber"]);
}

/// XML rule `yndlings` (`grammar.xml`), Java-probed via the corpus golden:
/// `ynglingshold` 12..24 -> `yndlingshold`.
#[test]
fn danish_xml_yndlings() {
    let _guard = engine_guard();
    let matches = one("Det var mit ynglingshold.", "yndlings");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 24);
    assert_eq!(suggestions(&matches[0]), vec!["yndlingshold"]);
}

/// The `BaseTagger`-derived `DanishTagger` tags known words and leaves
/// unknown ones untagged.
#[test]
fn danish_tagger() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = da.analyze("Han blev født i 1920'erne.");
    let readings: Vec<&lt::AnalyzedToken> = sentences
        .iter()
        .flat_map(|s| s.tokens.iter())
        .flat_map(|t| t.readings.iter())
        .filter(|r| !matches!(r.pos_tag.as_deref(), Some("SENT_START" | "SENT_END")))
        .collect();
    let tagged = readings.iter().filter(|r| r.pos_tag.is_some()).count();
    assert!(tagged > 0, "no tagged readings: {readings:?}");
}
