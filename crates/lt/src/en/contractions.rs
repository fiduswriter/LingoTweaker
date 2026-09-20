//! Port of `ContractionSpellingRule` (`EN_CONTRACTION_SPELLING`), a subclass
//! of `AbstractSimpleReplaceRule` with `setCheckLemmas(false)`: exact,
//! case-sensitive replacement of misspelled contractions
//! (`en/rules/contractions.txt`).

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

const RULE_ID: &str = "EN_CONTRACTION_SPELLING";
const MESSAGE: &str = "Possible spelling mistake found.";
const SHORT: &str = "Spelling mistake";
const DESCRIPTION: &str = "Spelling of English contractions";

pub struct ContractionSpellingRule {
    /// wrong form -> replacements (`SimpleReplaceDataLoader`)
    wrong_words: HashMap<String, Vec<String>>,
}

impl ContractionSpellingRule {
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("en/rules/contractions.txt");
        let text = lt_data::fs::read_to_string(&path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display()))
        })?;
        let mut wrong_words: HashMap<String, Vec<String>> = HashMap::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() != 2 {
                return Err(lt_core::CoreError::Data(format!(
                    "Could not load simple replacement data from {}: error in line '{line}', \
                     expected format 'word=replacement'",
                    path.display()
                )));
            }
            if parts[1].trim().is_empty() {
                return Err(lt_core::CoreError::Data(format!(
                    "Could not load simple replacement data from {}: replacement cannot be empty",
                    path.display()
                )));
            }
            let replacements: Vec<String> = parts[1].split('|').map(str::to_string).collect();
            for wrong_form in parts[0].split('|') {
                wrong_words.insert(wrong_form.to_string(), replacements.clone());
            }
        }
        Ok(Self { wrong_words })
    }

    /// `AbstractSimpleReplaceRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for token_readings in view {
            if token_readings
                .readings
                .first()
                .and_then(|r| r.pos_tag.as_deref())
                == Some(crate::pipeline::sentence_start_tag())
                || token_readings.is_immunized
                || token_readings.is_ignore_spelling
            {
                continue;
            }
            let original = token_readings.surface().to_string();
            let Some(possible_replacements) = self.wrong_words.get(&original) else {
                continue;
            };
            let mut replacements: Vec<String> = if lt_spell::morfologik::is_all_uppercase(&original)
            {
                possible_replacements
                    .iter()
                    .map(|s| s.to_uppercase())
                    .collect()
            } else {
                possible_replacements.clone()
            };
            replacements.retain(|r| r != &original);
            if replacements.is_empty() {
                continue;
            }
            let pos = token_readings.start_pos;
            let suggestions: Vec<Suggestion> = replacements
                .into_iter()
                .map(|value| Suggestion {
                    value,
                    short_description: None,
                })
                .collect();
            rule_matches.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    MESSAGE,
                    Some(SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + pos,
                        sentence_offset + token_readings.end_pos(),
                    ),
                    suggestions,
                    "TYPOS",
                    "Possible Typo",
                )
                .with_metadata(DESCRIPTION, "misspelling", 0),
            );
        }
        rule_matches
    }
}
