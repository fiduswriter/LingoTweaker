//! Norwegian Bokmål (`no`) engine tests.
//!
//! Hand-authored language: there is no upstream Java module to probe, so the
//! tests pin the staged rule wiring and the owner-approved examples from
//! `new-languages/norwegian-bokmal/proposed-rules.md`. Offsets are UTF-8
//! bytes (the engine format).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

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
    Engine::builder(Lang::No).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::No)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
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

/// Stage-3 wiring state: 24 active XML rules, no unmapped filter, no compile
/// failure.
#[test]
fn norwegian_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert_eq!(engine.active_rule_count(), 24);
    assert!(engine.compile_failures().is_empty());
    let skipped = engine.skipped_counts();
    assert_eq!(skipped.filters, 0);
    assert_eq!(skipped.uncompilable, 0);
}

/// Approved rule examples that must fire with default options.
#[test]
fn norwegian_rules_fire() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str)] = &[
        ("Pakka er klar til og hentes.", "NB_TIL_OG_A"),
        ("Jeg har lyst til og reise.", "NB_TIL_OG_A"),
        ("Vi skal ut å spise.", "NB_UT_OG_A"),
        ("Han sitter å leser.", "NB_SITTE_OG_A"),
        ("Han la meg gå.", "NB_LA_LOT"),
        ("Jeg gikk til han.", "NB_HAN_HAM"),
        ("Jeg gikk til hun.", "NB_HUN_HENNE"),
        ("Jeg vet at han kommer ikke.", "NB_NEG_SUB"),
        ("Hun ikke kan komme.", "NB_NEG_MAIN"),
        ("Nå jeg reiser til Bergen.", "NB_V2"),
        ("I dag jeg arbeider hjemme.", "NB_V2_DATE"),
        ("Dette er min bilen.", "NB_POSS_DEF"),
        ("Dette er den stor bilen.", "NB_DEF_ADJ"),
        ("Vi bor i et stor hus.", "NB_NEUTER_T"),
        ("Vi har stor biler.", "NB_PLURAL_ADJ"),
        ("Det virker som at han mener det.", "NB_VIRKE_SOM"),
        ("desverre, jeg er her", "NB_TYPOS"),
        ("Jeg har et abbonnement.", "NB_TYPOS"),
        ("Han sa det foresten.", "NB_TYPOS"),
        ("Jeg leste artikkler i avisen.", "NB_TYPOS"),
        ("ikkje", "NB_NYNORSK_FORMS"),
        ("idag", "NB_WORD_DIVISION"),
        ("Han snakker Norsk.", "NB_CAPITALIZATION"),
        ("Vi feirer 50årsdag.", "NB_HYPHEN_NUMBERS_JOINED"),
        ("Vi feirer 50 årsdag.", "NB_HYPHEN_NUMBERS_SPACED"),
        ("Han er NRK medarbeider.", "NB_ABBREV_HYPHEN"),
        ("Dem kommer i morgen.", "NB_DE_DEM_SUBJ"),
        ("Gi klærne til de som trenger dem.", "NB_DE_DEM_OBJ"),
        ("Vi har mye biler.", "NB_MYE_MANGE"),
        ("Det er lite biler i byen.", "NB_LITE_FA"),
        ("Hvem bok leser du?", "NB_HVEM_HVILKEN"),
        ("Hvis du kommer blir jeg glad.", "NB_COMMA_FRONTED_SUB"),
        ("I fjor, solgte de huset.", "NB_COMMA_PP_VERB"),
        ("Han han kommer.", "NB_WORD_REPETITION"),
        ("Et jente er her.", "NB_EN_ET_GENDER"),
        ("Jeg ser en hus.", "NB_EN_ET_GENDER"),
        ("Et bil står der.", "NB_EN_ET_GENDER"),
        ("En bord er der.", "NB_EN_ET_GENDER"),
        ("Han vasket hans bil.", "NB_SIN_HANS"),
        ("Hun lånte hennes sykkel.", "NB_SIN_HANS"),
        ("Han så ham selv.", "NB_SEG_REFLEX"),
    ];
    for (text, rule_id) in cases {
        assert!(
            hits(&engine, text, rule_id),
            "expected {rule_id} for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
}

/// Correct sentences must not trigger the checked rules (false-positive
/// regression guard; the speller is asserted separately).
#[test]
fn norwegian_correct_sentences() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in [
        "Da jeg var liten, bodde vi i Oslo.",
        "Hun må lære å lese.",
        "Vi skal ut og spise.",
        "Jeg gikk til ham.",
        "Jeg vet at han ikke kommer.",
        "I dag arbeider jeg hjemme.",
        "Dette er bilen min.",
        "Dette er den store bilen.",
        "Vi bor i et stort hus.",
        "Vi har store biler.",
        "Han lot meg gå.",
        "Det virker som om han mener det.",
        "Vi feirer 50-årsdag.",
        "Det er en 18-årsgrense for dette.",
        "Han er NRK-medarbeider.",
        "TV er gøy.",
        "De kommer i morgen.",
        "Gi klærne til dem som trenger dem.",
        "Vi har mange biler.",
        "Det er få biler i byen.",
        "Hvilken bok leser du?",
        "Hvis du kommer, blir jeg glad.",
        "I fjor solgte de huset.",
        "Om sommeren bader vi.",
        "Ja ja, jeg kommer.",
        "En jente er her.",
        "Jeg ser et hus.",
        "Jeg spiser et eple.",
        "Jeg ser et bord.",
        "En bil står der.",
        "Vi har en del å gjøre.",
        "Det er et par.",
        "Han vasket sin bil.",
        "Hun lånte sykkelen sin.",
        "Hun så seg selv.",
        "Jeg så ham selv.",
        "Det er hans bil.",
    ] {
        let ids = match_ids(&engine, text);
        assert!(ids.is_empty(), "unexpected matches for {text:?}: {ids:?}");
    }
}

