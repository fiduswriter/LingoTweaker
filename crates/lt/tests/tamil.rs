//! Tamil engine tests: stage-1/2/3 state and Java-probed rule behaviour.
//!
//! `Tamil` is a plain `Language` with a `BaseTagger` (`TamilTagger` over the
//! in-tree FSA5 `tamil.dict`), the base `WordTokenizer` (Tamil is written with
//! spaces — no segmentation) and the base no-op disambiguator. There is no
//! synthesizer and `createDefaultSpellingRule` is the base `null`, so there is
//! **no speller**. `Tamil.getRelevantRules` has exactly five generic classes.
//! Every expectation below is probed against the pinned Java build
//! (`scripts/oracle/ta/check-diff-ta.sh` and `scripts/oracle/ta/probe-rule.sh`);
//! UTF-16 offsets are asserted with the `common::assert_utf16` helper.

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
    Engine::builder(Lang::Ta).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ta)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn engine_with_options(options: EngineOptions) -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Ta)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ta) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ta.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: the `grammar.xml` rule units compile (`compile_failures()` = 0)
/// and the inventory-reported active count. Nine commented-out `<rule>` blocks
/// and two commented-out `<rulegroup>` blocks are XML comments; 210 live rules
/// (including the rulegroup sub-rules) are parsed and none is `default="off"`.
#[test]
fn tamil_engine_state() {
    let _guard = engine_guard();
    let Some(ta) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert!(
        ta.compile_failures().is_empty(),
        "{:?}",
        ta.compile_failures()
    );
    assert_eq!(ta.grammar_rule_count(), 210);
    assert_eq!(ta.active_rule_count(), 210);
    assert_eq!(ta.skipped_counts().filters, 0);
    assert_eq!(ta.skipped_counts().off_by_default, 0);
}

/// The engine resolves `ta` / `ta-IN` to the same language.
#[test]
fn tamil_language_metadata() {
    assert_eq!(Lang::from_long_code("ta"), Some(Lang::Ta));
    assert_eq!(Lang::from_long_code("ta-IN"), Some(Lang::Ta));
    assert_eq!(Lang::Ta.base_code(), "ta");
    assert_eq!(Lang::Ta.info().long_code, "ta-IN");
    assert_eq!(Lang::Ta.info().name, "Tamil");
}

/// `Tamil` does not override `createDefaultWordTokenizer`, so the base
/// `WordTokenizer` set applies: `=`/`*`/`|` are split (unlike `ml`/`km`).
#[test]
fn tamil_base_tokenizer() {
    let _guard = engine_guard();
    let Some(ta) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let surfaces = |text: &str| -> Vec<String> {
        ta.analyze(text)
            .into_iter()
            .flat_map(|s| s.tokens)
            .filter(|t| !t.is_sentence_start)
            .map(|t| t.surface().to_string())
            .collect()
    };
    assert_eq!(
        surfaces("ஒன்று,இரண்டு"),
        vec!["ஒன்று".to_string(), ",".to_string(), "இரண்டு".to_string()]
    );
    assert_eq!(
        surfaces("ஒன்று=இரண்டு"),
        vec!["ஒன்று".to_string(), "=".to_string(), "இரண்டு".to_string()]
    );
}

/// XML rule `noun_suffix_1`: `இது பற்றி` should be `இதுபற்றி` (postag/exception
/// machinery over the `TamilTagger`).
#[test]
fn tamil_xml_noun_suffix_1() {
    let _guard = engine_guard();
    let text = "நான் இது பற்றி உன்னிடம் நாளை பேசுகிறேன்.";
    let matches = one(text, "noun_suffix_1");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "இதுபற்றி");
    assert_utf16(text, &matches[0], (5, 14));
}

/// XML rule `illai`: `பார்ப்பது இல்லை` should be `பார்ப்பதில்லை`.
#[test]
fn tamil_xml_illai() {
    let _guard = engine_guard();
    let text = "ஏன் உன் விழிகள் என்னைப் பார்ப்பது இல்லை?";
    let matches = one(text, "illai");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "பார்ப்பதில்லை");
    assert_utf16(text, &matches[0], (24, 39));
}

