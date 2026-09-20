//! `Polish.getPriorityForId` (the `id2prio` switch) and the base
//! `Language.getPriorityForId` fallback for `CleanOverlappingFilter`.
//!
//! `Polish` does not override `getDefaultRulePriorityForStyle`, so the style
//! fallback is 0.

/// `Polish.getPriorityForId`'s exact-id switch.
fn table_priority(id: &str) -> i32 {
    match id {
        // "so that it does not override more important rules"
        "ZDANIA_ZLOZONE" => -1,
        _ => 0,
    }
}

/// Base `Language.getPriorityForId` defaults (Polish falls back to them).
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

/// `Polish.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    base_priority(id)
}

/// `Language.getRulePriority` for Polish (`getDefaultRulePriorityForStyle`
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
