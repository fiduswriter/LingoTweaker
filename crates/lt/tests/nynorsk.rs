//! Norwegian Nynorsk (`nn`) engine tests.
//!
//! Hand-authored language: there is no legacy Java module to probe, so the
//! tests pin the staged rule wiring and the verified rule examples from
//! `data/nn/` (every list pair is checked against `hunspell/nn_NO.dic` with
//! `tools/nn-dict/check_pairs.py`; every XML `<example>` word is in the
//! dictionary so correct sentences stay clean). Offset probes assert the
//! Java/HTTP-compatible UTF-16 code units with `common::assert_utf16`,
//! pinned from the engine's own output (no Java oracle exists).

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
    Engine::builder(Lang::Nn).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Nn)
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

/// Stage-1 wiring state: 20 active XML rules (24 loaded, 4 default-off),
/// no unmapped filter, no compile failure.
#[test]
fn nynorsk_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert_eq!(engine.active_rule_count(), 20);
    assert!(engine.compile_failures().is_empty());
    let skipped = engine.skipped_counts();
    assert_eq!(skipped.filters, 0);
    assert_eq!(skipped.uncompilable, 0);
}

/// Rule examples that must fire with default options.
#[test]
fn nynorsk_rules_fire() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str)] = &[
        ("Pakka er klar til og hentast.", "NN_TIL_OG_A"),
        ("Vi skal ut å handla.", "NN_UT_OG_A"),
        ("Han sitr å lesar.", "NN_SITTE_OG_A"),
        ("Han la meg gå.", "NN_LA_LOT"),
        ("Eg veit at han kjem ikkje i dag.", "NN_NEG_SUB"),
        ("Ho ikkje kan kome.", "NN_NEG_MAIN"),
        ("No eg reiser til Oslo.", "NN_V2"),
        ("I dag eg arbeider heime.", "NN_V2_DATE"),
        ("Dette er min bilen.", "NN_POSS_DEF"),
        ("Dette er den stor bilen.", "NN_DEF_ADJ"),
        ("Vi bur i eit stor hus.", "NN_NEUTER_T"),
        ("Vi har stor bilar.", "NN_PLURAL_ADJ"),
        ("Vi feirar 50årsdag.", "NN_HYPHEN_NUMBERS_JOINED"),
        (
            "Vi feirar 10 årsjubileum i dag.",
            "NN_HYPHEN_NUMBERS_SPACED",
        ),
        ("Han er NRK medarbeidar.", "NN_ABBREV_HYPHEN"),
        ("Viss du kjem blir eg glad.", "NN_COMMA_FRONTED_SUB"),
        // Språkrådet-guided rules added 2026-09-23
        ("Eg kjem og.", "NN_AKSENT_OG"),
        ("Dem kjem i dag.", "NN_DEI_DEM_SUBJ"),
        ("Gi boka til dei som treng henne.", "NN_DEI_DEM_OBJ"),
        ("Ho sa «Ja,» til meg.", "NN_COMMA_QUOTES"),
        // Bokmål interference (list rule)
        ("jeg ikke går hjem.", "NN_BOKMAAL_FORMS"),
        ("Jeg veit ikkje.", "NN_BOKMAAL_FORMS"),
        ("hvordan går det?", "NN_BOKMAAL_FORMS"),
        ("Vi har to gutter.", "NN_BOKMAAL_FORMS"),
        ("Ho snakker om sommeren.", "NN_BOKMAAL_FORMS"),
        ("Det er bedre no.", "NN_BOKMAAL_FORMS"),
        ("Han kom i kirken.", "NN_BOKMAAL_FORMS"),
        // expanded Bokmål-form list: frequent pronouns/auxiliaries,
        // hjem-/hver-/hvit-/syk- families, -ende participles, -heit nouns
        ("Han er veldig syk.", "NN_BOKMAAL_FORMS"),
        ("Ho skal på sykehuset.", "NN_BOKMAAL_FORMS"),
        ("Vi kjøpte en hjemmeside.", "NN_BOKMAAL_FORMS"),
        ("Det var hjemmebane.", "NN_BOKMAAL_FORMS"),
        ("Det er en spennende film.", "NN_BOKMAAL_FORMS"),
        ("Han hvisker det til henne.", "NN_BOKMAAL_FORMS"),
        ("Ho hadde sine meninger.", "NN_BOKMAAL_FORMS"),
        ("Sikkerheten er viktig.", "NN_BOKMAAL_FORMS"),
        ("Hun fikk to måneder.", "NN_BOKMAAL_FORMS"),
        ("En gammel mann.", "NN_BOKMAAL_FORMS"),
        ("Bilen stod på broen.", "NN_BOKMAAL_FORMS"),
        ("Navnet var kjent.", "NN_BOKMAAL_FORMS"),
        ("Det er en stående ordning.", "NN_BOKMAAL_FORMS"),
        // other list rules
        ("abbonnement", "NN_TYPOS"),
        ("imorgen", "NN_WORD_DIVISION"),
        ("etterhvert", "NN_WORD_DIVISION"),
        ("Ho snakkar Norsk.", "NN_CAPITALIZATION"),
        ("Vi kjem på Laurdag.", "NN_CAPITALIZATION"),
        // shared built-in
        ("Han han kjem.", "NN_WORD_REPETITION"),
    ];
    for (text, rule_id) in cases {
        assert!(
            hits(&engine, text, rule_id),
            "expected {rule_id} for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
}

/// The Bokmål-interference rule beats the speller on the shared range (the
/// curated forms are all rejected by the Nynorsk dictionary, so the speller
/// would flag the same token; the priority table gives the list rule the
/// win and no duplicate `NN_SPELLER` match remains).
#[test]
fn nynorsk_bokmaal_forms_beat_speller() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in ["jeg ikke går hjem.", "hvordan går det?"] {
        let ids = match_ids(&engine, text);
        assert!(
            ids.iter().all(|id| id == "NN_BOKMAAL_FORMS"),
            "unexpected rules for {text:?}: {ids:?}"
        );
    }
}

