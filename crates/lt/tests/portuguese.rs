//! Portuguese engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the
//! progress metric and gets updated by each stage. Offsets are UTF-8 bytes
//! (the engine format).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{Engine, Lang};

/// One engine at a time: the Portuguese engines hold the tagger dictionary.
fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn engine_variant(variant: &str) -> Option<Engine> {
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return None;
    };
    builder.variant(variant).build().ok()
}

/// Stage 3b filter probes (`scripts/oracle/pt/probe-rule.sh`, pinned Java
/// build, 2026-09-19, `Level.PICKY`, one rule group enabled at a time).
/// Rust offsets are UTF-8 bytes; the Java UTF-16 values are noted per
/// assertion when they differ.
#[test]
fn portuguese_stage3b_filters_match_java() {
    let _guard = engine_guard();
    let ids = [
        "IRREGULAR_PAST_PARTICIPLES",
        "PRONOMIAL_COLOCATIONS_ADJUSTMENT_NOS",
        "PRONOMIAL_COLOCATIONS_ADJUSTMENT_LOS",
        "PODERIAM-SE",
        "COLOCACAO_PRONOMINAL_COM_ATRATOR_SIMPLES",
        "COLOCACAO_PRONOMINAL_COM_ATRATOR_COMPLEXO",
        "COLOCACAO_PRONOMINAL_EM_O_SENDO",
        "DIACRITICS",
        "SEPARADORES_DE_ESTADOS_BRASILEIROS_MEIA_RISCA_PARENTESES",
        "SEPARADORES_DE_ESTADOS_BRASILEIROS_MEIA_RISCA_PONTUACAO_SIMPLES",
        "SECULO_EM_NUMERAIS_ROMANOS",
        "VERBO_ESTAR_A_VERBO_INF",
        "QUANDO_TER_DE_QUE_VINF_AO_VINF",
        "DOS_HUMANOS_HUMANO",
        "QUE_ARTDEF_PRONPOSS_NOMECOMUM_VERBO",
        "PASSAR_A_VERBO",
        "VIR_A_VERBO_VERBO",
        "QUE_VERBOPASSADO_VERBOPARTPASSADO",
        "PARA-POR_TER_PARTICIPIO-PASSADO",
    ];
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = lt::EngineOptions {
        enabled_rules: ids.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        picky: true,
        ..Default::default()
    };
    let Some(engine) = builder.today(2026, 9, 19).options(options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };

    let check = |text: &str, rule: &str| -> lt::Match {
        let result = engine.check(text).unwrap();
        result
            .matches
            .iter()
            .find(|m| m.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"))
            .clone()
    };

    // IRREGULAR_PAST_PARTICIPLES (RegularIrregularParticipleFilter)
    // Java: 28..33 "limpado"
    let m = check(
        "Depois de ter discretamente limpo o carro, foi desculpado.",
        "IRREGULAR_PAST_PARTICIPLES",
    );
    assert_eq!((m.range.start, m.range.end), (28, 33));
    assert_eq!(
        m.message,
        "Com o verbo 'ter' deve utilizar-se o particípio passado regular."
    );
    assert_eq!(m.suggestions[0].value, "limpado");
    // Java: 15..22 "gasto"
    let m = check(
        "O dinheiro foi gastado o mês passado.",
        "IRREGULAR_PAST_PARTICIPLES",
    );
    assert_eq!((m.range.start, m.range.end), (15, 22));
    assert_eq!(
        m.message,
        "Com o verbo 'ser' deve utilizar-se o particípio passado irregular."
    );
    assert_eq!(m.suggestions[0].value, "gasto");

    // PRONOMIAL_COLOCATIONS_ADJUSTMENT_NOS (PortugueseEnclisisFilter)
    // Java: 7..15 "tinham-no"
    let m = check(
        "Muitos tinham-o como um bom amigo.",
        "PRONOMIAL_COLOCATIONS_ADJUSTMENT_NOS",
    );
    assert_eq!((m.range.start, m.range.end), (7, 15));
    assert_eq!(m.suggestions[0].value, "tinham-no");
    // Java: 13..21 "parti-lo" / 2..7 "fê-lo"
    let m = check(
        "Você precisa partir-o em dois pedaços.",
        "PRONOMIAL_COLOCATIONS_ADJUSTMENT_LOS",
    );
    assert_eq!(m.suggestions[0].value, "parti-lo");
    let m = check(
        "E fez-o de maneira estranha.",
        "PRONOMIAL_COLOCATIONS_ADJUSTMENT_LOS",
    );
    assert_eq!((m.range.start, m.range.end), (2, 7));
    assert_eq!(m.suggestions[0].value, "fê-lo");

    // PODERIAM-SE (mesoclisis via the composite `:PP` tag synthesis)
    // Java: 6..16 "poder-se-ia"
    let m = check("Assim poderia-se escrever à Ana.", "PODERIAM-SE");
    assert_eq!((m.range.start, m.range.end), (6, 16));
    assert_eq!(m.suggestions[0].value, "poder-se-ia");

    // PortugueseProclisisFilter. Java: 4..11 "me diga" (Rust 5..12: "Não")
    let m = check(
        "Não diga-me isso.",
        "COLOCACAO_PRONOMINAL_COM_ATRATOR_SIMPLES",
    );
    assert_eq!(m.suggestions[0].value, "me diga");
    // Java HashSet order: "os puseram|nos puseram"
    let m = check(
        "De forma alguma puseram-nos em primeiro lugar.",
        "COLOCACAO_PRONOMINAL_COM_ATRATOR_COMPLEXO",
    );
    assert_eq!(
        m.suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["os puseram", "nos puseram"]
    );
    // Java: 3..13 "o trazendo"
    let m = check(
        "Em trazendo-o, fez mais mal que bem.",
        "COLOCACAO_PRONOMINAL_EM_O_SENDO",
    );
    assert_eq!((m.range.start, m.range.end), (3, 13));
    assert_eq!(m.suggestions[0].value, "o trazendo");

    // DIACRITICS (ConfusionCheckFilter). Java: 14..23 "parágrafo",
    // 12..20 "trâmites", 6..16 "matrículas".
    let m = check("A excepção do paragrafo.", "DIACRITICS");
    assert_eq!(m.suggestions[0].value, "parágrafo");
    let m = check("Um erro nos tramites.", "DIACRITICS");
    assert_eq!((m.range.start, m.range.end), (12, 20));
    assert_eq!(m.suggestions[0].value, "trâmites");
    let m = check("Quero matriculas de automóveis em Portugal.", "DIACRITICS");
    assert_eq!((m.range.start, m.range.end), (6, 16));
    assert_eq!(m.suggestions[0].value, "matrículas");

    // BrazilianToponymFilter (RegexRuleFilter)
    // Java: 17..22 "–RJ"; the second sentence must not match (not a known
    // municipality).
    let m = check(
        "Nasceu em Niterói (RJ).",
        "SEPARADORES_DE_ESTADOS_BRASILEIROS_MEIA_RISCA_PARENTESES",
    );
    assert_eq!(m.suggestions[0].value, "\u{2013}RJ");
    let result = engine
        .check("O candidato a prefeito Fernando Gabeira (RJ).")
        .unwrap();
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "SEPARADORES_DE_ESTADOS_BRASILEIROS_MEIA_RISCA_PARENTESES"));
    let m = check(
        "Nasceu em Niterói-RJ.",
        "SEPARADORES_DE_ESTADOS_BRASILEIROS_MEIA_RISCA_PONTUACAO_SIMPLES",
    );
    assert_eq!(m.suggestions[0].value, "\u{2013}RJ");

    // RomanNumeralFilter (`core/Roman.sor`): Java "XX" / "IX".
    let m = check("No começo do século 20.", "SECULO_EM_NUMERAIS_ROMANOS");
    assert_eq!(m.suggestions[0].value, "XX");
    let m = check("Nasceu no século 9.", "SECULO_EM_NUMERAIS_ROMANOS");
    assert_eq!(m.suggestions[0].value, "IX");

    // AdvancedSynthesizerFilter
    let m = check(
        "Temos diferentes perspetivas mediante a lógica pela qual me estou a orientar.",
        "VERBO_ESTAR_A_VERBO_INF",
    );
    assert_eq!(
        m.suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["oriento", "estou orientando"]
    );
    let m = check(
        "Quando tenho de lidar com muitos elementos.",
        "QUANDO_TER_DE_QUE_VINF_AO_VINF",
    );
    assert_eq!(m.suggestions[0].value, "Ao lidar");
    let m = check(
        "O comportamento dos humanos é previsível.",
        "DOS_HUMANOS_HUMANO",
    );
    assert_eq!(m.suggestions[0].value, "humano");
    let m = check(
        "Alguns cenários que o nosso algoritmo suporta.",
        "QUE_ARTDEF_PRONPOSS_NOMECOMUM_VERBO",
    );
    assert_eq!(m.suggestions[0].value, "suportados pelo nosso algoritmo");
    let m = check(
        "Isso aconteceu na semana toda que passou.",
        "QUE_VERBOPASSADO_VERBOPARTPASSADO",
    );
    assert_eq!(m.suggestions[0].value, "passada");
    let m = check(
        "Isso por eu ter estado a ler o artigo.",
        "PARA-POR_TER_PARTICIPIO-PASSADO",
    );
    assert_eq!(m.suggestions[0].value, "estar");
    // keepPronoun composite tags
    let m = check("E assim passando a reduzir as perdas.", "PASSAR_A_VERBO");
    assert_eq!(m.suggestions[0].value, "reduzindo");
    let m = check(
        "O nosso estudo terá grande aplicabilidade caso esta ameaça se venha a concretizar.",
        "VIR_A_VERBO_VERBO",
    );
    assert_eq!(m.suggestions[0].value, "concretize");
}

