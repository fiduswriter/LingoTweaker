//! French `AbstractSimpleReplaceRule` subclass (`SimpleReplaceRule`): the
//! single-token legacy base class (see `crate::simple_replace` for the
//! `AbstractSimpleReplaceRule2` family).

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

pub const SIMPLE_REPLACE_RULE_ID: &str = "FR_SIMPLE_REPLACE_SIMPLE";
const SHORT: &str = "Mot incorrect";
const DESCRIPTION: &str = "Mot incorrect : $match";
const CATEGORY_ID: &str = "TYPOS";
const CATEGORY_NAME: &str = "Faute de frappe possible";

/// `SimpleReplaceDataLoader.loadWords`: `wrong=right` lines (multiple wrong
/// forms separated by `|`, multiple replacements separated by `|`).
fn load_words(path: &Path) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if right.trim().is_empty() || right.contains('=') {
            continue;
        }
        let replacements: Vec<String> = right.split('|').map(|s| s.to_string()).collect();
        for wrong_form in left.split('|') {
            map.insert(wrong_form.to_string(), replacements.clone());
        }
    }
    map
}

/// `StringTools.toId` (French short code: no umlaut mapping).
fn to_id(input: &str) -> String {
    input
        .trim()
        .to_uppercase()
        .replace(' ', "_")
        .replace('\'', "_Q_")
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase()
                || ('\u{c0}'..='\u{d6}').contains(&c)
                || ('\u{d8}'..='\u{de}').contains(&c)
            {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `AbstractSimpleReplaceRule` (case-insensitive, sub-rule ids, tagged words
/// ignored, lemmas disabled) with the French `SimpleReplaceRule` messages.
pub struct FrenchSimpleReplaceRule {
    wrong_words: HashMap<String, Vec<String>>,
}

impl FrenchSimpleReplaceRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let rules_dir = data_dir.join("fr/rules");
        let mut wrong_words = load_words(&rules_dir.join("replace.txt"));
        wrong_words.extend(load_words(&rules_dir.join("replace_custom.txt")));
        Ok(Self { wrong_words })
    }

    pub fn rule_id(&self) -> &str {
        SIMPLE_REPLACE_RULE_ID
    }

    /// `AbstractSimpleReplaceRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut matches = Vec::new();
        for tr in &non_blank {
            if tr.is_sentence_start || tr.is_immunized || tr.is_ignore_spelling || tr.is_tagged {
                continue;
            }
            let original = tr.surface().to_string();
            let lower = original.to_lowercase();
            let is_all_uppercase = lt_spell::morfologik::is_all_uppercase(&original);
            let possible = self
                .wrong_words
                .get(&original)
                .or_else(|| self.wrong_words.get(&lower));
            let Some(possible) = possible else {
                continue;
            };
            let mut replacements: Vec<String> = if is_all_uppercase {
                possible.iter().map(|s| s.to_uppercase()).collect()
            } else {
                possible.clone()
            };
            replacements.retain(|r| *r != original);
            if replacements.is_empty() {
                continue;
            }
            matches.push(self.create_match(tr, replacements, sentence_offset, &original));
        }
        matches
    }

    /// `AbstractSimpleReplaceRule.createRuleMatch` with `subRuleSpecificIds`.
    fn create_match(
        &self,
        tr: &AnalyzedTokenReadings,
        mut replacements: Vec<String>,
        sentence_offset: usize,
        original: &str,
    ) -> Match {
        let id = to_id(&format!("{SIMPLE_REPLACE_RULE_ID}_{original}"));
        let description = DESCRIPTION.replace("$match", original);
        let message = match replacements.first() {
            Some(first) => format!("Vouliez-vous dire « {first} » ?"),
            None => SHORT.to_string(),
        };
        if original.chars().next().is_some_and(char::is_uppercase) {
            for r in &mut replacements {
                *r = uppercase_first(r);
            }
        }
        Match::new(
            &id,
            Option::<String>::None,
            &message,
            Some(SHORT.to_string()),
            TextRange::new(
                sentence_offset + tr.start_pos,
                sentence_offset + tr.start_pos + original.len(),
            ),
            replacements
                .into_iter()
                .map(|value| Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(&description, "misspelling", 0)
        .with_match_type("Other")
    }
}
