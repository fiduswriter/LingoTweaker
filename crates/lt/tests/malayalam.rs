//! Malayalam engine tests: stage-1/2/3 state and Java-probed rule behaviour.
//!
//! `Malayalam` is a plain (upstream-deprecated) `Language` with a `BaseTagger`
//! (`MalayalamTagger` over the in-tree FSA5 `malayalam.dict`), the
//! `MalayalamWordTokenizer` (a small delimiter `StringTokenizer` set — no
//! segmentation) and the base no-op disambiguator. `Malayalam.getRelevantRules`
//! has exactly seven classes: six generic built-ins plus the Morfologik speller
//! (`MORFOLOGIK_RULE_ML_IN`). Every expectation below was probed against the
//! pinned Java build (`scripts/oracle/ml/check-diff-ml.sh` and
//! `scripts/oracle/ml/probe-rule.sh`, 0/0/0 on the probe set); UTF-16 offsets
//! are asserted with the `common::assert_utf16` helper.

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
    Engine::builder(Lang::Ml).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ml)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ml) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ml.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: the `grammar.xml` rule units compile (`compile_failures()` = 0)
/// and the inventory-reported active count. The five commented-out rule blocks
/// in `grammar.xml` are XML comments, so only the 18 live rules are parsed;
/// none is `default="off"`.
#[test]
fn malayalam_engine_state() {
    let _guard = engine_guard();
    let Some(ml) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert!(
        ml.compile_failures().is_empty(),
        "{:?}",
        ml.compile_failures()
    );
    assert_eq!(ml.grammar().expect("grammar").rules.len(), 18);
    assert_eq!(ml.active_rule_count(), 18);
    assert_eq!(ml.skipped_counts().filters, 0);
    assert_eq!(ml.skipped_counts().off_by_default, 0);
}

/// The engine resolves `ml` / `ml-IN` to the same language.
#[test]
fn malayalam_language_metadata() {
    assert_eq!(Lang::from_long_code("ml"), Some(Lang::Ml));
    assert_eq!(Lang::from_long_code("ml-IN"), Some(Lang::Ml));
    assert_eq!(Lang::Ml.base_code(), "ml");
    assert_eq!(Lang::Ml.info().long_code, "ml-IN");
    assert_eq!(Lang::Ml.info().name, "Malayalam");
}

/// `MalayalamWordTokenizer` is the explicit small delimiter set, not the base
/// `WordTokenizer` set: comma and space split, `=`/`*`/`|` do **not**.
#[test]
fn malayalam_tokenizer_delimiters() {
    let _guard = engine_guard();
    let Some(ml) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let surfaces = |text: &str| -> Vec<String> {
        ml.analyze(text)
            .into_iter()
            .flat_map(|s| s.tokens)
            .filter(|t| !t.is_sentence_start)
            .map(|t| t.surface().to_string())
            .collect()
    };
    assert_eq!(
        surfaces("ഒന്ന്,രണ്ട്"),
        vec!["ഒന്ന്".to_string(), ",".to_string(), "രണ്ട്".to_string()]
    );
    assert_eq!(surfaces("ഒന്ന്=രണ്ട്"), vec!["ഒന്ന്=രണ്ട്".to_string()]);
    assert_eq!(surfaces("ഒന്ന്*രണ്ട്"), vec!["ഒന്ന്*രണ്ട്".to_string()]);
    assert_eq!(surfaces("ഒന്ന്|രണ്ട്"), vec!["ഒന്ന്|രണ്ട്".to_string()]);
}

/// XML rule `അവളെ` (No. 1): `അവളെ` should be `അവള്‍ക്ക്`.
#[test]
fn malayalam_xml_avale() {
    let _guard = engine_guard();
    let text = "ഞാന്‍ അവളെ ഒരു പുസ്തകം നല്‍കി.";
    let matches = one(text, "അവളെ");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "അവള്‍ക്ക്");
    assert_utf16(text, &matches[0], (6, 10));
}

/// XML rule `ഭയങ്കര` (No. 23): `ഭയങ്കര` should be `വളരെ`.
#[test]
fn malayalam_xml_bhayankara() {
    let _guard = engine_guard();
    let text = "അമ്മ്യക്ക് എന്നെ ഭയങ്കര ഇഷ്ടമാണ്.";
    let matches = one(text, "ഭയങ്കര");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "വളരെ");
    assert_utf16(text, &matches[0], (17, 23));
}