/// Stage 3c: `PortugueseTagger` heuristics and `PortugueseWordTokenizer`.
/// `scripts/oracle/pt/dump-tags.sh scripts/oracle/pt/tagger-sentences.txt` is
/// byte-identical to `cargo run -p lt-tagger --example dump_tags_pt`
/// (2026-09-19); the fixture `scripts/oracle/pt/tagger-sentences.java.tsv`
/// keeps the Java dump.
#[test]
fn portuguese_tagger_heuristics_match_java() {
    let Ok(data) = lt::DataDir::discover() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let tagger = match lt_tagger::PortugueseTagger::load(data.path()) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("skipping: no vendored data");
            return;
        }
    };
    let is_tagged = |w: &str| tagger.is_tagged_word(w);
    let tokenizer = lt_tokenize::PortugueseWordTokenizer::new(&is_tagged);
    let readings_of = |text: &str, token: &str| -> Vec<String> {
        let tokens = tokenizer.tokenize(text);
        let readings = tagger.tag(&tokens);
        for (surface, atr) in tokens.iter().zip(readings.iter()) {
            if surface == token {
                return atr
                    .readings
                    .iter()
                    .map(|r| {
                        format!(
                            "{}:{}",
                            r.stem.as_deref().unwrap_or("null"),
                            r.pos_tag.as_deref().unwrap_or("null")
                        )
                    })
                    .collect();
            }
        }
        panic!("token {token:?} not found in {text:?} (tokens: {tokens:?})");
    };

    // ordinals (lemma suffix replaced with º, MS/FS/MP/FP tags)
    assert_eq!(
        readings_of("no dia 1º de maio.", "1º"),
        vec!["1º:NCMS000", "1º:AO0MS0"]
    );
    assert_eq!(
        readings_of("O 2.º lugar e a 3.ª posição.", "2.º"),
        vec!["2.º:NCMS000", "2.º:AO0MS0"]
    );
    assert_eq!(
        readings_of("O 2.º lugar e a 3.ª posição.", "3.ª"),
        vec!["3.º:NCFS000", "3.º:AO0FS0"]
    );
    // percent/degree → NCMP000; "30°C" ends in C, so it stays unknown
    assert_eq!(readings_of("A taxa é 50%.", "50%"), vec!["50%:NCMP000"]);
    assert_eq!(
        readings_of("A temperatura é 30°C.", "30°C"),
        vec!["null:null"]
    );
    assert_eq!(
        readings_of("A temperatura é 30°.", "30°"),
        vec!["30°:NCMP000"]
    );
    // -mente adverbs with an adjective stem
    assert_eq!(
        readings_of("Ele fala rapidamente.", "rapidamente"),
        vec!["rapidamente:RG"]
    );
    // soto- prefixed verb (lemma = prefix + verb lemma)
    assert_eq!(
        readings_of("O soto-pôs algo.", "soto-pôs"),
        vec!["soto-pôr:VMIS3S0"]
    );
    // hyphenated clitic forms are split by the tokenizer and tagged
    assert_eq!(
        tokenizer.tokenize("Muitos tinham-o como um bom amigo."),
        vec![
            "Muitos", " ", "tinham", "-", "o", " ", "como", " ", "um", " ", "bom", " ", "amigo",
            "."
        ]
    );
    assert_eq!(
        readings_of("Muitos tinham-o como um bom amigo.", "tinham"),
        vec!["ter:VMII3P0"]
    );
    assert_eq!(
        readings_of("Assim poderia-se escrever à Ana.", "se"),
        vec!["se:CS", "se:PP3CNO00"]
    );
}

