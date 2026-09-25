//! Norwegian Bokmål (`no`) engine tests.
//!
//! Hand-authored language: there is no legacy Java module to probe, so the
//! tests pin the staged rule wiring and the owner-approved examples from
//! `new-languages/norwegian-bokmal/proposed-rules.md`. Offset probes assert
//! the Java/HTTP-compatible UTF-16 code units with `common::assert_utf16`,
//! pinned from the engine's own hunspell-reference output.

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

/// Stage-3 wiring state: 29 active XML rules (36 loaded, 7 default-off),
/// no unmapped filter, no compile failure.
#[test]
fn norwegian_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert_eq!(engine.active_rule_count(), 29);
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
        ("Denne stor bilen står der.", "NB_DEF_ADJ"),
        ("De gammel bilene står der.", "NB_DEF_ADJ"),
        ("Han har den ny bilen.", "NB_DEF_ADJ"),
        ("Vi bor i et stor hus.", "NB_NEUTER_T"),
        ("De kjøpte et gammel hus.", "NB_NEUTER_T"),
        ("Et gammel hus står der.", "NB_NEUTER_T"),
        ("Vi har stor biler.", "NB_PLURAL_ADJ"),
        ("Flere gammel biler står der.", "NB_PLURAL_ADJ"),
        ("Han har mange gammel biler.", "NB_PLURAL_ADJ"),
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
        ("Han sa «Ja,» til meg.", "NB_COMMA_QUOTES"),
        ("I fjor, solgte de huset.", "NB_COMMA_PP_VERB"),
        ("Han han kommer.", "NB_WORD_REPETITION"),
        ("Et jente er her.", "NB_EN_ET_GENDER"),
        ("Jeg ser en hus.", "NB_EN_ET_GENDER"),
        ("Et bil står der.", "NB_EN_ET_GENDER"),
        ("En bord er der.", "NB_EN_ET_GENDER"),
        ("Han vasket hans bil.", "NB_SIN_HANS"),
        ("Hun lånte hennes sykkel.", "NB_SIN_HANS"),
        ("Han så ham selv.", "NB_SEG_REFLEX"),
        ("Han har mange bil.", "NB_QUANT_PLU"),
        ("Flere gang har jeg sagt det.", "NB_QUANT_PLU"),
        ("Vi så begge side av saken.", "NB_QUANT_PLU"),
        ("Disse bilen er ny.", "NB_DISSE_PLU"),
        ("Disse jente leser mye.", "NB_DISSE_PLU"),
        ("Dette bilen står der.", "NB_DEM_COMMON"),
        ("Dette jenta ler.", "NB_DEM_COMMON"),
        ("Denne huset er gammelt.", "NB_DEM_NEUTER"),
        ("Denne barnet sover.", "NB_DEM_NEUTER"),
    ];
    for (text, rule_id) in cases {
        assert!(
            hits(&engine, text, rule_id),
            "expected {rule_id} for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
}

/// UTF-16 offset probes for owner-approved examples. No Java oracle exists,
/// so the expectations pin the engine's own hunspell-reference output in the
/// Java/HTTP-compatible UTF-16 format (`common::assert_utf16`).
#[test]
fn norwegian_utf16_offsets() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str, (usize, usize))] = &[
        ("Hun lånte hennes sykkel.", "NB_SIN_HANS", (10, 16)),
        ("Han så ham selv.", "NB_SEG_REFLEX", (7, 10)),
        ("Nå jeg reiser til Bergen.", "NB_V2", (3, 13)),
        ("Jeg har gådd hjem.", "NB_SPELLER", (8, 12)),
        ("Vi har mye biler.", "NB_MYE_MANGE", (7, 10)),
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
        // adjective agreement false-positive classes: the neuter/BF
        // arbitration must keep the invariant and verb-homograph
        // adjectives silent (adj:pos:neu doubles the bare form; adj:pos:BF
        // surfaces are already correct; ver:kor readings outrank)
        "Vi bor i et gammelt hus.",
        "Et gammelt hus står der.",
        "Vi bor i et moderne hus.",
        "Vi har et fornøyd barn.",
        "Hun har en fornøyd kunde.",
        "De har kjørt biler.",
        "Det var et glad barn.",
        "De indre organene er sårbare.",
        "Hun har et eget rom.",
        "Den blå bilen.",
        "De grå steinene.",
        "Et såkalt problem ble løst.",
        "Det er et reservert bord.",
        "Jeg snakker det norske språket.",
        "De gamle bilene står der.",
        "Han har mange gamle biler.",
        "Mitt gamle hus ligger i gaten.",
        "Alle gamle biler blir solgt.",
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
        "«Jeg kommer i morgen», sa han.",
        "Han sa «nei» til forslaget.",
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
        "Han har mange biler.",
        "Flere ganger har jeg sagt det.",
        "Begge deler er bra.",
        "Samtlige deltagere møtte opp.",
        "Disse jentene leser mye.",
        "Denne boken er god.",
        "Dette huset er gammelt.",
        "Dette året har vært vanskelig.",
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
    // Språkrådet-guided punctuation rules added 2026-09-23: three comma rules
    // and the dash/quote typography rules are off by default (surface
    // patterns carry residual false-positive risk).
    assert!(!hits(
        &engine,
        "Jeg går og du står.",
        "NB_COMMA_SIDEORDNING"
    ));
    assert!(!hits(
        &engine,
        "Vi kjøpte brød, kaker, og boller.",
        "NB_COMMA_OPPRAMSING"
    ));
    assert!(!hits(&engine, "Kapitlene 2-10 er korte.", "NB_DASH_RANGE"));
    assert!(!hits(
        &engine,
        "Han sa \"nei\" til forslaget.",
        "NB_STRAIGHT_QUOTES"
    ));
    let Some(enabled) = engine_with_rules(&[
        "NB_OG_A_VERB",
        "NB_DA_NAR",
        "NB_QUESTION_INVERSION",
        "NB_COMMA_SIDEORDNING",
        "NB_COMMA_OPPRAMSING",
        "NB_DASH_RANGE",
        "NB_STRAIGHT_QUOTES",
    ]) else {
        return;
    };
    assert!(hits(&enabled, "Hun må lære og lese.", "NB_OG_A_VERB"));
    assert!(hits(
        &enabled,
        "Når jeg var liten, bodde vi i Oslo.",
        "NB_DA_NAR"
    ));
    assert!(hits(&enabled, "Du har sett den?", "NB_QUESTION_INVERSION"));
    assert!(hits(
        &enabled,
        "Jeg går og du står.",
        "NB_COMMA_SIDEORDNING"
    ));
    assert!(hits(
        &enabled,
        "Vi kjøpte brød, kaker, og boller.",
        "NB_COMMA_OPPRAMSING"
    ));
    assert!(hits(&enabled, "Kapitlene 2-10 er korte.", "NB_DASH_RANGE"));
    assert!(hits(
        &enabled,
        "Han sa \"nei\" til forslaget.",
        "NB_STRAIGHT_QUOTES"
    ));
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

/// Native hunspell suggestions for misspellings (capped at five); Hunspell
/// stays the spelling authority. There is no legacy Java module for `no`, so
/// these pin the engine's own hunspell-reference behaviour.
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
}

