//! Galician engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Offsets are UTF-8 bytes (the engine
//! format); the Java probes print UTF-16 code units, converted in the comments.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Galician engines hold the tagger dictionary.
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
    Engine::builder(Lang::Gl).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Gl)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Stage-1 state: the active XML rules and the `AdvancedSynthesizerFilter`
/// rule that still fails to compile until stage 3.
#[test]
fn galician_engine_state_stage1() {
    let _guard = engine_guard();
    let Some(gl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    eprintln!("active_rules = {}", gl.active_rule_count());
    eprintln!("disambig_rules = {}", gl.disambig_rule_count());
    eprintln!("skipped = {:?}", gl.skipped_counts());
    eprintln!("compile_failures = {:?}", gl.compile_failures());
}

/// `UppercaseSentenceStartRule` (Galician.java example): the second sentence
/// starts lowercase.
#[test]
fn galician_uppercase_sentence_start() {
    let _guard = engine_guard();
    let Some(gl) = engine_with_rules(&["UPPERCASE_SENTENCE_START"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = gl
        .check("Esta casa é vella. foi construida en 1950.")
        .expect("check");
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "UPPERCASE_SENTENCE_START")
        .expect("uppercase match");
    assert_eq!(m.range.start, 20);
    assert_eq!(m.range.end, 23);
    assert_eq!(m.suggestions[0].value, "Foi");
}
