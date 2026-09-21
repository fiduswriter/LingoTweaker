//! Slovenian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/sl/probe-rule.sh`) print UTF-16 code units, converted
//! where a test exercises diacritics. Slovenian has no tagger/synthesizer/
//! disambiguator, so the analyzed sentence is the surface tokenization.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

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

/// `CommaWhitespaceRule` (1), Java probe: `To , je test.` 2..4.
#[test]
fn slovenian_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("To , je test.", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 2);
    assert_eq!(matches[0].range.end, 4);
    assert_eq!(
        matches[0].message,
        "Presledek vstavi po vejici, ne pa pred vejico"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe: `To je test..` 10..12.
#[test]
fn slovenian_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("To je test..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 10);
    assert_eq!(matches[0].range.end, 12);
    assert_eq!(matches[0].message, "Dve zaporedni piki");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UppercaseSentenceStartRule` (5), Java probe: the second sentence starts
/// lowercase, 12..14.
#[test]
fn slovenian_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one("To je test. to je test.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 14);
    assert_eq!(
        matches[0].message,
        "Ta poved se ne začenja z veliko začetnico"
    );
    assert_eq!(suggestions(&matches[0]), vec!["To"]);
}

/// `MultipleWhitespaceRule` (7), Java probe: `To  je test.` 2..4.
#[test]
fn slovenian_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("To  je test.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 2);
    assert_eq!(matches[0].range.end, 4);
    assert_eq!(
        matches[0].message,
        "Možna tipkarska napaka: ponovili ste presledek"
    );
}

/// `WordRepeatRule` (6), Java probe: `To to je test.` 0..5.
#[test]
fn slovenian_word_repeat() {
    let _guard = engine_guard();
    let matches = one("To to je test.", "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 5);
    assert_eq!(
        matches[0].message,
        "Možna tipkarska napaka: ponovili ste besedo"
    );
    assert_eq!(suggestions(&matches[0]), vec!["To"]);
}

/// `MorfologikSlovenianSpellerRule` (4), Java probe: `To je tst.` 6..9 with
/// the Java suggestion ranking (ISO-8859-2 dictionary).
#[test]
fn slovenian_speller() {
    let _guard = engine_guard();
    let matches = one("To je tst.", "MORFOLOGIK_RULE_SL_SI");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 6);
    assert_eq!(matches[0].range.end, 9);
    assert_eq!(
        matches[0].message,
        "Najdena morebitna napaka pri črkovanju."
    );
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "TNT", "Tit", "Tot", "Trst", "Tut", "ost", "pst", "tast", "tat", "test", "tet", "trst",
            "trt", "ust", "TXT"
        ]
    );
}

/// XML rule `STEVILA_DO_10` (`grammar.xml`), Java-probed via the corpus
/// golden (`sl-full.java.tsv`): `0 oseb ni manjkalo` 0..1 -> `nič`.
#[test]
fn slovenian_xml_numbers() {
    let _guard = engine_guard();
    let matches = one("0 oseb ni manjkalo", "STEVILA_DO_10");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 1);
    assert_eq!(suggestions(&matches[0]), vec!["nič"]);
}

/// XML rule `KRATICE` (abbreviations), Java-probed via the corpus golden:
/// `doo` at 21..24 -> `d. o. o.`.
#[test]
fn slovenian_xml_abbreviations() {
    let _guard = engine_guard();
    let matches = one("Podjetje Krivolovec, doo je šlo v stečaj.", "KRATICE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 21);
    assert_eq!(matches[0].range.end, 24);
    assert_eq!(suggestions(&matches[0]), vec!["d. o. o."]);
}

/// XML rule `PONOVLJENA_LOČILA` (repeated punctuation), Java-probed via the
/// corpus golden: `!!` at 15..17 -> `!`.
#[test]
fn slovenian_xml_repeated_punctuation() {
    let _guard = engine_guard();
    let matches = one("Ne prekinjaj me!!", "PONOVLJENA_LOČILA");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 15);
    assert_eq!(matches[0].range.end, 17);
    assert_eq!(suggestions(&matches[0]), vec!["!"]);
}

/// XML rule `PREDLOG_Z` (`z` vs `s` preposition), Java-probed via the corpus
/// golden: `z` at 26..27 -> `s`.
#[test]
fn slovenian_xml_preposition() {
    let _guard = engine_guard();
    let matches = one("Predsednik ZDA je odletel z helikopterjem.", "PREDLOG_Z");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 26);
    assert_eq!(matches[0].range.end, 27);
    assert_eq!(suggestions(&matches[0]), vec!["s"]);
}

/// XML rule `KAJ_BREZ_VEJICE`, Java-probed via the corpus golden: `kaj` at
/// 5..9 -> `, kaj`.
#[test]
fn slovenian_xml_missing_comma() {
    let _guard = engine_guard();
    let matches = one("Povej kaj mu manjka.", "KAJ_BREZ_VEJICE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 5);
    assert_eq!(matches[0].range.end, 9);
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
