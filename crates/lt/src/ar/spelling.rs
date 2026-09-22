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
