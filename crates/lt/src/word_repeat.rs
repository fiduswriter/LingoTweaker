//! Generic `WordRepeatRule` (`WORD_REPEAT_RULE`), the language-neutral base
//! class used by `Slovak.getRelevantRules` and `Slovenian.getRelevantRules`.
//! Only the message strings differ per language (the detection, the fixed
//! ignore list and the `duplication` issue type are the base class).

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

pub const RULE_ID: &str = "WORD_REPEAT_RULE";

pub struct WordRepeatConfig {
    pub description: &'static str,
    pub message: &'static str,
    pub short_message: &'static str,
    pub category_name: &'static str,
}

/// `WordRepeatRule.ignore`: the fixed base name list.
fn base_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if position > 0
            && tokens[position - 1].surface() == name
            && tokens[position].surface() == name
        {
            return true;
        }
    }
    false
}

pub struct WordRepeatRule {
    config: WordRepeatConfig,
}

impl WordRepeatRule {
    pub fn new(config: WordRepeatConfig) -> Self {
        Self { config }
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    /// `WordRepeatRule.match` over one sentence (the base class has no
    /// language override beyond the strings).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut prev_token = String::new();
        for i in 1..view.len() {
            let token = view[i].surface().to_string();
            if view[i].is_immunized {
                prev_token.clear();
                continue;
            }
            if is_word(&token) && eq_ignore_case(&prev_token, &token) && !base_ignore(&view, i) {
                let prev_pos = view[i - 1].start_pos;
                let pos = view[i].start_pos;
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        self.config.message,
                        Some(self.config.short_message.to_string()),
                        TextRange::new(
                            sentence_offset + prev_pos,
                            sentence_offset + pos + prev_token.len(),
                        ),
                        vec![Suggestion {
                            value: prev_token.clone(),
                            short_description: None,
                        }],
                        "MISC",
                        self.config.category_name,
                    )
                    .with_metadata(self.config.description, "duplication", 1),
                );
            }
            prev_token = token;
        }
        rule_matches
    }
}
