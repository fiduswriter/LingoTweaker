//! `Russian.getPriorityForId` (the `id2prio` switch) and the base
//! `Language.getPriorityForId` fallback for `CleanOverlappingFilter`.
//!
//! The legacy switch spells the speller ids `MORFOLOGIC_RULE_RU_RU(_YO)`
//! (without the `K`), while the rule classes return
//! `MORFOLOGIK_RULE_RU_RU(_YO)`, so those two entries are dead in the legacy
//! engine and are kept here verbatim for parity.

/// `Russian.getPriorityForId`'s exact-id switch.
fn table_priority(id: &str) -> i32 {
    match id {
        "RU_DASH_RULE" => 12,
        "RU_COMPOUNDS" => 11,
        "RUSSIAN_SIMPLE_REPLACE_RULE" => 10,
        "RUSSIAN_SPECIFIC_CASE" => 9,
        // Dead spell-id spellings (the real ids carry the `K`).
        "MORFOLOGIC_RULE_RU_RU_YO" => 2,
        "MORFOLOGIC_RULE_RU_RU" => 1,
        "Word_root_repeat" => -1,
        "PUNCT_DPT_2" => -2,
        "TOO_LONG_PARAGRAPH" => -15,
        _ => 0,
    }
}

/// Base `Language.getPriorityForId` defaults (Russian falls back to them).
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

/// `Russian.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    base_priority(id)
}

/// `Language.getRulePriority` for Russian.
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
