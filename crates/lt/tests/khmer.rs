//! Khmer engine tests: stage-1/2/3 state and Java-probed rule behaviour.
//!
//! `Khmer` is a plain `Language` with a `BaseTagger` (`KhmerTagger` over the
//! in-tree FSA5 `khmer.dict`), the `KhmerWordTokenizer` (a delimiter
//! `WordTokenizer` subclass — no segmentation), the plain
//! `XmlRuleDisambiguator` (`km/disambiguation.xml`) and the hunspell speller
//! (`km_KH`). `Khmer.getRelevantRules` has exactly five classes and no generic
//! built-ins. Every expectation below was probed against the pinned Java build
//! (`scripts/oracle/km/check-diff-km.sh`, 0/0/0 on the probe set); UTF-16
//! offsets are asserted with the `common::assert_utf16` helper.

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
    Engine::builder(Lang::Km).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Km)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(km) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    km.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: the `grammar.xml` rule units compile (`compile_failures()` = 0)
/// and the inventory-reported active count.
#[test]
fn khmer_engine_state() {
    let _guard = engine_guard();
    let Some(km) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert!(
        km.compile_failures().is_empty(),
        "{:?}",
        km.compile_failures()
    );
    // `grammar.xml` parses to 33 rule units; four are `default="off"`
    // (`NOUN_POSESIVEPRONOUN`, `ADJ_NOUN`, `Gender_spec`, `NEG_PART_POS`), so
    // 29 are active by default. `lt-cli inventory` reports the parsed 33.
    assert_eq!(km.grammar_rule_count(), 33);
    assert_eq!(km.active_rule_count(), 29);
    assert_eq!(km.skipped_counts().filters, 0);
    assert_eq!(km.skipped_counts().off_by_default, 4);
}

/// The engine resolves `km` / `km-KH` to the same language.
#[test]
fn khmer_language_metadata() {
    assert_eq!(Lang::from_long_code("km"), Some(Lang::Km));
    assert_eq!(Lang::from_long_code("km-KH"), Some(Lang::Km));
    assert_eq!(Lang::Km.base_code(), "km");
    assert_eq!(Lang::Km.info().long_code, "km-KH");
    assert_eq!(Lang::Km.info().name, "Khmer");
}

/// `KhmerWordTokenizer` is the base `WordTokenizer` plus a hand-written
/// delimiter set: the zero-width space `U+200B` splits, but `=` (in the base
/// set, not in the Khmer set) does not.
#[test]
fn khmer_tokenizer_delimiters() {
    let _guard = engine_guard();
    let Some(km) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let surfaces = |text: &str| -> Vec<String> {
        km.analyze(text)
            .into_iter()
            .flat_map(|s| s.tokens)
            .filter(|t| !t.is_sentence_start)
            .map(|t| t.surface().to_string())
            .collect()
    };
    // A Khmer run with no spaces stays a single token.
    assert_eq!(surfaces("កខគ"), vec!["កខគ".to_string()]);
    // ZWSP is a delimiter, `=` is not (unlike the full base tokenizing set).
    assert_eq!(
        surfaces("ក\u{200b}ខ"),
        vec!["ក".to_string(), "\u{200b}".to_string(), "ខ".to_string()]
    );
    assert_eq!(surfaces("ក=ខ"), vec!["ក=ខ".to_string()]);
}

/// XML `HOUY_NUNG`: `ហើយ នឹង` should be `ហើយ និង`.
#[test]
fn khmer_xml_houy_nung() {
    let _guard = engine_guard();
    let text = "នោះ\u{200b}ហើយ\u{200b}នឹង\u{200b}នេះ។";
    let matches = one(text, "HOUY_NUNG");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "ហើយ\u{200b}និង");
    assert_utf16(text, &matches[0], (4, 11));
}