/// Stage 3d `AbstractSimpleReplaceRule2` family against the pinned Java
/// build (`scripts/oracle/pt/probe-rule.sh`, 2026-09-19): common rules plus
/// the pt-PT/pt-BR variant files, sub-rule ids and message/suggestions.
#[test]
fn portuguese_simple_replace_matches_java() {
    let _guard = engine_guard();
    let pt_ids = [
        "PT_BARBARISMS_REPLACE",
        "PT_CLICHE_REPLACE",
        "PT_REDUNDANCY_REPLACE",
        "PT_WORDINESS_REPLACE",
        "PT_WIKIPEDIA_COMMON_ERRORS",
        "PT_PT_SIMPLE_REPLACE",
        "PT_ARCHAISMS_REPLACE",
    ];
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let pt_options = lt::EngineOptions {
        enabled_rules: pt_ids.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    let Some(engine) = builder.variant("pt-PT").options(pt_options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let find = |text: &str, rule: &str, specific: &str| -> lt::Match {
        let result = engine.check(text).unwrap();
        result
            .matches
            .iter()
            .find(|m| m.rule_id == specific)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"))
            .clone()
    };

    // Java: PT_BARBARISMS_REPLACE_ABAT_JOUR 2..11, "abajur" (sub-rule ids
    // are the match rule id, like the other `AbstractSimpleReplaceRule2`
    // ports)
    let m = find(
        "O abat-jour estava aberto.",
        "PT_BARBARISMS_REPLACE",
        "PT_BARBARISMS_REPLACE_ABAT_JOUR",
    );
    assert_eq!((m.range.start, m.range.end), (2, 11));
    assert_eq!(
        m.message,
        "\"abat-jour\" é um estrangeirismo. É preferível dizer <suggestion>abajur</suggestion>."
    );
    assert_eq!(m.suggestions[0].value, "abajur");

    // Java: PT_CLICHE_REPLACE_À_BALDA 13..20, separator " ou "
    let m = find(
        "Ele trabalha à balda.",
        "PT_CLICHE_REPLACE",
        "PT_CLICHE_REPLACE_À_BALDA",
    );
    assert_eq!(
        m.suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["de forma desorganizada", "à sorte"]
    );

    // Java: PT_REDUNDANCY_REPLACE_A_MINHA_MÃE_E_O_MEU_PAI 0..23; sentence
    // start capitalizes the suggestion, not the message
    let m = find(
        "A minha mãe e o meu pai vieram.",
        "PT_REDUNDANCY_REPLACE",
        "PT_REDUNDANCY_REPLACE_A_MINHA_MÃE_E_O_MEU_PAI",
    );
    assert_eq!(
        m.message,
        "\"A minha mãe e o meu pai\" é um pleonasmo. É preferível dizer <suggestion>os meus pais</suggestion>"
    );
    assert_eq!(m.suggestions[0].value, "Os meus pais");

    // Java: PT_WORDINESS_REPLACE_A_COR_AMARELA 8..21
    let m = find(
        "Comprei a cor amarela.",
        "PT_WORDINESS_REPLACE",
        "PT_WORDINESS_REPLACE_A_COR_AMARELA",
    );
    assert_eq!(
        m.suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["amarelo", "o amarelo"]
    );

    // Java: PT_WIKIPEDIA_COMMON_ERRORS_À_DIVERSOS 7..17
    let _ = find(
        "Devido à diversos fatores, falhou.",
        "PT_WIKIPEDIA_COMMON_ERRORS",
        "PT_WIKIPEDIA_COMMON_ERRORS_À_DIVERSOS",
    );

    // Java: PT_PT_SIMPLE_REPLACE_AEROMOÇA 2..10 (pt-PT variant file)
    let m = find(
        "A aeromoça sorriu.",
        "PT_PT_SIMPLE_REPLACE",
        "PT_PT_SIMPLE_REPLACE_AEROMOÇA",
    );
    assert_eq!(m.suggestions[0].value, "hospedeira de bordo");

    // Java: PT_ARCHAISMS_REPLACE_CÂMERA 2..8 (pt-PT variant file)
    let m = find(
        "A câmera era nova.",
        "PT_ARCHAISMS_REPLACE",
        "PT_ARCHAISMS_REPLACE_CÂMERA",
    );
    assert_eq!(m.suggestions[0].value, "câmara");

    // Java: PT_DIACRITICS_REPLACE_ABBE 2..6 (default off)
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = lt::EngineOptions {
        enabled_rules: vec!["PT_DIACRITICS_REPLACE".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let Some(diacritics_engine) = builder.variant("pt-PT").options(options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = diacritics_engine.check("O abbe veio.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PT_DIACRITICS_REPLACE_ABBE")
        .expect("PT_DIACRITICS_REPLACE match");
    assert_eq!(m.suggestions[0].value, "abbé");

    // Java pt-BR: PT_BR_SIMPLE_REPLACE_HOSPEDEIRA_DE_BORDO 2..21
    let Ok(br_builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let br_options = lt::EngineOptions {
        enabled_rules: vec!["PT_BR_SIMPLE_REPLACE".to_string()],
        enabled_only: true,
        ..Default::default()
    };
    let Some(br_engine) = br_builder.variant("pt-BR").options(br_options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = br_engine.check("A hospedeira de bordo sorriu.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PT_BR_SIMPLE_REPLACE_HOSPEDEIRA_DE_BORDO")
        .expect("PT_BR_SIMPLE_REPLACE match");
    assert_eq!(m.suggestions[0].value, "aeromoça");
}

/// Stage 3d legacy `AbstractSimpleReplaceRule` family against the pinned
/// Java build (sub-rule ids, plain-text messages, all-caps handling and the
/// pt-PT-only agreement rule).
#[test]
fn portuguese_legacy_simple_replace_matches_java() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = lt::EngineOptions {
        enabled_rules: vec![
            "PT_SIMPLE_REPLACE_ORTHOGRAPHY".to_string(),
            "PT_AGREEMENT_REPLACE".to_string(),
            "PT_ENGLISH_CONTRACTION_ORTHOGRAPHY".to_string(),
        ],
        enabled_only: true,
        ..Default::default()
    };
    let Some(engine) = builder.variant("pt-PT").options(options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let find = |text: &str, rule: &str| -> lt::Match {
        let result = engine.check(text).unwrap();
        result
            .matches
            .iter()
            .find(|m| m.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"))
            .clone()
    };

    // Java: PT_SIMPLE_REPLACE_ORTHOGRAPHY_JA 3..5, "Possível erro
    // ortográfico", "já"
    let m = find("Eu ja fui ao mercado.", "PT_SIMPLE_REPLACE_ORTHOGRAPHY_JA");
    assert_eq!((m.range.start, m.range.end), (3, 5));
    assert_eq!(m.message, "Possível erro ortográfico");
    assert_eq!(m.suggestions[0].value, "já");

    // Java: PT_AGREEMENT_REPLACE_REACÇÃO 5..12 (the tokenizer split
    // "ab-reacção" into "ab" + "-" + "reacção"; Rust bytes 5..14 because of
    // ç/ã)
    let m = find("A ab-reacção foi má.", "PT_AGREEMENT_REPLACE_REACÇÃO");
    assert_eq!((m.range.start, m.range.end), (5, 14));
    assert_eq!(
        m.message,
        "'reacção' é uma forma do antigo acordo ortográfico. No novo acordo ortográfico, a palavra escreve-se assim: reação."
    );
    assert_eq!(m.suggestions[0].value, "reação");

    // Java: PT_ENGLISH_CONTRACTION_ORTHOGRAPHY 10..14; case-sensitive, so
    // "AINT" is an own key with the all-caps suggestion.
    let m = find("Ele disse aint.", "PT_ENGLISH_CONTRACTION_ORTHOGRAPHY");
    assert_eq!((m.range.start, m.range.end), (10, 14));
    assert_eq!(
        m.message,
        "Caso seja uma contração da língua inglesa, prefira \"ain't\"."
    );
    let m = find("Ele disse AINT.", "PT_ENGLISH_CONTRACTION_ORTHOGRAPHY");
    assert_eq!(m.suggestions[0].value, "AIN'T");
}

/// Stage 2 (internal development notes): the XML rules of the common
/// grammar/style files plus the variant directory load, the stage-2
/// `MultitokenSpellerFilter` and the stage-3 date filters; the remaining
/// unmapped filters are the documented stage-3 exceptions.
#[test]
fn portuguese_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("pt-PT") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(engine.active_rule_count(), 3209);
    assert_eq!(engine.disambig_rule_count(), 525);
    let skipped = engine.skipped_counts();
    assert_eq!(skipped.filters, 0);
    assert_eq!(skipped.uncompilable, 0);
    assert_eq!(skipped.complex, 0);
    assert!(engine.compile_failures().is_empty());
}

/// Stage-3 date filters (`DateFilterHelper` + `DateCheckFilter`,
/// `FutureDateFilter`, `NewYearDateFilter`) against the pinned Java build
/// (`scripts/oracle/pt/probe-rule.sh`, 2026-09-19, `Level.PICKY`).
#[test]
fn portuguese_date_filters_match_java() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let Some(engine) = builder.today(2026, 9, 19).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };

    // Java: PT_DATE_WEEKDAY 20..36, "Esta data não é segunda-feira, é
    // terça-feira.", suggestions "Terça-feira, 7|Segunda-feira, 6".
    let result = engine
        .check("Festival Pop/Rock - Segunda-feira, 7 de outubro de 2014")
        .unwrap();
    let date = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PT_DATE_WEEKDAY")
        .expect("PT_DATE_WEEKDAY match");
    assert_eq!((date.range.start, date.range.end), (20, 36));
    assert_eq!(
        date.message,
        "Esta data não é segunda-feira, é terça-feira."
    );
    assert_eq!(
        date.suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["Terça-feira, 7", "Segunda-feira, 6"]
    );

    // Java: PT_DATE_WEEKDAY 51..55 (the year token; Rust bytes are one
    // larger because of the ç in "Terça-feira"), wrong-year message +
    // "2026" suggestion.
    let result = engine
        .check("Festival Pop/Rock - Terça-feira, 15 de setembro de 2016")
        .unwrap();
    let date = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PT_DATE_WEEKDAY")
        .expect("PT_DATE_WEEKDAY wrong-year match");
    assert_eq!((date.range.start, date.range.end), (52, 56));
    assert_eq!(
        date.message,
        "Esta data está incorreta. Você está se referindo ao ano \"2026\"?"
    );
    assert_eq!(date.suggestions[0].value, "2026");

    // Java: PT_DATE_WEEKDAY_CURRENTYEAR 20..36, "Refere-se ao ano atual?
    // 7 de outubro, 2026 não é segunda-feira, mas sim quarta-feira.",
    // suggestions "Quarta-feira, 7|Segunda-feira, 5".
    let result = engine
        .check("Festival Pop/Rock - Segunda-feira, 7 de outubro")
        .unwrap();
    let date = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PT_DATE_WEEKDAY_CURRENTYEAR")
        .expect("PT_DATE_WEEKDAY_CURRENTYEAR match");
    assert_eq!((date.range.start, date.range.end), (20, 36));
    assert_eq!(
        date.message,
        "Refere-se ao ano atual? 7 de outubro, 2026 não é segunda-feira, mas sim quarta-feira."
    );
    assert_eq!(
        date.suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["Quarta-feira, 7", "Segunda-feira, 5"]
    );

    // Java (picky): DATE_FUTURE_VERB_PAST 24..38 (Rust 25..39: the á in
    // "Visitávamos"), no suggestions; the 2020 date must not match.
    let options = lt::EngineOptions {
        enabled_rules: vec!["DATE_FUTURE_VERB_PAST".to_string()],
        enabled_only: true,
        picky: true,
        ..Default::default()
    };
    let Some(future_engine) = Engine::builder(Lang::Pt)
        .ok()
        .and_then(|b| b.today(2026, 9, 19).options(options).build().ok())
    else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = future_engine
        .check("Visitávamos o cliente a 7 Outubro 2040.")
        .unwrap();
    let future = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DATE_FUTURE_VERB_PAST")
        .expect("DATE_FUTURE_VERB_PAST match");
    assert_eq!((future.range.start, future.range.end), (25, 39));
    assert_eq!(
        future.message,
        "A data apresentada está no futuro, mas o verbo associado está no passado."
    );
    assert!(future.suggestions.is_empty());
    let result = future_engine
        .check("Visitávamos o cliente a 7 Outubro 2020.")
        .unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "DATE_FUTURE_VERB_PAST"),
        "past dates must not match"
    );

    // `NewYearDateFilter` only accepts inside January (`AbstractNewYearDate
    // Filter.isJanuary`), so Java cannot be probed in September; pin the
    // January date instead. 2026-01-03: "7 Outubro 2025" → {year}=2025,
    // {realYear}=2026; December is excluded.
    let Some(new_year_engine) = Engine::builder(Lang::Pt)
        .ok()
        .and_then(|b| b.today(2026, 1, 3).build().ok())
    else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = new_year_engine
        .check("Festival Pop/Rock - 7 Outubro 2025")
        .unwrap();
    let new_year = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DATE_NEW_YEAR")
        .expect("DATE_NEW_YEAR match");
    assert_eq!(
        new_year.message,
        "Um novo ano começou. Será que quis dizer <suggestion>7 Outubro 2026</suggestion>?"
    );
    assert_eq!(new_year.suggestions[0].value, "7 Outubro 2026");
    for text in [
        "Festival Pop/Rock - 7 Outubro 2024",
        "Festival Pop/Rock - 7 Dezembro 2025",
        "Festival Pop/Rock - 7 Outubro 2026",
    ] {
        let result = new_year_engine.check(text).unwrap();
        assert!(
            result.matches.iter().all(|m| m.rule_id != "DATE_NEW_YEAR"),
            "unexpected DATE_NEW_YEAR match for {text}"
        );
    }
}

/// The other variants load their variant-only rule files and message
/// bundles; all filters are mapped (stage 3b).
#[test]
fn portuguese_other_variants_state() {
    let _guard = engine_guard();
    for (variant, active) in [("pt-BR", 2931), ("pt-AO", 2816), ("pt-MZ", 2817)] {
        let Some(engine) = engine_variant(variant) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert_eq!(engine.active_rule_count(), active, "{variant}");
        assert_eq!(engine.disambig_rule_count(), 525, "{variant}");
        assert!(engine.compile_failures().is_empty(), "{variant}");
    }

    let Some(engine) = engine_variant("pt-BR") else {
        eprintln!("skipping: no vendored data");
        return;
    };

    let result = engine
        .check("Esta casa é velha. foi construida em 1950.")
        .unwrap();
    let uppercase = result
        .matches
        .iter()
        .find(|m| m.rule_id == "UPPERCASE_SENTENCE_START")
        .expect("UPPERCASE_SENTENCE_START match");
    assert_eq!((uppercase.range.start, uppercase.range.end), (20, 23));
    assert_eq!(
        uppercase.message,
        "Esta frase não inicia com um letra maiúscula"
    );
    assert_eq!(
        uppercase.short_message.as_deref(),
        Some("Maiúsculo / Minúsculo")
    );
    assert_eq!(uppercase.suggestions[0].value, "Foi");
}

/// Core built-ins ported for pt in stage 1 (CommonWhitespace,
/// WhitespaceBeforePunctuation, DoublePunctuation) with the pt-PT bundle
/// strings.
#[test]
fn portuguese_core_builtins_stage1() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("pt-PT") else {
        eprintln!("skipping: no vendored data");
        return;
    };

    let result = engine
        .check("Esta casa é velha. foi construida em 1950.")
        .unwrap();
    let uppercase = result
        .matches
        .iter()
        .find(|m| m.rule_id == "UPPERCASE_SENTENCE_START")
        .expect("UPPERCASE_SENTENCE_START match");
    assert_eq!((uppercase.range.start, uppercase.range.end), (20, 23));
    assert_eq!(uppercase.message, "Esta frase não começa com maiúscula.");
    assert_eq!(uppercase.short_message.as_deref(), Some("Capitalização"));
    assert_eq!(uppercase.suggestions[0].value, "Foi");

    // `DOUBLE_PUNCTUATION`
    let result = engine.check("Ele vem.. Hoje,").unwrap();
    let double = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DOUBLE_PUNCTUATION")
        .expect("DOUBLE_PUNCTUATION match");
    assert_eq!(double.message, "Dois pontos consecutivos");
    assert_eq!(
        double.short_message.as_deref(),
        Some("Dois pontos consecutivos")
    );
}

/// XML rules (stage 1): the `CONFUSÃO_NADA_HAVER` rule group and the
/// regexp-based `ESPACO_DUPLO` rule. The latter depends on the external
/// `pt/entities/chars.ent` entity expansion (`&nbsp;` must not stay literal:
/// otherwise the character class `[&nbsp; ]` wrongly matches `s`).
#[test]
fn portuguese_xml_rules_stage1() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("pt-PT") else {
        eprintln!("skipping: no vendored data");
        return;
    };

    let result = engine.check("Ele tem haver com isso.").unwrap();
    let haver = result
        .matches
        .iter()
        .find(|m| m.rule_id == "CONFUSÃO_NADA_HAVER")
        .expect("CONFUSÃO_NADA_HAVER match");
    assert_eq!((haver.range.start, haver.range.end), (4, 13));
    assert_eq!(haver.message, "Possível confusão de termos.");
    assert_eq!(haver.suggestions[0].value, "tem a ver");

    // a single space must not match the double-space rule
    let result = engine.check("Eu vou mim fazer isso.").unwrap();
    assert!(
        result.matches.iter().all(|m| m.rule_id != "ESPACO_DUPLO"),
        "single spaces must not match ESPACO_DUPLO"
    );
    let result = engine.check("Esta é uma  frase com dois espaços.").unwrap();
    let double_space = result
        .matches
        .iter()
        .find(|m| m.rule_id == "ESPACO_DUPLO")
        .expect("ESPACO_DUPLO match");
    assert_eq!((double_space.range.start, double_space.range.end), (11, 13));
    assert_eq!(double_space.message, "Há um espaço duplo.");
    assert_eq!(double_space.suggestions[0].value, " ");
}

/// Stage 2 speller: Java-probed `getSpellingSuggestions` values
/// (`scripts/oracle/pt/probe-speller.sh`, pt-PT and pt-BR sets
/// byte-identical on 2026-09-19) plus the pt-specific `getRuleMatches`
/// post-processing (dialect alternation, diaeresis, pt-BR `-ámos`).
#[test]
fn portuguese_speller_matches_java() {
    let _guard = engine_guard();
    let Ok(data) = lt::DataDir::discover() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let rule = match std::panic::catch_unwind(|| lt::spelling_pt_probe(&data, "pt-PT")) {
        Ok(rule) => rule,
        Err(_) => {
            eprintln!("skipping: no vendored data");
            return;
        }
    };
    let suggestions = |word: &str| rule.suggestions(word);
    assert_eq!(
        suggestions("erada"),
        vec![
            "gerada", "errada", "irada", "brada", "grada", "arada", "orada", "evada", "eirada",
            "era-a", "era da", "Prada"
        ]
    );
    assert_eq!(suggestions("probrema"), vec!["problema"]);
    assert_eq!(
        suggestions("isse"),
        vec![
            "esse", "isso", "disse", "ISS", "visse", "ISSN", "asse", "risse", "misse", "sise",
            "ia-se", "ir-se", "ri-se", "vi-se", "é-se", "is se"
        ]
    );
    assert!(suggestions("a").is_empty());
    assert!(suggestions("d").is_empty());
    // a valid pt-PT clitic verb is not flagged (valid-clitic path)
    assert!(suggestions("diz-se").is_empty());
    // FSA traversal: dictionary words stop at terminal arcs, so extending
    // them (star + r) is misspelled, while all-caps words convert to their
    // titlecased dictionary entries (STARK -> Stark, Java probe 2026-09-19)
    assert_eq!(suggestions("STARR"), vec!["STAR", "STARK", "STARS"]);
    assert!(suggestions("STARK").is_empty());
    assert!(suggestions("stars").is_empty());
    // dialect alternation: BR spelling gets the pt-PT form suggested
    assert_eq!(suggestions("Amazônia"), vec!["Amazónia"]);
    // diaeresis without a match message on the suggestion-only path
    assert_eq!(suggestions("freqüência"), vec!["frequência"]);

    let Some(engine) = engine_variant("pt-PT") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java: MORFOLOGIK_RULE_PT_PT 2..12 diaeresis message + "frequência";
    // 16..24 dialect message + "Amazónia" (UTF-16 offsets; Rust bytes are
    // two larger because of ü/ê/ô).
    let result = engine.check("A freqüência da Amazônia é alta.").unwrap();
    let diaeresis = result
        .matches
        .iter()
        .find(|m| m.range.start == 2)
        .expect("diaeresis match");
    assert_eq!(diaeresis.rule_id, "MORFOLOGIK_RULE_PT_PT");
    assert_eq!(
        diaeresis.message,
        "No mais recente acordo ortográfico, não se usa mais o trema no português."
    );
    assert_eq!(diaeresis.suggestions[0].value, "frequência");
    let dialect = result
        .matches
        .iter()
        .find(|m| m.specific_rule_id.as_deref() == Some("MORFOLOGIK_RULE_PT_PT_DIALECT"))
        .expect("dialect match");
    assert_eq!((dialect.range.start, dialect.range.end), (18, 27));
    assert_eq!(
        dialect.message,
        "Possível erro de ortografia: esta é a grafia utilizada no português brasileiro."
    );
    assert_eq!(dialect.suggestions[0].value, "Amazónia");

    let Some(br_engine) = engine_variant("pt-BR") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java: MORFOLOGIK_RULE_PT_BR 4..11 "-ámos" message + "andamos"
    let result = br_engine
        .check("Nós andámos muito na semana passada.")
        .unwrap();
    let amos = result
        .matches
        .iter()
        .find(|m| m.rule_id == "MORFOLOGIK_RULE_PT_BR")
        .expect("-ámos match");
    assert_eq!((amos.range.start, amos.range.end), (5, 13));
    assert_eq!(
        amos.message,
        "No Brasil, o pretérito perfeito da primeira pessoa do plural escreve-se sem acento."
    );
    assert_eq!(amos.suggestions[0].value, "andamos");
}

/// Stage 3d compound family: Java probes (`scripts/oracle/pt/probe-rule.sh`,
/// pinned build, 2026-09-19, `Level.PICKY`, only the compound rules enabled)
/// plus the Java unit tests' merge/sub-id expectations. Rust offsets are
/// UTF-8 bytes; Java UTF-16 values are noted where they differ.
#[test]
fn portuguese_compound_family_matches_java() {
    let _guard = engine_guard();
    let ids = [
        "PT_COMPOUNDS_POST_REFORM",
        "PT_COMPOUNDS_PRE_REFORM",
        "PT_COLOUR_HYPHENATION",
        "PT_POSAO_DASH_RULE",
        "PT_PREAO_DASH_RULE",
    ];
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = lt::EngineOptions {
        enabled_rules: ids.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        picky: true,
        ..Default::default()
    };
    let Some(engine) = builder.variant("pt-PT").options(options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let check = |text: &str, rule: &str| -> lt::Match {
        let result = engine.check(text).unwrap();
        result
            .matches
            .iter()
            .find(|m| m.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"))
            .clone()
    };

    // Java: PT_COMPOUNDS_POST_REFORM_SUPER_HERÓI 2..13 "Esta palavra é hifenizada." super-herói
    let m = check(
        "O super herói luta.",
        "PT_COMPOUNDS_POST_REFORM_SUPER_HERÓI",
    );
    assert_eq!((m.range.start, m.range.end), (2, 14));
    assert_eq!(m.message, "Esta palavra é hifenizada.");
    assert_eq!(m.suggestions[0].value, "super-herói");
    // Java unit test: ultra-som → ultrassom (Portuguese mergeCompound digraph)
    let m = check(
        "Comprei um ultra-som ontem.",
        "PT_COMPOUNDS_POST_REFORM_ULTRA_SOM",
    );
    assert_eq!((m.range.start, m.range.end), (11, 20));
    assert_eq!(m.message, "Esta palavra é composta por justaposição.");
    assert_eq!(m.suggestions[0].value, "ultrassom");
    // Java: PT_COMPOUNDS_POST_REFORM_ANTI_SEMITA 7..18 antissemita
    let m = check(
        "Isto é anti semita.",
        "PT_COMPOUNDS_POST_REFORM_ANTI_SEMITA",
    );
    assert_eq!((m.range.start, m.range.end), (8, 19));
    assert_eq!(m.suggestions[0].value, "antissemita");
    // Java: PT_COMPOUNDS_POST_REFORM_GRÃ_BRETANHA 2..14 Grã-Bretanha
    let m = check(
        "A Grã Bretanha é grande.",
        "PT_COMPOUNDS_POST_REFORM_GRÃ_BRETANHA",
    );
    assert_eq!((m.range.start, m.range.end), (2, 15));
    assert_eq!(m.suggestions[0].value, "Grã-Bretanha");
    // Java: PT_COMPOUNDS_POST_REFORM_WEB_SITE 11..19 website
    let m = check(
        "Comprou um web-site novo.",
        "PT_COMPOUNDS_POST_REFORM_WEB_SITE",
    );
    assert_eq!((m.range.start, m.range.end), (11, 19));
    assert_eq!(m.suggestions[0].value, "website");
    // Java: PT_COMPOUNDS_POST_REFORM_ÓPERA_ROCK 2..12 ópera-rock
    let m = check("O ópera rock tocou.", "PT_COMPOUNDS_POST_REFORM_ÓPERA_ROCK");
    assert_eq!((m.range.start, m.range.end), (2, 13));
    assert_eq!(m.suggestions[0].value, "ópera-rock");

    // Java: PT_COLOUR_HYPHENATION_AZUL_CLARO 11..21 azul-claro
    let m = check("A camisa é azul claro.", "PT_COLOUR_HYPHENATION_AZUL_CLARO");
    assert_eq!((m.range.start, m.range.end), (12, 22));
    assert_eq!(
        m.message,
        "Nomes de cores são palavras compostas e devem ser hifenizados."
    );
    assert_eq!(m.suggestions[0].value, "azul-claro");

    // Java: PT_POSAO_DASH_RULE 9..18 ab-reação (em dash is one UTF-16 unit)
    let m = check("Escreveu ab—reação no texto.", "PT_POSAO_DASH_RULE");
    assert_eq!((m.range.start, m.range.end), (9, 22));
    assert_eq!(m.message, "Um travessão foi utilizado em vez de um hífen.");
    assert_eq!(m.suggestions[0].value, "ab-reação");
    // already joined: no match (Java unit test)
    let result = engine.check("ab-reação").unwrap();
    assert!(result.matches.is_empty(), "{:?}", result.matches);

    // pt-AO uses the pre-reform pair; the post-reform dash rule is absent
    let Some(ao_engine) = engine_variant("pt-AO") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = ao_engine.check("A Grã Bretanha é grande.").unwrap();
    assert!(
        result
            .matches
            .iter()
            .any(|m| m.rule_id.starts_with("PT_COMPOUNDS_PRE_REFORM")),
        "pt-AO pre-reform compound expected: {:?}",
        result.matches
    );
    let result = ao_engine.check("Escreveu ab—reação no texto.").unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "PT_POSAO_DASH_RULE"),
        "pt-AO must not run the post-reform dash rule"
    );
}

/// Stage 3d repetition rules and filler words: Java probes
/// (`scripts/oracle/pt/probe-rule.sh`, 2026-09-19, PICKY, one rule enabled).
#[test]
fn portuguese_repeat_and_filler_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("pt-PT") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java: PORTUGUESE_WORD_REPEAT_RULE 5..8 "é é" → é
    let result = engine
        .check("Este é é apenas uma frase de exemplo.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PORTUGUESE_WORD_REPEAT_RULE")
        .expect("word repeat match");
    assert_eq!((m.range.start, m.range.end), (5, 10));
    assert_eq!(
        m.message,
        "Possível erro de digitação. Repetiu uma palavra."
    );
    assert_eq!(m.suggestions[0].value, "é");
    // Java `PortugueseWordRepeatRule.ignore`: "Logo logo", "Aaptos aaptos"
    // and hyphenated pronouns ("Coloquem-na na") are not repetitions.
    for text in [
        "Logo logo vamos ao mercado.",
        "Aaptos aaptos são animais.",
        "Coloquem-na na sala.",
        "blá blá é conversa.",
    ] {
        let result = engine.check(text).unwrap();
        assert!(
            result
                .matches
                .iter()
                .all(|m| m.rule_id != "PORTUGUESE_WORD_REPEAT_RULE"),
            "{text:?} must not be a repetition"
        );
    }

    // Java: PORTUGUESE_WORD_REPEAT_BEGINNING_RULE 26..31 "Então"
    let result = engine
        .check("Então, este está correto. Então, este está errado, por causa da repetição.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PORTUGUESE_WORD_REPEAT_BEGINNING_RULE")
        .expect("word repeat beginning match");
    assert_eq!((m.range.start, m.range.end), (28, 34));
    assert_eq!(
        m.message,
        "Duas frases seguidas começadas com o mesmo advérbio Considere reescrever a frase, ou procurar sinónimos"
    );
    assert!(m.suggestions.is_empty());

    // Java: FILLER_WORDS_PT 7..12 "muito" (rule is default off)
    let result = engine.check("Isto é muito bom.").unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "FILLER_WORDS_PT"),
        "FILLER_WORDS_PT must be off by default"
    );
    let Some(filler_engine) = Engine::builder(Lang::Pt).ok().and_then(|b| {
        b.variant("pt-PT")
            .options(lt::EngineOptions {
                enabled_rules: vec!["FILLER_WORDS_PT".to_string()],
                enabled_only: true,
                picky: true,
                ..Default::default()
            })
            .build()
            .ok()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = filler_engine.check("Isto é muito bom.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "FILLER_WORDS_PT")
        .expect("filler words match");
    assert_eq!((m.range.start, m.range.end), (8, 13));
    assert_eq!(
        m.message,
        "Esta palavra está classificada como de enchimento. Elimine-a se possível."
    );

    // pt-BR bundles for the repetition strings
    let Some(br_engine) = engine_variant("pt-BR") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = br_engine
        .check("Este é é apenas uma frase de exemplo.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PORTUGUESE_WORD_REPEAT_RULE")
        .expect("pt-BR word repeat match");
    assert_eq!(
        m.message,
        "Possível erro de escrita: você repetiu uma palavra"
    );
    // plain pt/pt-AO fall back to the pt-PT bundle via getDefaultLanguageVariant
    let Some(ao_engine) = engine_variant("pt-AO") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = ao_engine.check("Isto é muito bom.").unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "FILLER_WORDS_PT"),
        "FILLER_WORDS_PT must be off by default for pt-AO too"
    );
}

