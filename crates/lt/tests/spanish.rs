//! Spanish engine tests: Java-probed rule values (pinned LT build, Docker).
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes print UTF-16
//! code units, so non-ASCII probes are converted in the comments. Every case
//! comes from `scripts/oracle/es/probe-rule.sh` / `probe-disambig.sh`.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{Engine, EngineOptions, Lang};

/// One engine at a time: the Spanish engine is ~0.5 GB, and several tests
/// build engines with different settings.
fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn engine() -> Option<Engine> {
    let Ok(builder) = Engine::builder(Lang::Es) else {
        eprintln!("skipping: no vendored data found");
        return None;
    };
    builder.build().ok()
}

fn engine_today() -> Option<Engine> {
    let Ok(builder) = Engine::builder(Lang::Es) else {
        eprintln!("skipping: no vendored data found");
        return None;
    };
    builder.today(2026, 9, 18).build().ok()
}

#[test]
fn spanish_speller_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("El perro es un animal muy grande. La palabra 'perrro' está mal escrita.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "MORFOLOGIK_RULE_ES")
        .expect("speller match");
    assert_eq!((m.range.start, m.range.end), (46, 52));
    assert_eq!(m.message, "Se ha encontrado un posible error ortográfico.");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["perrero", "perero", "peroro", "peroró", "perro"]
    );
}

#[test]
fn spanish_question_mark_and_word_repeat_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Qué pasa? El el perro es muy muy grande.")
        .unwrap();
    let q = result
        .matches
        .iter()
        .find(|m| m.rule_id == "ES_QUESTION_MARK")
        .expect("question mark match");
    // Java: 0..3 UTF-16
    assert_eq!((q.range.start, q.range.end), (0, 4));
    assert_eq!(q.message, "Símbolo desparejado: Parece que falta un '¿'");
    assert_eq!(q.suggestions[0].value, "¿Qué");
    let r = result
        .matches
        .iter()
        .find(|m| m.rule_id == "SPANISH_WORD_REPEAT_RULE")
        .expect("word repeat match");
    // Java: 10..15 UTF-16
    assert_eq!((r.range.start, r.range.end), (11, 16));
    assert_eq!(r.message, "Posible error: repetición de una palabra");
    assert_eq!(r.suggestions[0].value, "El");
}

#[test]
fn spanish_simple_replace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("El gestionamiento de la empresa fue mejorado.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id.starts_with("ES_SIMPLE_REPLACE_SIMPLE"))
        .expect("simple replace match");
    assert_eq!(m.rule_id, "ES_SIMPLE_REPLACE_SIMPLE_GESTIONAMIENTO");
    assert_eq!((m.range.start, m.range.end), (3, 17));
    assert_eq!(m.message, "¿Quería decir «gestión»?");
    assert_eq!(m.suggestions[0].value, "gestión");
}

#[test]
fn spanish_simple_replace_verbs_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ES_SIMPLE_REPLACE_VERBS".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options("Él eruptó ayer.", &options)
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "ES_SIMPLE_REPLACE_VERBS_ERUPTÓ");
    // Java: 3..9 UTF-16
    assert_eq!((m.range.start, m.range.end), (4, 11));
    // Java emits the literal "$match" here (the legacy base does not expand
    // it in messages); Rust expands the placeholder like Java's description
    // handling does — deliberate divergence, docs/differences.md #2.
    assert_eq!(m.message, "Verbo incorrecto: eruptó");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["eructó", "hizo erupción", "entró en erupción", "erupcionó"]
    );
}

#[test]
fn spanish_compound_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ES_COMPOUNDS".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options("Guinea Conakri.", &options)
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "ES_COMPOUNDS_GUINEA_CONAKRI");
    assert_eq!((m.range.start, m.range.end), (0, 14));
    assert_eq!(m.message, "Se escribe con un guion.");
    assert_eq!(m.suggestions[0].value, "Guinea-Conakri");
}