/// UTF-16 offset probes. No Java oracle exists, so the expectations pin the
/// engine's own output in the Java/HTTP-compatible UTF-16 format
/// (`common::assert_utf16`).
#[test]
fn nynorsk_utf16_offsets() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str, (usize, usize))] = &[
        ("Pakka er klar til og hentast.", "NN_TIL_OG_A", (18, 20)),
        ("Ho står å ventar på deg.", "NN_SITTE_OG_A", (8, 9)),
        ("I dag eg arbeider heime.", "NN_V2_DATE", (6, 17)),
        (
            "Vi feirar 10 årsjubileum i dag.",
            "NN_HYPHEN_NUMBERS_SPACED",
            (10, 24),
        ),
    ];
    for (text, rule, expected) in cases {
        let m = engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"));
        assert_utf16(text, &m, *expected);
    }
}

/// Correct Nynorsk sentences must stay clean (dictionary-verified example
/// words, shared-form guards, antipatterns).
#[test]
fn nynorsk_correct_sentences() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in [
        "Pakka er klar til å hentast.",
        "Det er ein god ting til og med for barna.",
        "Vi skal ut og handla.",
        "Han sitr og lesar.",
        "Han la boka på bordet.",
        "Eg veit at han ikkje kjem i dag.",
        "Eg veit det ikkje.",
        "No reiser eg til Oslo.",
        "No må du gå.",
        "I dag arbeider eg heime.",
        "I går gjekk ho tidleg.",
        "Dette er bilen min.",
        "Dette er min bil.",
        "Dette er den store bilen.",
        "den vesle bilen",
        "Vi bur i eit stort hus.",
        "eit lite hus",
        "Vi har store bilar.",
        "Vi har få bilar.",
        "Viss du kjem, blir eg glad.",
        "Viss du vil, kan du kome.",
        "Eg kjem òg.",
        "Han òg ville vere med.",
        "«Eg kjem i morgon», sa han.",
        "Ho sa «nei» til forslaget.",
        "Dei kjem i dag.",
        "Gi boka til dem som treng henne.",
        "Da eg var liten, budde me i Oslo.",
        "Når eg er i Oslo, vitjar eg ofte museet.",
        "Vi kjøpte brød, kaker og bollar.",
        "Eg veit at han kjem og eg blir glad.",
        "Vi feirar 50-årsdag.",
        "Han er NRK-medarbeidar.",
        "TV er gøy.",
        "Ho likar å lære og lesa.",
        "Å vere eller ikkje vere.",
        "Ho snakkar norsk.",
        "Vi kjem på laurdag.",
        "Eg kjem òg.",
        "Det er ikkje bra.",
        "Ho har eige barn.",
        "samstundes",
        // Bokmål-looking words that are valid Nynorsk variants must NOT be
        // flagged (the list contains only unambiguous forms)
        "også",
        "hun",
        "dra",
        "være",
        "derfor",
        "arbeider",
        "biler",
        "bergensk",
    ] {
        let ids = match_ids(&engine, text);
        assert!(ids.is_empty(), "unexpected matches for {text:?}: {ids:?}");
    }
}

