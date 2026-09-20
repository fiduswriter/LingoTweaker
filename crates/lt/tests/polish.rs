//! Polish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Offsets are UTF-8 bytes (the engine
//! format); the Java probes print UTF-16 code units, converted in the comments
//! where a test exercises diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, Lang};

/// One engine at a time: the Polish engines hold the tagger dictionary.
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
    Engine::builder(Lang::Pl).ok()?.data_dir(data).build().ok()
}

/// Stage state: active XML rule count, disambiguation rules and
/// `compile_failures()` = 0. Updated by each stage.
#[test]
fn polish_engine_state() {
    let _guard = engine_guard();
    let Some(pl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    eprintln!(
        "pl state: active={} disambig={} filters={} off={} failures={:?}",
        pl.active_rule_count(),
        pl.disambig_rule_count(),
        pl.skipped_counts().filters,
        pl.skipped_counts().off_by_default,
        pl.compile_failures()
    );
    assert_eq!(pl.active_rule_count(), 1773);
    assert_eq!(pl.disambig_rule_count(), 1346);
    assert_eq!(pl.skipped_counts().filters, 0);
    assert_eq!(pl.skipped_counts().off_by_default, 13);
    assert!(
        pl.compile_failures().is_empty(),
        "{:?}",
        pl.compile_failures()
    );
}