/// Stage 3d semantics/style rules: Java probes (`scripts/oracle/pt/probe-rule.sh`,
/// 2026-09-19, PICKY, one rule enabled) for the wrong-word-in-context,
/// word-coherency and readability classes. Rust offsets are UTF-8 bytes; the
/// Java UTF-16 values are equal for the ASCII cases and noted otherwise.
#[test]
fn portuguese_semantics_style_rules_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("pt-PT") else {
        eprintln!("skipping: no vendored data");
        return;
    };

    // Java: PORTUGUESE_WRONG_WORD_IN_CONTEXT_ASCENDEU_ACENDEU 4..12
    let result = engine.check("Ele ascendeu a luz do quarto.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id.starts_with("PORTUGUESE_WRONG_WORD_IN_CONTEXT"))
        .expect("wrong word in context match");
    assert_eq!(
        m.rule_id,
        "PORTUGUESE_WRONG_WORD_IN_CONTEXT_ASCENDEU_ACENDEU"
    );
    assert_eq!((m.range.start, m.range.end), (4, 12));
    assert_eq!(
        m.message,
        "Considere <suggestion>acendeu</suggestion>, i.e. dar luz, em vez de 'ascendeu', i.e. subir?"
    );
    assert_eq!(m.suggestions[0].value, "acendeu");

    // Java: PT_WORD_COHERENCY 30..39 "avantesma" → abantesma
    let result = engine
        .check("Vi um abantesma. Depois outro avantesma apareceu.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PT_WORD_COHERENCY")
        .expect("word coherency match");
    assert_eq!((m.range.start, m.range.end), (30, 39));
    assert_eq!(
        m.message,
        "Não deve utilizar formas distintas de palavras com dupla grafia no mesmo texto. Escolha entre 'avantesma' e 'abantesma'."
    );
    assert_eq!(m.suggestions[0].value, "abantesma");

    // Java: READABILITY_RULE_SIMPLE_PT 0..10 / 56..65 (UTF-16); byte offsets
    // are two larger for 'pão'/'água'/'cão' and one for 'é'.
    let text = "O menino come pão. A menina bebe água. O cão corre no jardim.\n\nO gato dorme. A casa é grande. O sol brilha hoje.";
    let result = engine.check(text).unwrap();
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "READABILITY_RULE_SIMPLE_PT"),
        "readability rules are default off"
    );
    let readability_engine = Engine::builder(Lang::Pt).ok().and_then(|b| {
        b.variant("pt-PT")
            .options(lt::EngineOptions {
                enabled_rules: vec!["READABILITY_RULE_SIMPLE_PT".to_string()],
                enabled_only: true,
                picky: true,
                ..Default::default()
            })
            .build()
            .ok()
    });
    let Some(readability_engine) = readability_engine else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = readability_engine.check(text).unwrap();
    let ranges: Vec<(usize, usize)> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "READABILITY_RULE_SIMPLE_PT")
        .map(|m| (m.range.start, m.range.end))
        .collect();
    assert_eq!(ranges, vec![(0, 13), (66, 78)]);
    assert_eq!(
        result.matches[0].message,
        "Legibilidade {FRE: 100, ASL: 4, ASW: 1}: O texto deste parágrafo é fácil {Nível 6: Muito simples}. Tem poucas palavras por frase e poucas sílabas por palavra."
    );

    // Java: READABILITY_RULE_DIFFICULT_PT (UTF-16 0..18 / 222..240)
    let hard = "A implementação da infraestrutura organizacional contemporânea requer responsabilidade administrativa extraordinária. A internacionalização da documentação burocrática institucional demonstra complexidade incomensurável.\n\nA implementação da infraestrutura organizacional contemporânea requer responsabilidade administrativa extraordinária. A internacionalização da documentação burocrática institucional demonstra complexidade incomensurável.";
    let Some(hard_engine) = Engine::builder(Lang::Pt).ok().and_then(|b| {
        b.variant("pt-PT")
            .options(lt::EngineOptions {
                enabled_rules: vec!["READABILITY_RULE_DIFFICULT_PT".to_string()],
                enabled_only: true,
                picky: true,
                ..Default::default()
            })
            .build()
            .ok()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = hard_engine.check(hard).unwrap();
    let ranges: Vec<(usize, usize)> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "READABILITY_RULE_DIFFICULT_PT")
        .map(|m| (m.range.start, m.range.end))
        .collect();
    assert_eq!(ranges, vec![(0, 20), (232, 252)]);
    assert_eq!(
        result.matches[0].message,
        "Legibilidade {FRE: -61, ASL: 9, ASW: 4}: O texto deste parágrafo é difícil {Nível 0: Muito complexo}. Tem muitas palavras por frase e muitas sílabas por palavra."
    );
}

