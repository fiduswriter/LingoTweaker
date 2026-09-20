//! Dictionary-driven nasal-harmony rule for Guaraní (`GN_HARMONY`).
//!
//! Nasal harmony is productive in Guaraní but needs a nasal-feature lexicon
//! to be applied blindly (`johecha` is oral, `johetũ` must become `ñohetũ`).
//! The known word list (`gug.dic` + `ignore.txt`, the same sources as
//! `GN_ACCENTS`) is the oracle instead: for every known word the oral/nasal
//! swapped forms of the productive alternations are indexed
//!
//! * suffix `-pe` ↔ `-me` (postposition),
//! * suffix `-pa` ↔ `-mba`,
//! * suffix `-kuéra` ↔ `-nguéra` (plural),
//! * prefix `jo-` ↔ `ño-`,
//! * prefix `ja-` ↔ `ña-`,
//! * negation `nd-` ↔ `n-` before `-i` endings (conservative: only words
//!   that also end in `-i`),
//!
//! and a token that is unknown but whose swapped form is a known word is
//! reported with the known form (up to three suggestions). This generalizes
//! `GN_ACCENTS` without a full morphological analyzer.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::gn::accents::{read_dictionary, read_word_list};

pub const RULE_ID: &str = "GN_HARMONY";

/// Minimum length in `char`s for both the known word and the swapped variant.
/// Shorter swaps are coincidental (`upe`/`ume`, `ãme`/`ãpe`, `ñai`/`jai`) and
/// are left to the speller.
const MIN_LEN: usize = 4;

/// Oral ↔ nasal suffix pairs (both directions are indexed).
const SUFFIX_PAIRS: &[(&str, &str)] = &[("pe", "me"), ("pa", "mba"), ("kuéra", "nguéra")];

/// Oral ↔ nasal prefix pairs (both directions are indexed).
const PREFIX_PAIRS: &[(&str, &str)] = &[("jo", "ño"), ("ja", "ña")];

pub struct GuaraniHarmonyRule {
    exact: HashSet<String>,
    variants: HashMap<String, Vec<String>>,
}

impl GuaraniHarmonyRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join("gn/hunspell");
        let mut words: Vec<String> = Vec::new();
        read_dictionary(&dir.join("gug.dic"), &mut words);
        read_word_list(&dir.join("ignore.txt"), &mut words);

        let mut exact = HashSet::new();
        for word in &words {
            exact.insert(word.to_lowercase());
        }
        let mut variants: HashMap<String, Vec<String>> = HashMap::new();
        for word in words {
            let lower = word.to_lowercase();
            for variant in swapped_forms(&lower) {
                if exact.contains(&variant) {
                    // Both harmony forms are known: the token is accepted and
                    // there is nothing to suggest.
                    continue;
                }
                variants.entry(variant).or_default().push(word.clone());
            }
        }
        for candidates in variants.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        Ok(Self { exact, variants })
    }

    /// Number of indexed swap keys (test/diagnostic helper).
    pub fn index_size(&self) -> usize {
        self.variants.len()
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut matches = Vec::new();
        for token in tokens {
            if token.is_whitespace
                || token.is_sentence_start
                || token.is_immunized
                || token.is_ignore_spelling
            {
                continue;
            }
            let word = token.surface();
            if !word.chars().any(char::is_alphabetic) {
                continue;
            }
            let lower = word.to_lowercase();
            if self.exact.contains(&lower) {
                continue;
            }
            let Some(candidates) = self.variants.get(&lower) else {
                continue;
            };
            let suggestions: Vec<Suggestion> = candidates
                .iter()
                .filter(|candidate| candidate.to_lowercase() != lower)
                .take(3)
                .map(|candidate| Suggestion {
                    value: match_case(word, candidate),
                    short_description: None,
                })
                .collect();
            if suggestions.is_empty() {
                continue;
            }
            matches.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    "Nasal harmony: the stem selects the nasal or oral form.",
                    Some("Nasal harmony".to_string()),
                    TextRange::new(
                        sentence_offset + token.start_pos,
                        sentence_offset + token.end_pos(),
                    ),
                    suggestions,
                    "TYPOS",
                    "Possible Typo",
                )
                .with_metadata("Nasal harmony (dictionary-driven)", "misspelling", 0)
                .with_match_type("UnknownWord"),
            );
        }
        matches
    }
}

