//! Lithuanian engine tests.
//!
//! The upstream Lithuanian module is unusable: `getRelevantRules` includes
//! `MorfologikLithuanianSpellerRule` over `/lt/hunspell/lt_LT.dict`, but that
//! dictionary is not shipped in the pinned checkout or any pinned Maven
//! artifact, so the legacy engine throws on every check. The Rust engine
//! disables the missing speller and runs the XML + generic rules; the values
//! below are Java-probed per rule (`scripts/oracle/lt/probe-rule.sh`, which
//! enables one rule and therefore never initializes the speller) and asserted
//! in Java UTF-16 code units via `common::assert_utf16`.
//! `Lithuanian` uses the `DemoTagger` (all tokens untagged), has no
//! disambiguator/synthesizer, and the language is gated tests-only.

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
    Engine::builder(Lang::Lt).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Lt)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(lt) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    lt.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 4 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0. The speller is disabled (dictionary not shipped).
#[test]
fn lithuanian_engine_state() {
    let _guard = engine_guard();
    let Some(lt) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(lt.active_rule_count(), 4);
    assert_eq!(lt.skipped_counts().filters, 0);
    assert!(
        lt.compile_failures().is_empty(),
        "{:?}",
        lt.compile_failures()
    );
}

/// `BRAK_PRZECINKA_ZE` XML rule -> `pajuto, kad`.
#[test]
fn lithuanian_xml_brak_przecinka_ze() {
    let _guard = engine_guard();
    let text = "Jaroslavas pajuto kad jo draugas yra Mantas.";
    let matches = one(text, "BRAK_PRZECINKA_ZE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (11, 21));
    assert_eq!(
        matches[0].message,
        "Prieš jungtuką „kad“ šiuo atveju reiktų kablelio: <suggestion>pajuto, kad</suggestion>."
    );
    assert_eq!(suggestions(&matches[0]), vec!["pajuto, kad"]);
}

/// `BRAK_PRZECINKA_JESLI` XML rule -> `virusai, jeigu`.
#[test]
fn lithuanian_xml_brak_przecinka_jesli() {
    let _guard = engine_guard();
    let text = "Tavo kompiuterio negadintu virusai jeigu tu naudotum Linux sistema.";
    let matches = one(text, "BRAK_PRZECINKA_JESLI");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (27, 40));
    assert_eq!(
        matches[0].message,
        "Prieš jungtuką „jei(gu)“ šiuo atveju reiktų kablelio:<suggestion>virusai, jeigu</suggestion> "
    );
    assert_eq!(suggestions(&matches[0]), vec!["virusai, jeigu"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (Lithuanian `no_space_before_dot`).
#[test]
fn lithuanian_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Jaroslavas pajuto kad .";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (21, 23));
    assert_eq!(matches[0].message, "Nereikia dėti tarpo prieš tašką");
    assert_eq!(suggestions(&matches[0]), vec!["."]);
}

/// `DOUBLE_PUNCTUATION` (Lithuanian `two_dots`).
#[test]
fn lithuanian_double_punctuation() {
    let _guard = engine_guard();
    let text = "Jaroslavas pajuto ..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (18, 20));
    assert_eq!(matches[0].message, "Du iš eilės einantys taškai");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Du iš eilės einantys taškai")
    );
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` (`incorrect_case`/`category_case`).
#[test]
fn lithuanian_uppercase_start() {
    let _guard = engine_guard();
    let text = "jaroslavas pajuto.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 10));
    assert_eq!(
        matches[0].message,
        "Sakinys turi prasidėti iš didžiosios raidės"
    );
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Didžiosios/mažosios raidės")
    );
    assert_eq!(suggestions(&matches[0]), vec!["Jaroslavas"]);
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`, Lithuanian strings).
#[test]
fn lithuanian_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Jaroslavas  pajuto.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (10, 12));
    assert_eq!(
        matches[0].message,
        "Galima rinkimo klaida: pakartojote tarpą"
    );
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `UNPAIRED_BRACKETS` (`GenericUnpairedBracketsRule`).
#[test]
fn lithuanian_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Jaroslavas pajuto.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: ')' seems to be missing"
    );
}

/// Correct Lithuanian text is clean (the missing speller stays disabled).
#[test]
fn lithuanian_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(lt) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = lt
        .check("Jaroslavas pajuto, kad jo draugas yra Mantas.")
        .expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
