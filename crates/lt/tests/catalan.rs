//! Catalan engine tests: stage-1 XML wiring state plus Java-probed built-in
//! rule values.
//!
//! Stage gates follow internal development notes: this file pins the
//! progress metric and gets updated by each stage. Offsets are UTF-8 bytes
//! (the engine format); the Java probes (`scripts/oracle/ca/probe-rule.sh`,
//! pinned LT build) print UTF-16 code units, converted in the comments.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Catalan engines hold the tagger dictionary.
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

fn engine_variant(variant: &str) -> Option<Engine> {
    let data = data_dir()?;
    let builder = Engine::builder(Lang::Ca).ok()?;
    builder.variant(variant).data_dir(data).build().ok()
}

fn engine_with_rules(variant: &str, rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ca)
        .ok()?
        .variant(variant)
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Engine with only `rules` enabled and a pinned `today` (the date filters).
fn engine_with_rules_today(
    variant: &str,
    rules: &[&str],
    year: i32,
    month: u32,
    day: u32,
) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ca)
        .ok()?
        .variant(variant)
        .data_dir(data)
        .options(options)
        .today(year, month, day)
        .build()
        .ok()
}

/// Stage-3h state: all Catalan stages are wired — 9,325 active XML rules
/// for ca-ES / ca-ES-balear and 9,329 for ca-ES-valencia, 1,996
/// disambiguation rules, and `compile_failures()` = 0 / 0 / 0. The
/// stage-3 XML-referenced filter classes are all mapped (see
/// `ca-rule-port.md` for the documented triage stubs).
#[test]
fn catalan_engine_state_stage3h() {
    let _guard = engine_guard();
    let Some(ca) = engine_variant("ca-ES") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(ca.active_rule_count(), 9325);
    assert_eq!(ca.disambig_rule_count(), 1996);
    assert_eq!(ca.skipped_counts().filters, 0);
    assert!(
        ca.compile_failures().is_empty(),
        "{:?}",
        ca.compile_failures()
    );

    let Some(valencia) = engine_variant("ca-ES-valencia") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // the valencia variant additionally compiles the 8
    // `getDefaultEnabledRulesForVariant` rulegroups (D-154)
    assert_eq!(valencia.active_rule_count(), 9357);
    assert_eq!(valencia.disambig_rule_count(), 1996);
    assert!(valencia.compile_failures().is_empty());

    let Some(balear) = engine_variant("ca-ES-balear") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // + `EXIGEIX_VERBS_BALEARS` (D-154)
    assert_eq!(balear.active_rule_count(), 9337);
    assert!(balear.compile_failures().is_empty());
}

/// Stage-3d filters: `AdjustVerbSuggestionsFilter` (93 XML refs) and
/// `AdjustPronounsFilter` (43 refs) over `VerbSynthesizer` +
/// `PronomsFeblesHelper`, probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19). Offsets are
/// UTF-8 bytes; the Java probe prints UTF-16 code units (`marró` gains one
/// byte in UTF-8).
#[test]
fn catalan_verb_filters_match_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a str, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "GIRAR_LES_TORNES",
            "Ara poden girar-se les tornes.",
            10,
            29,
            "Expressió incorrecta.",
            &[
                "girar-se la truita",
                "bufar nous vents",
                "bufar vent contrari",
                "haver-hi un capgirell",
                "mudar-se els daus",
                "canviar la sort",
            ],
        ),
        (
            "LIAR_SE_A",
            "Es van liar a pals.",
            0,
            18,
            "Expressió incorrecta.",
            &["Se les van heure a bastonades"],
        ),
        (
            "MENJAR_SE_UN_MARRO",
            "Sempre li toca menjar-se el marró.",
            15,
            // Java 15..33 (UTF-16); `ó` is two bytes in UTF-8
            34,
            "Expressió incorrecta.",
            &["pagar la festa", "carregar el mort", "menjar-se un gripau"],
        ),
        (
            "TROBAR_SE_A_PUNT",
            "Es trobaven a punt d'anar-se'n.",
            0,
            11,
            "Val més usar el verb 'estar'.",
            &["Estaven"],
        ),
        (
            "TROBAR_SE_OBLIGAT",
            "Es va trobar obligat a anar-se'n.",
            0,
            27,
            "Val més usar una altra expressió més natural.",
            &[
                "Es va veure obligat a anar",
                "Va haver d'anar",
                "Va ser obligat a anar",
                "Va estar obligat a anar",
            ],
        ),
    ];
    for (rule, text, start, end, message, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        assert_eq!(m.message, *message, "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }
}

/// Stage-3e filter: `CatalanSuppressMisspelledSuggestionsFilter` (87 refs)
/// with the full analyze path (`_incorrect_verb_` chunk tag + the Catalan
/// speller over the analyzed sentence), probed against the pinned Java
/// build (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19). The
/// `Faré`/`Dese mbre` cases cover `FilterPostag` and the speller path.
#[test]
fn catalan_suppress_misspelled_filter_matches_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a str, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "VERBS_NO_INCOATIUS",
            "grunyeix.",
            0,
            8,
            "Aquest verb no és incoatiu.",
            &["gruny"],
        ),
        (
            "VERBS_NO_INCOATIUS",
            "abateixen.",
            0,
            9,
            "Aquest verb no és incoatiu.",
            &["abaten"],
        ),
        (
            "TASCAS_TASQUES",
            "Faré totes les tascas que em diguis.",
            // Java 15..21 (UTF-16); `é` is two bytes in UTF-8
            16,
            22,
            "Error ortogràfic.",
            &["tasques"],
        ),
        (
            "CA_SPLIT_WORDS",
            "Dese mbre",
            0,
            9,
            "¿Volíeu dir <suggestion>Desembre</suggestion>?",
            &["Desembre"],
        ),
        (
            "CA_SPLIT_WORDS",
            "ADVER TIMENT",
            0,
            12,
            "¿Volíeu dir <suggestion>ADVERTIMENT</suggestion>?",
            &["ADVERTIMENT"],
        ),
    ];
    for (rule, text, start, end, message, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        assert_eq!(m.message, *message, "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }
}

/// Stage-3f filter: `ConvertToGenderAndNumberFilter` (82 refs) with the
/// backwards/forwards agreement walk and `ApostophationHelper`, probed
/// against the pinned Java build (`scripts/oracle/ca/probe-rule.sh ca-ES`,
/// 2026-09-19). Only cases where Java's `RuleMatch.trimMatchEnds`
/// post-processing (part of `Catalan.filterRuleMatchesAfterOverlapping`,
/// still unported) does not shrink the range are pinned here; the raw
/// filter output of e.g. `PENDENT_CORRENT` (Rust 3..18 `corrent altern`)
/// matches Java's raw filter match and will be pinned with that
/// post-processing.
#[test]
fn catalan_gender_number_filter_matches_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a str, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "CALOR",
            "Feia un calor xafogós.",
            // Java 5..21 (UTF-16); `ó` is two bytes in UTF-8
            5,
            22,
            "El substantiu 'calor' és femení.",
            &["una calor xafogosa"],
        ),
        (
            "ANALISI_FEM",
            "Un anàlisi clar.",
            // Java 0..15 (UTF-16); `à` is two bytes in UTF-8
            0,
            16,
            "\"Anàlisi\" és un nom femení.",
            &["Una anàlisi clara"],
        ),
        (
            "CONDOLS_CONDOL",
            "Vam donar les condolences a la família.",
            // all-ASCII span ("les condolences"), UTF-16 == UTF-8
            10,
            25,
            "Aquest nom s'usa normalment en singular. El plural pot ser una influència de l'anglès.",
            &["el condol", "la condolença"],
        ),
        (
            "EDITORIAL",
            "Vaig llegir aquella editorial horrorosa.",
            12,
            39,
            "En el sentit de 'article de fons' és masculí. En el sentit de 'empresa editora' és femení.",
            &["aquell editorial horrorós"],
        ),
        // The cases below are shrunk by `RuleMatch.trimMatchEnds`
        // (`Catalan.filterRuleMatchesAfterOverlapping`, wired in D-146).
        (
            "COSTUM_MASCULI",
            "Les ancestrals costums.",
            0,
            3,
            "Aquest nom és masculí.",
            &["Els"],
        ),
        (
            "PENDENT_CORRENT",
            "De corrent alterna.",
            11,
            18,
            "\"Corrent\" és un substantiu masculí.",
            &["altern"],
        ),
        (
            "FI_FINAL_MASC_FEM",
            "Un fi d'etapa.",
            0,
            2,
            "En aquest context, cal usar el gènere femení.",
            &["Una"],
        ),
        (
            "CONCORDANCES_DET_POSSESSIU",
            "La seva bones amigues.",
            0,
            7,
            "Possible error de concordança.",
            &["Les seves"],
        ),
    ];
    for (rule, text, start, end, message, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        assert_eq!(m.message, *message, "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }
}

/// Stage-3g filters: `AdvancedSynthesizerFilter` (60 refs, incl. the
/// `_NounToVerb` hook and the `language.adaptSuggestion` rendering) and
/// `FindSuggestionsFilter` (51 refs, `wordFrom`/`inmarker`,
/// `Mode:diacritics`, `removeSuggestionsRegexp`), probed against the pinned
/// Java build (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19). Cases
/// whose Java output is further shrunk by `RuleMatch.trimMatchEnds`
/// (`LAFUMIGA_RULE`, `PASSAT_PERIFRASTIC`) are not pinned yet.
#[test]
fn catalan_synth_find_suggestions_filters_match_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "CONCORDANCES_PRONOM_VERB",
            "Jo pensem que és així.",
            0,
            9,
            &["Ho pensem", "Jo penso"],
        ),
        (
            "HA_A",
            "Ho ha escoles que no ensenyen les taules de multiplicar.",
            0,
            13,
            &[
                "Hi ha escoles",
                "Ho ha escolat",
                "Ho ha escomès",
                "Ho ha escomés",
                "Ho ha esculat",
            ],
        ),
        (
            "ELA_GEMINADA_TYPO",
            "Potser co.lectiu.",
            7,
            16,
            &["col·lectiu"],
        ),
        ("APOSTROF_ACCENT", "Un cami`o", 3, 9, &["camió"]),
        (
            "FALTA_ELEMENT_ENTRE_VERBS",
            "Es tractava d'una àrea de visualització arbitraria.",
            // Java 40..50 (UTF-16); `à` and `ó` are two bytes each in UTF-8
            42,
            52,
            &["arbitrària"],
        ),
        // The cases below also exercise the match post-processing (D-146):
        // `trimMatchEnds` (LAFUMIGA, PASSAT_PERIFRASTIC) and
        // `adjustCatalanMatch`'s old-diacritics filtering (HA_APROVAT), the
        // number filters and `SynthesizeWithDAFilter`/`Oblidarse`/
        // `EnNoInfinitiu`/`Possessius`.
        (
            "LAFUMIGA_RULE",
            "Com no te vaig a estimar!",
            4,
            24,
            &["vols que no t'estimi", "voleu que no t'estimi"],
        ),
        (
            "PASSAT_PERIFRASTIC",
            "Se li van riures les gràcies.",
            10,
            16,
            &["riure"],
        ),
        (
            "HA_APROVAT_VA_APROVAR",
            "El passat dissabte s'ha fet efectiu.",
            19,
            27,
            &["es va fer", "es feu"],
        ),
        (
            "NOMBRES_NO_LLETRES",
            "És un quinze % més barata.",
            // Java 6..12 (UTF-16); `É` is two bytes in UTF-8
            7,
            13,
            &["15"],
        ),
        (
            "EN_NO_INFINITIU_CAUSAL",
            "En no poder venir, vam decidir deixar-ho córrer.",
            0,
            11,
            &["Com que no podia", "Com que no podíem"],
        ),
        (
            "OBLIDARSE",
            "Se'm va oblidar dur la carmanyola.",
            0,
            15,
            &["Em vaig oblidar de"],
        ),
        (
            "POSSESSIUS_PARTS_COS",
            "Em vaig recolzar en el meu braç.",
            23,
            27,
            &[""],
        ),
        (
            "CONCORDANCES_DET_ADJ",
            "als únic",
            // Java 0..8 (UTF-16); `ú` is two bytes in UTF-8
            0,
            9,
            &["A l'únic", "Als únics", "A les úniques", "A l'única"],
        ),
    ];
    for (rule, text, start, end, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }
}