#[test]
fn spanish_wikipedia_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ES_WIKIPEDIA_COMMON_ERRORS".to_string()],
        ..Default::default()
    };
    let result = engine
        .check_with_options("Vino a basto para el trabajo.", &options)
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "ES_WIKIPEDIA_COMMON_ERRORS");
    assert_eq!((m.range.start, m.range.end), (5, 12));
    assert_eq!(
        m.message,
        "'a basto' es una expresión errónea. Pruebe a utilizar <suggestion>abasto</suggestion>"
    );
    assert_eq!(m.suggestions[0].value, "abasto");
}

#[test]
fn spanish_wrong_word_in_context_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["SPANISH_WRONG_WORD_IN_CONTEXT".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options("El juez infringió un revés.", &options)
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(
        m.rule_id,
        "SPANISH_WRONG_WORD_IN_CONTEXT_INFRINGIÓ_INFLIGIÓ"
    );
    // Java: 8..17 UTF-16
    assert_eq!((m.range.start, m.range.end), (8, 18));
    assert!(m.message.contains("infligió"));
    assert_eq!(m.suggestions[0].value, "infligió");
}

#[test]
fn spanish_unpaired_brackets_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ES_UNPAIRED_BRACKETS".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options("Dijo: «hola amigo.", &options)
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "ES_UNPAIRED_BRACKETS");
    assert_eq!((m.range.start, m.range.end), (6, 8));
    assert_eq!(m.message, "Símbolo desparejado: Parece que falta un ‘»’.");
    assert_eq!(m.suggestions[0].value, "«»");
}

#[test]
fn spanish_repeated_words_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        picky: true,
        enabled_rules: vec!["ES_REPEATEDWORDS".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options("Te sugiero esto. Te sugiero aquello.", &options)
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "ES_REPEATEDWORDS");
    assert_eq!((m.range.start, m.range.end), (20, 27));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["propongo", "recomiendo"]);
}

#[test]
fn spanish_word_repeat_beginning_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        picky: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options(
            "Además, hace frío. Además, llueve mucho. Además, nieva.",
            &options,
        )
        .unwrap();
    let matches: Vec<&lt::Match> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "SPANISH_WORD_REPEAT_BEGINNING_RULE")
        .collect();
    assert_eq!(matches.len(), 2);
    // Java: 19..25 / 41..47 UTF-16
    assert_eq!((matches[0].range.start, matches[0].range.end), (21, 28));
    let values: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(
        values,
        vec![
            "Adicionalmente",
            "También",
            "Asimismo",
            "Igualmente",
            "Así mismo"
        ]
    );
}

#[test]
fn spanish_long_sentence_60_words_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let words: Vec<String> = (0..65).map(|i| format!("palabra{i}")).collect();
    let text = format!("El {}.", words.join(" "));
    let options = EngineOptions {
        picky: true,
        enabled_rules: vec!["TOO_LONG_SENTENCE".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine.check_with_options(&text, &options).unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "TOO_LONG_SENTENCE");
    assert_eq!(m.range.start, 0);
    assert_eq!(
        m.message,
        "Esta frase tiene más de 60 palabras. Considere la posibilidad de revisarla."
    );
}

#[test]
fn spanish_tagger_uses_frequency_stripped_dictionary() {
    // `es-ES.dict` has `fsa.dict.frequency-included=true`; the tag's last
    // byte must be stripped (D-055). Compare the probe word's readings.
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = engine.analyze_raw("zorro");
    let readings = &sentences[0].tokens[1].readings;
    let tags: Vec<&str> = readings
        .iter()
        .filter_map(|r| r.pos_tag.as_deref())
        .filter(|t| *t != "SENT_END")
        .collect();
    assert_eq!(tags, vec!["NCMS000"]);
}

#[test]
fn spanish_text_level_rules_reject_clean_text() {
    let _guard = engine_guard();
    let Some(engine) = engine_today() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("El rápido zorro marrón salta sobre el perro perezoso.")
        .unwrap();
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}

