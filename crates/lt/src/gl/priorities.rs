//! `Galician.getPriorityForId` (the `id2prio` switch) and the base
//! `Language.getPriorityForId` fallback for `CleanOverlappingFilter`.
//!
//! `Galician` does not override `getDefaultRulePriorityForStyle`, so the
//! style fallback is 0.

/// `Galician.getPriorityForId`'s exact-id switch.
fn table_priority(id: &str) -> i32 {
    match id {
        "DEGREE_MINUTES_SECONDS" => 30,
        "UNPAIRED_BRACKETS" => -5,
        "GL_BARBARISM_REPLACE" => -10,
        "GL_SIMPLE_REPLACE" => -11,
        "GL_REDUNDANCY_REPLACE" => -12,
        "GL_WORDINESS_REPLACE" => -13,
        "TOO_LONG_PARAGRAPH" => -15,
        "GL_WIKIPEDIA_COMMON_ERRORS" => -45,
        "HUNSPELL_RULE" => -50,
        "REPEATED_WORDS" => -210,
        "REPEATED_WORDS_3X" => -211,
        "TOO_LONG_SENTENCE_20" => -997,
        "TOO_LONG_SENTENCE_25" => -998,
        "TOO_LONG_SENTENCE_30" => -999,
        "TOO_LONG_SENTENCE_35" => -1000,
        "TOO_LONG_SENTENCE_40" => -1001,
        "TOO_LONG_SENTENCE_45" => -1002,
        "TOO_LONG_SENTENCE_50" => -1003,
        "TOO_LONG_SENTENCE_60" => -1004,
        _ => 0,
    }
}

/// Base `Language.getPriorityForId` defaults (Galician falls back to them).
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

/// `Galician.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    base_priority(id)
}

/// `Language.getRulePriority` for Galician (`getDefaultRulePriorityForStyle`
/// is not overridden, so the style fallback is 0).
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
    let _ = issue_type;
    0
}
