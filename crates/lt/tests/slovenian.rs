//! Slovenian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/sl/probe-rule.sh`,
//! `check-diff-sl.sh`); the probes use real Slovenian orthography (č/š/ž).
//! Slovenian has no tagger/synthesizer/disambiguator, so the analyzed
//! sentence is the surface tokenization.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::assert_utf16;

/// One engine at a time: the Slovenian engines hold the speller dictionary.
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
    Engine::builder(Lang::Sl).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Sl)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(sl) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    sl.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: active XML rule count and `compile_failures()` = 0.
#[test]
fn slovenian_engine_state() {
    let _guard = engine_guard();
    let Some(sl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(sl.active_rule_count(), 85);
    assert_eq!(sl.disambig_rule_count(), 0);
    assert_eq!(sl.skipped_counts().filters, 0);
    assert!(
        sl.compile_failures().is_empty(),
        "{:?}",
        sl.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe.
#[test]
fn slovenian_comma_whitespace() {
    let _guard = engine_guard();
    let text = "To je čaj , a kava.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (9, 11));
    assert_eq!(
        matches[0].message,
        "Presledek vstavi po vejici, ne pa pred vejico"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe.
#[test]
fn slovenian_double_punctuation() {
    let _guard = engine_guard();
    let text = "To je čaj..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (9, 11));
    assert_eq!(matches[0].message, "Dve zaporedni piki");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UppercaseSentenceStartRule` (5), Java probe: the second sentence starts
/// lowercase.
#[test]
fn slovenian_uppercase_sentence_start() {
    let _guard = engine_guard();
    let text = "To je čaj. to je kava.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (11, 13));
    assert_eq!(
        matches[0].message,
        "Ta poved se ne začenja z veliko začetnico"
    );
    assert_eq!(suggestions(&matches[0]), vec!["To"]);
}

/// `MultipleWhitespaceRule` (7), Java probe.
#[test]
fn slovenian_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "To je čaj  in kava.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (9, 11));
    assert_eq!(
        matches[0].message,
        "Možna tipkarska napaka: ponovili ste presledek"
    );
}

/// `WordRepeatRule` (6), Java probe.
#[test]
fn slovenian_word_repeat() {
    let _guard = engine_guard();
    let text = "Dober čaj čaj je.";
    let matches = one(text, "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (6, 13));
    assert_eq!(
        matches[0].message,
        "Možna tipkarska napaka: ponovili ste besedo"
    );
    assert_eq!(suggestions(&matches[0]), vec!["čaj"]);
}

/// `MorfologikSlovenianSpellerRule` (4), Java probe with real Slovenian
/// misspellings (`caj`, `tezko`, `mogoce`); the Java suggestion lists restore
/// č/ž/č (ISO-8859-2 dictionary).
#[test]
fn slovenian_speller() {
    let _guard = engine_guard();

    let text = "To je caj.";
    let matches = one(text, "MORFOLOGIK_RULE_SL_SI");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (6, 9));
    assert_eq!(
        matches[0].message,
        "Najdena morebitna napaka pri črkovanju."
    );
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "čaj", "Car", "Gaj", "Kaj", "Maj", "Taj", "ca.", "cap", "car", "daj", "gaj", "kaj",
            "maj", "naj", "raj", "saj", "vaj", "Čap", "čad", "čaja", "čaje", "čaji", "čaju", "čar",
            "čas", "čaš"
        ]
    );

    let text = "To je tezko.";
    let matches = one(text, "MORFOLOGIK_RULE_SL_SI");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (6, 11));
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "težko", "Nežko", "Rezko", "Terko", "Tezno", "rezko", "teko", "tenko", "tepko",
            "tetko", "tezo", "težjo", "težka", "težke", "težki", "težno", "težo", "tez ko"
        ]
    );

    let text = "To je mogoce.";
    let matches = one(text, "MORFOLOGIK_RULE_SL_SI");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (6, 12));
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "mogoče", "mogotce", "mogoč", "mogoča", "mogočem", "mogočen", "mogoči", "mogočne",
            "mogočo"
        ]
    );
}

/// XML rule `STEVILA_DO_10` (`grammar.xml`), Java-probed via the corpus
/// golden (`sl-full.java.tsv`): `0 oseb ni manjkalo` 0..1 -> `nič`.
#[test]
fn slovenian_xml_numbers() {
    let _guard = engine_guard();
    let text = "0 oseb ni manjkalo";
    let matches = one(text, "STEVILA_DO_10");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(suggestions(&matches[0]), vec!["nič"]);
}

/// XML rule `KRATICE` (abbreviations), Java-probed via the corpus golden:
/// `doo` at 21..24 -> `d. o. o.`.
#[test]
fn slovenian_xml_abbreviations() {
    let _guard = engine_guard();
    let text = "Podjetje Krivolovec, doo je šlo v stečaj.";
    let matches = one(text, "KRATICE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (21, 24));
    assert_eq!(suggestions(&matches[0]), vec!["d. o. o."]);
}

/// XML rule `PONOVLJENA_LOČILA` (repeated punctuation), Java-probed via the
/// corpus golden: `!!` at 15..17 -> `!`.
#[test]
fn slovenian_xml_repeated_punctuation() {
    let _guard = engine_guard();
    let text = "Ne prekinjaj me!!";
    let matches = one(text, "PONOVLJENA_LOČILA");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (15, 17));
    assert_eq!(suggestions(&matches[0]), vec!["!"]);
}

/// XML rule `PREDLOG_Z` (`z` vs `s` preposition), Java-probed via the corpus
/// golden: `z` at 26..27 -> `s`.
#[test]
fn slovenian_xml_preposition() {
    let _guard = engine_guard();
    let text = "Predsednik ZDA je odletel z helikopterjem.";
    let matches = one(text, "PREDLOG_Z");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (26, 27));
    assert_eq!(suggestions(&matches[0]), vec!["s"]);
}

/// XML rule `KAJ_BREZ_VEJICE`, Java-probed via the corpus golden: `kaj` at
/// 5..9 -> `, kaj`.
#[test]
fn slovenian_xml_missing_comma() {
    let _guard = engine_guard();
    let text = "Povej kaj mu manjka.";
    let matches = one(text, "KAJ_BREZ_VEJICE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (5, 9));
    assert_eq!(suggestions(&matches[0]), vec![", kaj"]);
}

/// Slovenian has no tagger: the analyzed tokens carry no POS tag
/// (`DemoTagger`), so tag-dependent XML conditions cannot match.
#[test]
fn slovenian_has_no_tagger() {
    let _guard = engine_guard();
    let Some(sl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = sl.analyze("To je test.");
    let readings: Vec<&lt::AnalyzedToken> = sentences
        .iter()
        .flat_map(|s| s.tokens.iter())
        .flat_map(|t| t.readings.iter())
        .filter(|r| !matches!(r.pos_tag.as_deref(), Some("SENT_START" | "SENT_END")))
        .collect();
    assert!(readings.iter().all(|r| r.pos_tag.is_none()), "{readings:?}");
}
