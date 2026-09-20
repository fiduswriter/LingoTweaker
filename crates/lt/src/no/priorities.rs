//! Norwegian Bokmål `getPriorityForId` for `CleanOverlappingFilter`.
//!
//! Hand-authored language: specific rules (with suggestions) must win over
//! the generic spelling rule, which therefore gets a low priority. The base
//! LanguageTool defaults for style rules are kept.

pub fn priority_for_id(id: &str) -> i32 {
    if id.eq_ignore_ascii_case("TOO_LONG_SENTENCE") {
        return -101;
    }
    if id == "REPETITIONS_STYLE" {
        return -55;
    }
    if id.contains("STYLE") {
        return -50;
    }
    if id.ends_with("_SPELLER") {
        return -1000;
    }
    0
}

/// `Language.getRulePriority` (base style default 0).
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
    priority_for_id(category_id)
}
