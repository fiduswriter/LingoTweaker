//! Breton engine tests: stage-1 XML wiring state plus Java-probed built-in
//! rule, speller, topo, date-filter and match-reference values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/br/check-diff-br.sh`,
//! `scripts/oracle/br/probe-rule.sh`); the probes use real Breton
//! orthography (`ñ`, `c’h`, the `’` apostrophe).
//! `Breton` has a plain `XmlRuleDisambiguator` (no global rules) and no
//! synthesizer; the `BretonTagger` reads the FSA5 `breton.dict`.

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
    Engine::builder(Lang::Br).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Br)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(br) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    br.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 675 active XML rules, 4 XML-referenced `DateCheckFilter`
/// filters and `compile_failures()` = 0.
#[test]
fn breton_engine_state() {
    let _guard = engine_guard();
    let Some(br) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(br.active_rule_count(), 675);
    assert_eq!(br.skipped_counts().filters, 0);
    assert!(
        br.compile_failures().is_empty(),
        "{:?}",
        br.compile_failures()
    );
}

/// `COMMA_PARENTHESIS_WHITESPACE` (Breton `no_space_before_dot`).
#[test]
fn breton_comma_whitespace() {
    let _guard = engine_guard();
    let text = "An ti zo bras .";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (13, 15));
    assert_eq!(matches[0].message, "Na lakait ur spas dirak ur pik: \".\"");
    assert_eq!(suggestions(&matches[0]), vec!["."]);
}

/// `DOUBLE_PUNCTUATION` (Breton `two_dots`/`double_dots_short`).
#[test]
fn breton_double_punctuation() {
    let _guard = engine_guard();
    let text = "Petra ..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (6, 8));
    assert_eq!(matches[0].message, "Daou bik diouzh renk");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Daou bik diouzh renk")
    );
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` (`incorrect_case`/`category_case`).
#[test]
fn breton_uppercase_start() {
    let _guard = engine_guard();
    let text = "an ti zo bras.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 2));
    assert_eq!(
        matches[0].message,
        "Ne grog ket ar frazenn-mañ gant ur bennlizherenn"
    );
    assert_eq!(matches[0].short_message.as_deref(), Some("Pennlizherennoù"));
    assert_eq!(suggestions(&matches[0]), vec!["An"]);
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`, Breton strings).
#[test]
fn breton_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "An  ti zo bras.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (2, 4));
    assert_eq!(
        matches[0].message,
        "Fazi bizskrivañ posupl: daou spas ho peus lakaet"
    );
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `SentenceWhitespaceRule` (`SENTENCE_WHITESPACE`, Breton strings).
#[test]
fn breton_sentence_whitespace() {
    let _guard = engine_guard();
    let text = "An ti.Mat eo.";
    let matches = one(text, "SENTENCE_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (6, 9));
    assert_eq!(matches[0].message, "Ouzhpennañ ur spas etre ar frazennoù");
    assert_eq!(suggestions(&matches[0]), vec![" Mat"]);
}

/// `TopoReplaceRule` (`BR_TOPO`): the French place name `Allemagne` is
/// flagged with the two Breton alternatives.
#[test]
fn breton_topo_replace() {
    let _guard = engine_guard();
    let text = "Emaon e Allemagne.";
    let matches = one(text, "BR_TOPO");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (8, 17));
    assert_eq!(
        matches[0].message,
        "Allemagne zo un anv lec’h gallek. Ha fellout a rae deoc’h skrivañ \
         <suggestion>Alamagn</suggestion> pe <suggestion>bro-Alamagn</suggestion>?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Alamagn", "bro-Alamagn"]);
}

/// `MorfologikBretonSpellerRule` (`MORFOLOGIK_RULE_BR_FR`): real orthography
/// and the Java suggestion list.
#[test]
fn breton_speller_suggestions() {
    let _guard = engine_guard();
    let text = "Ur ger zzqqx am eus.";
    let matches = one(text, "MORFOLOGIK_RULE_BR_FR");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (7, 12));
    assert_eq!(matches[0].message, "Fazi reizhskrivañ posupl kavet.");
    assert_eq!(suggestions(&matches[0]), vec!["ZPAQ"]);
}

/// The speller's multi-word `IGNORE_SPELLING` anti-patterns from
/// `core/spelling_global.txt` (`Jacques Tati`): the phrase is clean, while
/// `Tati` alone is flagged.
#[test]
fn breton_speller_multiword_antipattern() {
    let _guard = engine_guard();
    let phrase = one("Jacques Tati", "MORFOLOGIK_RULE_BR_FR");
    assert!(phrase.is_empty(), "{phrase:?}");
    let alone = one("Tati Bonaparte", "MORFOLOGIK_RULE_BR_FR");
    assert!(
        alone.iter().any(|m| m.range.start == 0),
        "expected Tati to be flagged: {alone:?}"
    );
}

/// `DEIZ_DEIZIAD` via the XML-referenced `DateCheckFilter`: the weekday of
/// `D’ar Merc'her 6 a viz C'hwevrer 2014` is not a Wednesday.
#[test]
fn breton_date_filter() {
    let _guard = engine_guard();
    let text = "D’ar Merc'her 6 a viz C'hwevrer 2014.";
    let matches = one(text, "DEIZ_DEIZIAD");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (5, 36));
    assert_eq!(
        matches[0].message,
        "N’eo ket ur «Merc’her» deiz an deiziad «6 a viz C’hwevrer 2014», ur/ul «Yaou» eo."
    );
}

/// `KLANV_PE_GLANVOCH`: `<match no="0" regexp_match=... regexp_replace=...>`
/// builds the comparative regexp from the first matched token.
#[test]
fn breton_match_reference_rule() {
    let _guard = engine_guard();
    let text = "Klañv pe klañvoc’h.";
    let matches = one(text, "KLANV_PE_GLANVOCH");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (9, 18));
    assert_eq!(
        matches[0].message,
        "Ur c’hemmadur de vlotaat a zlefe bezañ goude «pe». (CHALM Sb 1.4)"
    );
    assert_eq!(suggestions(&matches[0]), vec!["glañvoc’h"]);
}

/// `OCH_AN`: the second rule variant builds `Gwell` from `Gwelloc’h` via the
/// match reference.
#[test]
fn breton_och_an_rule() {
    let _guard = engine_guard();
    let text = "Diaesoc’h diaesañ eo kavout labour.";
    let matches = one(text, "OCH_AN");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 17));
    assert_eq!(
        matches[0].message,
        "Implijit <suggestion>Diaesoc’h-diaesañ</suggestion>. (CHALM Sb 8)"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Diaesoc’h-diaesañ"]);
}

/// Correct Breton text is clean.
#[test]
fn breton_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(br) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = br
        .check("An ti zo bras hag ar c’harr a zo ruz.")
        .expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
