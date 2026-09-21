//! Esperanto spelling rule:
//! `org.languagetool.rules.spelling.hunspell.HunspellRule` over the vendored
//! `eo/hunspell/eo.{aff,dic}` (with suggestions).

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result};
use regex::Regex;

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};
use crate::wordutil::{is_email, is_url};

pub const RULE_ID: &str = "HUNSPELL_RULE";

/// `HunspellRule.tokenizeText` with the `eo.aff` `WORDCHARS` (all letters plus
/// `'` and `-`): maximal runs of letters plus `'`/`-`
/// (`nonWordPattern = (?![-']) [^\p{L}]`).
static LETTER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}'-]+").unwrap());

pub struct EsperantoSpellingRule(InnerRule);

impl EsperantoSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Ebla mistajpaĵo",
                message: "Ebla mistajpaĵo trovita",
                short_message: "Mistajpaĵo",
                category_id: "TYPOS",
                category_name: "Ebla misliterumo",
                lang_dir: "eo",
                aff: "eo.aff",
                dic: "eo.dic",
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