/// Stage-3k filters: the `DonarTemps`/`AnarA`/`PortarGerundi`/`PortarTemps`
/// suggestion filters, probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19). Offsets are UTF-8
/// bytes; the Java probes print UTF-16 (`dóna`/`donarà` gain one byte).
#[test]
fn catalan_small_suggestion_filters_match_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "DONAR_TEMPS",
            "No em dóna temps de fer-ho.",
            // Java 3..10 (UTF-16)
            3,
            11,
            &["hi ha", "tinc"],
        ),
        (
            "DONAR_TEMPS",
            "Enguany no em donarà temps de fer-ho.",
            // Java 11..20 (UTF-16)
            11,
            21,
            &["hi haurà", "tindré"],
        ),
        (
            "ANAR_A_INFINITIU",
            "Com ho vaig a saber!",
            7,
            19,
            &["he de saber", "sabré", "sé"],
        ),
        (
            "PORTAR_GERUNDI",
            "Porto escoltant això des dels anys noranta.",
            0,
            15,
            &["He sentit", "Sento"],
        ),
        (
            "PORTAR_GERUNDI",
            "Els ho portava escoltant des dels anys noranta.",
            7,
            24,
            &["havia sentit", "sentia"],
        ),
        (
            "PORTA_UNA_HORA",
            "Porta una hora dient això.",
            0,
            20,
            &["Fa una hora que diu"],
        ),
        (
            "PORTA_UNA_HORA",
            "Portava vint-i-cinc anys sense dir res.",
            0,
            34,
            &["Feia vint-i-cinc anys que no deia"],
        ),
        (
            "PORTA_UNA_HORA",
            "Portava tota la vida en aquella empresa.",
            0,
            7,
            &["Havia estat"],
        ),
    ];
    for (rule, text, start, end, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }
}

/// Date filters (`DateCheckFilter`, `NewYearDateFilter`) probed against the
/// pinned Java build (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19;
/// `NewYearDateFilter` pinned with `today` in January like the XML example).
#[test]
fn catalan_date_filters_match_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a str, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "CA_DATE_WEEKDAY",
            "Dilluns, 7 d'octubre de 2014",
            0,
            10,
            "Aquesta data no és un dilluns sinó un dimarts.",
            &["Dimarts, 7", "Dilluns, 6"],
        ),
        (
            "CA_DATE_WEEKDAY",
            "Dimarts, 7 d'octubre de 2014",
            0,
            0,
            "",
            &[],
        ),
        (
            "CA_DATE_WEEKDAY",
            "Dilluns, 7 d'octubre del 2014",
            0,
            10,
            "Aquesta data no és un dilluns sinó un dimarts.",
            &["Dimarts, 7", "Dilluns, 6"],
        ),
    ];
    for (rule, text, start, end, message, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let matches: Vec<_> = result
            .matches
            .iter()
            .filter(|m| m.rule_id == *rule)
            .collect();
        if suggestions.is_empty() && *start == 0 && *end == 0 {
            assert!(matches.is_empty(), "unexpected match for {rule} / {text}");
            continue;
        }
        let m = matches
            .first()
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        assert_eq!(m.message, *message, "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }

    // `DATE_NEW_YEAR`: `today` in January 2014, as in the XML example.
    let Some(engine) = engine_with_rules_today("ca-ES", &["DATE_NEW_YEAR"], 2014, 1, 10) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("7 de octubre de 2013").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DATE_NEW_YEAR")
        .expect("no DATE_NEW_YEAR match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["2014"]);
}

/// `SuppressIfAnyRuleMatchesFilter` (4 XML refs) over the compiled-rule
/// back-reference, probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19). The negative
/// `MES1` case exercises the untouched pre-filter state.
#[test]
fn catalan_suppress_if_any_rule_matches_filter_matches_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, &'a str, usize, usize, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "QUE_INICIAL_AMBACCENT_HO",
            "Que els ho fa pensar?",
            0,
            3,
            &["Què"],
        ),
        (
            "QUE_INICIAL_SENSEACCENT_VERB",
            "Què hi ha la Maria?",
            // Java 0..3 (UTF-16); `è` is two bytes in UTF-8
            0,
            4,
            &["Que"],
        ),
        ("MES1", "Mes tard t'ho explicaré.", 0, 3, &["Més"]),
        ("MES1", "Destaca juliol com a mes sec.", 0, 0, &[]),
    ];
    for (rule, text, start, end, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let matches: Vec<_> = result
            .matches
            .iter()
            .filter(|m| m.rule_id == *rule)
            .collect();
        if suggestions.is_empty() {
            assert!(matches.is_empty(), "unexpected match for {rule} / {text}");
            continue;
        }
        let m = matches
            .first()
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{rule}");
    }
}

