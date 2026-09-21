//! Icelandic spelling rule:
//! `org.languagetool.rules.spelling.hunspell.HunspellNoSuggestionRule` over
//! the vendored `is/hunspell/is_IS.{aff,dic}`.
//!
//! The rule flags misspelled words with no suggested replacements
//! (`getSuggestions` returns an empty list), so the engine's suggestion
//! machinery is disabled (`max_suggestions: 0`).

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result};
use regex::Regex;

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};
use crate::wordutil::{is_email, is_url};

pub const RULE_ID: &str = "HUNSPELL_NO_SUGGEST_RULE";

/// `HunspellRule.tokenizeText` with the `is_IS.aff` `WORDCHARS "-./="`:
/// maximal runs of letters plus `-`, `.`, `/` and `=`
/// (`nonWordPattern = (?![-./=])[^\p{L}]`).
static LETTER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\-./=]+").unwrap());

pub struct IcelandicSpellingRule(InnerRule);

impl IcelandicSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Possible spelling mistake (without suggestions)",
                message: "Possible spelling mistake found.",
                short_message: "Ritvilla",
                category_id: "TYPOS",
                category_name: "Hugsanleg ritvilla",
                lang_dir: "is",
                aff: "is_IS.aff",
                dic: "is_IS.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 0,
                native_suggestions: false,
                cap_native_suggestions: false,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    /// Dictionary membership for the context rules.
    pub fn is_known(&self, word: &str) -> bool {
        self.0.is_known(word)
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_text: &str,
        sentence_offset: usize,
    ) -> Vec<Match> {
        // `HunspellRule.match` spell-checks `tokenizeText(text)` over the
        // sentence text (URLs/immunized tokens replaced by whitespace), not
        // the engine token stream; the `is_IS.aff` `WORDCHARS "-./="` keeps
        // those characters inside the checked token.
        let mut matches: Vec<Match> = Vec::new();
        for m in LETTER_RUN.find_iter(sentence_text) {
            let raw = m.as_str();
            let start = m.start();
            let run_end = start + raw.len();
            let skipped = tokens.iter().any(|tr| {
                if tr.is_whitespace || start < tr.start_pos {
                    return false;
                }
                let end = tr.start_pos + tr.raw_byte_len.max(tr.surface().len());
                end >= run_end
                    && tr.start_pos <= start
                    && (tr.is_immunized
                        || tr.is_ignore_spelling
                        || is_url(tr.surface())
                        || is_email(tr.surface()))
            });
            if skipped {
                continue;
            }
            let mut sub = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(raw, None, None)]);
            sub.start_pos = start;
            sub.raw_byte_len = raw.len();
            matches.extend(self.0.check_sentence(&[sub], sentence_offset));
        }
        matches
    }
}
