//! Shared helpers for the per-language integration tests.
//!
//! The Java oracle (`scripts/oracle/<cc>/probe-rule.sh`,
//! `check-diff-<cc>.sh`) prints UTF-16 code-unit offsets; the engine works in
//! UTF-8 bytes. These helpers convert an engine range to the Java format so a
//! probe expectation can be asserted directly, for any text, ASCII or not.
//! A test should therefore never state both offset systems or pick ASCII-only
//! sentences to dodge the difference.
#![allow(dead_code)]

/// Engine [`lt::TextRange`] (UTF-8 bytes) as UTF-16 code-unit offsets, i.e.
/// the format the Java probes print.
pub fn utf16(text: &str, range: lt::TextRange) -> (usize, usize) {
    lt::to_utf16_range(text, range)
}

/// Assert that `m` covers `expected` in UTF-16 code units for `text`.
pub fn assert_utf16(text: &str, m: &lt::Match, expected: (usize, usize)) {
    assert_eq!(utf16(text, m.range), expected, "match {m:?}");
}

/// Assert that `range` covers `expected` in UTF-16 code units for `text`
/// (for matches reduced to tuples, e.g. long-paragraph probes).
pub fn assert_utf16_range(text: &str, range: lt::TextRange, expected: (usize, usize)) {
    assert_eq!(utf16(text, range), expected, "range {range:?}");
}
