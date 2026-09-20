//! Small helpers shared by the German rule ports.

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings};
use lt_pattern::matcher as pm;
use std::sync::Arc;

/// `AnalyzedTokenReadings.isPosTagUnknown`: the flag snapshotted when the
/// tagger built the token (Java keeps it across disambiguation; the port
/// stores it on the token, see `AnalyzedTokenReadings::is_pos_tag_unknown`).
pub(crate) fn is_pos_tag_unknown(token: &AnalyzedTokenReadings) -> bool {
    token.is_pos_tag_unknown
}

/// `AnalyzedTokenReadings.hasPartialPosTag`: any POS tag containing `tag`.
pub(crate) fn has_partial_pos_tag(token: &AnalyzedTokenReadings, tag: &str) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(tag)))
}

/// `AnalyzedTokenReadings.matchesPosTagRegex` (full match per reading).
pub(crate) fn matches_pos_tag_regex(token: &AnalyzedTokenReadings, pattern: &regex::Regex) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| pattern.is_match(t)))
}

/// Full-match anchor for the Java regexes used with `matchesPosTagRegex`
/// (`Pattern.matcher(...).matches()`).
pub(crate) fn anchored(pattern: &str) -> regex::Regex {
    regex::Regex::new(&format!("^(?:{pattern})$")).expect("rule regex")
}

/// `new HashSet<>(list)` iteration order (`new ArrayList<>(set)` in Java):
/// Java's `HashMap` table order for the given insertion sequence. Used where
/// a Java rule builds a `HashSet<String>` and iterates it (verb agreement
/// suggestions); the order is observable in `RuleMatch` suggestions.
pub(crate) fn java_hash_set_order(items: &[String]) -> Vec<String> {
    fn java_string_hash(s: &str) -> i32 {
        s.encode_utf16()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
    }
    // `new HashSet<>(collection)`: initial capacity from the collection size,
    // rounded up to a power of two; resized while unique elements exceed 0.75
    // of the capacity.
    let mut capacity = ((items.len() as f64 / 0.75) as usize + 1).max(16);
    capacity = capacity.next_power_of_two();
    let mut seen: std::collections::HashSet<&String> = std::collections::HashSet::new();
    let unique: Vec<&String> = items.iter().filter(|i| seen.insert(*i)).collect();
    while unique.len() as f64 > capacity as f64 * 0.75 {
        capacity *= 2;
    }
    let mut buckets: Vec<Vec<String>> = vec![Vec::new(); capacity];
    for item in unique {
        let h = java_string_hash(item);
        let spread = (h ^ ((h as u32 >> 16) as i32)) as u32;
        buckets[(spread as usize) & (capacity - 1)].push(item.clone());
    }
    buckets.into_iter().flatten().collect()
}

/// `StringTools.startsWithUppercase`.
pub(crate) fn starts_with_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
}

/// `List.sort` with the Apache Commons Text default Levenshtein distance
/// (`sortBySimilarity`): stable sort by distance to `marked_text`.
pub(crate) fn sort_by_similarity(suggestions: &mut [String], marked_text: &str) {
    suggestions.sort_by_key(|s| levenshtein(marked_text, s));
}

/// Plain Levenshtein distance over `char`s (Apache Commons Text operates on
/// UTF-16 code units; identical for the BMP inputs seen here).
pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// `Rule.getSentenceWithImmunization`: applies the rule's antipatterns to a
/// copy of the sentence over the non-whitespace token view (each pattern in
/// order; antipatterns do not skip already-immunized tokens, see D-010).
///
/// The copy is only made when an antipattern actually immunizes a token, so
/// the (common) no-antipattern-match case only borrows the sentence.
pub(crate) fn immunize_sentence<'a>(
    sentence: &'a AnalyzedSentence,
    antipatterns: &[Arc<pm::CompiledPattern>],
) -> std::borrow::Cow<'a, AnalyzedSentence> {
    let view = sentence.tokens_without_whitespace();
    let mut immune = vec![false; view.len()];
    for ap in antipatterns {
        if view.is_empty() {
            break;
        }
        for m in pm::find_matches(ap, &[], &view) {
            let last = m.end_tok().min(immune.len() - 1);
            for flag in immune.iter_mut().take(last + 1).skip(m.start_tok()) {
                *flag = true;
            }
        }
    }
    if !immune.iter().any(|x| *x) {
        return std::borrow::Cow::Borrowed(sentence);
    }
    let mut immunized = sentence.clone();
    let mut view_index = 0usize;
    for token in &mut immunized.tokens {
        if !token.is_whitespace
            || token.is_sentence_start
            || token.is_sentence_end
            || token.is_paragraph_end
        {
            if immune.get(view_index).copied().unwrap_or(false) {
                token.is_immunized = true;
            }
            view_index += 1;
        }
    }
    std::borrow::Cow::Owned(immunized)
}
