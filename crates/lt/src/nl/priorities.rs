//! `Dutch.getPriorityForId` (`id2prio` map + the prefix rules) for
//! `CleanOverlappingFilter`.

/// `Dutch.id2prio`.
fn table_priority(id: &str) -> i32 {
    match id {
        "TOO_LONG_SENTENCE" => -1,
        "SINT_X" => 3,                         // higher than simple replace
        "ET_AL" => 1,                          // needs higher priority than MORFOLOGIK_RULE_NL_NL
        "N_PERSOONS" => 1,                     // needs higher priority than MORFOLOGIK_RULE_NL_NL
        "HOOFDLETTERS_OVERBODIG_A" => 1,       // needs higher priority than MORFOLOGIK_RULE_NL_NL
        "VERSCHILLENDE_AANHALINGSTEKENS" => 1, // needs higher priority than UNPAIRED_BRACKETS
        "STAM_ZONDER_IK" => -1,
        "KOMMA_ONTBR" => -1,
        "KOMMA_KOMMA" => -1,  // needs higher priority than DOUBLE_PUNCTUATION
        "HET_FIETS" => -2,    // first let other rules check for compound words
        "JIJ_JOU_JOUW" => -2, // needs higher priority than JOU_JOUW
        "JOU_JOUW" => -3,
        "BE" => -3, // needs lower priority than BE_GE_SPLITST
        "DOUBLE_PUNCTUATION" => -3,
        "EINDE_ZIN_ONVERWACHT" => -5, // so that spelling errors are recognized first
        "TOO_LONG_PARAGRAPH" => -15,
        "ERG_LANG_WOORD" => -20, // below spell checker and simple replace rule
        "DE_ONVERWACHT" => -20,  // below spell checker and simple replace rule
        _ => 0,
    }
}

/// Base `Language.getPriorityForId` defaults (Dutch falls back to them).
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

/// `Dutch.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    if id.starts_with("NL_SIMPLE_REPLACE") || id.starts_with("NL_SPACE_IN_COMPOUND") {
        return 1;
    }
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    if id.starts_with("AI_NL_HYDRA_LEO") {
        if id.starts_with("AI_NL_HYDRA_LEO_MISSING_COMMA") {
            return -51; // prefer comma style rules
        }
        return -5;
    }
    base_priority(id)
}

/// `Language.getRulePriority` for Dutch (base style default 0).
pub fn rule_priority(
    rule_id: &str,
    category_id: &str,
    _issue_type: &str,
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
    0
}
