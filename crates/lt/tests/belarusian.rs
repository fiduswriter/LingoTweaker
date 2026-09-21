//! Belarusian engine tests: stage-1 XML wiring state plus Java-probed
//! built-in rule, speller and language-rule values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/be/check-diff-be.sh`,
//! `scripts/oracle/be/probe-rule.sh`); the probes use real Belarusian
//! orthography (`ў і ё` and the apostrophe).
//! `Belarusian` has a `DemoTagger` (all tokens untagged), a custom
//! `BelarusianWordTokenizer` (apostrophes stay inside the word), no
//! disambiguator/synthesizer, and **no** `GenericUnpairedBracketsRule`.

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
    Engine::builder(Lang::Be).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Be)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(be) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    be.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 66 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn belarusian_engine_state() {
    let _guard = engine_guard();
    let Some(be) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(be.active_rule_count(), 66);
    assert_eq!(be.skipped_counts().filters, 0);
    assert!(
        be.compile_failures().is_empty(),
        "{:?}",
        be.compile_failures()
    );
}

/// `U_KAROTKAJE` XML rule: `жанчына-урач` -> `жанчына-ўрач`.
#[test]
fn belarusian_xml_u_karotkaje() {
    let _guard = engine_guard();
    let text = "жанчына-урач";
    let matches = one(text, "U_KAROTKAJE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 12));
    assert_eq!(
        matches[0].message,
        "Пасля галoсных літар замест «у» трэба пісаць «ў»: <suggestion>жанчына-ўрач</suggestion>"
    );
    assert_eq!(suggestions(&matches[0]), vec!["жанчына-ўрач"]);
}

/// `MORFOLOGIK_RULE_BE_BY` over the vendored `be_BY` dictionary:
/// `кампутар` -> `камп'ютар` (with the apostrophe-preserving tokenizer).
#[test]
fn belarusian_speller_suggestions() {
    let _guard = engine_guard();
    let text = "кампутар";
    let matches = one(text, "MORFOLOGIK_RULE_BE_BY");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 8));
    assert_eq!(matches[0].message, "Знойдзена магчымая памылка.");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Арфаграфічная памылка")
    );
    assert_eq!(matches[0].category_id, "TYPOS");
    assert_eq!(matches[0].category_name, "Магчымыя памылкі набору");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "Камунар",
            "камп'ютар",
            "кампотам",
            "кампотах",
            "кампотаў",
            "камунар",
        ]
    );
}

/// `BE_SIMPLE_REPLACE` (`be/rules/replace.txt`): `З большага` -> `Збольшага`.
#[test]
fn belarusian_simple_replace() {
    let _guard = engine_guard();
    let text = "З большага, гэта быў добры дзень.";
    let matches = one(text, "BE_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 10));
    assert_eq!(
        matches[0].message,
        "«З большага» — памылка, нелітаратурны выраз або прастамоўе, правільна: <suggestion>збольшага</suggestion>"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Збольшага"]);
}

/// `BE_SPECIFIC_CASE` (`be/words/specific_case.txt`): the phrase gets the
/// dictionary capitalization.
#[test]
fn belarusian_specific_case() {
    let _guard = engine_guard();
    let text = "Вялікая айчынная Вайна — гэта тэрмін.";
    let matches = one(text, "BE_SPECIFIC_CASE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 22));
    assert_eq!(
        matches[0].message,
        "Калі гэта уласнае імя або назва, выкарыстоўвайце прапанаванае напісанне."
    );
    assert_eq!(matches[0].short_message.as_deref(), Some("Proper noun"));
    assert_eq!(suggestions(&matches[0]), vec!["Вялікая Айчынная вайна"]);
}

/// `UPPERCASE_SENTENCE_START` (Belarusian `MessagesBundle_be` strings).
#[test]
fn belarusian_uppercase_start() {
    let _guard = engine_guard();
    let text = "гэта тэст";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 4));
    assert_eq!(
        matches[0].message,
        "Гэты сказ не пачынаецца з вялікай літары"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Гэта"]);
}

/// Correct Belarusian text with the `’`/`'` apostrophe variants is clean
/// (the tokenizer keeps the apostrophe inside the word).
#[test]
fn belarusian_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(be) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    for text in [
        "Беларуская мова — гэта прыгожая мова.",
        "Беларусь — мая Радзіма.",
        "камп'ютар",
        "камп’ютар",
    ] {
        let result = be.check(text).expect("check");
        assert!(result.matches.is_empty(), "{text}: {:?}", result.matches);
    }
}