/// Disambiguation actions with a `skip` gap: Java's
/// `DisambiguationPatternRuleReplacer.executeAction` adjusts the target
/// range with `lastMarkerMatchToken`, not the last matched token, so
/// `hace <gap> años` still adds `LOC_ADV` to the marker.
#[test]
fn spanish_hace_x_tiempo_disambiguation_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    for text in [
        "Hace exactamente años que no vamos.",
        // 3 filler tokens between the marker and the time expression
        "Hace cuestión de 2 años aproximadamente.",
    ] {
        let sentences = engine.analyze(text);
        let hace = sentences[0]
            .tokens
            .iter()
            .find(|t| t.surface() == "Hace")
            .expect("hace token");
        assert!(
            hace.readings
                .iter()
                .any(|r| r.pos_tag.as_deref() == Some("LOC_ADV")),
            "LOC_ADV missing in {text}: {:?}",
            hace.readings
        );
    }

    // Java: no match — the LOC_ADV reading on `hace` hits the marker
    // exception of both AGREEMENT_PRONOUNSUBJECT_VERB rules and
    // AGREEMENT_SUBJECT_VERB_PL_SG.
    let options = EngineOptions {
        enabled_rules: vec![
            "AGREEMENT_PRONOUNSUBJECT_VERB".to_string(),
            "AGREEMENT_SUBJECT_VERB_PL_SG".to_string(),
        ],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options(
            "Nosotros hace cuestión de 2 años aproximadamente.",
            &options,
        )
        .unwrap();
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}

/// Disambiguation text hints are evaluated against the *current* sentence
/// state (`RuleSet.textHinted` + `anchorHint.getPossibleIndices(sentence)`):
/// the `buen` → `bueno` replace rule must run before `amigo_n`'s lemma
/// anchor can match, and `NC`-only filtering changes which agreement rule
/// wins.
#[test]
fn spanish_buen_amigos_disambiguation_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = engine.analyze("Buen amigos.");
    let amigos = sentences[0]
        .tokens
        .iter()
        .find(|t| t.surface() == "amigos")
        .expect("amigos token");
    assert_eq!(
        amigos
            .readings
            .iter()
            .map(|r| r.pos_tag.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["NCMP000"],
        "amigo_n must filter the AQ reading"
    );

    let options = EngineOptions {
        enabled_rules: vec![
            "CONCORDANCIA_BUEN".to_string(),
            "AGREEMENT_ADJ_NOUN".to_string(),
        ],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine.check_with_options("Buen amigos.", &options).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "AGREEMENT_ADJ_NOUN")
        .unwrap_or_else(|| panic!("no AGREEMENT_ADJ_NOUN match: {:?}", result.matches));
    assert_eq!(m.sub_id.as_deref(), Some("3"));
    assert_eq!((m.range.start, m.range.end), (0, 11));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Buen amigo", "Buenos amigos", "Buen, amigos"]);
}

