//! Galician spelling rule: `org.languagetool.rules.spelling.hunspell.HunspellRule`
//! over the vendored VOLGa-based `gl/hunspell/gl_ES.{aff,dic}`.
//!
//! `HunspellRule` is the plain `SpellingCheckRule` hunspell backend: the
//! `gl/hunspell/{ignore,spelling,spelling_custom}.txt` and
//! `core/spelling_global.txt` word lists, `gl/hunspell/prohibit*.txt`, the
//! `_english_ignore_` tag and the `desc_spelling`/`spelling`/
//! `desc_spelling_short` messages. Suggestions come from the shared bounded
//! search over the dictionary words (`HunspellSpellingRule`); the legacy engine's
//! native `hunspell.suggest` ranking is not reproduced (internal notes).

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result};
use regex::Regex;

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};
use crate::wordutil::{is_email, is_url};

pub const RULE_ID: &str = "HUNSPELL_RULE";

/// `HunspellRule.tokenizeText` with the `gl_ES.aff` `WORDCHARS -'`: maximal
/// runs of letters plus `-` and `'` (`nonWordPattern = (?![-'])[^\p{L}]`).
static LETTER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}'-]+").unwrap());

pub struct GalicianSpellingRule(InnerRule);

impl GalicianSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Posíbel erro ortográfico",
                message: "Atopouse un posíbel erro ortográfico",
                short_message: "Erro ortográfico",
                category_id: "TYPOS",
                category_name: "Posíbeis erros tipográficos",
                lang_dir: "gl",
                aff: "gl_ES.aff",
                dic: "gl_ES.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: true,
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
        // the engine token stream: alphanumeric and quoted tokens are split
        // at non-letters (`241Am` -> `Am`, `'a'` stays one token).
        let mut matches: Vec<Match> = Vec::new();
        for m in LETTER_RUN.find_iter(sentence_text) {
            let start = m.start();
            // Skip runs inside URLs / e-mail / immunized / speller-ignored
            // tokens (`getSentenceTextWithoutUrlsAndImmunizedTokens`). Only a
            // token that covers the whole WORDCHARS run suppresses it (an
            // ignored sub-token must not hide the run), matching the legacy
            // engine.
            let run_end = m.end();
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
            let mut sub =
                AnalyzedTokenReadings::new(vec![AnalyzedToken::new(m.as_str(), None, None)]);
            sub.start_pos = start;
            sub.raw_byte_len = m.end() - start;
            matches.extend(self.0.check_sentence(&[sub], sentence_offset));
        }
        matches
    }
}
