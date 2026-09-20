//! Dictionary-driven accent/tilde restoration for Guaraní (`GN_ACCENTS`).
//!
//! The `gug.dic` word list is indexed both as written and with the
//! diacritics stripped; a token that is not a known word but matches a known
//! word after removing the accents (`ara` → `ára`, `mokói` → `mokõi`,
//! `tupa` → `tupã`) is reported with the dictionary spelling as suggestion.
//! This implements the accent/nasal-tilde part of the ALG orthography rules
//! without a full morphological analyzer.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

pub const RULE_ID: &str = "GN_ACCENTS";

pub struct GuaraniAccentRule {
    exact: HashSet<String>,
    stripped: HashMap<String, Vec<String>>,
}

impl GuaraniAccentRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join("gn/hunspell");
        let mut words: Vec<String> = Vec::new();
        read_dictionary(&dir.join("gug.dic"), &mut words);
        read_word_list(&dir.join("ignore.txt"), &mut words);

        let mut exact = HashSet::new();
        let mut stripped: HashMap<String, Vec<String>> = HashMap::new();
        for word in words {
            let lower = word.to_lowercase();
            exact.insert(lower.clone());
            stripped
                .entry(strip_accents(&lower))
                .or_default()
                .push(word);
        }
        for candidates in stripped.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        Ok(Self { exact, stripped })
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
            let Some(candidates) = self.stripped.get(&strip_accents(&lower)) else {
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
                    "Falta la tilde o el acento ortográfico.",
                    Some("Acento".to_string()),
                    TextRange::new(
                        sentence_offset + token.start_pos,
                        sentence_offset + token.end_pos(),
                    ),
                    suggestions,
                    "TYPOS",
                    "Possible Typo",
                )
                .with_metadata("Acento o tilde faltante/incorrecto", "misspelling", 0)
                .with_match_type("UnknownWord"),
            );
        }
        matches
    }
}

/// Remove the Guaraní diacritics (acute accents, nasal tildes, the combining
/// tilde of `g̃`) and normalize the puso variants, keeping `ñ` distinct.
fn strip_accents(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for ch in word.chars() {
        match ch {
            'á' | 'ã' => out.push('a'),
            'é' | 'ẽ' => out.push('e'),
            'í' | 'ĩ' => out.push('i'),
            'ó' | 'õ' => out.push('o'),
            'ú' | 'ũ' => out.push('u'),
            'ý' | 'ỹ' => out.push('y'),
            '\u{0303}' => {}
            '\u{2019}' | '\u{02BC}' => out.push('\''),
            other => out.push(other),
        }
    }
    out
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

/// Read a Hunspell-style flat dictionary (`word[/flags]`, optional tab
/// commentary); shared with [`crate::gn::context`].
pub(crate) fn read_dictionary(path: &Path, out: &mut Vec<String>) {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines().skip(1) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let word = line.split('\t').next().unwrap_or("");
        let word = word.split('/').next().unwrap_or("").trim();
        if !word.is_empty() {
            out.push(word.to_string());
        }
    }
}

/// Read a plain word list (one word per line, `#` comments); shared with
/// [`crate::gn::context`].
pub(crate) fn read_word_list(path: &Path, out: &mut Vec<String>) {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if !line.is_empty() {
            out.push(line.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn strip_accents_handles_guarani_diacritics() {
        assert_eq!(strip_accents("ára"), "ara");
        assert_eq!(strip_accents("mokõi"), "mokoi");
        assert_eq!(strip_accents("g̃uahẽ"), "guahe");
        assert_eq!(strip_accents("ñe'ẽ"), "ñe'e");
    }

    #[test]
    fn accent_rule_finds_dictionary_candidates() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !path.lt_is_dir() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let rule = GuaraniAccentRule::load(&path).expect("rule loads");
        assert!(rule.exact.contains("ára"), "ára must be in gug.dic");
        assert!(!rule.exact.contains("ara"));
        let candidates = rule.stripped.get("ara");
        assert!(
            candidates.is_some_and(|list| list.iter().any(|w| w == "ára")),
            "ara must map to ára, got {candidates:?}"
        );
    }
}