/// Default-off rules only fire when enabled explicitly.
#[test]
fn norwegian_default_off_rules() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(&engine, "Hun må lære og lese.", "NB_OG_A_VERB"));
    assert!(!hits(
        &engine,
        "Når jeg var liten, bodde vi i Oslo.",
        "NB_DA_NAR"
    ));
    assert!(!hits(&engine, "Du har sett den?", "NB_QUESTION_INVERSION"));
    let Some(enabled) = engine_with_rules(&["NB_OG_A_VERB", "NB_DA_NAR", "NB_QUESTION_INVERSION"])
    else {
        return;
    };
    assert!(hits(&enabled, "Hun må lære og lese.", "NB_OG_A_VERB"));
    assert!(hits(
        &enabled,
        "Når jeg var liten, bodde vi i Oslo.",
        "NB_DA_NAR"
    ));
    assert!(hits(&enabled, "Du har sett den?", "NB_QUESTION_INVERSION"));
}

/// Default-off lexicon-driven split-compound check.
#[test]
fn norwegian_split_compound_lex() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(&engine, "sommer ferie", "NB_SPLIT_COMPOUND_LEX"));
    let Some(enabled) = engine_with_rules(&["NB_SPLIT_COMPOUND_LEX"]) else {
        return;
    };
    for text in ["sommer ferie", "skole gård", "bil motor"] {
        assert!(
            hits(&enabled, text, "NB_SPLIT_COMPOUND_LEX"),
            "expected NB_SPLIT_COMPOUND_LEX for {text:?}"
        );
    }
    for text in [
        "god morgen",
        "stor bil",
        "den samme",
        "til stede",
        "første gang",
        "jeg har en bil",
    ] {
        assert!(
            !hits(&enabled, text, "NB_SPLIT_COMPOUND_LEX"),
            "unexpected NB_SPLIT_COMPOUND_LEX for {text:?}"
        );
    }
}

/// The Hunspell speller (`NB_SPELLER`).
#[test]
fn norwegian_speller() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(
        &engine,
        "Dette er en korrekt skrevet setning.",
        "NB_SPELLER"
    ));
    assert!(hits(
        &engine,
        "Dette er en korrekt skrevet setnign.",
        "NB_SPELLER"
    ));
    // specific rules outrank the speller on the same range
    let ids = match_ids(&engine, "desverre");
    assert_eq!(ids, vec!["NB_TYPOS".to_string()]);
}

/// Morfologik-backed suggestions for misspellings (`no.dict`, one-off build);
/// Hunspell stays the spelling authority.
#[test]
fn norwegian_speller_suggestions() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let result = engine.check("Dette er en setnign.").unwrap();
    let spelling: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "NB_SPELLER")
        .collect();
    assert_eq!(
        spelling.len(),
        1,
        "matches: {:?}",
        match_ids(&engine, "Dette er en setnign.")
    );
    let values: Vec<&str> = spelling[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(values.first().copied(), Some("setning"), "got {values:?}");
    assert!(values.len() <= 5, "too many suggestions: {values:?}");
    let mut unique = values.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        values.len(),
        "duplicate suggestions: {values:?}"
    );
    // the misspelling's capitalization is preserved
    let result = engine.check("Dette er Setnign.").unwrap();
    let spelling: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "NB_SPELLER")
        .collect();
    assert_eq!(spelling.len(), 1);
    assert_eq!(
        spelling[0].suggestions.first().map(|s| s.value.as_str()),
        Some("Setning")
    );
}