/// Multiword chunker fidelity: stacked open markers resolve to the longest
/// span (later wins ties, `getMultiWordAnalyzedToken`) and Romance
/// continuation tokens get `getNextPosTag` (`NC…` → `AQ0..0`), so
/// `Echo Show` is `NPMSO00` and `res publica` is `NCFS000` + `AQ0FS0`.
#[test]
fn spanish_multiword_chunker_tags_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let tags = |text: &str, word: &str| -> Vec<(String, String)> {
        let sentences = engine.analyze(text);
        let token = sentences[0]
            .tokens
            .iter()
            .find(|t| t.surface() == word)
            .unwrap_or_else(|| panic!("no {word} in {text}"));
        token
            .readings
            .iter()
            .map(|r| {
                (
                    r.stem.clone().unwrap_or_default(),
                    r.pos_tag.clone().unwrap_or_default(),
                )
            })
            .collect()
    };
    // Java probe: [Echo Show:NPMSO00, Echo:_GN_MS, Echo:complement, Echo:_possible_NP]
    let echo = tags("Esto también pasa en el Echo Show.", "Echo");
    assert!(
        echo.contains(&("Echo Show".to_string(), "NPMSO00".to_string())),
        "{echo:?}"
    );
    // Java probe: [res publica:AQ0FS0, publica:_GN_FS, publica:complement,
    //              publica:ignore_concordance]
    let publica = tags("Se preocupa de la res publica.", "publica");
    assert!(
        publica.contains(&("res publica".to_string(), "AQ0FS0".to_string())),
        "{publica:?}"
    );
    // Java raw tagger leaves a null reading; the chunker's addReading drops
    // it, so only NPCN000 remains (this flips ES_MULTITOKEN_SPELLING_THREE
    // from sub 1 to sub 2).
    assert_eq!(
        tags("Yuval Noha Hariri.", "Yuval"),
        vec![("Yuval".to_string(), "NPCN000".to_string())]
    );

    let options = EngineOptions {
        enabled_rules: vec!["ECHO_HECHO".to_string(), "PUBLICO".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    for text in [
        "Esto también pasa en el Echo Show.",
        "Se preocupa de la res publica.",
    ] {
        let result = engine.check_with_options(text, &options).unwrap();
        assert!(result.matches.is_empty(), "{text}: {:?}", result.matches);
    }
}

/// `GenericUnpairedBracketsRule.getPrecededByWhitespace`/`getSpecialCase`:
/// a quote-like symbol attached to the previous token only opens inside a
/// sentence, so a trailing `22º23'` is reported as a closing symbol.
#[test]
fn spanish_unpaired_brackets_quote_guards_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ES_UNPAIRED_BRACKETS".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    // Java (UTF-16 probe): 5..6 with suggestion `''`
    let result = engine.check_with_options("22º23'", &options).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "ES_UNPAIRED_BRACKETS")
        .unwrap_or_else(|| panic!("no match: {:?}", result.matches));
    assert_eq!((m.range.start, m.range.end), (6, 7));
    assert_eq!(m.suggestions[0].value, "''");
    assert_eq!(m.message, "Símbolo desparejado: Parece que falta un ‘'’.");

    // Java: no match (the apostrophe is an allowed apostrophe)
    let result = engine
        .check_with_options("Se volvió hacia O'Neill.", &options)
        .unwrap();
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}

/// Multiword casing variants: Java's `getTokenLettercaseVariants` only adds
/// a derived variant when the map does not already contain it, so the first
/// `;RN` acronym-ish entry wins over a later lowercase `LOC_ADV` entry for
/// `Bajo ningún concepto`.
#[test]
fn spanish_multiword_variant_order_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = engine.analyze("Bajo ningún concepto guardes dinero.");
    let bajo = &sentences[0].tokens[1];
    assert_eq!(
        bajo.readings
            .iter()
            .map(|r| (
                r.stem.clone().unwrap_or_default(),
                r.pos_tag.clone().unwrap_or_default()
            ))
            .collect::<Vec<_>>(),
        vec![("bajo ningún concepto".to_string(), "RN".to_string())]
    );
    let options = EngineOptions {
        enabled_rules: vec!["SUBJUNTIVO_INCORRECTO".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options("Bajo ningún concepto guardes dinero, pasaportes.", &options)
        .unwrap();
    assert!(result.matches.is_empty(), "{:?}", result.matches);
}

/// `FindSuggestionsFilter` removes `match.getOriginalErrorStr()` (only set
/// for `wordFrom:inmarker`, and taken from the token-concatenated sentence
/// text), not the word-from token; that keeps `\2` in PREP_VERB and the
/// spelling candidates for `L’ALCOIA`.
#[test]
fn spanish_find_suggestions_filter_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let values = |rule: &str, text: &str| -> Vec<String> {
        let options = EngineOptions {
            enabled_rules: vec![rule.to_string()],
            enabled_only: true,
            ..Default::default()
        };
        let result = engine.check_with_options(text, &options).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == rule)
            .unwrap_or_else(|| panic!("no {rule} match for {text}"));
        m.suggestions.iter().map(|s| s.value.clone()).collect()
    };
    // Java probe: podemos|la podemos|a poder|a poderos
    assert_eq!(
        values("PREP_VERB", "Y a podemos hacer."),
        vec!["podemos", "la podemos", "a poder", "a poderos"]
    );
    // Java probe (UTF-16 0-8): L'ALCOIÀ|L'ALCORA
    assert_eq!(
        values("APOSTROFO_ACENTO", "L’ALCOIA"),
        vec!["L'ALCOIÀ", "L'ALCORA"]
    );
    assert_eq!(
        values("APOSTROFO_ACENTO", "L’Alcoia"),
        vec!["L'Alcoià", "l'Alcoià", "L'Alcora", "l'Alcora"]
    );
}