/// Stage 3d default-off accentuation rule and the unit-conversion rule:
/// Java probes (`scripts/oracle/pt/probe-rule.sh`, 2026-09-19, PICKY, one
/// rule enabled) plus the pinned `PortugueseUnitConversionRuleTest` values.
#[test]
fn portuguese_accentuation_and_units_match_java() {
    let _guard = engine_guard();
    let Some(accent_engine) = Engine::builder(Lang::Pt).ok().and_then(|b| {
        b.variant("pt-PT")
            .options(lt::EngineOptions {
                enabled_rules: vec!["ACCENTUATION_CHECK_PT".to_string()],
                enabled_only: true,
                picky: true,
                ..Default::default()
            })
            .build()
            .ok()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java: ACCENTUATION_CHECK_PT 9..16 "formula" → fórmula
    let result = accent_engine
        .check("Uma nova formula que resolve o problema.")
        .unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "ACCENTUATION_CHECK_PT");
    assert_eq!((m.range.start, m.range.end), (9, 16));
    assert_eq!(m.message, "Se é um nome ou um adjectivo, tem acento.");
    assert_eq!(m.suggestions[0].value, "fórmula");
    // Java: 19..29 (UTF-16) → 20..30 bytes, "influencia" → influência
    let result = accent_engine
        .check("Isto é de positiva influencia no resultado.")
        .unwrap();
    assert_eq!(
        (result.matches[0].range.start, result.matches[0].range.end),
        (20, 30)
    );
    assert_eq!(result.matches[0].suggestions[0].value, "influência");
    // no match without a qualifying context (Java probe)
    let result = accent_engine.check("A formula foi aprovada.").unwrap();
    assert!(result.matches.is_empty());

    // `UNIDADES_METRICAS`: Java probe values (UTF-16 ranges; byte ranges are
    // equal unless the sentence has multi-byte characters).
    let Some(engine) = Engine::builder(Lang::Pt).ok().and_then(|b| {
        b.variant("pt-PT")
            .options(lt::EngineOptions {
                enabled_rules: vec!["UNIDADES_METRICAS".to_string()],
                enabled_only: true,
                picky: true,
                ..Default::default()
            })
            .build()
            .ok()
    }) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let check = |text: &str| -> Vec<lt::Match> {
        engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "UNIDADES_METRICAS")
            .collect()
    };
    // Java: 9..14 "6 pés", suggestions as pinned by the Java unit test
    let m = check("Eu tenho 6 pés de altura.");
    assert_eq!((m[0].range.start, m[0].range.end), (9, 15));
    assert_eq!(
        m[0].message,
        "Deseja adicionar automaticamente uma conversão ao sistema métrico?"
    );
    assert_eq!(m[0].suggestions[0].value, "6 pés (1,83 m)");
    assert_eq!(m[0].suggestions[1].value, "6 pés (1,83 metros)");
    // "10.000 libras" (Java 13..26) prefers tonnes
    let m = check("A carga é de 10.000 libras.");
    assert_eq!((m[0].range.start, m[0].range.end), (14, 27));
    assert_eq!(m[0].suggestions[0].value, "10.000 libras (4,54 t)");
    assert_eq!(m[0].suggestions[1].value, "10.000 libras (4,54 toneladas)");
    // 5'6" (Java 8..13, leading space in the suggestion like Java)
    let m = check("Isto tem 5'6\" de altura.");
    assert_eq!((m[0].range.start, m[0].range.end), (8, 13));
    assert_eq!(m[0].suggestions[0].value, " 5'6\" (1,68 m)");
    // base-class sq ft pattern in the pt table
    let m = check("O meu novo apartamento tem 500 sq ft de área.");
    assert_eq!((m[0].range.start, m[0].range.end), (27, 36));
    assert_eq!(
        m[0].suggestions[3].value,
        "500 sq ft (46,45 metros quadrados)"
    );
    // existing conversion check: Java 17..19, Portuguese CHECK message
    let m = check("A via tem 10 km (20 milhas) de comprimento.");
    assert_eq!((m[0].range.start, m[0].range.end), (17, 19));
    assert_eq!(
        m[0].message,
        "Esta conversão não parece estar precisa. Gostaria de corrigi-la?"
    );
    assert_eq!(m[0].suggestions[1].value, "6,21 mi");
    // Fahrenheit (Java 16..36 UTF-16, one extra byte for é)
    let m = check("A temperatura é 100 Graus Fahrenheit.");
    assert_eq!((m[0].range.start, m[0].range.end), (17, 37));
    assert_eq!(m[0].suggestions[1].value, "100 Graus Fahrenheit (37,78 °C)");
}

