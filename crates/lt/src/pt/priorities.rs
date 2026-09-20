//! `Portuguese.getPriorityForId` / `getRulePriority`: the base Portuguese
//! `id2prio` table plus the `MORFOLOGIK_RULE*` / `PT_SIMPLE_REPLACE_*` /
//! `ACENTUAÇÃO_VOGAL_ÊNCLISE` / `COLOCACAO_PRONOMINAL_COM_ATRATOR` prefix
//! rules. Variant overrides (`PortugalPortuguese`, `BrazilianPortuguese`)
//! are applied in stage 3 when the variant-specific rule sets land.

/// Base `Language.getPriorityForId` (the `super.getPriorityForId` fallback of
/// `Portuguese`).
fn base_priority(id: &str) -> i32 {
    if id.eq_ignore_ascii_case("TOO_LONG_SENTENCE") {
        return -101;
    }
    if id == "REPETITIONS_STYLE" {
        return -55;
    }
    if id.contains("STYLE") {
        return -50;
    }
    0
}

/// `Portuguese.id2prio`.
fn table_priority(id: &str) -> i32 {
    match id {
        "FRAGMENT_TWO_ARTICLES" => 50,
        "DEGREE_MINUTES_SECONDS" => 30,
        "INTERJECTIONS_PUNTUATION" => 20,
        "CONFUSION_POR_PÔR_V2" => 10,
        "PARONYM_POLITICA_523" => 10,
        "PARONYM_PRONUNCIA_262" => 10,
        "PARONYM_CRITICA_397" => 10,
        "PARONYM_INICIO_169" => 10,
        "LP_PARONYMS" => 10,
        "PARONYM_MUSICO_499_bis" => 10,
        "NA_NÃO" => 10,
        "VERB_COMMA_CONJUNCTION" => 10, // greater than PORTUGUESE_WORD_REPEAT_RULE
        "HOMOPHONE_AS_CARD" => 5,
        "TODOS_FOLLOWED_BY_NOUN_PLURAL" => 3,
        "TODOS_FOLLOWED_BY_NOUN_SINGULAR" => 2,
        "AUSENCIA_VIRGULA" => 1,
        "EMAIL" => 1,
        "UNPAIRED_BRACKETS" => -5,
        "PROFANITY" => -6,
        "PT_BARBARISMS_REPLACE" => -10,
        "BARBARISMS_PT_PT_V4" => -10,
        // for pt-PT, not lower than speller, not sure why
        "PT_PT_SIMPLE_REPLACE" => -11,
        "PT_REDUNDANCY_REPLACE" => -12,
        "PT_WORDINESS_REPLACE" => -13,
        "PT_CLICHE_REPLACE" => -17,
        "INTERNET_ABBREVIATIONS" => -24,
        "CHILDISH_LANGUAGE" => -25,
        "ARCHAISMS" => -26,
        "INFORMALITIES" => -27,
        "BIASED_OPINION_WORDS" => -31,
        "PT_AGREEMENT_REPLACE" => -35,
        "CONTA_TO" => -44,
        // prefer over spell checker
        "PT_DIACRITICS_REPLACE" => -45,
        "DIACRITICS" => -45,
        "PT_COMPOUNDS_POST_REFORM" => -45,
        "AUX_VERBO" => -45,
        "ENSINO_A_DISTANCIA" => -45,
        "OQ_O_QUE_ORTHOGRAPHY" => -45,
        "PT_ENGLISH_CONTRACTION_ORTHOGRAPHY" => -45,
        // HIGHER THAN SPELLER
        "EMAIL_SEM_HIFEN" => -45,
        // MORFOLOGIK SPELLER FITS HERE AT -50
        // LOWER THAN SPELLER
        "PRETERITO_PERFEITO" => -51,
        "PT_BR_SIMPLE_REPLACE" => -51,
        "CRASE_CONFUSION" => -54,
        "NAO_MILITARES_CIVIS" => -54,
        "NA_QUELE" => -54,
        "NOTAS_FICAIS" => -54,
        "GENERAL_VERB_AGREEMENT_ERRORS" => -55,
        "GENERAL_NUMBER_AGREEMENT_ERRORS" => -56,
        "GENERAL_GENDER_NUMBER_AGREEMENT_ERRORS" => -56,
        "FINAL_STOPS" => -75,
        "EU_NÓS_REMOVAL" => -90,
        "COLOCAÇÃO_ADVÉRBIO" => -90,
        "FAZER_USO_DE-USAR-RECORRER" => -90,
        "FORMAL_T-V_DISTINCTION" => -100,
        "FORMAL_T-V_DISTINCTION_ALL" => -101,
        "REPEATED_WORDS" => -210,
        "PT_WIKIPEDIA_COMMON_ERRORS" => -500,
        "UPPERCASE_SENTENCE_START" => -600,
        "FILLER_WORDS_PT" => -990,
        "TOO_LONG_SENTENCE" => -997,
        "TOO_LONG_PARAGRAPH" => -998,
        "READABILITY_RULE_SIMPLE_PT" => -1100,
        "READABILITY_RULE_DIFFICULT_PT" => -1101,
        "UNKNOWN_WORD" => -2000,
        _ => 0,
    }
}