/// XML rule `uNdu`: `வருவது உண்டு` should be `வருவதுண்டு`.
#[test]
fn tamil_xml_undu() {
    let _guard = engine_guard();
    let text = "அவர் இங்கு வருவது உண்டு.";
    let matches = one(text, "uNdu");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "வருவதுண்டு");
    assert_utf16(text, &matches[0], (11, 23));
}

/// XML rule `vegu`: `வெகு காலம்` should be `வெகுகாலம்`.
#[test]
fn tamil_xml_vegu() {
    let _guard = engine_guard();
    let text = "காந்தாரி வெகு காலம் கர்ப்பமாக இருந்தாள்.";
    let matches = one(text, "vegu");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "வெகுகாலம்");
    assert_utf16(text, &matches[0], (9, 19));
}

/// `CommaWhitespaceRule` (`COMMA_PARENTHESIS_WHITESPACE`) with the
/// `MessagesBundle_ta` strings.
#[test]
fn tamil_comma_whitespace() {
    let _guard = engine_guard();
    let text = "அவன் வந்தான் .";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "முற்றுப்புள்ளிக்கு முன் ஒரு வெளியை இடாதே");
    assert_eq!(matches[0].category_name, "அச்சுக்கலை");
    assert_eq!(matches[0].suggestions[0].value, ".");
    assert_utf16(text, &matches[0], (12, 14));

    let open = "அவன் ( வந்தான்.";
    let matches = one(open, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "திறப்பு அடைப்புக்குப் பின் ஒரு வெளியை இடாதே");
    assert_utf16(open, &matches[0], (5, 7));
}

/// `DoublePunctuationRule` (`DOUBLE_PUNCTUATION`).
#[test]
fn tamil_double_punctuation() {
    let _guard = engine_guard();
    let text = "அவன் வந்தான் ..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "இரு அடுத்தடுத்த புள்ளிகள்");
    assert_eq!(matches[0].category_name, "நிறுத்தக்குறியீடு");
    assert_utf16(text, &matches[0], (13, 15));
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`).
#[test]
fn tamil_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "அவன்  வந்தான்.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "அச்சுப்பிழை: நீங்கள் ஒரு வெண்வெளியைத் திரும்ப இட்டிருக்கிறீர்கள்"
    );
    assert_eq!(matches[0].suggestions[0].value, " ");
    assert_utf16(text, &matches[0], (4, 6));
}

/// `SentenceWhitespaceRule` (`SENTENCE_WHITESPACE`).
#[test]
fn tamil_sentence_whitespace() {
    let _guard = engine_guard();
    let text = "இது ஒரு சோதனை.  அவன் வந்தான்";
    let matches = one(text, "SENTENCE_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "அச்சுப்பிழை: நீங்கள் ஒரு வெண்வெளியைத் திரும்ப இட்டிருக்கிறீர்கள்"
    );
    assert_utf16(text, &matches[0], (14, 16));
}

/// `LongSentenceRule(messages, userConfig, 50)` (`TOO_LONG_SENTENCE`, picky):
/// a 55-word sentence is flagged with the Tamil description and English
/// message fallback.
#[test]
fn tamil_long_sentence() {
    let _guard = engine_guard();
    let Some(ta) = engine_with_options(EngineOptions {
        picky: true,
        ..Default::default()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let text = vec!["சொல்"; 55].join(" ") + ".";
    let matches: Vec<lt::Match> = ta
        .check(&text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == "TOO_LONG_SENTENCE")
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].description,
        "வாசிப்பு: 50 சொற்களுக்குக் கூடுதலான வாக்கியம்"
    );
    assert_eq!(
        matches[0].message,
        "This sentence is over 50 words long at the marked position, consider revising"
    );
    assert_eq!(matches[0].category_name, "பாணி");
}

/// There is no speller: a misspelled Latin token produces no spelling match.
#[test]
fn tamil_has_no_speller() {
    let _guard = engine_guard();
    let Some(ta) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = ta.check("Aagohw").expect("check");
    let rules: Vec<&str> = result.matches.iter().map(|m| m.rule_id.as_str()).collect();
    assert!(
        rules
            .iter()
            .all(|r| *r != "MORFOLOGIK_RULE_TA_IN" && !r.contains("SPELL")),
        "{rules:?}"
    );
}
