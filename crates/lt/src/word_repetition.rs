//! Generic adjacent word-repetition rule for the hand-authored languages
//! (`*_WORD_REPETITION`). Same detection shape as the ported
//! `EnglishWordRepeatRule`: a word immediately followed by the same word
//! (case-insensitive, punctuation breaks the pair) is reported with a
//! suggestion that keeps one occurrence. Legitimate emphatic repetitions
//! (`ja ja`, `nei nei`) are configured per language in
//! `<lang>/hunspell/repetition_exceptions.txt`.

use std::collections::HashSet;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

pub struct WordRepetitionConfig {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub message: &'static str,
    pub short_message: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    /// Directory under `data/`, e.g. `no`.
    pub lang_dir: &'static str,
}

pub struct WordRepetitionRule {
    config: WordRepetitionConfig,
    exceptions: HashSet<String>,
}

impl WordRepetitionRule {
    pub fn load(data_dir: &Path, config: WordRepetitionConfig) -> Result<Self> {
        let mut exceptions = HashSet::new();
        load_exceptions(
            &data_dir
                .join(config.lang_dir)
                .join("hunspell/repetition_exceptions.txt"),
            &mut exceptions,
        );
        load_exceptions(
            &data_dir.join("core/repetition_exceptions.txt"),
            &mut exceptions,
        );
        Ok(Self { config, exceptions })
    }

    pub fn rule_id(&self) -> &str {
        self.config.rule_id
    }

    /// `WordRepeatRule.check`: compare each non-whitespace token with its
    /// predecessor and report the pair.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut matches = Vec::new();
        let mut prev_token = String::new();
        for i in 1..view.len() {
            let token = view[i].surface().to_string();
            if view[i].is_immunized {
                prev_token.clear();
                continue;
            }
            if is_word(&token)
                && eq_ignore_case(&prev_token, &token)
                && !self.exceptions.contains(&token.to_lowercase())
            {
                let from = view[i - 1].start_pos;
                let to = view[i].start_pos + token.len();
                matches.push(
                    Match::new(
                        self.config.rule_id,
                        Option::<String>::None,
                        self.config.message,
                        Some(self.config.short_message.to_string()),
                        TextRange::new(sentence_offset + from, sentence_offset + to),
                        vec![Suggestion {
                            value: token.clone(),
                            short_description: None,
                        }],
                        self.config.category_id,
                        self.config.category_name,
                    )
                    .with_metadata(self.config.description, "duplication", 1),
                );
            }
            prev_token = token;
        }
        matches
    }
}

fn load_exceptions(path: &Path, out: &mut HashSet<String>) {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for word in line.split_whitespace() {
            out.insert(word.to_lowercase());
        }
    }
}