/// The oral/nasal swapped forms of a known word, in both directions and in
/// the same case as the word (the caller lowercases first).
fn swapped_forms(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    if char_len(word) < MIN_LEN {
        return out;
    }
    for (oral, nasal) in SUFFIX_PAIRS {
        for (from, to) in [(oral, nasal), (nasal, oral)] {
            if let Some(stem) = word.strip_suffix(from) {
                push_variant(&mut out, format!("{stem}{to}"));
            }
        }
    }
    for (oral, nasal) in PREFIX_PAIRS {
        for (from, to) in [(oral, nasal), (nasal, oral)] {
            if let Some(rest) = word.strip_prefix(from) {
                push_variant(&mut out, format!("{to}{rest}"));
            }
        }
    }
    // Negation: `nd-` before oral stems, `n-` before nasal stems. Only words
    // that also carry the final `-i` are swapped, keeping this conservative.
    if word.ends_with('i') {
        if let Some(rest) = word.strip_prefix("nd") {
            push_variant(&mut out, format!("n{rest}"));
        } else if let Some(rest) = word.strip_prefix('n') {
            push_variant(&mut out, format!("nd{rest}"));
        }
    }
    out
}

fn push_variant(out: &mut Vec<String>, variant: String) {
    if char_len(&variant) >= MIN_LEN {
        out.push(variant);
    }
}

fn char_len(word: &str) -> usize {
    word.chars().count()
}

/// Keep the capitalization of the input for the suggestion.
fn match_case(original: &str, candidate: &str) -> String {
    if original.chars().next().is_some_and(char::is_uppercase) {
        let mut chars = candidate.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    } else {
        candidate.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn swapped_forms_cover_the_alternations() {
        assert!(swapped_forms("ñúme").contains(&"ñúpe".to_string()));
        assert!(swapped_forms("ñúpe").contains(&"ñúme".to_string()));
        assert!(swapped_forms("osẽmba").contains(&"osẽpa".to_string()));
        assert!(swapped_forms("ohopa").contains(&"ohomba".to_string()));
        assert!(swapped_forms("mitãnguéra").contains(&"mitãkuéra".to_string()));
        assert!(swapped_forms("ñohetũ").contains(&"johetũ".to_string()));
        assert!(swapped_forms("johecha").contains(&"ñohecha".to_string()));
        assert!(swapped_forms("ñamba'apo").contains(&"jamba'apo".to_string()));
        assert!(swapped_forms("ndojapói").contains(&"nojapói".to_string()));
        assert!(swapped_forms("noñaníi").contains(&"ndoñaníi".to_string()));
    }

    #[test]
    fn short_words_are_not_indexed() {
        assert!(swapped_forms("upe").is_empty());
        assert!(swapped_forms("ãme").is_empty());
        assert!(swapped_forms("jai").is_empty());
    }

    #[test]
    fn harmony_rule_finds_dictionary_candidates() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !path.lt_is_dir() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let rule = GuaraniHarmonyRule::load(&path).expect("rule loads");
        let cases: &[(&str, &str)] = &[
            ("ñúpe", "ñúme"),
            ("johetũ", "ñohetũ"),
            ("osẽpa", "osẽmba"),
            ("mitãkuéra", "mitãnguéra"),
            ("jamba'apo", "ñamba'apo"),
        ];
        for (unknown, known) in cases {
            let candidates = rule
                .variants
                .get(*unknown)
                .unwrap_or_else(|| panic!("no index entry for {unknown}"));
            assert!(
                candidates.iter().any(|w| w == known),
                "{unknown} must map to {known}, got {candidates:?}"
            );
        }
        for text in ["ñúme", "johecha", "ohopa", "mitãnguéra", "jakaru"] {
            assert!(rule.exact.contains(text), "{text} must be a known word");
        }
        assert!(
            rule.index_size() > 300,
            "index too small: {}",
            rule.index_size()
        );
    }
}
