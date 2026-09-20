//! `Catalan.getPriorityForId` (the `id2prio` switch + the prefix rules) and
//! `Catalan.getDefaultRulePriorityForStyle` (-50) for
//! `CleanOverlappingFilter`.

/// `Catalan.getPriorityForId`'s exact-id switch.
fn table_priority(id: &str) -> i32 {
    match id {
        "CONFUSIONS2" => 80,
        "DEU_NI_DO" => 80, // greater than rules about pronouns
        "FER_LOGIN" => 70, // greater than anglicisms
        "L_OK" => 70,      // greater than anglicisms
        "INCORRECT_EXPRESSIONS" => 50,
        "PERSONATGES_FAMOSOS" => 50,
        "CONEIXO_CONEC" => 50,
        "COMETES_INCORRECTES" => 50, // greater than PRONOMS_FEBLES
        "OFERTAR_OFERIR" => 50,      // greater than PRONOMS_FEBLES_SOLTS2
        "PREGUEM_DISCULPIN" => 45,   // greater than ESPERANT_US_AGRADI
        "DESDE_UN" => 40,
        "CONEIXET" => 40,
        "CONEIXENTS" => 40,
        "MOTS_NO_SEPARATS" => 40,
        "REPETEAD_ELEMENTS" => 40,
        "ESPERANT_US_AGRADI" => 40,
        "LO_NEUTRE" => 40,       // lower than other INCORRECT_EXPRESSIONS
        "ESPAIS_SOBRANTS" => 40, // greater than L
        "PER_A_QUE_PERQUE" => 40,
        "PRONOMS_FEBLES_COMBINACIONS_SE" => 40,
        "ELA_GEMINADA" => 35, // greater than agreement rules, pronoun rules
        "TENIR_QUE" => 35,    // greater than CA_SIMPLE_REPLACE
        "CONFUSIONS_PRONOMS_FEBLES" => 35, // greater than ES (DIACRITICS), PRONOMS_FEBLES_DARRERE_VERB
        "COMMA_PERO1" => 35,               // greater than CA_SIMPLE_REPLACE
        "PASSAR_SE" => 35,                 // greater than OBLIDARSE
        "OBLIDARSE" => 30,                 // greater than ACOSTUMAR_A
        "CA_SPLIT_WORDS" => 30,
        "PRONOMS_FEBLES_TEMPS_VERBAL" => 35,
        "ET_AL" => 30,                      // greater than apostrophes and pronouns
        "PRONOMS_FEBLES_COLLOQUIALS" => 30, // greater than PRONOMS_FEBLES_SOLTS2
        "CONCORDANCES_CASOS_PARTICULARS" => 30,
        "GERUNDI_PERD_T" => 30,
        "CONFUSIONS" => 30,
        "PRONOMS_FEBLES_DARRERE_VERB" => 30, // greater than PRONOMS_FEBLES_SOLTS2
        "VERBS_NO_INCOATIUS" => 30,          // greater than PRONOMS_FEBLES_SOLTS2
        "ARRIBAN_ARRIBANT" => 30,
        "PERO_PERO" => 30,                  // lower than COMMA_PERO1
        "PUNT_LLETRA" => 30,                // greater than CONCORDANCES_DET_NOM
        "REEMPRENDRE" => 28,                // equal to CA_SIMPLE_REPLACE_VERBS
        "INCORRECT_WORDS_IN_CONTEXT" => 28, // similar to but lower than CONFUSIONS, greater than ES_KNOWN
        "PRONOMS_FEBLES_SOLTS2" => 26, // greater than PRONOMS_FEBLES_SOLTS, ES, HAVER_SENSE_HAC
        "ES_UNKNOWN" => 25,
        "HAVER_SENSE_HAC" => 25, // greater than CONFUSIONS_ACCENT avia, lower than CONFUSIONS_E
        "HA_A" => 25,            // lower than CA_SIMPLE_REPLACE_VERBS
        "PASSAT_PERIFRASTIC" => 25, // greater than CONFUSIONS_ACCENT
        "PREPOSITIONS" => 25,
        "CONFUSIONS_ACCENT" => 20,
        "CONFUSIO_PASSAT_INFINITIU" => 20, // greater than ACCENTUATION_CHECK
        "DIACRITICS" => 20,
        "COMMA_ENTRE_DALTRES" => 20, // greater than CONCORDANCES_DET_NOM
        "CAP_GENS" => 20,            // greater than CAP_ELS_CAP_ALS, CONCORDANCES_DET_NOM
        "MOTS_SENSE_GUIONETS" => 20, // greater than CONCORDANCES_NUMERALS
        "ORDINALS" => 20,            // greater than SEPARAT
        "SUPER" => 20,
        "PRONOM_FEBLE_HI" => 20, // greater than HAVER_PARTICIPI_HAVER_IMPERSONAL
        "HAVER_PARTICIPI_HAVER_IMPERSONAL" => 15, // greater than ACCENTUATION_CHECK
        "SE_LI_VA_FER_CALLAR" => 15,
        "CA_REMOTE_RULE" => 15,
        "CONCORDANCES_NUMERALS_DUES" => 10, // greater than CONCORDANCES_NUMERALS
        "POSTULARSE" => 10,
        "FALTA_CONDICIONAL" => 10, // greater than POTSER_SIGUI
        "ACCENTUATION_CHECK" => 10,
        "CONCORDANCA_GRIS" => 10,
        "SELS_EN_VA_DE_LES_MANS" => 10,
        "A_PER" => 10,
        "CONCORDANCES_NUMERALS" => 10,
        "COMMA_IJ" => 10,
        "AVIS" => 10,
        "CAP_ELS_CAP_ALS" => 10, // greater than DET_GN
        "CASING" => 10,          // greater than CONCORDANCES_DET_NOM
        "DOS_ARTICLES" => 10,    // greater than apostrophation rules
        "MOTS_GUIONET" => 10,    // greater than CONCORDANCES_DET_NOM
        "SELS_EN_VA" => 10,
        "RECENT" => 10,
        "CONCORDANCES_NOUNS_PRIORITY" => 10,
        "PREFIXOS_SENSE_GUIONET_EN_DICCIONARI" => 10, // greater than SPELLING
        "ZERO_O" => 10,                               // greater than SPELLING
        "URL" => 10,                                  // greater than SPELLING
        "EL_FAN_AGENOLLAR" => 10,                     // greater than PRONOMS_FEBLES_DUPLICATS
        "CONCORDANCES_DET_NOM" => 5,                  // greater than DE_EL_S_APOSTROFEN
        "CONCORDANCES_DET_ADJ" => 5,                  // greater than DE_EL_S_APOSTROFEN
        "CONCORDANCES_DET_POSSESSIU" => 5,            // greater than CONCORDANCES_ADJECTIUS_NEUTRES
        "DET_GN" => 5,                                // greater than DE_EL_S_APOSTROFEN
        "SPELLING" => 5,
        "APOSTROF_ANYS" => 5, // greater than typography options
        "VENIR_NO_REFLEXIU" => 5,
        "DEUS_SEUS" => 5,
        "SON_BONIC" => 5,
        "ACCENTUACIO" => 5,
        "FIDEUA" => 5, // la cremà
        "L_NO_APOSTROFA" => 5,
        "EN_NO_INFINITIU_CAUSAL_REMOTE" => 5,
        "L_D_N_NO_S_APOSTROFEN" => 5,
        "AMB_EM" => 5,
        "CONTRACCIONS" => 0, // lesser than apostrophations
        "CASING_START" => -5,
        "CA_WORD_COHERENCY" => -10, // lesser than EVITA_DEMOSTRATIUS_ESTE
        "CA_WORD_COHERENCY_VALENCIA" => -10, // lesser than EVITA_DEMOSTRATIUS_ESTE
        "QUAN_PREPOSICIO" => -10,   // lesser than QUANT_MES_MES
        "ARTICLE_TOPONIM_MIN" => -10, // lesser than CONTRACCIONS, CONCORDANCES_DET_NOM
        "PEL_QUE" => -10,           // lesser than PEL_QUE_FA
        "COMMA_LOCUTION" => -10,
        "REGIONAL_VERBS" => -10,
        "UN_ALTRE_DISTRIBUTIVES" => -10, // no suggestions
        "PRONOMS_FEBLES_SOLTS" => -10,   // lesser than SPELLING
        "CONCORDANCA_PRONOMS_CATCHALL" => -10,
        "AGREEMENT_POSTPONED_ADJ" => -15,
        "FALTA_COMA_FRASE_CONDICIONAL" => -20,
        "ESPAIS_QUE_FALTEN_PUNTUACIO" => -20,
        "VERBS_NOMSPROPIS" => -20,
        "VERBS_PRONOMINALS" => -25,
        "PORTO_LLEGINT" => -30,
        "PORTA_UNA_HORA" => -40,
        "MAJOR_MES_GRAN0" => -40, // higher than MAJOR_MES_GRAN (style, -50)
        "REPETITIONS_STYLE" => -50,
        "MUNDAR" => -50,
        "NOMBRES_ROMANS" => -90,
        "TASCAS_TASQUES" => -97,
        "PREPOSICIONS_MINUSCULA" => -97, // less than CA_MULTITOKEN_SPELLING
        "SUGGERIMENTS_LE" => -97,        // less than CA_MULTITOKEN_SPELLING
        "MORFOLOGIK_RULE_CA_ES" => -100,
        "EXIGEIX_ACCENTUACIO_VALENCIANA" => -120,
        // "APOSTROFACIO_MOT_DESCONEGUT" => -120, // commented out in the legacy engine
        "PHRASE_REPETITION" => -150,
        "SUBSTANTIUS_JUNTS" => -150,
        "REPETITION_ADJ_N_ADJ" => -155,
        "FALTA_ELEMENT_ENTRE_VERBS" => -200,
        "PUNT_FINAL" => -200,
        "PUNCTUATION_PARAGRAPH_END" => -200,
        "CA_END_PARAGRAPH_PUNCTUATION" => -250,
        "DICENDI_QUE" => -250,
        "UPPERCASE_SENTENCE_START" => -300,
        "MAJUSCULA_IMPROBABLE" => -300,
        "ELA_GEMINADA_WIKI" => -300,
        "CA_SPLIT_LONG_SENTENCE" => -90,
        _ => 0,
    }
}

