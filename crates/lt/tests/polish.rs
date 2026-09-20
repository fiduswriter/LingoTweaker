//! Polish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Offsets are UTF-8 bytes (the engine
//! format); the Java probes print UTF-16 code units, converted in the comments
//! where a test exercises diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

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

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Pl)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule suggestions as plain strings.
fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
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

/// `MorfologikPolishSpellerRule` (`MORFOLOGIK_RULE_PL_PL`), Java-probed with
/// `scripts/oracle/pl/probe-speller.sh` (UTF-16 offsets; the test sentences
/// are ASCII up to the offsets, so byte == code unit here).
#[test]
fn polish_speller_matches_java_probe() {
    let _guard = engine_guard();
    let Some(pl) = engine_with_rules(&["MORFOLOGIK_RULE_PL_PL"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let matches = |text: &str| -> Vec<lt::Match> {
        pl.check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "MORFOLOGIK_RULE_PL_PL")
            .collect()
    };

    // Two misspellings, with the full first suggestion list (Java probe).
    let ms = matches("To jest zdanie z bledem i slowem.");
    assert_eq!(ms.len(), 2);
    assert_eq!((ms[0].range.start, ms[0].range.end), (17, 23));
    assert_eq!((ms[1].range.start, ms[1].range.end), (26, 32));
    assert_eq!(
        suggestions(&ms[0]),
        vec![
            "błędem",
            "Bledem",
            "biedę",
            "błotem",
            "biletem",
            "blatem",
            "obłędem",
            "pledem",
            "błędom",
            "baletem",
            "blefem",
            "fletem",
            "Biedę",
            "Blidę",
            "betem",
            "Bredę",
            "Kletem",
            "bleedem",
            "bletek",
            "ględę",
            "łętem",
            "ble dem",
        ]
    );
    assert_eq!(suggestions(&ms[1])[0], "słowem");

    // `tokenizingPattern`: the `Niby-` prefix is dropped, the offset moves.
    let ms = matches("Niby-czlowiek przyszedl.");
    assert_eq!(
        ms.iter()
            .map(|m| (m.range.start, m.range.end))
            .collect::<Vec<_>>(),
        vec![(5, 13), (14, 23)]
    );
    assert_eq!(suggestions(&ms[0])[0], "Człowiek");
    assert_eq!(suggestions(&ms[1])[0], "przyszedł");

    // `Quasi-naukowy` is accepted (no match); `wyklad` is matched at its
    // offset inside the sentence.
    let ms = matches("quasi-naukowy wyklad.");
    assert_eq!(
        ms.iter()
            .map(|m| (m.range.start, m.range.end))
            .collect::<Vec<_>>(),
        vec![(14, 20)]
    );

    // `isNotCompound` accepts valid compounds/prefix words (Java: no match).
    for word in [
        "trzynastobitowy",
        "supernowoczesny",
        "pseudonaukowy",
        "arcytrudny",
        "wysokoprocentowy",
        "dwudziestoczterogodzinny",
    ] {
        assert!(
            matches(word).is_empty(),
            "{word} should be accepted by isNotCompound"
        );
    }

    // The sentence-start capitalization of suggestions.
    let ms = matches("Zrobilem to wczoraj.");
    assert_eq!(suggestions(&ms[0])[0], "Zrobiłem");
}