/// `PostponedAdjectiveConcordanceFilter` probed against the pinned Java
/// build (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20). Offsets are
/// UTF-8 bytes; Java prints UTF-16 (`Anàlisis clínica` gains two bytes).
#[test]
fn catalan_postponed_adjective_filter_matches_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, usize, usize, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "Disculpeu el retard mestra.",
            19,
            26,
            &[", mestra", " mestre"],
        ),
        ("La casa del poble blanques.", 18, 26, &["blanc", "blanca"]),
        (
            "Anàlisis clínica.",
            // Java 9..16 (UTF-16); `à` and `í` are two bytes each
            10,
            18,
            &["clíniques"],
        ),
        ("En un restaurant luxosa", 17, 23, &["luxós"]),
        (
            "amb rigor i honor barrejades",
            18,
            28,
            &["barrejat", "barrejats"],
        ),
        ("D'Ariany a Inca, camins rural.", 24, 29, &["rurals"]),
        ("Compra una taula petits.", 17, 23, &["petita"]),
    ];
    for (text, start, end, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &["AGREEMENT_POSTPONED_ADJ"]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "AGREEMENT_POSTPONED_ADJ")
            .unwrap_or_else(|| panic!("no match for {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{text}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{text}");
    }
    // negative example
    let Some(engine) = engine_with_rules("ca-ES", &["AGREEMENT_POSTPONED_ADJ"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Això no sempre dona resultats.").unwrap();
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "AGREEMENT_POSTPONED_ADJ"));
}

/// `DonarseliBeFilter` probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20). Offsets are UTF-8
/// bytes; Java prints UTF-16 (each accent in the span adds one byte).
#[test]
fn catalan_donarseli_be_filter_matches_java() {
    let _guard = engine_guard();
    type Case<'a> = (&'a str, usize, usize, &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "Diu a vegades que no se li donen bé les matemàtiques.",
            // Java 21..35 (UTF-16); `é` is two bytes
            21,
            36,
            &[
                "té traça per a",
                "fa bé",
                "se'n surt en",
                "li van bé",
                "li surten bé",
            ],
        ),
        (
            "Què se'm donava millor?",
            // Java 0..22 (UTF-16)
            0,
            23,
            &[
                "En què tenia traça",
                "Què feia millor",
                "En què me'n sortia",
                "Què m'anava millor",
                "Què em sortia millor",
            ],
        ),
        (
            "Eren coses que a ells no se'ls havien donat mai bé.",
            // Java 11..50 (UTF-16)
            11,
            51,
            &[
                "en què no havien tingut mai traça",
                "que no havien fet mai bé",
                "en què no se n'havien sortit",
                "que a ells no els havien anat mai bé",
                "que a ells no els havien sortit mai bé",
            ],
        ),
        (
            "Eren coses que a ells se'ls havien donat fatal.",
            11,
            46,
            &[
                "en què no havien tingut traça",
                "que havien fet malament",
                "en què no se n'havien sortit",
                "que a ells els havien anat malament",
                "que a ells els havien sortit malament",
            ],
        ),
        (
            "Les coses que mai no se m'ha donat bé.",
            // Java 10..37 (UTF-16)
            10,
            38,
            &[
                "en què mai no he tingut traça",
                "que mai no he fet bé",
                "en què mai no me n'he sortit",
                "que mai no m'ha anat bé",
                "que mai no m'ha sortit bé",
            ],
        ),
        (
            "Les coses que mai no se m'ha donat malament.",
            10,
            43,
            &[
                "en què he tingut traça",
                "que mai no he fet malament",
                "en què me n'he sortit",
                "que mai no m'ha anat malament",
                "que mai no m'ha sortit malament",
            ],
        ),
    ];
    for (text, start, end, suggestions) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &["DONARSELI_BE"]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "DONARSELI_BE")
            .unwrap_or_else(|| panic!("no match for {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{text}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{text}");
    }
    // negative example
    let Some(engine) = engine_with_rules("ca-ES", &["DONARSELI_BE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Ha seguit les instruccions tan bé com ha pogut.")
        .unwrap();
    assert!(result.matches.iter().all(|m| m.rule_id != "DONARSELI_BE"));
}

/// Stage-2 speller: `MorfologikCatalanSpellerRule.getSpellingSuggestions`
/// over the `ca-ES_spelling.dict` + plain-text dictionaries
/// (`spelling.txt`, `multiwords.txt`, `spelling-special.txt`,
/// `core/spelling_global.txt`), the `ca/hunspell/{ignore,prohibit}.txt`
/// lists and `setIgnoreTaggedWords()`. Java-probed with
/// `scripts/oracle/ca/probe-speller.sh ca-ES` (2026-09-19).
///
/// The probed sets contain only words whose suggestion list is identical;
/// `MorfologikCatalanSpellerRule.orderSuggestions` (weight-jump filter,
/// `inalambric`, capitalized-suggestion dropping) and
/// `getAdditionalTopSuggestions` (apostrophe/hyphen splits) are stage 3, so
/// e.g. `catalunya` (`Catalunya` in Java vs the full distance list here)
/// and `anarsen` (`anar-se'n` first) are not pinned yet.
#[test]
fn catalan_speller_matches_java() {
    let _guard = engine_guard();
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let rule = lt::spelling_ca_probe(&data);
    let cases: &[(&str, &str)] = &[
        ("ttets", "trets|tats|t'ets|tets|tiets|titets"),
        ("universidat", "universitat"),
        (
            "paguina",
            "pàgina|pagina|paguin|pegolina|peguin|beguina|eguina|eguine|eguinà|pagine|paginà|pagui na|paguin a",
        ),
        ("govenar", "governar"),
        // `orderSuggestions` (2026-09-20 probe): capitalized-suggestion
        // removals, the tagger filters, the `inalambric` rewrite and the
        // `MAX_WEIGHT_DIFF` weight-jump cut.
        ("pianiste", "pianista"),
        ("llargaries", "llargàries"),
        ("retaula", "retaule"),
        ("amino", "Amino"),
        ("Ercole", "Escola"),
        (
            "presones",
            "persones|presoners|presons|presoner|pregones|grassones|personés|pregonés|pressiones|pressionés|pressudes|pressures|presumes|presures|pres ones",
        ),
        ("panteixants", "panteixant|pantaixant|pantaixats|panteixats|panteix ants"),
        ("correiol", "corriol|corre iol"),
        ("cesarea", "Cesarea"),
        ("peridistes", "periodistes|partidistes|paradistes|parodistes|peridis tes"),
        ("Ofical", "Oficial"),
        ("quesar", "quedar|quefer|kasai|kasseri|queer|querar|quàsar"),
        ("feb", "fa|fer|fet|fan|web|fem|fe|feu|fas|fes|fam|far|féu|av|fen|fai|fava|faç|fat|fax|fel|lev|cav|falb|gab|AEB|FCB|FEF|FEN|FEP|FFB|FGB|FIB|FLB|FNB|FPB|FSB|FVB|FWB|OEB|bav|cab|efeb|fad|favó|febr|fé|hab|jab|nev|FB"),
        // `getAdditionalTopSuggestions` (apostrophe/hyphen splits and the
        // `PronomsFeblesHelper.transformDarrere` multiple-pronoun case).
        ("lhora", "hora|l'hora|alhora|llora|lora|l'ora|hura|l'Orà|l'hura|l'ore|l'orà|laura"),
        ("anarsen", "anar-se'n|anaren|anassen|anessen|enarcen|enarten|enarça|enarçam|enarçant|enarçar|enarçat|enarçau|enerven|enrasen|enversen|anar sen"),
        ("danarsen", "d'anar-se'n|denerven"),
        ("inalambric", "sense fils|sense fil|sense cables|autònom"),
        ("Ryanair2", "Ryanair"),
        ("tele5", "tele|tela|teles|teler|tala|tales|tale|talat|talar|tèlex|talam|talau|talec|talem|talen|taler|taleu|talés|telam|telar|telat|telau|telem|telen|teleu|telés|télex"),
        ("trets", ""),
        ("tets", ""),
        ("tiets", ""),
        ("setmana", ""),
        ("estudiar", ""),
        ("begudes", ""),
        ("taul", ""),
        ("cadira", ""),
        ("metge", ""),
        ("treball", ""),
        ("hivern", ""),
        ("estiu", ""),
        ("tardor", ""),
        ("primavera", ""),
        ("cotxe", ""),
        ("finestra", ""),
        ("llibre", ""),
        ("taula", ""),
        ("Universitat", ""),
        ("església", ""),
        ("informació", ""),
        ("pàgina", ""),
        ("telèfon", ""),
        ("ciutat", ""),
        ("viatge", ""),
        ("governar", ""),
        ("parlar", ""),
        ("menjar", ""),
    ];
    for (word, expected) in cases {
        assert_eq!(
            rule.suggestions(word).join("|"),
            *expected,
            "getSpellingSuggestions({word})"
        );
    }
}

/// The `IGNORE_ENGLISH_WORDS` disambiguation rules stay inert like in the
/// pinned Java oracle: the ca-only Maven classpath has no en-US language, so
/// `IsEnglishWordFilter` rejects every match and English words inside
/// Catalan sentences keep being reported by the speller. Pinned Java
/// (`RawSpellerProbe`-style probe in the ca module container, 2026-09-20):
/// `Languages.getLanguageForShortCode("en-US")` throws
/// `'en-US' is not a language code known to LanguageTool`; the corpus
/// `check-diff-ca.sh` output reports both words in `Això of the.` (UTF-16
/// 5..7 and 8..11).
#[test]
fn catalan_english_disambiguation_filter_is_inert() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["MORFOLOGIK_RULE_CA_ES"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Això of the.").unwrap();
    let ranges: Vec<(usize, usize)> = result
        .matches
        .iter()
        .map(|m| (m.range.start, m.range.end))
        .collect();
    assert_eq!(ranges, vec![(6, 8), (9, 12)], "speller matches for of/the");
}