/// Default-off rules only fire when enabled explicitly.
#[test]
fn nynorsk_default_off_rules() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(&engine, "Ho må lære og lesa.", "NN_OG_A_VERB"));
    // Språkrådet-guided rules added 2026-09-23: da/når and the two comma
    // rules are off by default (surface patterns carry residual
    // false-positive risk).
    assert!(!hits(
        &engine,
        "Når eg var liten, budde me i Oslo.",
        "NN_DA_NAR"
    ));
    assert!(!hits(&engine, "Eg går og du står.", "NN_COMMA_SIDEORDNING"));
    assert!(!hits(
        &engine,
        "Vi kjøpte brød, kaker, og bollar.",
        "NN_COMMA_OPPRAMSING"
    ));
    let Some(enabled) = engine_with_rules(&[
        "NN_OG_A_VERB",
        "NN_DA_NAR",
        "NN_COMMA_SIDEORDNING",
        "NN_COMMA_OPPRAMSING",
    ]) else {
        return;
    };
    assert!(hits(&enabled, "Ho må lære og lesa.", "NN_OG_A_VERB"));
    assert!(hits(
        &enabled,
        "Når eg var liten, budde me i Oslo.",
        "NN_DA_NAR"
    ));
    assert!(hits(&enabled, "Eg går og du står.", "NN_COMMA_SIDEORDNING"));
    assert!(hits(
        &enabled,
        "Vi kjøpte brød, kaker, og bollar.",
        "NN_COMMA_OPPRAMSING"
    ));
}

/// The generated/vendored Hunspell dictionary: valid Nynorsk is accepted,
/// nonsense is flagged with native suggestions.
#[test]
fn nynorsk_speller() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(&engine, "ikkje", "NN_SPELLER"));
    assert!(!hits(&engine, "heimelaga", "NN_SPELLER"));
    // a realistic typo gets a Nynorsk suggestion from the native hunspell
    // suggest()
    let result = engine.check("ikjje").unwrap();
    let spelling: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "NN_SPELLER")
        .collect();
    assert_eq!(spelling.len(), 1);
    assert!(spelling[0].suggestions.iter().any(|s| s.value == "ikkje"));
    // pure nonsense is flagged (suggestions may be empty)
    assert!(hits(&engine, "xkvcd", "NN_SPELLER"));
}

/// The curated Bokmål-form pairs: every suggestion the rule offers must be
/// accepted by the Nynorsk speller (i.e. the corrected sentence is clean),
/// spot-checked over the list-driven rule config (`suggestions_separator`
/// handling for `a|b` pairs).
#[test]
fn nynorsk_bokmaal_forms_suggestions_are_valid_nynorsk() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let corrected: &[(&str, &str)] = &[
        ("jeg", "eg"),
        ("ikke", "ikkje"),
        ("noen", "nokon"),
        ("hvordan", "korleis"),
        ("hjemme", "heime"),
        ("kirken", "kyrkja"),
        ("mandag", "måndag"),
        ("etterhvert", "etter kvart"),
        // expanded list: frequent function words and the new families
        ("syk", "sjuk"),
        ("sykehus", "sjukehus"),
        ("hjem", "heim"),
        ("hjemmeside", "heimeside"),
        ("hverdag", "kvardag"),
        ("hvit", "kvit"),
        ("efter", "etter"),
        ("sommer", "sommar"),
        ("vannet", "vatnet"),
        ("meninger", "meiningar"),
        ("muligheter", "moglegheiter"),
        ("kjærlighet", "kjærleik"),
        ("navn", "namn"),
        ("spennende", "spennande"),
        ("sikkerhet", "sikkerheit"),
        ("tidligere", "tidlegare"),
        ("selv", "sjølv"),
        ("tror", "trur"),
        ("gammel", "gammal"),
        ("stolthet", "stoltheit"),
    ];
    for (wrong, right) in corrected {
        let result = engine.check(wrong).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "NN_BOKMAAL_FORMS" || m.rule_id == "NN_WORD_DIVISION")
            .unwrap_or_else(|| panic!("no rule fired for {wrong:?}"));
        // sentence-start corrections come back capitalized
        assert!(
            m.suggestions
                .iter()
                .any(|s| s.value.eq_ignore_ascii_case(right)),
            "no suggestion {right:?} for {wrong:?}: {:?}",
            m.suggestions
        );
        assert!(
            match_ids(&engine, right).is_empty(),
            "corrected form {right:?} is not clean Nynorsk"
        );
    }
}
