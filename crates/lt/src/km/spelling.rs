//! Khmer spelling rule: `org.languagetool.rules.km.KhmerHunspellRule`
//! (`HunspellRule`, rule id `HUNSPELL_RULE`) over the vendored SBBIC
//! `km/hunspell/km_KH.{aff,dic}` (GPLv3; see `hunspell/LICENCES-km.txt`).
//!
//! Faithful specifics: `isLatinScript() = false` (Khmer-script words are
//! checked), and `tokenizeText` splits on every non-letter character
//! (`nonWordPattern = [^\p{L}]`, since `km_KH.aff` has no `WORDCHARS`), so a
//! checked word is a maximal run of letters (here the [`LETTER_RUN`] regex
//! over the sentence text, like `da`/`ar`).

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result};
use regex::Regex;

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};
use crate::wordutil::{is_email, is_url};

pub const RULE_ID: &str = "HUNSPELL_RULE";

/// `HunspellRule.tokenizeText` with no `WORDCHARS`: maximal runs of letters.
static LETTER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}]+").unwrap());

pub struct KhmerSpellingRule(InnerRule);

impl KhmerSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "អាចមានការប្រកបដែលមានកំហុស",
                message: "អាចរកឃើញការប្រកបដែលមានកុំហុស",
                short_message: "ប្រកបមានកំហុស",
                category_id: "TYPOS",
                category_name: "ប្រហែល\u{200B}មាន\u{200B}\u{200B}ប្រកប\u{200B}ខុស",
                lang_dir: "km",
                aff: "km_KH.aff",
                dic: "km_KH.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: true,
                cap_native_suggestions: false,
                latin_script: false,
                strip_tashkeel: false,
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

    /// `HunspellRule.match`: spell-check `tokenizeText(sentence)` runs over the
    /// sentence text (URL/immunized tokens suppressed), not the engine token
    /// stream.
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
