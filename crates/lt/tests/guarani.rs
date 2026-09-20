//! Guaraní (`gn`/`gug`) engine tests.
//!
//! Hand-authored language: no upstream Java module. The tests pin the staged
//! rules and the curated examples from
//! `new-languages/guarani/proposed-rules.md` (constructed examples are marked
//! there and still need owner confirmation).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, Lang};

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

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = lt::EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Gn)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Gn).ok()?.data_dir(data).build().ok()
}

fn match_ids(engine: &Engine, text: &str) -> Vec<String> {
    engine
        .check(text)
        .map(|r| r.matches.iter().map(|m| m.rule_id.clone()).collect())
        .unwrap_or_default()
}

fn hits(engine: &Engine, text: &str, rule_id: &str) -> bool {
    match_ids(engine, text).iter().any(|id| id == rule_id)
}

/// Stage-3 wiring state: 3 active XML rules, no unmapped filter, no compile
/// failure.
#[test]
fn guarani_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert_eq!(engine.active_rule_count(), 3);
    assert!(engine.compile_failures().is_empty());
    let skipped = engine.skipped_counts();
    assert_eq!(skipped.filters, 0);
    assert_eq!(skipped.uncompilable, 0);
}

/// Approved rule examples that must fire with default options.
#[test]
fn guarani_rules_fire() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str)] = &[
        ("ñee", "GN_ORTHOGRAPHY"),
        ("kaa", "GN_ORTHOGRAPHY"),
        ("mokói", "GN_ORTHOGRAPHY"),
        ("ñúpe", "GN_ORTHOGRAPHY"),
        ("nde tĩ", "GN_ORTHOGRAPHY"),
        ("johetũ", "GN_HARMONY"),
        ("osẽpa", "GN_HARMONY"),
        ("óga pe", "GN_POSTPOSITIONS"),
        ("mitãpe", "GN_PE_ME"),
        ("aha ta", "GN_PARTICLES"),
        ("óga óga", "GN_WORD_REPETITION"),
    ];
    for (text, rule_id) in cases {
        assert!(
            hits(&engine, text, rule_id),
            "expected {rule_id} for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
}

/// `GN_ACCENTS`: the dictionary-driven accent/tilde restoration.
#[test]
fn guarani_accents() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in ["ara", "purahei", "mbo'ehara"] {
        assert!(
            hits(&engine, text, "GN_ACCENTS"),
            "expected GN_ACCENTS for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
    for text in ["ára", "purahéi", "mbo'ehára", "mokõi", "tupa"] {
        assert!(
            !hits(&engine, text, "GN_ACCENTS"),
            "unexpected GN_ACCENTS for {text:?}"
        );
    }
}

/// `GN_HARMONY`: the dictionary-driven nasal-harmony alternations. The
/// harmony-specific assertions use an engine with only `GN_HARMONY` active
/// because curated rules cover some of the same tokens.
#[test]
fn guarani_harmony() {
    let _guard = engine_guard();
    let Some(harmony) = engine_with_rules(&["GN_HARMONY"]) else {
        return;
    };
    let cases: &[(&str, &str)] = &[
        ("ñúpe", "ñúme"),
        ("johetũ", "ñohetũ"),
        ("osẽpa", "osẽmba"),
        ("mitãkuéra", "mitãnguéra"),
        ("jamba'apo", "ñamba'apo"),
    ];
    for (text, expected) in cases {
        let result = harmony.check(text).expect("check");
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "GN_HARMONY")
            .unwrap_or_else(|| {
                panic!(
                    "expected GN_HARMONY for {text:?}, got {:?}",
                    result
                        .matches
                        .iter()
                        .map(|m| m.rule_id.clone())
                        .collect::<Vec<_>>()
                )
            });
        assert!(
            m.suggestions.iter().any(|s| s.value == *expected),
            "expected suggestion {expected:?} for {text:?}, got {:?}",
            m.suggestions
                .iter()
                .map(|s| s.value.clone())
                .collect::<Vec<_>>()
        );
    }
    for text in ["ñúme", "johecha", "ohopa", "mitãnguéra", "jakaru"] {
        assert!(
            !hits(&harmony, text, "GN_HARMONY"),
            "unexpected GN_HARMONY for {text:?}"
        );
    }
}

/// Default wiring: `GN_HARMONY` covers harmony errors the curated lists miss
/// and outranks the generic `GN_SPELLER`, while curated `GN_ORTHOGRAPHY`
/// entries keep winning over it.
#[test]
fn guarani_harmony_priority() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in ["johetũ", "osẽpa"] {
        assert!(
            hits(&engine, text, "GN_HARMONY"),
            "expected GN_HARMONY for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
    for text in ["ñúpe", "mitãkuéra", "jamba'apo"] {
        let ids = match_ids(&engine, text);
        assert!(
            ids.contains(&"GN_ORTHOGRAPHY".to_string()),
            "curated GN_ORTHOGRAPHY must win for {text:?}, got {ids:?}"
        );
        assert!(
            !ids.contains(&"GN_HARMONY".to_string()),
            "GN_HARMONY must not shadow GN_ORTHOGRAPHY for {text:?}"
        );
    }
}

/// Correct Guaraní stays clean.
#[test]
fn guarani_correct_sentences() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in [
        "ñe'ẽ",
        "ka'a",
        "mokõi",
        "ñúme",
        "ne tĩ",
        "ógape",
        "mitãme",
        "ka'aguýpe",
        "mba'éichapa nde",
        "aháta",
        "johecha",
        "ohopa",
        "mitãnguéra",
        "jakaru",
    ] {
        let ids = match_ids(&engine, text);
        assert!(ids.is_empty(), "unexpected matches for {text:?}: {ids:?}");
    }
}

/// Capitalization (default on) and the default-off loan/jopara rules.
#[test]
fn guarani_stage4_rules() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(hits(&engine, "ñe'ẽ Guarani", "GN_CAPITALIZATION"));
    assert!(!hits(&engine, "ñe'ẽ guarani", "GN_CAPITALIZATION"));
    for (text, rule) in [
        ("queso", "GN_FOREIGN_QU"),
        ("zapato", "GN_FOREIGN_Z"),
        ("centro", "GN_FOREIGN_C"),
        ("pero", "GN_JOPARA"),
    ] {
        assert!(
            !hits(&engine, text, rule),
            "{rule} must be default-off ({text:?})"
        );
    }
    let Some(enabled) =
        engine_with_rules(&["GN_FOREIGN_QU", "GN_FOREIGN_Z", "GN_FOREIGN_C", "GN_JOPARA"])
    else {
        return;
    };
    for (text, rule) in [
        ("queso", "GN_FOREIGN_QU"),
        ("zapato", "GN_FOREIGN_Z"),
        ("centro", "GN_FOREIGN_C"),
        ("pero", "GN_JOPARA"),
    ] {
        assert!(
            hits(&enabled, text, rule),
            "expected {rule} for {text:?}, got {:?}",
            match_ids(&enabled, text)
        );
    }
    for text in ["keso", "sapato", "sentro", "ha katu"] {
        assert!(
            !match_ids(&enabled, text)
                .iter()
                .any(|id| id.starts_with("GN_FOREIGN") || id == "GN_JOPARA"),
            "unexpected adapted-loan match for {text:?}"
        );
    }
}

/// The `gug` Hunspell speller plus the curated added vocabulary.
#[test]
fn guarani_speller() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(&engine, "óga", "GN_SPELLER"));
    assert!(!hits(&engine, "mitãnguéra", "GN_SPELLER"));
    let result = engine.check("jaguá").unwrap();
    assert!(
        result
            .matches
            .iter()
            .any(|m| m.rule_id == "GN_ACCENTS" || m.rule_id == "GN_SPELLER"),
        "expected an accent/spelling match, got {:?}",
        result
            .matches
            .iter()
            .map(|m| m.rule_id.clone())
            .collect::<Vec<_>>()
    );
}