/// Suggestions keep Norwegian orthography (real ä/ø/å handling).
#[test]
fn norwegian_speller_suggestions_diacritics() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let result = engine.check("Jeg har gådd hjem.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "NB_SPELLER")
        .expect("gådd must be flagged");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["nådd", "sådd", "gård", "rådd", "gidd"]);
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

/// The hand-authored disambiguation rulegroups (plan item B2,
/// `data/no/disambiguation.xml`) filter the homograph readings they target.
/// Each case: the ambiguous token carries both readings before
/// disambiguation and only the expected class afterwards.
#[test]
fn norwegian_disambiguation_rulegroups() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let tags = |raw: bool, text: &str, token: &str| -> Vec<String> {
        let sentences = if raw {
            engine.analyze_raw(text)
        } else {
            engine.analyze(text)
        };
        let tok = sentences
            .iter()
            .flat_map(|s| s.tokens.iter())
            .find(|t| t.surface() == token)
            .unwrap_or_else(|| panic!("token {token:?} not found in {text:?}"));
        let mut tags: Vec<String> = tok
            .readings
            .iter()
            .filter_map(|r| r.pos_tag.clone())
            .filter(|t| !matches!(t.as_str(), "SENT_START" | "SENT_END"))
            .collect();
        tags.sort();
        tags.dedup();
        tags
    };
    let tags_after = |text: &str, token: &str| tags(false, text, token);
    let tags_raw = |text: &str, token: &str| tags(true, text, token);
    // det-pron: "det huset" -> determiner; "Det snør" / "De kommer" -> pronoun.
    let raw = tags_raw("I det huset bor en familie.", "det");
    assert!(
        raw.iter().any(|t| t == "det") && raw.iter().any(|t| t == "pron"),
        "expected a det/pron homograph: {raw:?}"
    );
    let after = tags_after("I det huset bor en familie.", "det");
    assert!(
        after.iter().all(|t| t == "det"),
        "det+sub must keep only det: {after:?}"
    );
    let after = tags_after("Det snør i dag.", "Det");
    assert!(
        after.iter().all(|t| t == "pron"),
        "det+ver must keep only pron: {after:?}"
    );
    let after = tags_after("De kommer snart.", "De");
    assert!(
        after.iter().all(|t| t == "pron"),
        "de+ver must keep only pron: {after:?}"
    );
    // pron-ver-sub: "Han spiller." -> verb (the -er noun readings are
    // filtered); "En spiller" (no pronoun) stays untouched. An adj-homograph
    // (rask/ny/norsk carry an imperative verb reading as well) is left to
    // det-adj-sub: stripping the adjective would break NB_DEF_ADJ after
    // "den" ("den ny bilen").
    let raw = tags_raw("Han spiller fotball.", "spiller");
    assert!(
        raw.iter().any(|t| t.starts_with("ver:")) && raw.iter().any(|t| t.starts_with("sub:")),
        "expected a ver+sub homograph: {raw:?}"
    );
    let after = tags_after("Han spiller fotball.", "spiller");
    assert!(
        after.iter().all(|t| t.starts_with("ver:")),
        "pron+ver&sub must keep only ver: {after:?}"
    );
    let after = tags_after("En spiller løper fort.", "spiller");
    assert!(
        after.iter().any(|t| t.starts_with("sub:")),
        "det+ver&sub noun must stay untouched: {after:?}"
    );
    let after = tags_after("Han kjøpte den ny bilen.", "ny");
    assert!(
        after.iter().any(|t| t.starts_with("adj:")),
        "pron+ver&sub must leave adj homographs alone: {after:?}"
    );
    let after = tags_after("Den rask bilen står der.", "rask");
    assert!(
        after.iter().any(|t| t.starts_with("adj:")),
        "pron+ver&sub must leave adj homographs alone: {after:?}"
    );
    // aa-infm-ver-sub: "begynte å renne" -> the verb survives.
    let after = tags_after("Vannet begynte å renne ut av kjelleren.", "renne");
    assert!(
        after.iter().all(|t| t.starts_with("ver:")),
        "å+ver&sub must keep only ver: {after:?}"
    );
    let after = tags_after("En renne leder vannet bort.", "renne");
    assert!(
        after.iter().any(|t| t.starts_with("sub:")),
        "det+ver&sub noun must keep sub: {after:?}"
    );
    // prep-ver-sub: "i løpet av dagen" -> the substantive survives.
    let after = tags_after("I løpet av dagen regnet det.", "løpet");
    assert!(
        after.iter().all(|t| t.starts_with("sub:")),
        "prep+ver&sub must keep only sub: {after:?}"
    );
    // det-adj-sub: "den voksne eleven" -> adjective; "den voksne" ->
    // nominalized substantive.
    let raw = tags_raw("Den voksne eleven leser mye.", "voksne");
    assert!(
        raw.iter().any(|t| t.starts_with("adj:")) && raw.iter().any(|t| t.starts_with("sub:")),
        "expected an adj+sub homograph: {raw:?}"
    );
    let after = tags_after("Den voksne eleven leser mye.", "voksne");
    assert!(
        after.iter().all(|t| t.starts_with("adj:")),
        "det+adj&sub+sub must keep only adj: {after:?}"
    );
    let after = tags_after("Den voksne sover lenge.", "voksne");
    assert!(
        after.iter().all(|t| t.starts_with("sub:")),
        "det+adj&sub must keep only sub: {after:?}"
    );
}