/// XML rulegroup `Classifier_PEOPLE` (sub-rule 2, POS `NUM` + `CLS`).
#[test]
fn khmer_xml_classifier_people() {
    let _guard = engine_guard();
    let text = "ពួក\u{200b}គេ\u{200b}មនុស្ស\u{200b}ពីរ\u{200b}ក្បាល\u{200b}បាន\u{200b}ទៅ\u{200b}ផ្សារ។";
    let matches = one(text, "Classifier_PEOPLE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].sub_id.as_deref(), Some("2"));
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(
        suggestions,
        vec![
            "មនុស្ស\u{200b}ពីរ\u{200b}នាក់",
            "មនុស្ស\u{200b}ពីរ\u{200b}ក្រុម",
            "មនុស្ស\u{200b}ពីរ\u{200b}ពួក",
            "មនុស្ស\u{200b}ពីរ\u{200b}គូ",
        ]
    );
    assert_utf16(text, &matches[0], (7, 23));
}

/// XML `TonakeatWithNoun` (`<match regexp_match/regexp_replace>`): drop `ន៍`.
#[test]
fn khmer_xml_tonakeat_with_noun() {
    let _guard = engine_guard();
    let text = "ការ\u{200b}អភិវឌ្ឍន៍\u{200b}សង្គម។";
    let matches = one(text, "TonakeatWithNoun");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "អភិវឌ្ឍ");
    assert_utf16(text, &matches[0], (4, 13));
}

/// `KhmerSimpleReplaceRule` (`KM_SIMPLE_REPLACE`) over `km/rules/coherency.txt`.
#[test]
fn khmer_simple_replace() {
    let _guard = engine_guard();
    let text = "សំដី\u{200b}របស់\u{200b}គាត់";
    let matches = one(text, "KM_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        " Consider following the spelling of Chuon Nath "
    );
    assert_eq!(matches[0].suggestions[0].value, "សម្ដី");
    assert_utf16(text, &matches[0], (0, 4));
}

/// `KhmerWordRepeatRule` (`KM_WORD_REPEAT_RULE`).
#[test]
fn khmer_word_repeat() {
    let _guard = engine_guard();
    let text = "ខ្ញុំ\u{200b}បាន\u{200b}បាន\u{200b}ទៅ។";
    let matches = one(text, "KM_WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, vec!["បាន បាន", "បាន", "បានៗ"]);
    assert_utf16(text, &matches[0], (6, 13));
}

/// `KhmerSpaceBeforeRule` (`KM_SPACE_BEFORE_CONJUNCTION`, default on).
#[test]
fn khmer_space_before_conjunction() {
    let _guard = engine_guard();
    let text = "ខ្ញុំ\u{200b}និង\u{200b}អ្នក";
    let matches = one(text, "KM_SPACE_BEFORE_CONJUNCTION");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, " និង");
    assert_utf16(text, &matches[0], (6, 9));
    // A real space before the conjunction is fine.
    assert_eq!(one("ខ្ញុំ និង អ្នក", "KM_SPACE_BEFORE_CONJUNCTION").len(), 0);
}

/// `KhmerUnpairedBracketsRule` (`KM_UNPAIRED_BRACKETS`): an excess closing
/// symbol is reported (an unclosed opening one is not, because Khmer
/// sentences end with `។`, which `endsLikeRealSentence` does not accept).
#[test]
fn khmer_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "ខ្ញុំ\u{200b}បាន\u{200b}ទៅ)។";
    let matches = one(text, "KM_UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "Unpaired symbol: '(' seems to be missing"
    );
    assert_utf16(text, &matches[0], (12, 13));
}

/// `KhmerHunspellRule` (`HUNSPELL_RULE`, `isLatinScript() = false`).
#[test]
fn khmer_hunspell_speller() {
    let _guard = engine_guard();
    let text = "សំដី\u{200b}របស់\u{200b}គាត់";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "អាចរកឃើញការប្រកបដែលមានកុំហុស");
    assert_eq!(matches[0].suggestions[0].value, "រប");
    assert_utf16(text, &matches[0], (5, 8));
}
