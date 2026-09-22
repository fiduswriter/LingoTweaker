//! Port of `AbstractSpaceBeforeRule` (`SPACE_BEFORE_CONJUNCTION`): a
//! conjunction token that is not preceded by a space or an opening
//! parenthesis is missing its space. Persian's `PersianSpaceBeforeRule`
//! (`FA_SPACE_BEFORE_CONJUNCTION`) is the first-ported subclass and is
//! default off.

use regex::Regex;

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub struct SpaceBeforeRule {
    rule_id: &'static str,
    /// `getConjunctions()`; Java `matcher(...).matches()` is a full match.
    conjunctions: Regex,
    description: &'static str,
    short: &'static str,
    suggestion: &'static str,
    category_id: &'static str,
    category_name: &'static str,
}

impl SpaceBeforeRule {
    pub fn rule_id(&self) -> &str {
        self.rule_id
    }

    /// `PersianSpaceBeforeRule` (`FA_SPACE_BEFORE_CONJUNCTION`): the
    /// conjunction set `و|به|با|تا|زیرا|چون|بنابراین|چونکه`, category MISC,
    /// default off.
    pub fn persian() -> Self {
        Self {
            rule_id: "FA_SPACE_BEFORE_CONJUNCTION",
            conjunctions: Regex::new(r"^(?:و|به|با|تا|زیرا|چون|بنابراین|چونکه)$")
                .expect("persian conjunctions"),
            description: "بررسی‌کردن فاصله قبل از حرف ربط",
            short: "فاصلهٔ حذف‌شده",
            suggestion: "فاصلهٔ قبل از حرف ربط حذف شده‌است",
            category_id: "MISC",
            category_name: "متفرقه",
        }
    }

    /// `AbstractSpaceBeforeRule.match` over one sentence (raw token view,
    /// including whitespace: the previous token is compared to `" "`/`"("`).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        for i in 1..tokens.len() {
            let token = tokens[i].surface().to_string();
            if !self.conjunctions.is_match(&token) {
                continue;
            }
            let previous_token = tokens[i - 1].surface();
            if previous_token == " " || previous_token == "(" {
                continue;
            }
            let replacement = format!(" {token}");
            let pos = tokens[i].start_pos;
            rule_matches.push(
                Match::new(
                    self.rule_id,
                    Option::<String>::None,
                    self.suggestion,
                    Some(self.short.to_string()),
                    TextRange::new(sentence_offset + pos, sentence_offset + pos + token.len()),
                    vec![Suggestion {
                        value: replacement,
                        short_description: None,
                    }],
                    self.category_id,
                    self.category_name,
                )
                .with_metadata(self.description, "other", 0),
            );
        }
        rule_matches
    }
}