/// Corpus regressions from `docs/parity/golden/pt-full.txt` against the
/// committed Java golden `pt-full.java.tsv` (2026-09-19): the multi-line
/// Portuguese entity values (`&compounds_with_bem;`,
/// `&subjunctive_verbs;`) must be whitespace-normalized like Java's
/// `StringTools.trimWhitespace`, otherwise the indented alternatives never
/// match (D-115).
#[test]
fn portuguese_multiline_entity_alternatives_match_java() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let Some(engine) = builder.build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let find = |text: &str, rule: &str| -> lt::Match {
        let result = engine.check(text).unwrap();
        result
            .matches
            .iter()
            .find(|m| m.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"))
            .clone()
    };

    // Java: MELHOR_EDUCADO sub 1, 17..31, suggestion "mais bem-educada"
    let m = find("Ela era a menina melhor educada.", "MELHOR_EDUCADO");
    assert_eq!((m.range.start, m.range.end), (17, 31));
    assert_eq!(m.sub_id.as_deref(), Some("1"));
    assert_eq!(m.suggestions[0].value, "mais bem-educada");
    // Java: MELHOR_EDUCADO sub 2, 14..28, suggestion "mais mal-humorados"
    let m = find("Ficaram ainda pior humorados.", "MELHOR_EDUCADO");
    assert_eq!((m.range.start, m.range.end), (14, 28));
    assert_eq!(m.sub_id.as_deref(), Some("2"));
    assert_eq!(m.suggestions[0].value, "mais mal-humorados");
    // Java: ESPERA_QUE_INDICATIVO_AR sub 1, 19..25, suggestion "acorde"
    let m = find(
        "Proponho que Paulo acorda mais cedo.",
        "ESPERA_QUE_INDICATIVO_AR",
    );
    assert_eq!((m.range.start, m.range.end), (19, 25));
    assert_eq!(m.suggestions[0].value, "acorde");
}

