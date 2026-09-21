//! `Belarusian.getPriorityForId` (the `id2prio` switch) and the base
//! `Language.getPriorityForId` fallback for `CleanOverlappingFilter`.
//!
//! The legacy switch lists `RUSSIAN_SIMPLE_REPLACE_RULE` (10) and
//! `BELARUSIAN_SPECIFIC_CASE` (9), but the actual Belarusian rule classes
//! return `BE_SIMPLE_REPLACE` and `BE_SPECIFIC_CASE`, so those two entries are
//! dead in the legacy engine and are kept here verbatim for parity. The
//! effective overrides are `Word_root_repeat` (`ParagraphRepeatBeginningRule`,
//! -1), `PUNCT_DPT_2` (`PunctuationMarkAtParagraphEnd2`, -2) and
//! `TOO_LONG_PARAGRAPH` (`LongParagraphRule`, -15).
//!
//! `Belarusian` does not override `getDefaultRulePriorityForStyle`, so the
//! style fallback is 0.

/// `Belarusian.getPriorityForId`'s exact-id switch.
fn table_priority(id: &str) -> i32 {
    match id {
        // Dead entries: the legacy switch targets the Russian-based ids, not
        // the ids the Belarusian rule classes actually return.
        "RUSSIAN_SIMPLE_REPLACE_RULE" => 10,
        "BELARUSIAN_SPECIFIC_CASE" => 9,
        "Word_root_repeat" => -1,
        "PUNCT_DPT_2" => -2,
        "TOO_LONG_PARAGRAPH" => -15,
        _ => 0,
    }
}

/// Base `Language.getPriorityForId` defaults (Belarusian falls back to them).
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

/// `Belarusian.getPriorityForId`.
pub fn priority_for_id(id: &str) -> i32 {
    let prio = table_priority(id);
    if prio != 0 {
        return prio;
    }
    base_priority(id)
}

/// `Language.getRulePriority` for Belarusian (`getDefaultRulePriorityForStyle`
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
