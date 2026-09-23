//! Arabic spelling rule (`ArabicHunspellSpellerRule`, `HUNSPELL_RULE_AR`) over
//! the vendored Hunspell-ar dictionary (`ar/hunspell/ar.dic`, tri-licensed
//! GPL-2.0+/LGPL-2.1+/MPL-1.1+; see `hunspell/COPYING`).
//!
//! Faithful specifics: `isLatinScript() = false` (Arabic-script words are
//! checked), `tokenizeText` splits on every non-letter/non-tashkeel character
//! (here the [`LETTER_RUN`] regex over the sentence text, like `da`/`sv`), and
//! both `ignoreWord`/`isMisspelled` strip tashkeel first.

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result};
use regex::Regex;

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};
use crate::wordutil::{is_email, is_url};

pub const RULE_ID: &str = "HUNSPELL_RULE_AR";

/// `ArabicHunspellSpellerRule.tokenizeText`:
/// `[^\p{L}\u064B-\u0656\u0640]` splits, so a checked word is a maximal run of
/// letters plus tashkeel/tatweel.
static LETTER_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{L}\u064B-\u0656\u0640]+").unwrap());

pub struct ArabicSpellingRule(InnerRule);

impl ArabicSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "خطأ إملائي محتمل",
                message: "وُجد خطأ إملائي محتمل",
                short_message: "خطأ إملائي",
                category_id: "TYPOS",
                category_name: "أخطاء إملائية محتملة",
                lang_dir: "ar",
                aff: "ar.aff",
                dic: "ar.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
                native_suggestions: true,
                cap_native_suggestions: false,
                latin_script: false,
                strip_tashkeel: true,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    pub fn is_known(&self, word: &str) -> bool {
        self.0.is_known(word)
    }

    /// `HunspellRule.match`: spell-check `tokenizeText(sentence)` runs over the
    /// sentence text (URL/immunized tokens suppressed), not the engine token
    /// stream.
    ///
    /// `ArabicHunspellSpellerRule.tokenizeText` splits on single
    /// non-letter characters, so two adjacent separators produce an empty
    /// element in the token array (`الباب، فهو` -> `[الباب, "", فهو]`); the
    /// element before a run is therefore the previous run only when the gap
    /// is exactly one separator, which is what the `HunspellRule` wrong-split
    /// check needs (`prevWord.length() > 0`).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_text: &str,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut matches: Vec<Match> = Vec::new();
        // the previous split element that was spell-checked, with its start
        let mut prev: Option<(&str, usize)> = None;
        let mut prev_end = 0usize;
        for m in LETTER_RUN.find_iter(sentence_text) {
            let raw = m.as_str();
            let start = m.start();
            let run_end = start + raw.len();
            let gap = start - prev_end;
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
                // Java replaces the token with whitespace, so the next run's
                // previous split element is an empty string.
                prev = None;
                prev_end = run_end;
                continue;
            }
            if self.0.is_misspelled_word(raw) {
                if gap == 1 {
                    if let Some((prev_word, prev_start)) = prev {
                        self.wrong_split(
                            &mut matches,
                            prev_word,
                            prev_start + sentence_offset,
                            raw,
                            sentence_offset + start,
                        );
                    }
                }
                let mut sub = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(raw, None, None)]);
                sub.start_pos = start;
                sub.raw_byte_len = raw.len();
                matches.extend(self.0.check_sentence(&[sub], sentence_offset));
            }
            prev = Some((raw, start));
            prev_end = run_end;
        }
        matches
    }

    /// `HunspellRule.match`'s "thanky ou" / "than kyou" wrong-split check,
    /// which re-splits the previous token and the current one into two known
    /// words (`جرائمي طالها` -> `جرائم يطالها` via `فهو مقفول` ->
    /// `فهوم قفول`).
    fn wrong_split(
        &self,
        matches: &mut Vec<Match>,
        prev_word: &str,
        prev_start: usize,
        word: &str,
        word_start: usize,
    ) {
        if prev_word.is_empty() {
            return;
        }
        let clean_word = word.strip_suffix('.').unwrap_or(word);
        let from = prev_start;
        let to = word_start + clean_word.len();
        // "thanky ou" -> "thank you"
        let prev_chars: Vec<char> = prev_word.chars().collect();
        let sugg1a: String = prev_chars[..prev_chars.len() - 1].iter().collect();
        let mut sugg1b: String = prev_chars[prev_chars.len() - 1..].iter().collect();
        sugg1b.push_str(word);
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