/// Stage-1 core built-ins with the Catalan `MessagesBundle_ca` strings,
/// probed against the pinned Java build (`scripts/oracle/ca/probe-rule.sh
/// ca-ES`, one rule enabled at a time, 2026-09-19). The probes print UTF-16
/// offsets; the asserts use the UTF-8 byte offsets (`ò`/`é` are 2 bytes).
#[test]
fn catalan_core_rules_match_java() {
    let _guard = engine_guard();

    // UPPERCASE_SENTENCE_START (4): Java 0..2, suggestion "De"
    let Some(engine) = engine_with_rules("ca-ES", &["UPPERCASE_SENTENCE_START"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("de gat dorm.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "UPPERCASE_SENTENCE_START");
    assert_eq!((m.range.start, m.range.end), (0, 2));
    assert_eq!(m.message, "Aquesta frase no comença amb majúscula.");
    assert_eq!(m.suggestions[0].value, "De");

    // COMMA_PARENTHESIS_WHITESPACE (1): Java 17..19, suggestion "."
    let Some(engine) = engine_with_rules(
        "ca-ES",
        &["COMMA_PARENTHESIS_WHITESPACE", "DOUBLE_PUNCTUATION"],
    ) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Això és una prova .").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "COMMA_PARENTHESIS_WHITESPACE")
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (19, 21));
    assert_eq!(m.message, "No deixeu cap espai abans del punt.");
    assert_eq!(m.suggestions[0].value, ".");

    // DOUBLE_PUNCTUATION (2): Java 7..9, suggestion ","
    let result = engine.check("Això és,, una prova.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DOUBLE_PUNCTUATION")
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (9, 11));
    assert_eq!(m.message, "Dues comes consecutives");
    assert_eq!(m.suggestions[0].value, ",");

    // UNPAIRED_BRACKETS (3, CatalanUnpairedBracketsRule): Java 8..9 with the
    // hardcoded message and the combined-symbol + remove suggestions
    let Some(engine) = engine_with_rules("ca-ES", &["UNPAIRED_BRACKETS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Això és (una prova.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "UNPAIRED_BRACKETS");
    assert_eq!((m.range.start, m.range.end), (10, 11));
    assert_eq!(
        m.message,
        "Símbol sense parella. Afegiu-lo i situeu-lo manualment en el lloc adequat, o bé esborreu-lo."
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["()", ""]);

    // WHITESPACE_RULE (6, MultipleWhitespaceRule): Java 7..9 / 7..10,
    // suggestion " "
    let Some(engine) = engine_with_rules("ca-ES", &["WHITESPACE_RULE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Això és  bo.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "WHITESPACE_RULE");
    assert_eq!((m.range.start, m.range.end), (9, 11));
    assert_eq!(m.message, "Possible error: heu repetit un espai en blanc");
    assert_eq!(m.suggestions[0].value, " ");

    let result = engine.check("Això és   bo.").unwrap();
    let m = &result.matches[0];
    assert_eq!((m.range.start, m.range.end), (9, 12));
}

/// Stage-3c tokenizer/tagger: the whole
/// `scripts/oracle/ca/tagger-sentences.java.tsv` dump (450 lines, pinned
/// Java build 2026-09-19) must be reproduced byte-for-byte by
/// `lt_tokenize::CatalanWordTokenizer` + the `CatalanTagger` heuristics
/// (typographic apostrophes, ela geminada, decimal point/comma, digit
/// spaces, contractions, pronoms febles, `-ment` adverbs, ALLUPPERCASE
/// exceptions, incorrect verbs from `replace_verbs.txt`).
#[test]
fn catalan_tagger_heuristics_match_java() {
    let _guard = engine_guard();
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture =
        match std::fs::read_to_string(root.join("scripts/oracle/ca/tagger-sentences.java.tsv")) {
            Ok(text) => text,
            Err(_) => {
                eprintln!("skipping: no tagger fixture");
                return;
            }
        };
    let sentences = std::fs::read_to_string(root.join("scripts/oracle/ca/tagger-sentences.txt"))
        .expect("sentences file");
    let tagger = lt::catalan_tagger_ca_probe(&data);
    let is_tagged = |w: &str| tagger.is_tagged_word(w);
    let tokenizer = lt_tokenize::CatalanWordTokenizer::new(&is_tagged);
    let mut out = String::new();
    for line in sentences.lines() {
        if line.is_empty() {
            continue;
        }
        out.push_str(&format!("S\t{line}\n"));
        let tokens = tokenizer.tokenize(line);
        let readings = tagger.tag(&tokens);
        for (token, atr) in tokens.iter().zip(readings.iter()) {
            let mut sb = String::from("T\t");
            sb.push_str(token);
            sb.push('\t');
            for (j, r) in atr.readings.iter().enumerate() {
                if j > 0 {
                    sb.push('|');
                }
                sb.push_str(r.stem.as_deref().unwrap_or("null"));
                sb.push(':');
                sb.push_str(r.pos_tag.as_deref().unwrap_or("null"));
            }
            out.push_str(&sb);
            out.push('\n');
        }
    }
    assert_eq!(out, fixture);
}

/// Stage-3 filters: `AdaptSuggestionsFilter` (with `Catalan.adaptSuggestion`)
/// and `AddCommasFilter`, probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19). `DET_GN` and
/// `CONCORDANCES_DET_NOM` are byte-identical; the `MOLT_DE_MOLTA`/
/// `PASSAR_SE`/`TENIR_QUE` probes differ in the pre-filter match range and
/// suggestion extraction (corpus-triage item, not the adapt filter).
#[test]
fn catalan_stage3a_filters_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["CONCORDANCES_DET_NOM"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Les Amazones muntaven a cavall.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "CONCORDANCES_DET_NOM");
    assert_eq!((m.range.start, m.range.end), (0, 12));
    assert_eq!(
        m.message,
        "¿Volíeu dir \"amazones\" (dones guerreres) o bé \"l'Amazones\" (riu)?"
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Les amazones", "l'Amazones"]);

    let Some(engine) = engine_with_rules("ca-ES", &["DET_GN"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("A causa dels poc trànsit web.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DET_GN")
        .unwrap();
    // Java: 8..12 "del" (all-ASCII prefix, so UTF-16 and UTF-8 agree)
    assert_eq!((m.range.start, m.range.end), (8, 12));
    assert_eq!(m.message, "Error de concordança.");
    assert_eq!(m.suggestions[0].value, "del");
}

/// `SynthesizeWithAnyDeterminerFilter` (CONCORDANCES_DET_NOM sub 7 and the
/// other 18 refs): Java returns a match starting at the determiner
/// (`firstUnderlinedToken`) and splices the `_QM_OPEN` token into the
/// suggestions. Probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20):
/// `Porta aquell camions.` -> 6..20 `aquells camions|aquell camió`;
/// `Farem una "arreplec de pistrincs.` -> 6..19 `un "arreplec|uns "arreplecs`.
#[test]
fn catalan_synthesize_any_determiner_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["CONCORDANCES_DET_NOM"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Porta aquell camions.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CONCORDANCES_DET_NOM" && m.sub_id.as_deref() == Some("7"))
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (6, 20));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["aquells camions", "aquell camió"]);

    let result = engine.check("Farem una \"arreplec de pistrincs.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CONCORDANCES_DET_NOM" && m.sub_id.as_deref() == Some("7"))
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (6, 19));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["un \"arreplec", "uns \"arreplecs"]);
}

/// Java's `Catalan.adjustCatalanMatch` rebuilds the empty-suggestion match
/// with the plain `RuleMatch(rule, sentence, …)` constructor, which resets
/// the match type to `Other` (the rule-level `Hint` is lost). Pinned
/// against the pinned Java build (`check-diff-ca.sh`, 2026-09-20):
/// `Segur que no li senta mal al meu estómac.` -> POSSESSIUS_PARTS_COS[3]
/// 29..33 (UTF-16), type Other, no suggestions.
#[test]
fn catalan_empty_suggestion_resets_match_type() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["POSSESSIUS_PARTS_COS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Segur que no li senta mal al meu estómac.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "POSSESSIUS_PARTS_COS" && m.sub_id.as_deref() == Some("3"))
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (29, 33));
    assert_eq!(m.match_type, "Other");
    assert_eq!(m.suggestions.len(), 1);
    assert!(m.suggestions[0].value.is_empty());
}

/// `<match postag="…">` without `postag_regexp` goes through Java's 2-arg
/// `Synthesizer.synthesize`, which `CatalanSynthesizer` overrides to treat
/// the tag as a regexp. Pinned against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20):
/// `Trenta-dos home.` -> CONCORDANCES_NUMERALS `homes|hòmens`;
/// `Era propi del segle 20.` -> SEGLES_ROMANS `XX`.
#[test]
fn catalan_two_arg_synthesis_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["CONCORDANCES_NUMERALS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Trenta-dos home.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CONCORDANCES_NUMERALS")
        .unwrap();
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["homes", "hòmens"]);

    let Some(engine) = engine_with_rules("ca-ES", &["SEGLES_ROMANS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Era propi del segle 20.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "SEGLES_ROMANS")
        .unwrap();
    assert_eq!(m.suggestions[0].value, "XX");
}

/// Stage-3b filter: `DiacriticsCheckFilter` (`ca/confusion_pairs.txt`,
/// 47 XML refs) probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-19).
#[test]
fn catalan_diacritics_filter_matches_java() {
    let _guard = engine_guard();
    let cases: &[(&str, &str, usize, usize, &str, &str)] = &[
        (
            "DIACRITICS_00",
            "Numero 1.",
            0,
            6,
            "Si és adjectiu o nom, s'escriu amb accent.",
            "Número",
        ),
        (
            "DIACRITICS_01",
            "Fa rabia.",
            3,
            8,
            "Si és adjectiu o nom, s'escriu amb accent.",
            "ràbia",
        ),
        (
            "DIACRITICS_02",
            "Una trucada sol·licitant presencia policial.",
            // Java 25..34 (UTF-16); `·` is 2 bytes in UTF-8
            26,
            35,
            "Si és adjectiu o nom, s'escriu amb accent.",
            "presència",
        ),
        (
            "ACCENTUATION_CHECK",
            "Amb industria.",
            4,
            13,
            "Si és un adjectiu o un nom, s'escriu amb accent.",
            "indústria",
        ),
    ];
    for (rule, text, start, end, message, suggestion) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("no match for {rule} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule}");
        assert_eq!(m.message, *message, "{rule}");
        assert!(
            m.suggestions.iter().any(|s| s.value == *suggestion),
            "{rule}: missing {suggestion} in {:?}",
            m.suggestions
                .iter()
                .map(|s| s.value.as_str())
                .collect::<Vec<_>>()
        );
    }
}

/// Stage-3p filter: `QueIniciFilter` (the last reject stub), probed against
/// the pinned Java build (`scripts/oracle/ca/probe-rule.sh ca-ES`,
/// 2026-09-20) on `QUE_INICIAL_AMBACCENT_VERB` (both inner rules: the
/// `mode:completive` pattern and the plain verb pattern). All 37 probe
/// sentences are byte-identical (24 positive, 13 negative); offsets are
/// UTF-8 bytes (all-ASCII here, so UTF-16 agrees).
#[test]
fn catalan_que_inici_filter_matches_java() {
    const MSG_VERB: &str = "Si és un pronom interrogatiu (=quina cosa), s'escriu amb accent. \
S'escriu sense accent si la resposta és \"sí\" o \"no\" (\"Que vindràs?\").";
    const MSG_COMPLETIVE: &str = "Si equival a \"quina cosa\", cal escriure \
<suggestion>què</suggestion>. S’escriu sense accent si la resposta és «sí» o «no».";
    #[allow(clippy::type_complexity)]
    let cases: &[(&str, Option<(usize, usize, &str, &str)>)] = &[
        ("Que ha passat?", Some((0, 3, MSG_COMPLETIVE, "Què"))),
        ("Que vols?", Some((0, 3, MSG_COMPLETIVE, "Què"))),
        ("Que vols que faci?", Some((0, 3, MSG_COMPLETIVE, "Què"))),
        ("Que vols que t'ho porti?", None),
        (
            "Que passa, que no t'agrada el que faig, potser?",
            Some((0, 3, MSG_COMPLETIVE, "Què")),
        ),
        ("Que passa dimecres?", Some((0, 3, MSG_COMPLETIVE, "Què"))),
        ("Que passa les vacances a Menorca?", None),
        ("Que passa alguna desgràcia doctor?", None),
        ("Que et fa mal?", None),
        ("Que et fa por?", None),
        ("Que faig aquí?", Some((0, 3, MSG_VERB, "Què"))),
        ("Que hi faig aquí?", Some((0, 3, MSG_VERB, "Què"))),
        ("Que fa Joan?", Some((0, 3, MSG_VERB, "Què"))),
        ("Que fa Correus?", Some((0, 3, MSG_VERB, "Què"))),
        ("Que fa la setmana que ve?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que pot ser més versemblant?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que creus que ha passat?", Some((0, 3, MSG_VERB, "Què"))),
        ("Que creus que té raó?", None),
        (
            "Que dius que va dir en Joan?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que dius que sap la resposta?", None),
        ("Que penses que diran?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que compta més, ser president o ser príncep?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que convé més?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que seria de mi sense la vostra ajuda?",
            Some((0, 3, MSG_COMPLETIVE, "Què")),
        ),
        (
            "Que recordes més dels teus estius infantils?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que t'ha agradat més?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que se'n va fer, de la teva mare?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que hi ha a la caixa?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que fèieu els dos aquí dins?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que la pot retirar en qualsevol moment?", None),
        ("Que en vols fer, d'això?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que amagava o què preparava?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        ("Que busca la Mercè?", Some((0, 3, MSG_VERB, "Què"))),
        (
            "Que et va agradar de la cançó?",
            Some((0, 3, MSG_VERB, "Què")),
        ),
        (
            "Que passa cada vegada que ho intenten?",
            Some((0, 3, MSG_COMPLETIVE, "Què")),
        ),
        (
            "Que passarà el febrer vinent?",
            Some((0, 3, MSG_COMPLETIVE, "Què")),
        ),
        (
            "Que menjarien l'endemà?",
            Some((0, 3, MSG_COMPLETIVE, "Què")),
        ),
    ];
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["QUE_INICIAL_AMBACCENT_VERB"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    for (text, expected) in cases {
        let result = engine.check(text).unwrap();
        let found = result
            .matches
            .iter()
            .find(|m| m.rule_id == "QUE_INICIAL_AMBACCENT_VERB");
        match expected {
            None => assert!(found.is_none(), "unexpected match for {text}: {found:?}"),
            Some((start, end, message, suggestion)) => {
                let m = found.unwrap_or_else(|| panic!("no match for {text}"));
                assert_eq!((m.range.start, m.range.end), (*start, *end), "{text}");
                assert_eq!(m.message, *message, "{text}");
                assert_eq!(m.suggestions[0].value, *suggestion, "{text}");
            }
        }
    }
}

/// Stage-3q/b D-154: `ValencianCatalan`/`BalearicCatalan` variant rule
/// defaults (`getDefaultEnabledRulesForVariant` /
/// `getDefaultDisabledRulesForVariant`) probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES{,-valencia}`, 2026-09-20).
#[test]
fn catalan_variant_default_rules_match_java() {
    let _guard = engine_guard();
    let Some(es) = engine_variant("ca-ES") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // ca-ES: general accentuation + `v` possessives + central verbs are
    // default on; the valencia counterparts stay off.
    let result = es.check("El café és bo.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EXIGEIX_ACCENTUACIO_GENERAL")
        .expect("GENERAL");
    // Java 3..7 (UTF-16); `è` is 2 bytes in UTF-8
    assert_eq!((m.range.start, m.range.end), (3, 8));
    assert_eq!(m.message, "Useu accentuació general.");
    assert_eq!(m.suggestions[0].value, "cafè");
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "EXIGEIX_ACCENTUACIO_VALENCIANA"));
    let result = es.check("La meua casa és gran.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EXIGEIX_POSSESSIUS_V")
        .expect("POSSESSIUS_V");
    assert_eq!((m.range.start, m.range.end), (3, 7));
    assert_eq!(m.message, "Esteu usant possessius amb 'v'.");
    assert_eq!(m.suggestions[0].value, "meva");
    let result = es.check("Ell ens exigix un resultat.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EXIGEIX_VERBS_CENTRAL")
        .expect("VERBS_CENTRAL");
    assert_eq!((m.range.start, m.range.end), (8, 14));
    assert_eq!(
        m.message,
        "Aquesta forma verbal no es correspon amb la varietat seleccionada (central)."
    );
    assert_eq!(m.suggestions[0].value, "exigeix");
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "EXIGEIX_VERBS_EIX"));

    // ca-ES-valencia: the valencia rules are on, the central ones off.
    let Some(valencia) = engine_variant("ca-ES-valencia") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = valencia.check("El cafè és bo.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EXIGEIX_ACCENTUACIO_VALENCIANA")
        .expect("VALENCIANA");
    // Java 3..7 (UTF-16); `é` is 2 bytes in UTF-8
    assert_eq!((m.range.start, m.range.end), (3, 8));
    assert_eq!(m.message, "Useu accentuació valenciana.");
    assert_eq!(m.suggestions[0].value, "café");
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "EXIGEIX_ACCENTUACIO_GENERAL"));
    let result = valencia.check("La meva casa és gran.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EXIGEIX_POSSESSIUS_U")
        .expect("POSSESSIUS_U");
    assert_eq!((m.range.start, m.range.end), (3, 7));
    assert_eq!(m.message, "Esteu usant possessius amb 'u'.");
    assert_eq!(m.suggestions[0].value, "meua");
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "EXIGEIX_POSSESSIUS_V"));
    let result = valencia.check("Ell ens exigix un resultat.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EXIGEIX_VERBS_EIX")
        .expect("VERBS_EIX");
    assert_eq!((m.range.start, m.range.end), (8, 14));
    assert_eq!(m.message, "Esteu usant terminacions verbals en -eix.");
    assert_eq!(m.suggestions[0].value, "exigeix");
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "EXIGEIX_VERBS_CENTRAL"));
}