/// The derived Norwegian synthesizer dictionary (plan item B2,
/// infrastructure over `no_synth.dict`): `lemma|tag` lookup returns the
/// inflected forms of `no_pos.dict` and postag-regexp synthesis expands over
/// `no_synth_tags.txt`. The tags are the M1 tagset tags (no gender-bearing
/// noun tags).
#[test]
fn norwegian_synthesizer() {
    let _guard = engine_guard();
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let synth = lt_tagger::NorwegianSynthesizer::from_data(data.path()).expect("synth");
    let plain = |lemma: &str, tag: &str| {
        synth.synthesize(
            &lt::AnalyzedToken::new("", Some(lemma.to_string()), Some(tag.to_string())),
            tag,
            false,
        )
    };
    assert_eq!(plain("bil", "sub:ube:sin"), ["bil"]);
    assert_eq!(plain("bil", "sub:ube:plu"), ["biler"]);
    assert_eq!(plain("bil", "sub:bes:plu"), ["bilene"]);
    // unknown lemma|tag keys synthesize nothing
    assert!(plain("ikkeetord", "sub:ube:sin").is_empty());
    // `postag_regexp` synthesis walks the tag list (`sub:.*:plu`)
    let forms = synth.synthesize(
        &lt::AnalyzedToken::new("", Some("bil".to_string()), Some("sub:.*:plu".to_string())),
        "sub:.*:plu",
        true,
    );
    assert_eq!(forms, ["bilene", "biler"]);
}

