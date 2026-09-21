//! Slovak engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/sk/probe-rule.sh`) print UTF-16 code units, converted
//! where a test exercises diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Slovak engines hold the tagger dictionary.
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
    Engine::builder(Lang::Sk).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Sk)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(sk) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    sk.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: active XML rule count, disambiguation rules and
/// `compile_failures()` = 0.
#[test]
fn slovak_engine_state() {
    let _guard = engine_guard();
    let Some(sk) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(sk.active_rule_count(), 62);
    assert_eq!(sk.disambig_rule_count(), 0);
    assert_eq!(sk.skipped_counts().filters, 0);
    assert!(
        sk.compile_failures().is_empty(),
        "{:?}",
        sk.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe: `Toto , je test.` 4..6.
#[test]
fn slovak_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("Toto , je test.", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 4);
    assert_eq!(matches[0].range.end, 6);
    assert_eq!(
        matches[0].message,
        "Vložte medzeru za čiarku, ale nie pred čiarku"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe: `Toto je test..` 12..14.
#[test]
fn slovak_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Toto je test..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 14);
    assert_eq!(matches[0].message, "Dve po sebe idúce bodky");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UppercaseSentenceStartRule` (4), Java probe: the second sentence starts
/// lowercase, 14..18.
#[test]
fn slovak_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one("Toto je test. toto je test.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 14);
    assert_eq!(matches[0].range.end, 18);
    assert_eq!(matches[0].message, "Táto veta nezačína s veľkým písmenom");
    assert_eq!(suggestions(&matches[0]), vec!["Toto"]);
}

/// `MultipleWhitespaceRule` (6), Java probe: `Toto  je test.` 4..6.
#[test]
fn slovak_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Toto  je test.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 4);
    assert_eq!(matches[0].range.end, 6);
    assert_eq!(
        matches[0].message,
        "Možný preklep: zopakovali ste \"biely znak\" (whitespace)"
    );
}

/// `WordRepeatRule` (5), Java probe: `Toto toto je test.` 0..9.
#[test]
fn slovak_word_repeat() {
    let _guard = engine_guard();
    let matches = one("Toto toto je test.", "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 9);
    assert_eq!(matches[0].message, "Možný preklep: zopakovali ste slovo");
    assert_eq!(suggestions(&matches[0]), vec!["Toto"]);
}

/// `MorfologikSlovakSpellerRule` (8), Java probe: `Toto je tset.` 8..12 with
/// the Java suggestion ranking.
#[test]
fn slovak_speller() {
    let _guard = engine_guard();
    let matches = one("Toto je tset.", "MORFOLOGIK_RULE_SK_SK");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 8);
    assert_eq!(matches[0].range.end, 12);
    assert_eq!(matches[0].message, "Nájdený pravdepodobný preklep");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["set", "test", "tesť", "tiet", "tse", "šét", "t set", "tse t"]
    );
}

/// `CompoundRule` (7, `SK_COMPOUNDS`), Java probe: `Toto je bielo červená.`
/// (Java 8..21 UTF-16 = 8..23 UTF-8).
#[test]
fn slovak_compound() {
    let _guard = engine_guard();
    let matches = one("Toto je bielo červená.", "SK_COMPOUNDS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 8);
    assert_eq!(matches[0].range.end, 23);
    assert_eq!(
        matches[0].message,
        "Toto slovo sa zvyčajne píše so spojovníkom."
    );
    assert_eq!(suggestions(&matches[0]), vec!["bielo-červená"]);
}

/// XML rule `POCITACOVE_UVODZOVKY` (`grammar-typography.xml`), Java-probed
/// via the corpus golden (`sk-full.java.tsv`).
#[test]
fn slovak_typography_quotes() {
    let _guard = engine_guard();
    let matches = one("Toto je (\"test\").", "POCITACOVE_UVODZOVKY");
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert_eq!(matches[0].range.start, 9);
    assert_eq!(matches[0].range.end, 10);
    assert_eq!(suggestions(&matches[0]), vec!["„"]);
    assert_eq!(matches[1].range.start, 14);
    assert_eq!(matches[1].range.end, 15);
    assert_eq!(suggestions(&matches[1]), vec!["“"]);
}

/// XML agreement rule `STREDNY_ROD_A` over the Slovak tagger (Java-probed
/// via the corpus golden): `pekné` should be `pekného` (Java 17..22 UTF-16 =
/// 17..23 UTF-8).
#[test]
fn slovak_agreement_rule() {
    let _guard = engine_guard();
    let matches = one(
        "Zamilovala sa do pekné inteligentného chlapa.",
        "STREDNY_ROD_A",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("2"));
    assert_eq!(matches[0].range.start, 17);
    assert_eq!(matches[0].range.end, 23);
    assert!(matches[0]
        .message
        .contains("Nespravny tvar prídavného mena"));
    assert_eq!(suggestions(&matches[0]), vec!["pekného"]);
}