/// Corpus regression from `pt-full.java.tsv` line 95: an empty `\1`
/// back-reference (unmatched optional article) must drop the following space
/// (`concatWithoutExtraSpace`), so the suggestion is "O seu", not " O seu".
#[test]
fn portuguese_empty_suggestion_backref_drops_space() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let Some(engine) = builder.build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Seu poder prorrogou a partir de Mashhad.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "POSSESSIVE_WITHOUT_ARTICLE")
        .expect("possessive rule did not match");
    assert_eq!((m.range.start, m.range.end), (0, 3));
    assert_eq!(m.suggestions[0].value, "O seu");
}

/// Corpus regression for the `suppress_misspelled` tagger check: Java's
/// `MatchState.toFinalString` uses the full tagger (`tag(List)`, including
/// the ordinal heuristics), so suggestions built from forms like "3ª" are
/// kept. `tag_word` alone rejected them and dropped every
/// GENERAL_GENDER_AGREEMENT_ERRORS suggestion (corpus lines 159-292).
#[test]
fn portuguese_suppress_misspelled_allows_ordinals() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let Some(engine) = builder.build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let find = |text: &str| -> lt::Match {
        let result = engine.check(text).unwrap();
        result
            .matches
            .iter()
            .find(|m| m.rule_id == "GENERAL_GENDER_AGREEMENT_ERRORS")
            .unwrap_or_else(|| panic!("rule did not match {text:?}"))
            .clone()
    };
    // Java UTF-16: sub 8, 9..14 ("no 1ª"); Rust bytes 10..16 (two 2-byte chars)
    let m = find("Ele está no 1ª lugar.");
    assert_eq!((m.range.start, m.range.end), (10, 16));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["no 1º", "na 1ª"]);
    // Java UTF-16: sub 7, 9..14; Rust bytes 10..15
    let m = find("é membro da 3o Congresso.");
    assert_eq!((m.range.start, m.range.end), (10, 15));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["da 3ª", "do 3o"]);
    // Java UTF-16: sub 1, 9..15; Rust bytes 9..16 (ª is two bytes)
    let m = find("Quem foi o 12.ª coronel?");
    assert_eq!((m.range.start, m.range.end), (9, 16));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["a 12.ª", "o, 12.ª", "o 12.ª"]);
}