/// The POS-based agreement rules (plan item B2) suggest the synthesized
/// `<match postag>` forms from the derived synthesizer dictionary.
#[test]
fn norwegian_synth_rule_suggestions() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let correction = |text: &str, rule_id: &str| -> String {
        engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .find(|m| m.rule_id == rule_id)
            .unwrap_or_else(|| panic!("{rule_id} did not match {text:?}"))
            .suggestions
            .first()
            .map(|s| s.value.clone())
            .unwrap_or_default()
    };
    // `<match no="2" postag="sub:ube:plu"/>`: the plural of the tagger lemma.
    assert_eq!(correction("Han har mange bil.", "NB_QUANT_PLU"), "biler");
    // `<match no="2" postag="sub:bes:plu"/>`: the definite plural.
    assert_eq!(correction("Disse bilen er ny.", "NB_DISSE_PLU"), "bilene");
    // `<match no="1" regexp_match/regexp_replace>`: the other demonstrative,
    // with the sentence-initial capitalization preserved.
    assert_eq!(
        correction("Dette bilen står der.", "NB_DEM_COMMON"),
        "Denne"
    );
    assert_eq!(correction("dette jenta ler.", "NB_DEM_COMMON"), "denne");
    assert_eq!(
        correction("Denne huset er gammelt.", "NB_DEM_NEUTER"),
        "Dette"
    );
    // the adjective agreement rules synthesize over adj:pos:BF /
    // adj:pos:neu (never hardcoded), including the blå-class BF spellings
    // and the -tt doubling straight from the dictionary.
    assert_eq!(
        correction("Dette er den stor bilen.", "NB_DEF_ADJ"),
        "store"
    );
    assert_eq!(correction("Han har den ny bilen.", "NB_DEF_ADJ"), "nye");
    assert_eq!(
        correction("De gammel bilene står der.", "NB_DEF_ADJ"),
        "gamle"
    );
    assert_eq!(correction("Vi bor i et stor hus.", "NB_NEUTER_T"), "stort");
    assert_eq!(
        correction("De kjøpte et gammel hus.", "NB_NEUTER_T"),
        "gammelt"
    );
    assert_eq!(correction("Vi har stor biler.", "NB_PLURAL_ADJ"), "store");
    assert_eq!(
        correction("Flere gammel biler står der.", "NB_PLURAL_ADJ"),
        "gamle"
    );
}