/// XML `rule14`: `<token skip="1">` + an empty `<token/>` + the `\3`
/// back-reference in the suggestion (`കൂടെ അവളും കൂടി പോയി` -> `പോയി`).
#[test]
fn malayalam_xml_rule14_backref() {
    let _guard = engine_guard();
    let text = "അച്ഛന്‍റെ കൂടെ അവളും കൂടി പോയി.";
    let matches = one(text, "rule14");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "പോയി");
    assert_utf16(text, &matches[0], (21, 30));
}

/// XML `rule12`: the four-token redundancy with two suggestions.
#[test]
fn malayalam_xml_rule12_multi_suggestion() {
    let _guard = engine_guard();
    let text = "വേറെയും ഒരു കാര്യം കൂടി പറയാന്‍ ഞാന്‍ ആഗ്രഹിക്കുന്നു .";
    let matches = one(text, "rule12");
    assert_eq!(matches.len(), 1);
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, vec!["ഒരു കാര്യം കൂടി", "വേറെയും ഒരു കാര്യം"]);
    assert_utf16(text, &matches[0], (0, 23));
}

/// `CommaWhitespaceRule` (`COMMA_PARENTHESIS_WHITESPACE`): the space before the
/// final full stop (base English message, no `MessagesBundle_ml`).
#[test]
fn malayalam_comma_whitespace() {
    let _guard = engine_guard();
    let text = "വേറെയും ഒരു കാര്യം കൂടി പറയാന്‍ ഞാന്‍ ആഗ്രഹിക്കുന്നു .";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "Don't put a space before the full stop."
    );
    assert_eq!(matches[0].suggestions[0].value, ".");
    assert_utf16(text, &matches[0], (52, 54));
}

/// `CommaWhitespaceRule` around parentheses.
#[test]
fn malayalam_comma_whitespace_parens() {
    let _guard = engine_guard();
    let open = "ഞാന്‍ ( ഒരു പുസ്തകം.";
    let matches = one(open, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "Don't put a space after the opening parenthesis."
    );
    assert_utf16(open, &matches[0], (6, 8));

    let close = "ഞാന്‍ ഒരു പുസ്തകം )";
    let matches = one(close, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "Don't put a space before the closing parenthesis."
    );
    assert_utf16(close, &matches[0], (17, 19));
}

/// `WordRepeatRule` (`WORD_REPEAT_RULE`, base English strings).
#[test]
fn malayalam_word_repeat() {
    let _guard = engine_guard();
    let text = "പുസ്തകം പുസ്തകം വായിച്ചു.";
    let matches = one(text, "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Possible typo: you repeated a word.");
    assert_eq!(matches[0].suggestions[0].value, "പുസ്തകം");
    assert_utf16(text, &matches[0], (0, 15));
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`).
#[test]
fn malayalam_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "ഞാന്‍  ഒരു പുസ്തകം.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].message,
        "Possible typo: you repeated a whitespace"
    );
    assert_eq!(matches[0].suggestions[0].value, " ");
    assert_utf16(text, &matches[0], (5, 7));
}

/// `MorfologikMalayalamSpellerRule` (`MORFOLOGIK_RULE_ML_IN`,
/// `isLatinScript() = true`): only Latin-script tokens are checked; a pure
/// Malayalam token is never a spelling error.
#[test]
fn malayalam_morfologik_speller() {
    let _guard = engine_guard();
    let matches = one("Aagohw", "MORFOLOGIK_RULE_ML_IN");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Possible spelling mistake found.");
    assert!(matches[0].suggestions.is_empty());
    assert_utf16("Aagohw", &matches[0], (0, 6));

    let mixed = "എaങ്ങനെ";
    let matches = one(mixed, "MORFOLOGIK_RULE_ML_IN");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].suggestions[0].value, "എങ്ങനെ");
    assert_utf16(mixed, &matches[0], (0, 7));

    // A pure Malayalam word is ignored (no Latin letters).
    assert_eq!(one("ഞാന്‍ ഒരു പുസ്തകം.", "MORFOLOGIK_RULE_ML_IN").len(), 0);
}