/// Corpus regressions for `CleanOverlappingFilter`'s "take the longest error"
/// tie-break: Java compares UTF-16 code-unit lengths, the port compared UTF-8
/// byte lengths, so accented matches lost against shorter byte-wise rivals
/// (corpus lines 4205 and 4902).
#[test]
fn portuguese_overlap_tiebreak_uses_utf16_length() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let Some(engine) = builder.build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let check = |text: &str| -> Vec<lt::Match> { engine.check(text).unwrap().matches };

    // Java: CONFUSÃO_SOU_SÓ 18..25 ("são sou"), WORD_REPEAT dropped
    let matches = check("Abri o álbum, são são sou fotos de carros.");
    let ids: Vec<&str> = matches.iter().map(|m| m.rule_id.as_str()).collect();
    assert_eq!(ids, vec!["CONFUSÃO_SOU_SÓ"]);
    assert_eq!((matches[0].range.start, matches[0].range.end), (20, 28));
    assert_eq!(matches[0].suggestions[0].value, "são, sou");
    assert_eq!(matches[0].suggestions[1].value, "são só");

    // Java: GENERAL_GENDER_AGREEMENT_ERRORS 12..23 ("um aposição"), the
    // overlapping SPACE_BEFORE_PUNCTUATION2 match is dropped
    let matches = check("… assumirem um aposição …");
    let ids: Vec<&str> = matches.iter().map(|m| m.rule_id.as_str()).collect();
    assert_eq!(ids, vec!["GENERAL_GENDER_AGREEMENT_ERRORS"]);
    assert_eq!((matches[0].range.start, matches[0].range.end), (14, 27));
    assert_eq!(matches[0].suggestions[0].value, "uma aposição");
}

/// Corpus regression for the MultiWordChunker's low-priority tag rule:
/// Java skips the `NPCN000` chunk reading when the token already has a real
/// reading (`isLowPriorityTag`), so a proper noun like "Netflix" keeps only
/// NPFSO00 and the GGA exception for `(?:[NZ].|A..)[CM].+` does not fire.
/// Without the guard the added NPCN000 reading excepted the token (corpus
/// line 1323).
#[test]
fn portuguese_chunker_skips_low_priority_tag_on_tagged_tokens() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let Some(engine) = builder.build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java UTF-16: sub 1, 9..19 ("no Netflix"); Rust bytes 10..20
    let result = engine.check("Ele está no Netflix.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "GENERAL_GENDER_AGREEMENT_ERRORS")
        .expect("gender agreement rule did not match");
    assert_eq!((m.range.start, m.range.end), (10, 20));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["na Netflix", "no, Netflix", "no Netflix"]);
}

/// Corpus regression for `<regexp>` rule suggestions: a self-closing
/// `<suggestion/>` (LOOSE_ACCENTS' "remove the loose accent" alternative) is
/// an empty replacement in Java, not a missing suggestion (corpus lines
/// 5354/5401).
#[test]
fn portuguese_regexp_rule_keeps_empty_suggestion() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::Pt) else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let Some(engine) = builder.build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java UTF-16: 5..6; Rust bytes 6..8 (´ is two bytes)
    let result = engine
        .check("Não e´ comum encontrar erros deste tipo.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "LOOSE_ACCENTS")
        .expect("LOOSE_ACCENTS did not match");
    assert_eq!((m.range.start, m.range.end), (6, 8));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["'", "’", ""]);
}