/// Rules whose Java subclasses call `useSubRuleSpecificIds()` build their
/// matches with a `SpecificIdRule`, so Java's `getRulePriority` looks up the
/// *specific* id (`<base>_<toId>`). That misses the base-table entries: a
/// `PT_COMPOUNDS_POST_REFORM_…` match has priority 0, not -45 (probed
/// 2026-09-19: with all rules active pt-PT reports the `AO90_…` XML match,
/// pt-BR the compound match). The port must therefore not map specific ids
/// back to their base id.
///
/// `Portuguese.getPriorityForId`.
/// `Portuguese.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    if id.starts_with("MORFOLOGIK_RULE") {
        return -50;
    }
    if id.starts_with("PT_SIMPLE_REPLACE_ORTHOGRAPHY") {
        return -49;
    }
    if id.starts_with("AI_PT_GGEC_REPLACEMENT_ORTHOGRAPHY_SPELL") {
        return -48;
    }
    if id.starts_with("PT_MULTITOKEN_SPELLING") {
        return -48;
    }
    if id.starts_with("AI_PT_GGEC_REPLACEMENT_OTHER") {
        return -4;
    }
    // enclitic diacritics always take precedence over pronoun placement
    if id.starts_with("ACENTUAÇÃO_VOGAL_ÊNCLISE") {
        return -51;
    }
    if id.starts_with("COLOCACAO_PRONOMINAL_COM_ATRATOR") {
        return -52;
    }
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    if id.starts_with("AI_PT_HYDRA_LEO") {
        // prefer more specific rules (also speller)
        if id.starts_with("AI_PT_HYDRA_LEO_MISSING_COMMA") {
            return -51; // prefer comma style rules.
        }
        return -51;
    }
    base_priority(id)
}

/// Variant-aware `getPriorityForId`: `PortugalPortuguese.id2prio` overrides
/// the base table for pt-PT.
pub fn priority_for_id_variant(id: &str, variant: Option<&str>) -> i32 {
    if variant == Some("pt-PT") {
        if id == "PORTUGUESE_OLD_SPELLING_INTERNAL" {
            return -9;
        }
        if id == "PT_COMPOUNDS_POST_REFORM" {
            return 1;
        }
    }
    priority_for_id(id)
}

/// `Language.getRulePriority` for Portuguese (base style default 0).
pub fn rule_priority_variant(
    rule_id: &str,
    category_id: &str,
    _issue_type: &str,
    rule_priority: i32,
    variant: Option<&str>,
) -> i32 {
    let rule_specific = priority_for_id_variant(rule_id, variant);
    if rule_specific != 0 {
        return rule_specific;
    }
    if rule_priority != 0 {
        return rule_priority;
    }
    let category = priority_for_id_variant(category_id, variant);
    if category != 0 {
        return category;
    }
    0
}
