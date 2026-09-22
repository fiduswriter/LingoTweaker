//! Ports of `JLanguageTool.check`'s match post-processing:
//! `SameRuleGroupFilter`, English's `filterRuleMatches` (the
//! `LanguageDependentRuleMatchFilter` step) and `CleanOverlappingFilter`.
//!
//! Order matches Java: same-rule-group collapse (stable sort by start
//! position), then English suggestion space/dedupe handling, then overlap
//! removal by rule priority.

use lt_core::Match;

use crate::en::priorities::included_in_all_at_once;
use crate::Lang;

/// `SameRuleGroupFilter`: collapse overlapping matches that share a rule id
/// (keeps the first in start-position order).
pub fn same_rule_group_filter(mut matches: Vec<Match>) -> Vec<Match> {
    // `Collections.sort(ruleMatches)` orders by fromPos only (stable).
    matches.sort_by_key(|m| m.range.start);
    let mut filtered = Vec::with_capacity(matches.len());
    let mut i = 0usize;
    while i < matches.len() {
        let current = matches[i].clone();
        while i < matches.len() - 1 && overlap_and_same_id(&current, &matches[i + 1]) {
            i += 1;
        }
        filtered.push(current);
        i += 1;
    }
    filtered
}

fn overlap_and_same_id(a: &Match, b: &Match) -> bool {
    a.range.start <= b.range.end && a.range.end >= b.range.start && a.rule_id == b.rule_id
}

/// English `filterRuleMatches`: prefix a space to suggestions for contraction
/// errors, then drop duplicate (replacement, short description) pairs.
pub fn english_filter_rule_matches(matches: Vec<Match>, text: &str) -> Vec<Match> {
    matches
        .into_iter()
        .map(|mut m| {
            let error = text.get(m.range.start..m.range.end).unwrap_or("");
            let mut new_suggestions = Vec::with_capacity(m.suggestions.len());
            for suggestion in &m.suggestions {
                let mut replacement = suggestion.value.clone();
                if error.chars().count() > 2 {
                    // add a whitespace when the error is in a contraction and
                    // the suggestion is not
                    if error.starts_with('\'')
                        && !replacement.starts_with('\'')
                        && !replacement.starts_with('’')
                        && !replacement.starts_with(' ')
                    {
                        replacement.insert(0, ' ');
                    }
                    if error.starts_with("n't")
                        && !replacement.starts_with("n't")
                        && !replacement.starts_with("n’t")
                    {
                        replacement.insert(0, ' ');
                    }
                }
                let new = lt_core::Suggestion {
                    value: replacement,
                    short_description: suggestion.short_description.clone(),
                };
                if !new_suggestions.contains(&new) {
                    new_suggestions.push(new);
                }
            }
            m.suggestions = new_suggestions;
            m
        })
        .collect()
}

