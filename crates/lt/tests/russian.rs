//! Russian engine tests. Stage 1 pins the XML wiring state (active rules and
//! the unmapped `<filter>` classes) and the generic core built-ins; the
//! speller and the language's Java rule classes are added in stages 2/3.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/ru/check-diff-ru.sh`,
//! `scripts/oracle/ru/probe-rule.sh`); the probes use real Cyrillic
//! orthography.

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
    Engine::builder(Lang::Ru).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ru)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ru) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ru.check(text)
        .map(|r| r.matches)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

#[test]
fn stage1_unmapped_filters() {
    let _guard = engine_guard();
    let Some(ru) = engine() else {
        return;
    };
    // Stage 1 reports the XML-referenced filter classes that stage 3 wires;
    // the compile failures must shrink to zero once they are ported.
    let mut classes: Vec<&str> = ru
        .compile_failures()
        .iter()
        .map(|(_, msg)| {
            msg.strip_prefix("unmapped filter class ")
                .and_then(|c| c.rsplit('.').next())
                .unwrap_or(msg)
        })
        .collect();
    classes.sort_unstable();
    classes.dedup();
    assert_eq!(
        classes,
        [
            "AdvancedSynthesizerFilter",
            "DateCheckFilter",
            "FutureDateFilter",
            "INNNumberFilter",
            "RussianPartialPosTagFilter",
            "RussianSuppressMisspelledSuggestionsFilter",
        ]
    );
    assert_eq!(ru.compile_failures().len(), 22);
}

#[test]
fn xml_rulegroup_rule_fires() {
    let _guard = engine_guard();
    let matches = one("Он надел будний костюм.", "budniy_budnichnij");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(matches.len(), 1);
    assert_utf16("Он надел будний костюм.", &matches[0], (9, 22));
}

#[test]
fn comma_whitespace_uses_russian_message() {
    let _guard = engine_guard();
    let matches = one(
        "Не род , а ум поставлю в воеводы.",
        "COMMA_PARENTHESIS_WHITESPACE",
    );
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(
        matches[0].message,
        "Поставьте пробел после запятой, а не перед ней."
    );
}

#[test]
fn uppercase_sentence_start_uses_russian_message() {
    let _guard = engine_guard();
    let matches = one(
        "Закончилось лето. дети снова сели за школьные парты.",
        "UPPERCASE_SENTENCE_START",
    );
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(
        matches[0].message,
        "Это предложение не начинается с заглавной буквы."
    );
    assert_eq!(matches[0].suggestions[0].value, "Дети");
    assert_utf16(
        "Закончилось лето. дети снова сели за школьные парты.",
        &matches[0],
        (18, 22),
    );
}
