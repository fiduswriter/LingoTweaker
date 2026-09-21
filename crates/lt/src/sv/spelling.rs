//! Swedish spelling rule: `org.languagetool.rules.spelling.hunspell.HunspellRule`
//! over the vendored `sv/hunspell/sv_SE.{aff,dic}`.
//!
//! The `sv_SE.aff` `WORDCHARS -0123456789.` keeps digits, `-` and `.` inside
//! the checked token, and `BREAK -`/`.$`/`:` gives the checker its compound
//! split points. Suggestions come from the shared bounded search over the
//! dictionary words; the legacy engine's native `hunspell.suggest` ranking is
//! not reproduced (internal notes).

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result};
use regex::Regex;

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};
use crate::wordutil::{is_email, is_url};

pub const RULE_ID: &str = "HUNSPELL_RULE";

/// `HunspellRule.tokenizeText` with the `sv_SE.aff`
/// `WORDCHARS -0123456789.`: maximal runs of letters/digits plus `-` and `.`
/// (`nonWordPattern = (?![-0123456789.])[^\p{L}]`).
static LETTER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}0-9.-]+").unwrap());

pub struct SwedishSpellingRule(InnerRule);

impl SwedishSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Möjligt stavfel",
                message: "Hittat ett möjligt stavfel.",
                short_message: "Stavfel",
                category_id: "TYPOS",
                category_name: "Möjligt skrivfel",
                lang_dir: "sv",
                aff: "sv_SE.aff",
                dic: "sv_SE.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: true,
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
        let mut matches: Vec<Match> = Vec::new();
        for m in LETTER_RUN.find_iter(sentence_text) {
            let raw = m.as_str();
            let start = m.start();
            let skipped = tokens.iter().any(|tr| {
                if tr.is_whitespace || start < tr.start_pos {
                    return false;
                }
                let end = tr.start_pos + tr.raw_byte_len.max(tr.surface().len());
                start < end
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
