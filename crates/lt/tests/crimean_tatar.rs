//! Crimean Tatar engine tests: stage-1 XML wiring state plus Java-probed
//! built-in rule, speller and synthesizer values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/crh/check-diff-crh.sh`,
//! `scripts/oracle/crh/probe-rule.sh`); the probes use real Crimean Tatar
//! orthography (`ı ñ ğ ü ş ö ç â`).
//! `CrimeanTatar` has no `MessagesBundle_crh` (core English strings), a
//! `BaseTagger` with `tagLowercaseWithUppercase = false`, a `BaseSynthesizer`
//! and no disambiguator.

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
    Engine::builder(Lang::Crh).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Crh)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(crh) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    crh.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 93 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn crimean_tatar_engine_state() {
    let _guard = engine_guard();
    let Some(crh) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(crh.active_rule_count(), 93);
    assert_eq!(crh.skipped_counts().filters, 0);
    assert!(
        crh.compile_failures().is_empty(),
        "{:?}",
        crh.compile_failures()
    );
}

/// `COMMA_PARENTHESIS_WHITESPACE` (core English `space_after_comma`).
#[test]
fn crimean_tatar_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Özü bir çaqrım uzaqta , demiryol keçidi yanında yaşay.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (21, 23));
    assert_eq!(
        matches[0].message,
        "Put a space after the comma, but not before the comma."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DOUBLE_PUNCTUATION` (core English `two_dots`).
#[test]
fn crimean_tatar_double_punctuation() {
    let _guard = engine_guard();
    let text = "Özü ..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (4, 6));
    assert_eq!(matches[0].message, "Two consecutive dots");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UPPERCASE_SENTENCE_START` (core English `incorrect_case`).
#[test]
fn crimean_tatar_uppercase_start() {
    let _guard = engine_guard();
    let text = "özü bir çaqrım.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 3));
    assert_eq!(
        matches[0].message,
        "This sentence does not start with an uppercase letter."
    );
    assert_eq!(suggestions(&matches[0]), vec!["Özü"]);
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`, core English strings).
#[test]
fn crimean_tatar_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Özü  bir çaqrım.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (3, 5));
    assert_eq!(
        matches[0].message,
        "Possible typo: you repeated a whitespace"
    );
    assert_eq!(suggestions(&matches[0]), vec![" "]);
}

/// `POSTPOSITION_CASE_COLLOCATION`: the `postag_regexp`/`postag_replace`
/// `<match>` synthesizes the dative form through the
/// `CrimeanTatarSynthesizer`.
#[test]
fn crimean_tatar_synthesis_suggestion() {
    let _guard = engine_guard();
    let text = "Terekniñ qarşı oturdı.";
    let matches = one(text, "POSTPOSITION_CASE_COLLOCATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 8));
    assert_eq!(
        matches[0].message,
        "Munasebetçi kelişi şöyle uyğulaşmalı: \n            \
         <suggestion>Terekke</suggestion>, <suggestion>terekke</suggestion>\n          "
    );
    // Java's `RuleMatch` `LinkedHashSet` dedupes after the sentence-start
    // case conversion, so the two case forms collapse to one.
    assert_eq!(suggestions(&matches[0]), vec!["Terekke"]);
}

/// `MorfologikCrimeanTatarSpellerRule` (`MORFOLOGIK_RULE_CRH_UA`) with the
/// Java suggestion list.
#[test]
fn crimean_tatar_speller_suggestions() {
    let _guard = engine_guard();
    let text = "Meclis toplaşuvı olıp keçti.";
    let matches = one(text, "MORFOLOGIK_RULE_CRH_UA");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (7, 16));
    assert_eq!(matches[0].message, "Possible spelling mistake found.");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "toplanuvı",
            "toplaşuv",
            "toplaşuvnı",
            "toplaşuvuı",
            "toplaşuvım",
            "toplaşuvıñ"
        ]
    );
}

/// Correct Crimean Tatar text is clean.
#[test]
fn crimean_tatar_correct_text_is_clean() {
    let _guard = engine_guard();
    let Some(crh) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = crh.check("Men keldim.").expect("check");
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}
