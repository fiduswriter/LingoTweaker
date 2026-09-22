//! Port of `AbstractSpecificCaseRule`: proper nouns / expressions with a
//! specific upper/lowercase spelling. The English rule
//! (`EnglishSpecificCaseRule`, `EN_SPECIFIC_CASE`) and the Greek rule
//! (`GreekSpecificCaseRule`, `EL_SPECIFIC_CASE`) are the same algorithm with
//! different strings and phrase files.

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

/// Per-language strings and phrase-file path.
pub struct SpecificCaseConfig {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub short_message: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    pub initial_capital_message: &'static str,
    pub other_capitalization_message: &'static str,
    /// Phrase file path relative to `data/`.
    pub phrases_path: &'static str,
}

pub struct SpecificCaseRule {
    config: SpecificCaseConfig,
    /// lowercase phrase -> properly spelled phrase
    phrases: HashMap<String, String>,
    max_len: usize,
}

impl SpecificCaseRule {
    pub fn rule_id(&self) -> &'static str {
        self.config.rule_id
    }

    /// `EnglishSpecificCaseRule` (`EN_SPECIFIC_CASE`).
    pub fn from_data(data_dir: &Path) -> Self {
        Self::from_data_with(
            data_dir,
            SpecificCaseConfig {
                rule_id: "EN_SPECIFIC_CASE",
                description: "Checks upper/lower case spelling of some proper nouns",
                short_message: "Proper noun",
                category_id: "CASING",
                category_name: "Upper/Lowercase",
                initial_capital_message: "If the term is a proper noun, use initial capitals.",
                other_capitalization_message:
                    "If the term is a proper noun, use the suggested capitalization.",
                phrases_path: "en/words/specific_case.txt",
            },
        )
    }

    /// `AbstractSpecificCaseRule` over `config.phrases_path`.
    pub fn from_data_with(data_dir: &Path, config: SpecificCaseConfig) -> Self {
        let mut phrases = HashMap::new();
        let mut max_len = 0usize;
        let path = data_dir.join(config.phrases_path);
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
        Self {
            config,
            phrases,
            max_len,
        }
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
                    self.config.initial_capital_message
                } else {
                    self.config.other_capitalization_message
                };
                let start = sentence_offset + non_blank[i].start_pos;
                let end = sentence_offset + non_blank[i + j - 1].end_pos();
                matches.push(
                    Match::new(
                        self.config.rule_id,
                        Option::<String>::None,
                        message,
                        Some(self.config.short_message.to_string()),
                        TextRange::new(start, end),
                        vec![Suggestion {
                            value: proper_spelling.clone(),
                            short_description: None,
                        }],
                        self.config.category_id,
                        self.config.category_name,
                    )
                    .with_metadata(self.config.description, "misspelling", 0)
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
