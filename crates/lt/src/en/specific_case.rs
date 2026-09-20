//! Port of `AbstractSpecificCaseRule` / `EnglishSpecificCaseRule`
//! (`EN_SPECIFIC_CASE`): proper nouns with a specific upper/lowercase
//! spelling.

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

const RULE_ID: &str = "EN_SPECIFIC_CASE";
const DESCRIPTION: &str = "Checks upper/lower case spelling of some proper nouns";
const SHORT_MESSAGE: &str = "Proper noun";
const CATEGORY_ID: &str = "CASING";
const CATEGORY_NAME: &str = "Upper/Lowercase";
const INITIAL_CAPITAL_MESSAGE: &str = "If the term is a proper noun, use initial capitals.";
const OTHER_CAPITALIZATION_MESSAGE: &str =
    "If the term is a proper noun, use the suggested capitalization.";

pub struct SpecificCaseRule {
    /// lowercase phrase -> properly spelled phrase
    phrases: HashMap<String, String>,
    max_len: usize,
}

impl SpecificCaseRule {
    pub fn from_data(data_dir: &Path) -> Self {
        let mut phrases = HashMap::new();
        let mut max_len = 0usize;
        let path = data_dir.join("en/words/specific_case.txt");
        if let Ok(text) = lt_data::fs::read_to_string(path) {
            for line in text.lines() {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let phrase = line.trim().to_string();
                if phrase.is_empty() {
                    continue;
                }
                max_len = max_len.max(phrase.split(' ').count());
                phrases.insert(phrase.to_lowercase(), phrase);
            }
        }
        Self { phrases, max_len }
    }

    /// `AbstractSpecificCaseRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut matches = Vec::new();
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for i in 0..non_blank.len() {
            let mut collected: Vec<String> = Vec::new();
            let mut j = 0usize;
            while collected.len() < self.max_len && i + j < non_blank.len() {
                collected.push(non_blank[i + j].surface().to_string());
                j += 1;
                let phrase = collected.join(" ");
                let lower = phrase.to_lowercase();
                let Some(proper_spelling) = self.phrases.get(&lower) else {
                    continue;
                };
                if lt_spell::morfologik::is_all_uppercase(&phrase) || phrase == *proper_spelling {
                    continue;
                }
                if i > 0
                    && non_blank[i - 1].is_sentence_start
                    && !starts_with_uppercase(proper_spelling)
                {
                    // avoid suggesting e.g. "vitamin C" at sentence start
                    continue;
                }
                let message = if all_words_uppercase(proper_spelling) {
                    INITIAL_CAPITAL_MESSAGE
                } else {
                    OTHER_CAPITALIZATION_MESSAGE
                };
                let start = sentence_offset + non_blank[i].start_pos;
                let end = sentence_offset + non_blank[i + j - 1].end_pos();
                matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        message,
                        Some(SHORT_MESSAGE.to_string()),
                        TextRange::new(start, end),
                        vec![Suggestion {
                            value: proper_spelling.clone(),
                            short_description: None,
                        }],
                        CATEGORY_ID,
                        CATEGORY_NAME,
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0)
                    .with_match_type("Other"),
                );
            }
        }
        matches
    }
}

fn all_words_uppercase(s: &str) -> bool {
    s.split(' ').all(starts_with_uppercase)
}

fn starts_with_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}