/// ESTA_TILDE antipattern immunization only covers the `<marker>` span
/// (Java `IMMUNIZE` position corrections), not the whole antipattern match.
#[test]
fn spanish_esta_tilde_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ESTA_TILDE".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    // Java (UTF-16 probe): sub 2 @5-9, sub 3 @7-11, sub 7 @14-18,
    // sub 8 @21-25, sub 9 @13-17 (UTF-8 offsets below because of `¿`/`í`).
    for (text, sub, range) in [
        ("¿Qué esta haciendo esta silla aquí?", "2", (7, 11)),
        ("¿Quién esta en esta habitación?", "3", (9, 13)),
        ("Esta película esta basada en una novela.", "7", (15, 19)),
        ("A las 9, esta tienda esta cerrada.", "8", (21, 25)),
        ("Esta manzana esta mala.", "9", (13, 17)),
    ] {
        let result = engine.check_with_options(text, &options).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "ESTA_TILDE")
            .unwrap_or_else(|| panic!("no match for {text}"));
        assert_eq!(m.sub_id.as_deref(), Some(sub), "{text}");
        assert_eq!((m.range.start, m.range.end), range, "{text}");
    }
}

/// `max` repetition runs report the run's *first* token (Java
/// `firstMatchToken`), not the last one consumed by the greedy run.
#[test]
fn spanish_repeated_onomatopeyas_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ONOMATOPEYAS".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    for (text, sub, range) in [
        ("ja ja ja ja", "4", (0, 11)),
        ("je je je je", "5", (0, 11)),
        ("ji ji ji ji", "6", (0, 11)),
    ] {
        let result = engine.check_with_options(text, &options).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "ONOMATOPEYAS")
            .unwrap_or_else(|| panic!("no match for {text}"));
        assert_eq!(m.sub_id.as_deref(), Some(sub), "{text}");
        assert_eq!((m.range.start, m.range.end), range, "{text}");
    }
}

/// Self-closing `<token min="2" .../>` duplicates the element like Java
/// (`XMLRuleHandler.setToken`), so the rulegroup antipatterns that need two
/// consecutive `_GN_` tokens do not fire on a single noun.
#[test]
fn spanish_self_closing_min_tokens_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["AGREEMENT_SUBJECT_VERB_PL_SG".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    for (text, sub, range, suggestion) in [
        ("Los amigos tiene sed.", "1", (11, 16), "tienen"),
        ("Aquellas personas sigue igual.", "1", (18, 23), "siguen"),
        ("Los amigos ha tenido sed.", "2", (11, 13), "han"),
    ] {
        let result = engine.check_with_options(text, &options).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == "AGREEMENT_SUBJECT_VERB_PL_SG")
            .unwrap_or_else(|| panic!("no match for {text}: {:?}", result.matches));
        assert_eq!(m.sub_id.as_deref(), Some(sub), "{text}");
        assert_eq!((m.range.start, m.range.end), range, "{text}");
        assert_eq!(m.suggestions[0].value, suggestion, "{text}");
    }
}