/// `CleanOverlappingFilter`: remove overlapping errors according to the
/// priorities established for the language. Assumes the input list is
/// ordered by start position.
pub fn clean_overlapping_filter(
    matches: Vec<Match>,
    text: &str,
    lang: Lang,
    variant: Option<&str>,
) -> Vec<Match> {
    let mut clean_list: Vec<Match> = Vec::with_capacity(matches.len());
    let mut prev: Option<Match> = None;
    for rule_match in matches {
        let Some(previous) = prev.take() else {
            prev = Some(rule_match);
            continue;
        };
        debug_assert!(
            rule_match.range.start >= previous.range.start,
            "CleanOverlappingFilter expects matches ordered by start position"
        );

        let mut is_duplicate_suggestion = false;
        if !rule_match.suggestions.is_empty() && !previous.suggestions.is_empty() {
            let suggestion = &rule_match.suggestions[0].value;
            let prev_suggestion = &previous.suggestions[0].value;
            // juxtaposed errors adding a comma in the same place
            if rule_match.range.start == previous.range.end
                && prev_suggestion.ends_with(',')
                && suggestion.starts_with(", ")
            {
                is_duplicate_suggestion = true;
            }
            // duplicate suggestion for the same position
            if suggestion.contains(' ')
                && prev_suggestion.contains(' ')
                && rule_match.range.start == previous.range.end + 1
            {
                // Java `String.split` drops trailing empty strings
                let parts = java_split_space(suggestion);
                let parts_prev = java_split_space(prev_suggestion);
                if parts_prev.len() > 1 && parts.len() > 1 && parts_prev[1] == parts[0] {
                    is_duplicate_suggestion = true;
                }
            }
        }

        // no overlapping (juxtaposed errors are not removed)
        if rule_match.range.start >= previous.range.end && !is_duplicate_suggestion {
            clean_list.push(previous);
            prev = Some(rule_match);
            continue;
        }

        // overlapping
        let mut current_priority = priority(&rule_match, lang, variant);
        let mut previous_priority = priority(&previous, lang, variant);
        // `CleanOverlappingFilter`: picky matches get the
        // `Integer.MIN_VALUE + 10000` penalty (Java int wrapping)
        const NEGATIVE_CONSTANT: i32 = i32::MIN + 10000;
        if rule_match.picky {
            current_priority = current_priority.wrapping_add(NEGATIVE_CONSTANT);
        }
        if previous.picky {
            previous_priority = previous_priority.wrapping_add(NEGATIVE_CONSTANT);
        }
        // If both matches only change punctuation, prefer the rule that
        // participates in "correct all errors at once".
        let current_is_punctuation_only = is_punctuation_only_change(&rule_match, text);
        let previous_is_punctuation_only = is_punctuation_only_change(&previous, text);
        if current_is_punctuation_only && previous_is_punctuation_only {
            let current_all_at_once = included_in_all_at_once(&rule_match.rule_id);
            let previous_all_at_once = included_in_all_at_once(&previous.rule_id);
            if current_all_at_once != previous_all_at_once {
                if current_all_at_once {
                    if current_priority < previous_priority {
                        current_priority = previous_priority + 1;
                    }
                } else if previous_priority < current_priority {
                    previous_priority = current_priority + 1;
                }
            }
        }
        if current_priority == previous_priority {
            // take the longest error — Java compares `toPos - fromPos`, i.e.
            // UTF-16 code units, not UTF-8 bytes (D-118)
            current_priority = utf16_len(text, &rule_match);
            previous_priority = utf16_len(text, &previous);
        }
        if current_priority == previous_priority {
            // take the last one (to keep the current results in the web UI)
            current_priority += 1;
        }
        prev = Some(if current_priority > previous_priority {
            rule_match
        } else {
            previous
        });
    }
    // last match
    if let Some(last) = prev {
        clean_list.push(last);
    }
    clean_list
}

/// `String.split(" ")` (trailing empty strings removed).
fn java_split_space(s: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split(' ').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Length of a match in UTF-16 code units, like Java's
/// `RuleMatch.getToPos() - RuleMatch.getFromPos()`.
fn utf16_len(text: &str, m: &Match) -> i32 {
    text.get(m.range.start..m.range.end)
        .map_or((m.range.end - m.range.start) as i32, |s| {
            s.encode_utf16().count() as i32
        })
}

/// `CleanOverlappingFilter`'s `getRulePriority`: the language's table
/// (English `id2prio` for everything not yet ported, German `id2prio` for
/// `de-*`).
fn priority(m: &Match, lang: Lang, variant: Option<&str>) -> i32 {
    match lang {
        Lang::De => {
            crate::de::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Es => {
            crate::es::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Fr => {
            crate::fr::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::It => {
            crate::it::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Pt => crate::pt::priorities::rule_priority_variant(
            &m.rule_id,
            &m.category_id,
            &m.issue_type,
            0,
            variant,
        ),
        Lang::Nl => {
            crate::nl::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Ca => {
            crate::ca::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Gl => {
            crate::gl::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Pl => {
            crate::pl::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::No => {
            crate::no::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Nrd => {
            crate::nrd::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Gn => {
            crate::gn::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Be => {
            crate::be::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Ru => {
            crate::ru::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        Lang::Uk => {
            crate::uk::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0)
        }
        // Arabic overrides neither `getPriorityForId` nor
        // `getDefaultRulePriorityForStyle` and has no `priority` attributes, so
        // Java's `Language.getRulePriority` returns 0 for every rule. Do not
        // fall back to the English style penalty (D-271).
        Lang::Ar => 0,
        // Persian also overrides neither `getPriorityForId` nor
        // `getDefaultRulePriorityForStyle` and has no `priority` attributes,
        // so Java's `Language.getRulePriority` returns 0 for every rule.
        Lang::Fa => 0,
        _ => crate::en::priorities::rule_priority(&m.rule_id, &m.category_id, &m.issue_type, 0),
    }
}

/// `CleanOverlappingFilter.isPunctuationOnlyChange`.
fn is_punctuation_only_change(m: &Match, text: &str) -> bool {
    let Some(replacement) = m.suggestions.first() else {
        return false;
    };
    let Some(original) = text.get(m.range.start..m.range.end) else {
        return false;
    };
    if original.is_empty() {
        return false;
    }
    if replacement.value == original {
        return false;
    }
    keep_letters_and_digits(original) == keep_letters_and_digits(&replacement.value)
}

fn keep_letters_and_digits(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
}
