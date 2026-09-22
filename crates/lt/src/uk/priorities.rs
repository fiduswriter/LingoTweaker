//! `Ukrainian` has no `getPriorityForId` override, so `CleanOverlappingFilter`
//! falls back to the base `Language.getPriorityForId` defaults. Notably
//! Ukrainian does **not** override `getDefaultRulePriorityForStyle`, so the
//! `ITSIssueType.Style` penalty that the English table applies does not apply
//! here.

/// Base `Language.getPriorityForId` defaults (Ukrainian falls back to them).
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

/// `Language.getRulePriority` for Ukrainian (`getPriorityForId` +
/// `rule.getPriority()` + category priority; no style penalty).
pub fn rule_priority(
    rule_id: &str,
    category_id: &str,
    _issue_type: &str,
    rule_priority: i32,
) -> i32 {
    let rule_specific = base_priority(rule_id);
    if rule_specific != 0 {
        return rule_specific;
    }
    if rule_priority != 0 {
        return rule_priority;
    }
    base_priority(category_id)
}