/// `<phraseref>` expansion (Java `PatternRuleHandler.preparePhrase`): rules
/// using `_GN_SINGULAR`/`_GN_PLURAL` match only with the phrase tokens
/// inline, and the expanded alternatives keep the Java rule/sub id.
#[test]
fn spanish_phraseref_rules_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec![
            "SE_CREO2".to_string(),
            "SE_FIE".to_string(),
            "AGREEMENT_SUBJECT_VERB_SG_PL".to_string(),
        ],
        enabled_only: true,
        ..Default::default()
    };

    // Java (UTF-16): SE_CREO2[1] 5..10 `habló`
    let result = engine
        .check_with_options("Juan hablo con ella.", &options)
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "SE_CREO2")
        .expect("SE_CREO2 match");
    assert_eq!(m.sub_id.as_deref(), Some("1"));
    assert_eq!((m.range.start, m.range.end), (5, 10));
    assert_eq!(m.suggestions[0].value, "habló");

    // Java: no match — the nagging `_GN_SINGULAR` subject is absent
    let result = engine
        .check_with_options("Siempre colaboro con la policía.", &options)
        .unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "SE_CREO2"),
        "{:?}",
        result.matches
    );

    // Java (UTF-16): SE_CREO2[1] 11..19 (no sentence-final period)
    let result = engine
        .check_with_options("La policía colaboro con el juez", &options)
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "SE_CREO2")
        .expect("SE_CREO2 match");
    assert_eq!(m.sub_id.as_deref(), Some("1"));
    assert_eq!((m.range.start, m.range.end), (12, 20));

    // Java (UTF-16): AGREEMENT_SUBJECT_VERB_SG_PL[1] 10..13 `es`
    let result = engine
        .check_with_options("Esta sopa son muy buena.", &options)
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "AGREEMENT_SUBJECT_VERB_SG_PL")
        .expect("agreement match");
    assert_eq!(m.sub_id.as_deref(), Some("1"));
    assert_eq!((m.range.start, m.range.end), (10, 13));
    assert_eq!(m.suggestions[0].value, "es");

    // Java (UTF-16): AGREEMENT_SUBJECT_VERB_SG_PL[2] 10..13 `ha`
    let result = engine
        .check_with_options("Esta sopa han sido muy buena.", &options)
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "AGREEMENT_SUBJECT_VERB_SG_PL")
        .expect("agreement match");
    assert_eq!(m.sub_id.as_deref(), Some("2"));
    assert_eq!((m.range.start, m.range.end), (10, 13));
    assert_eq!(m.suggestions[0].value, "ha");

    // Java: no match (the pre-phraseref port matched the verb here)
    let result = engine.check_with_options("Son validos.", &options).unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "AGREEMENT_SUBJECT_VERB_SG_PL"),
        "{:?}",
        result.matches
    );

    // Java (UTF-16): SE_FIE[4] 24..28 `guíe`
    let result = engine
        .check_with_options("Necesitamos que el taxi guie.", &options)
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "SE_FIE")
        .expect("SE_FIE match");
    assert_eq!(m.sub_id.as_deref(), Some("4"));
    assert_eq!((m.range.start, m.range.end), (24, 28));
    assert_eq!(m.suggestions[0].value, "guíe");
}

/// `suppress_misspelled` on the rule/rulegroup: Java's
/// `MatchState.toFinalString` marks a rendered suggestion unknown to the
/// language tagger as `<mistake/>`; `removeSuppressMisspelled` then drops the
/// whole suggestion and a match left without suggestions disappears.
#[test]
fn suppress_misspelled_drops_unknown_joins() {
    let _guard = engine_guard();
    let Some(engine) = engine_today() else {
        eprintln!("skipping: no vendored data");
        return;
    };

    // Java golden: no match at all ("remayor" is not a word)
    let result = engine.check("Concierto en re mayor.").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "NO_SEPARADO"),
        "{:?}",
        result.matches
    );

    // Java golden: no match ("pronobis"/"prornobis" are not words)
    let result = engine.check("Recitan el ora pro nobis.").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "NO_SEPARADO"),
        "{:?}",
        result.matches
    );

    // Java golden [line 2729]: NO_SEPARADO 8..22 (UTF-16; byte end 23 for
    // the two-byte "á"), only the known join
    let result = engine.check("Hizo un macro análisis.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "NO_SEPARADO")
        .expect("NO_SEPARADO match");
    assert_eq!((m.range.start, m.range.end), (8, 23));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["macroanálisis"]);

    // Java golden [line 2679]: the specific ultra+violetas subrule wins
    // because the general ultra rule's "ultravioletas" is unknown
    let result = engine.check("Son rayos ultra violetas.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "NO_SEPARADO")
        .expect("NO_SEPARADO match");
    assert_eq!((m.range.start, m.range.end), (10, 24));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["ultravioleta"]);
}