/// Stage-3r built-ins (D-155): `CatalanWrongWordInContextRule` (12),
/// `WordCoherencyRule` (29) + `WordCoherencyValencianRule` and
/// `LongSentenceRule` (7), probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20).
#[test]
fn catalan_stage3r_builtin_rules_match_java() {
    let _guard = engine_guard();
    // `CATALAN_WRONG_WORD_IN_CONTEXT` (sub-rule specific ids, setMatchLemmmas)
    let cases: &[(&str, &str, usize, usize, &str, &str)] = &[
        (
            "CATALAN_WRONG_WORD_IN_CONTEXT_INFRINGIR_INFLIGIR",
            "Li va infringir un mal terrible.",
            6,
            15,
            "¿Volíeu dir <suggestion>infligir</suggestion> (aplicar) en lloc de \"infringir\" (transgredir)?",
            "infligir",
        ),
        (
            "CATALAN_WRONG_WORD_IN_CONTEXT_BETES_VETES",
            "No li va cosir bé les betes.",
            // Java 22..27 (UTF-16); `é` is 2 bytes in UTF-8
            23,
            28,
            "¿Volíeu dir <suggestion>vetes</suggestion> (cinta de teixit o filó de mineral) en lloc de \"betes\" (lletra grega)?",
            "vetes",
        ),
    ];
    for (rule_id, text, start, end, message, suggestion) in cases {
        let Some(engine) = engine_with_rules("ca-ES", &["CATALAN_WRONG_WORD_IN_CONTEXT"]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *rule_id)
            .unwrap_or_else(|| panic!("no match for {rule_id} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{rule_id}");
        assert_eq!(m.message, *message, "{rule_id}");
        assert_eq!(m.suggestions[0].value, *suggestion, "{rule_id}");
    }

    // `CA_WORD_COHERENCY` (text level, suggestion from the other spelling)
    let Some(engine) = engine_with_rules("ca-ES", &["CA_WORD_COHERENCY"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Un pesebre ací i un altre pessebre allà.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CA_WORD_COHERENCY")
        .expect("CA_WORD_COHERENCY");
    // Java 26..34 (UTF-16); `í` is 2 bytes in UTF-8
    assert_eq!((m.range.start, m.range.end), (27, 35));
    assert_eq!(
        m.message,
        "No és coherent usar 'pessebre' i 'pesebre' dins d'un mateix text."
    );
    assert_eq!(m.suggestions[0].value, "pesebre");

    // `WordCoherencyValencianRule` (ca-ES-valencia addition)
    let Some(engine) = engine_with_rules("ca-ES-valencia", &["CA_WORD_COHERENCY_VALENCIA"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Este home d'ací parla amb aquest altre ací.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CA_WORD_COHERENCY_VALENCIA")
        .expect("CA_WORD_COHERENCY_VALENCIA");
    // Java 26..32 (UTF-16); `í` is 2 bytes in UTF-8
    assert_eq!((m.range.start, m.range.end), (27, 33));
    assert_eq!(
        m.message,
        "No és coherent usar 'aquest' i 'este' dins d'un mateix text."
    );
    assert_eq!(m.suggestions[0].value, "este");

    // `TOO_LONG_SENTENCE` (60 words, picky)
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["TOO_LONG_SENTENCE".to_string()],
        enabled_only: true,
        picky: true,
        ..Default::default()
    };
    let Some(engine) = Engine::builder(Lang::Ca).ok().and_then(|b| {
        b.variant("ca-ES")
            .data_dir(data)
            .options(options)
            .build()
            .ok()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let long = format!("{}.", vec!["mot"; 65].join(" "));
    let result = engine.check(&long).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "TOO_LONG_SENTENCE")
        .expect("TOO_LONG_SENTENCE");
    assert_eq!((m.range.start, m.range.end), (0, 260));
    assert_eq!(
        m.message,
        "Aquesta frase té més de 60 paraules. Considereu revisar-la. Les frases més curtes fan el text més llegible."
    );
}

/// Stage-3s (D-156): the `AbstractSimpleReplaceRule` legacy family
/// (13–18, 22), the `AbstractSimpleReplaceRule2` instances (16, 19) and the
/// DNV lemma rules (26–28), probed against the pinned Java build
/// (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20).
#[test]
fn catalan_stage3s_simple_replace_family_matches_java() {
    fn engine_picky(variant: &str, rules: &[&str]) -> Option<Engine> {
        let data = data_dir()?;
        let options = EngineOptions {
            enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
            enabled_only: true,
            picky: true,
            ..Default::default()
        };
        Engine::builder(Lang::Ca)
            .ok()?
            .variant(variant)
            .data_dir(data)
            .options(options)
            .build()
            .ok()
    }
    let _guard = engine_guard();
    #[allow(clippy::type_complexity)]
    let cases: &[(&str, &str, &str, usize, usize, &[&str])] = &[
        (
            "CA_SIMPLE_REPLACE_VERBS",
            "Ell va lliberar el pres.",
            "CA_SIMPLE_REPLACE_VERBS_LLIBERAR",
            7,
            15,
            &["Verb incorrecte.", "alliberar"],
        ),
        (
            "CA_SIMPLE_REPLACE_BALEARIC",
            "L'articul és nou.",
            "CA_SIMPLE_REPLACE_BALEARIC_ARTICUL",
            2,
            9,
            &[
                "Possible error ortogràfic (forma verbal vàlida en la varietat balear).",
                "article",
            ],
        ),
        (
            "CA_SIMPLE_REPLACE_SIMPLE",
            "Viatjo a Butan.",
            "CA_SIMPLE_REPLACE_SIMPLE_BUTAN",
            9,
            14,
            &["¿Volíeu dir «Bhutan»?", "Bhutan"],
        ),
        (
            "NOMS_OPERACIONS",
            "El assecat és bo.",
            "NOMS_OPERACIONS_ASSECAT",
            0,
            10,
            &[
                "Si és el nom d'una operació tècnica, val més usar una altra forma.",
                "L'assecament|L'assecatge|L'eixugada",
            ],
        ),
        (
            "CA_SIMPLE_REPLACE_DIACRITICS_IEC",
            "Ahir vaig dir adéu.",
            "CA_SIMPLE_REPLACE_DIACRITICS_IEC_ADÉU",
            // Java 14..18 (UTF-16); `é` is 2 bytes in UTF-8
            14,
            19,
            &["Hi sobra l'accent diacrític (segons les normes noves).", "adeu"],
        ),
        (
            "ADVERBIS_MENT",
            "Actualment plou.",
            "ADVERBIS_MENT_ACTUALMENT",
            0,
            10,
            &[
                "A vegades s'abusa dels adverbis acabats en -ment en detriment de formes més àgils.",
                "Avui|Avui dia|Hui|Hui dia",
            ],
        ),
        (
            "CA_SIMPLE_REPLACE_MULTIWORDS",
            "Cal hidrat carboni.",
            "CA_SIMPLE_REPLACE_MULTIWORDS_HIDRAT_CARBONI",
            4,
            10,
            &["Expressió incorrecta.", "hidrat de"],
        ),
        (
            "CA_SIMPLE_REPLACE_ANGLICISM",
            "Vaig comprar un power bank.",
            "CA_SIMPLE_REPLACE_ANGLICISM_POWER_BANK",
            16,
            26,
            &[
                "Anglicisme innecessari. Considereu fer servir una altra paraula.",
                "bateria externa|carregador portàtil",
            ],
        ),
        (
            "CA_SIMPLE_REPLACE_DNV_SECONDARY",
            "Eren uns armatosts.",
            "CA_SIMPLE_REPLACE_DNV_SECONDARY_ARMATOST",
            9,
            18,
            &["Paraula o forma secundària.", "baluernes"],
        ),
        (
            "CA_SIMPLE_REPLACE_DNV_SECONDARY",
            "Era un armatost.",
            "CA_SIMPLE_REPLACE_DNV_SECONDARY_ARMATOST",
            7,
            15,
            &["Paraula o forma secundària.", "baluerna"],
        ),
    ];
    for (rule, text, match_id, start, end, message_and_suggestions) in cases {
        let Some(engine) = engine_picky("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *match_id)
            .unwrap_or_else(|| panic!("no match for {match_id} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{match_id}");
        assert_eq!(m.message, message_and_suggestions[0], "{match_id}");
        let suggestions: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(
            suggestions,
            message_and_suggestions[1].split('|').collect::<Vec<_>>(),
            "{match_id}"
        );
    }
}

/// Stage-3t (D-157): `CatalanWordRepeatRule` (8),
/// `PronomFebleDuplicateRule` (20), `CheckCaseRule` (21),
/// `CatalanWordRepeatBeginningRule` (23), `CompoundRule` (24) and the
/// unpaired question/exclamation marks (10/11), probed against the pinned
/// Java build (`scripts/oracle/ca/probe-rule.sh ca-ES`, 2026-09-20).
#[test]
fn catalan_stage3t_remaining_builtins_match_java() {
    fn engine_picky(variant: &str, rules: &[&str]) -> Option<Engine> {
        let data = data_dir()?;
        let options = EngineOptions {
            enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
            enabled_only: true,
            picky: true,
            ..Default::default()
        };
        Engine::builder(Lang::Ca)
            .ok()?
            .variant(variant)
            .data_dir(data)
            .options(options)
            .build()
            .ok()
    }
    #[allow(clippy::type_complexity)]
    let cases: &[(&str, &str, &str, usize, usize, &str, &[&str])] = &[
        (
            "CATALAN_WORD_REPEAT_RULE",
            "El el cotxe.",
            "CATALAN_WORD_REPEAT_RULE",
            0,
            5,
            "Possible error: heu repetit una paraula",
            &["El"],
        ),
        (
            "PRONOMS_FEBLES_DUPLICATS",
            "S'ha de fer-se.",
            "PRONOMS_FEBLES_DUPLICATS",
            0,
            14,
            "Combinació incorrecta de pronoms febles. Deixeu els de davant o els de darrere del verb.",
            &["Ha de fer-se", "S'ha de fer"],
        ),
        (
            "CA_CHECKCASE",
            "Coneix en joan pau.",
            "CA_CHECKCASE_JOAN_PAU",
            10,
            18,
            "Majúscules i minúscules recomanades. Alguns llibres d'estil poden suggerir solucions diferents en alguns casos.",
            &["Joan Pau"],
        ),
        (
            "CATALAN_WORD_REPEAT_BEGINNING_RULE",
            "Plou molt. Plou poc. Plou gens.",
            "CATALAN_WORD_REPEAT_BEGINNING_RULE",
            21,
            25,
            "Tres frases successives comencen amb la mateixa paraula. Considereu reescriure la frase o usar un sinònim.",
            &[],
        ),
        (
            "CATALAN_WORD_REPEAT_BEGINNING_RULE",
            "Igualment plou. Igualment fa sol.",
            "CATALAN_WORD_REPEAT_BEGINNING_RULE",
            16,
            25,
            "Dues frases consecutives comencen amb el mateix element. Considereu reescriure la frase o usar un sinònim.",
            &["Addicionalment", "També", "Així mateix", "A més a més"],
        ),
        (
            "CA_UNPAIRED_QUESTION",
            "Què fas?",
            "CA_UNPAIRED_QUESTION",
            // Java 0..3 (UTF-16); `è` is 2 bytes in UTF-8
            0,
            4,
            "Símbol sense parella: Sembla que falta un '¿'",
            &["¿Què"],
        ),
        (
            "CA_COMPOUNDS",
            "El vol de Ryan-Air.",
            "CA_COMPOUNDS_RYAN_AIR",
            10,
            18,
            "S'escriu junt sense espai ni guionet.",
            &["Ryanair"],
        ),
        (
            "CA_UNPAIRED_EXCLAMATION",
            "Quina sorpresa!",
            "CA_UNPAIRED_EXCLAMATION",
            0,
            5,
            "Símbol sense parella: Sembla que falta un '¡'",
            &["¡Quina"],
        ),
    ];
    let _guard = engine_guard();
    for (rule, text, match_id, start, end, message, suggestions) in cases {
        let Some(engine) = engine_picky("ca-ES", &[rule]) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == *match_id)
            .unwrap_or_else(|| panic!("no match for {match_id} / {text}"));
        assert_eq!((m.range.start, m.range.end), (*start, *end), "{match_id}");
        assert_eq!(m.message, *message, "{match_id}");
        let found: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(found, *suggestions, "{match_id}");
    }

    // `CA_SIMPLE_REPLACE_BALEARIC` is disabled by the ca-ES-balear variant
    // default unless explicitly enabled.
    let Some(balear) = engine_variant("ca-ES-balear") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = balear.check("L'articul és nou.").unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| !m.rule_id.starts_with("CA_SIMPLE_REPLACE_BALEARIC")),
        "balearic rule should be variant-disabled: {:?}",
        result
            .matches
            .iter()
            .map(|m| m.rule_id.as_str())
            .collect::<Vec<_>>()
    );
    let Some(engine) = engine_picky("ca-ES-balear", &["CA_SIMPLE_REPLACE_BALEARIC"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("L'articul és nou.").unwrap();
    assert!(result
        .matches
        .iter()
        .any(|m| m.rule_id == "CA_SIMPLE_REPLACE_BALEARIC_ARTICUL"));
}

/// Java `PatternRule.isInterpretPosTagsPreDisambiguation`
/// (`<pattern raw_pos="yes">`): Catalan rules match the token view captured
/// before the disambiguators, like Java's `getPreDisambigTokensWithoutWhitespace`.
/// Pinned against the pinned Java build (golden `ca-full.java.tsv`,
/// 2026-09-20): `Entre le riu i el camp.` -> `SUGGERIMENTS_LE` sub-rule 9
/// (`le` + `(A..|N.|PX.)[MC]S.*`) with `el|de|al`, not the preposition
/// catch-all sub-rule 19. `riu` loses its noun reading in the
/// disambiguated view, so only the raw view matches.
#[test]
fn catalan_raw_pos_rules_match_pre_disambiguation_tokens() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["SUGGERIMENTS_LE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Entre le riu i el camp.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "SUGGERIMENTS_LE")
        .expect("no SUGGERIMENTS_LE match");
    assert_eq!(m.sub_id.as_deref(), Some("9"));
    assert_eq!((m.range.start, m.range.end), (6, 8));
    assert_eq!(m.message, "¿Volíeu dir <suggestion>el</suggestion>?");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["el", "de", "al"]);
}

/// Java `PatternRuleMatcher.createRuleMatch` only builds a match when
/// `fromPos < toPos`. A pattern whose marker is a content-less token can
/// consume the zero-length synthetic SENT_START reading, which yields an
/// empty range; Java then drops the match. Pinned against the golden
/// (`ca-full.java.tsv`, 2026-09-20): `cosa que li feia empipar molt.` has no
/// `COMMA_COSA_QUE` match in Java (ours reported 0..0), while the intended
/// comma insert on `Ho van fer aviat cosa que va ser útil.` keeps the
/// FIXME-adjusted range up to the marker (UTF-16 12..17 -> UTF-8 11..16).
#[test]
fn catalan_zero_length_matches_are_dropped() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["COMMA_COSA_QUE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("cosa que li feia empipar molt.").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "COMMA_COSA_QUE"),
        "zero-length COMMA_COSA_QUE match should be dropped: {:?}",
        result.matches
    );
    let result = engine
        .check("Ho van fer aviat cosa que va ser útil.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "COMMA_COSA_QUE")
        .expect("no COMMA_COSA_QUE match");
    assert_eq!((m.range.start, m.range.end), (11, 16));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["aviat,"]);
}

/// `CatalanSynthesizer.getTargetPosTag` sorts the verb's candidate POS tags
/// with its `PostagComparator` (indicative over imperative, 3rd over 1st
/// person) before picking the last one; the naive last-tag port picked
/// `VMM02S00` for `té` and the `AdjustVerbSuggestionsFilter` synthesized
/// nothing. Pinned against the golden (`ca-full.java.tsv`, 2026-09-20):
/// `El té que posa ell.` -> `TENIR_QUE`[3] 0..14 (UTF-16), suggestion
/// `L'ha de posar` (the filter extends the range over the article and
/// apostrophizes).
#[test]
fn catalan_verb_suggestion_target_postag_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["TENIR_QUE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("El té que posa ell.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "TENIR_QUE" && m.sub_id.as_deref() == Some("3"))
        .expect("no TENIR_QUE sub-3 match");
    assert_eq!((m.range.start, m.range.end), (0, 15));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["L'ha de posar"]);
}

/// Java's `RuleMatch` constructor fills `originalErrorStr` from the
/// underlined range (`setOriginalErrorStr=true` for pattern and regexp
/// rules) and `Catalan.adjustCatalanMatch` appends a space to a suggestion
/// when the error text ends with an apostrophe. Pinned against the golden
/// (`ca-full.java.tsv`, 2026-09-20): `41° 22'44.` -> `GRAUS_MINUTS_SEGONS`[2]
/// suggestion `41°\u{202f}22′ ` (trailing space).
#[test]
fn catalan_apostrophe_error_appends_suggestion_space() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["GRAUS_MINUTS_SEGONS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("41° 22'44.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "GRAUS_MINUTS_SEGONS" && m.sub_id.as_deref() == Some("2"))
        .expect("no GRAUS_MINUTS_SEGONS sub-2 match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["41°\u{202f}22′ "]);
}

/// Java's `MatchState.getTargetPosTag` sorts the candidate tag list in place
/// exactly once (`synthesizer.getTargetPosTag`), then takes the last entry;
/// sorting again reorders the list because the Catalan comparator is not a
/// total order. Pinned against the golden (`ca-full.java.tsv`, 2026-09-20):
/// `No m'entere de res.` -> `ENTERARSE` suggestions `sé` (not `sàpia`) and
/// `Jo pensem que és així.` -> `JO_HEM_DIT`[2] `Ja pensem|Jo penso`.
#[test]
fn catalan_synth_target_tag_sort_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["ENTERARSE", "JO_HEM_DIT"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("No m'entere de res.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "ENTERARSE")
        .expect("no ENTERARSE match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["m'adone", "sé", "entenc", "estic al cas", "m'assabente"]
    );
    let result = engine.check("Jo pensem que és així.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "JO_HEM_DIT" && m.sub_id.as_deref() == Some("2"))
        .expect("no JO_HEM_DIT sub-2 match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Ja pensem", "Jo penso"]);
}

/// `SpellingCheckRule.filterSuggestions` ends with `filterDupes` =
/// `Stream.distinct()`, which compares `SuggestedReplacement` equality
/// (replacement + short description) and not the morfologik weight; the
/// ported `Cand` comparison kept a raw duplicate of the curated top
/// suggestion and the weight-jump filter then cut everything after it.
/// Java golden: `Sera` -> `Serà|S'era|Sara`.
#[test]
fn catalan_speller_dedupes_curated_weight() {
    let _guard = engine_guard();
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let rule = lt::spelling_ca_probe(&data);
    assert_eq!(rule.suggestions("Sera").join("|"), "Serà|S'era|Sara");
}

/// Java's `AbstractSimpleReplaceRule.createRuleMatch` builds the message
/// from the cleaned replacement list *before* uppercasing first characters
/// for capitalized input: `Aixó` -> message `¿Volíeu dir «això»?` with
/// suggestion `Això`.
#[test]
fn catalan_simple_replace_message_before_case() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["CA_SIMPLE_REPLACE_SIMPLE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Aixó no no he vist a venir.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id.starts_with("CA_SIMPLE_REPLACE_SIMPLE"))
        .expect("no simple-replace match");
    assert_eq!(m.message, "¿Volíeu dir «això»?");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Això"]);
}

/// `TextToNumberFilter.formatResult` replaces `.` with `,` after the
/// percentage suffix: `dos coma quatre per cent` -> `2,4\u{202f}%`.
#[test]
fn catalan_text_to_number_decimal_comma() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["NOMBRES_NO_LLETRES"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("L'economia creixerà el dos coma quatre per cent.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "NOMBRES_NO_LLETRES")
        .expect("no NOMBRES_NO_LLETRES match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["2,4\u{202f}%"]);
}

/// `PossessiusRedundantsFilter` checks `hasAnyPartialPosTag("VMN", "VMG")`
/// (prefix match) for the gerund pronoun placement: `Acariciant la seva
/// galta.` -> `Acariciant-li la` (not `Li acariciant la`).
#[test]
fn catalan_possessius_gerund_pronoun() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["POSSESSIUS_PARTS_COS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Acariciant la seva galta.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "POSSESSIUS_PARTS_COS")
        .expect("no POSSESSIUS_PARTS_COS match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Acariciant-li la"]);
}

/// `SynthesizeWithAnyDeterminerFilter` iterates every synthesized determiner
/// form: `Els mig relleus.` -> `migs relleus|mitjos relleus|mig relleu`.
#[test]
fn catalan_synthesize_any_determiner_all_forms() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["CONCORDANCES_DET_NOM"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Els mig relleus.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CONCORDANCES_DET_NOM")
        .expect("no CONCORDANCES_DET_NOM match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["migs relleus", "mitjos relleus", "mig relleu"]);
}

/// Java's `MultiWordChunker`/XML disambiguator mutate token objects in place
/// (`removeReading`/`addReading`/`immunize`/`ignoreSpelling`), and
/// `JLanguageTool.getAnalyzedSentence` keeps those *same* objects as the
/// pre-disambiguation view, so `raw_pos="yes"` rules see the in-place
/// mutations but not the wrapper-replacing actions. The port snapshots the
/// pre view after the Catalan chunkers and mirrors the in-place XML
/// actions. Golden: `M'ho va vendre a meitat de preu.` has no `A_MEITAT`
/// match in Java (the chunker removed `de`'s noun reading from the pre
/// view); `A meitat tarda.` still matches.
#[test]
fn catalan_raw_pos_sees_inplace_disambiguation() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["A_MEITAT"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("M'ho va vendre a meitat de preu.").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "A_MEITAT"),
        "A_MEITAT should not match: {:?}",
        result.matches
    );
    let result = engine.check("A meitat tarda.").unwrap();
    assert!(
        result.matches.iter().any(|m| m.rule_id == "A_MEITAT"),
        "A_MEITAT should match: {:?}",
        result.matches
    );
}

/// `FindSuggestionsEsFilter` returns null when the first token is an
/// accented `és`, only nominal suggestions were found and no 3rd-person
/// verb suggestion exists ("show just the spelling rule"); Java's
/// `equalsIgnoreCase` is Unicode-aware, so `És` counts. Golden: `És
/// preferieble …` reports `MORFOLOGIK_RULE_CA_ES` on `preferieble`, no
/// `ES_UNKNOWN`.
#[test]
fn catalan_es_unknown_filter_rejects_accented_es() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["ES_UNKNOWN"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("És preferieble que faci la revisió alguna altra persona.")
        .unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "ES_UNKNOWN"),
        "ES_UNKNOWN should be rejected: {:?}",
        result.matches
    );
}

/// Java's `XMLRuleHandler` adds no exception for an attribute-less
/// `<exception/>` (used by Catalan `ECLIPSIS_ECLIPSI`[3] and
/// `ROTAR_GIRAR`); the empty spec must not match every token. Golden:
/// `El mot eclipsis és regular.` -> `eclipsi`; `Va rotar un angle de 180
/// graus.` -> `girar`.
#[test]
fn catalan_empty_exception_does_not_block() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["ECLIPSIS_ECLIPSI", "ROTAR_GIRAR"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("El mot eclipsis és regular.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "ECLIPSIS_ECLIPSI")
        .expect("no ECLIPSIS_ECLIPSI match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["eclipsi"]);
    let result = engine.check("Va rotar un angle de 180 graus.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "ROTAR_GIRAR")
        .expect("no ROTAR_GIRAR match");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["girar"]);
}

/// Java `StringTools.isCamelCase` is the ASCII-anchored
/// `[a-z]+[A-Z][A-Za-z]+`, so `al-Àndalus` (hyphen + `À`) is not camel case
/// and the `MultiWordChunker` adds its all-uppercase variant. Golden:
/// `AL-ÀNDALUS` gets the `NPCSG00` chunk reading on `al`, which satisfies the
/// rule's `<exception postag="NPCSG00"/>`, while lowercase `al-Andalus`
/// still matches.
#[test]
fn catalan_al_andalus_all_uppercase_chunk() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["AL_ANDALUS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("AL-ÀNDALUS").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "AL_ANDALUS"),
        "AL_ANDALUS should be blocked by the chunk reading: {:?}",
        result.matches
    );
    let result = engine.check("Un estudi sobre al-Andalus.").unwrap();
    assert!(
        result.matches.iter().any(|m| m.rule_id == "AL_ANDALUS"),
        "AL_ANDALUS should match: {:?}",
        result.matches
    );
}

/// Java's `AbstractPatternRulePerformer.prevMatched` is a performer field:
/// the `min="0"` lookahead calls `testAllReadings` for the following element,
/// and a `scope="next"` exception matching the token after the candidate sets
/// the flag, which then blocks the current element's `skipMaxTokens`
/// extension. Golden: `Deixa sempre tot en mans dels altres.` matches (the
/// `RG|LOC_ADV` element must not greedily consume `tot`).
#[test]
fn catalan_ho_fa_tot_optional_chunk_gv() {
    let _guard = engine_guard();
    let Some(engine) = engine_with_rules("ca-ES", &["HO_FA_TOT"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Deixa sempre tot en mans dels altres.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "HO_FA_TOT")
        .expect("no HO_FA_TOT match");
    assert_eq!((m.range.start, m.range.end), (0, 5));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Ho deixa"]);
}