/// `Catalan.getPriorityForId`'s prefix rules (checked in Java's order).
fn prefix_priority(id: &str) -> Option<i32> {
    if id.starts_with("CA_MULTITOKEN_SPELLING") {
        return Some(-95);
    }
    if id.starts_with("CA_SIMPLE_REPLACE_MULTIWORDS") {
        return Some(70);
    }
    if id.starts_with("CA_SIMPLE_REPLACE_ANGLICISM") {
        return Some(65); // greater than CA_SIMPLE_REPLACE_BALEARIC
    }
    if id.starts_with("CA_SIMPLE_REPLACE_BALEARIC") {
        return Some(60);
    }
    if id.starts_with("CA_SIMPLE_REPLACE_VERBS") {
        return Some(28);
    }
    if id.starts_with("CA_COMPOUNDS") {
        return Some(50);
    }
    if id.starts_with("CA_SIMPLE_REPLACE_DIACRITICS_IEC") {
        return Some(0);
    }
    if id.starts_with("CA_SIMPLE_REPLACE") {
        return Some(30);
    }
    None
}

/// Base `Language.getPriorityForId` defaults (Catalan falls back to them).
fn base_priority(id: &str) -> i32 {
    if id.eq_ignore_ascii_case("TOO_LONG_SENTENCE") {
        return -101; // don't hide spelling errors
    }
    if id == "REPETITIONS_STYLE" {
        return -55; // don't let style issues hide more important errors
    }
    if id.contains("STYLE") {
        return -50;
    }
    0
}

/// `Catalan.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    if let Some(prio) = prefix_priority(id) {
        return prio;
    }
    base_priority(id)
}

/// `Language.getRulePriority` for Catalan
/// (`getDefaultRulePriorityForStyle` = -50).
pub fn rule_priority(
    rule_id: &str,
    category_id: &str,
    issue_type: &str,
    rule_priority: i32,
) -> i32 {
    let rule_specific = priority_for_id(rule_id);
    if rule_specific != 0 {
        return rule_specific;
    }
    if rule_priority != 0 {
        return rule_priority;
    }
    let category = priority_for_id(category_id);
    if category != 0 {
        return category;
    }
    if issue_type.eq_ignore_ascii_case("style") {
        return -50;
    }
    0
}