#[test]
fn suppress_misspelled_r_rr_yields_to_speller() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java golden [line 2582]: R_RR ("Autorrízase" unknown) is dropped and
    // the overlapping speller error surfaces.
    let result = engine.check("Autorízase el ingreso de agente.").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "R_RR"),
        "{:?}",
        result.matches
    );
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "MORFOLOGIK_RULE_ES")
        .expect("speller match");
    assert_eq!((m.range.start, m.range.end), (0, 11));
}

/// LingoTweaker hand-authored rule (`es/rules/local.xml`, not upstream):
/// sentence-initial demonstrative pronoun subjects whose verb disagrees in
/// number. The pinned Java engine reports none of these
/// (`docs/differences.md` #8); personal pronouns keep using the upstream
/// `AGREEMENT_PRONOUNSUBJECT_VERB`.
#[test]
fn spanish_demonstrative_verb_agreement() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };

    // (text, rule id, sub id, from, to, expected suggestion)
    for (text, rule, sub, from, to, suggestion) in [
        (
            "Estos es un problema.",
            "AGREEMENT_DEMONSTRATIVE_VERB",
            "1",
            6,
            8,
            "son",
        ),
        (
            "Esta son muy buena.",
            "AGREEMENT_DEMONSTRATIVE_VERB",
            "2",
            5,
            8,
            "es",
        ),
        (
            "Ese son un problema.",
            "AGREEMENT_DEMONSTRATIVE_VERB",
            "2",
            4,
            7,
            "es",
        ),
        (
            "Este están muy bueno.",
            "AGREEMENT_DEMONSTRATIVE_VERB",
            "2",
            5,
            11,
            "está",
        ),
        (
            "Este son un problema.",
            "AGREEMENT_DEMONSTRATIVE_VERB",
            "3",
            5,
            8,
            "es",
        ),
        (
            "Ella son profesora.",
            "AGREEMENT_PRONOUNSUBJECT_VERB",
            "5",
            5,
            8,
            "es",
        ),
    ] {
        let result = engine.check(text).unwrap();
        let m = result
            .matches
            .iter()
            .find(|m| m.rule_id == rule && m.sub_id.as_deref() == Some(sub))
            .unwrap_or_else(|| panic!("{rule}[{sub}] missing for {text:?}: {:?}", result.matches));
        assert_eq!((m.range.start, m.range.end), (from, to), "{text:?}");
        let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
        assert!(values.contains(&suggestion), "{text:?}: {values:?}");
    }

    // Valid sentences (including the RAE-accepted neuter `Esto son`, and the
    // noun-phrase reading of `Este son`) must stay untouched.
    for text in [
        "Estos son un problema.",
        "Esta es muy buena.",
        "Este son es bonito.",
        "Esto son los motivos.",
        "Este son, un clásico, es bonito.",
    ] {
        let result = engine.check(text).unwrap();
        assert!(
            !result
                .matches
                .iter()
                .any(|m| m.rule_id == "AGREEMENT_DEMONSTRATIVE_VERB"),
            "{text:?} must not trigger the local rule: {:?}",
            result.matches
        );
    }
}

/// `enabledOnly` with both `enabledRules` and `enabledCategories` keeps the
/// union (Java `Tools.selectRules`, #12194/#aece4da): the explicitly enabled
/// rule and the rules of the enabled category both run, and nothing else.
#[test]
fn spanish_enabled_only_union_of_rules_and_categories() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["ID_HUBO_HUBIERON".to_string()],
        enabled_categories: vec!["AGREEMENT_DEMONSTRATIVE".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let result = engine
        .check_with_options(
            "Este son un prueba. Habían muchas personas en la calle.",
            &options,
        )
        .unwrap();
    let ids: Vec<&str> = result.matches.iter().map(|m| m.rule_id.as_str()).collect();
    assert!(ids.contains(&"AGREEMENT_DEMONSTRATIVE_VERB"), "{ids:?}");
    assert!(ids.contains(&"ID_HUBO_HUBIERON"), "{ids:?}");
    assert!(!ids.contains(&"AGREEMENT_DET_NOUN"), "{ids:?}");
}
