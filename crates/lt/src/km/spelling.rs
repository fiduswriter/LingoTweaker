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
    /// stream. The run list is kept so the `HunspellRule` wrong-split check can
    /// re-split the previous token and the current one into two known words
    /// (`ហ យន` -> `ហយ ន`).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_text: &str,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut matches: Vec<Match> = Vec::new();
        // Java's `tokenizeText` splits on every non-letter, so a run is a
        // `[^\p{L}]`-delimited token. `HunspellRule`'s wrong-split check looks
        // at `tokens[i-1]`, which is non-empty only when the previous run is
        // separated from the current one by exactly one delimiter character
        // (two or more delimiters produce an empty split token in between).
        let mut runs: Vec<(&str, usize, usize)> = Vec::new();
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
            runs.push((raw, start, run_end));
            if self.0.is_misspelled_word(raw) {
                self.wrong_split(
                    &mut matches,
                    sentence_text,
                    &runs,
                    start,
                    raw,
                    sentence_offset,
                );
            }
            let mut sub = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(raw, None, None)]);
            sub.start_pos = start;
            sub.raw_byte_len = raw.len();
            matches.extend(self.0.check_sentence(&[sub], sentence_offset));
        }
        matches
    }

    /// `HunspellRule.match`'s "thanky ou" / "than kyou" wrong-split check.
    fn wrong_split(
        &self,
        matches: &mut Vec<Match>,
        sentence_text: &str,
        runs: &[(&str, usize, usize)],
        start: usize,
        word: &str,
        sentence_offset: usize,
    ) {
        if runs.len() < 2 {
            return;
        }
        let (prev_word, prev_start, prev_end) = runs[runs.len() - 2];
        if prev_word.is_empty() {
            return;
        }
        // `tokens[i-1]` is the previous run only with a single separator char;
        // otherwise Java's split yields an empty token and skips the check.
        if sentence_text[prev_end..start].chars().count() != 1 {
            return;
        }
        let clean_word = word.strip_suffix('.').unwrap_or(word);
        let to = sentence_offset + start + clean_word.len();
        let from = sentence_offset + prev_start;
        // "thanky ou" -> "thank you"
        let prev_chars: Vec<char> = prev_word.chars().collect();
        let sugg1a: String = prev_chars[..prev_chars.len() - 1].iter().collect();
        let mut sugg1b: String = prev_chars[prev_chars.len() - 1..].iter().collect();
        sugg1b.push_str(clean_word);
        let sugg1b = cut_off_dot(&sugg1b);
        if !self.0.is_misspelled_word(&sugg1a) && !self.0.is_misspelled_word(&sugg1b) {
            self.push_wrong_split(matches, &sugg1a, &sugg1b, from, to);
        }
        // "than kyou" -> "thank you"
        let mut chars = clean_word.chars();
        if let Some(first) = chars.next() {
            let sugg2a = format!("{prev_word}{first}");
            let sugg2b = cut_off_dot(chars.as_str());
            if !self.0.is_misspelled_word(&sugg2a) && !self.0.is_misspelled_word(&sugg2b) {
                self.push_wrong_split(matches, &sugg2a, &sugg2b, from, to);
            }
        }
    }

    /// `SpellingCheckRule.createWrongSplitMatch`: replace the previous match
    /// when it starts at the same position, and report the whole span.
    fn push_wrong_split(
        &self,
        matches: &mut Vec<Match>,
        suggestion1: &str,
        suggestion2: &str,
        from: usize,
        to: usize,
    ) {
        if let Some(prev) = matches.last() {
            if prev.range.start == from {
                matches.pop();
            }
        }
        let value = format!("{suggestion1} {suggestion2}").trim().to_string();
        matches.push(self.0.wrong_split_match(from, to, value));
    }
}

/// `StringTools.cutOffDot`.
fn cut_off_dot(s: &str) -> String {
    s.strip_suffix('.').unwrap_or(s).to_string()
}
