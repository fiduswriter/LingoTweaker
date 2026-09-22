//! Generic `WordRepeatRule` (`WORD_REPEAT_RULE`), the language-neutral base
//! class used by `Slovak.getRelevantRules` and `Slovenian.getRelevantRules`.
//! Only the message strings differ per language (the detection, the fixed
//! ignore list and the `duplication` issue type are the base class).

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

pub const RULE_ID: &str = "WORD_REPEAT_RULE";

pub struct WordRepeatConfig {
    pub description: &'static str,
    pub message: &'static str,
    pub short_message: &'static str,
    pub category_name: &'static str,
}

/// `WordRepeatRule.ignore`: the fixed base name list.
fn base_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if position > 0
            && tokens[position - 1].surface() == name
            && tokens[position].surface() == name
        {
            return true;
        }
    }
    false
}

pub struct WordRepeatRule {
    config: WordRepeatConfig,
}

impl WordRepeatRule {
    pub fn new(config: WordRepeatConfig) -> Self {
        Self { config }
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    /// `WordRepeatRule.match` over one sentence (the base class has no
    /// language override beyond the strings).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut prev_token = String::new();
        for i in 1..view.len() {
            let token = view[i].surface().to_string();
            if view[i].is_immunized {
                prev_token.clear();
                continue;
            }
            if is_word(&token) && eq_ignore_case(&prev_token, &token) && !base_ignore(&view, i) {
                let prev_pos = view[i - 1].start_pos;
                let pos = view[i].start_pos;
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        self.config.message,
                        Some(self.config.short_message.to_string()),
                        TextRange::new(
                            sentence_offset + prev_pos,
                            sentence_offset + pos + prev_token.len(),
                        ),
                        vec![Suggestion {
                            value: prev_token.clone(),
                            short_description: None,
                        }],
                        "MISC",
                        self.config.category_name,
                    )
                    .with_metadata(self.config.description, "duplication", 1),
                );
            }
            prev_token = token;
        }
        rule_matches
    }
}

pub const UK_RULE_ID: &str = "UKRAINIAN_WORD_REPEAT_RULE";
const UK_DESCRIPTION: &str = "Повторення слів (напр., 'буде буде')";
const UK_MESSAGE: &str = "Можлива механічна помилка: повторення слова";
const UK_SHORT: &str = "Повторення слів";
const UK_CATEGORY_NAME: &str = "Можлива механічна помилка";

fn has_pos_tag(readings: &[lt_core::AnalyzedToken], re: &regex::Regex) -> bool {
    readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| re.is_match(t)))
}

/// `UkrainianWordRepeatRule.ignore`.
fn uk_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    static DATE_TIME_NUM: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:date|time|number.*)$").unwrap());
    let tr = tokens[position];
    let token = tr.surface();
    if position > 2 && token == "добра" && eq_ignore_case(tokens[position - 2].surface(), "від")
    {
        return true;
    }
    if position > 1 && token == "що" && eq_ignore_case(tokens[position - 2].surface(), "тому")
    {
        return true;
    }
    if position > 3
        && token == "ні"
        && tokens[position - 2].surface() == ","
        && eq_ignore_case(tokens[position - 3].surface(), "так")
    {
        return true;
    }
    if token.to_lowercase() == "ст." {
        return true;
    }
    if ["Джей", "Бі", "Сі", "Ла"].contains(&token) {
        return true;
    }
    if has_pos_tag(&tr.readings, &DATE_TIME_NUM) {
        return true;
    }
    for at in &tr.readings {
        if let Some(pos_tag) = at.pos_tag.as_deref() {
            let is_initial = pos_tag.contains("abbr")
                || (at.token.chars().count() == 1
                    && at.token.chars().next().is_some_and(|c| c.is_uppercase())
                    && position < tokens.len() - 1
                    && tokens[position + 1].surface() == ".");
            if !is_initial && pos_tag != "SENT_END" {
                return false;
            }
        }
    }
    true
}

/// `UkrainianWordRepeatRule` over one sentence.
pub fn check_sentence_uk(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized {
            prev_token.clear();
            continue;
        }
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !uk_ignore(&view, i) {
            let double_i = prev_token == "І" && token == "і";
            let mut msg = UK_MESSAGE.to_string();
            if double_i {
                msg.push_str(" або, можливо, перша І має бути латинською.");
            }
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            let mut suggestions = vec![Suggestion {
                value: prev_token.clone(),
                short_description: None,
            }];
            if double_i {
                suggestions.push(Suggestion {
                    value: "I і".to_string(),
                    short_description: None,
                });
            }
            rule_matches.push(
                Match::new(
                    UK_RULE_ID,
                    Option::<String>::None,
                    msg,
                    Some(UK_SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    suggestions,
                    "MISC",
                    UK_CATEGORY_NAME,
                )
                .with_metadata(UK_DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}

pub const FA_RULE_ID: &str = "PERSIAN_WORD_REPEAT_RULE";
const FA_DESCRIPTION: &str = "تکرار کلمه (برای نمونه 'شد شد)";
const FA_MESSAGE: &str = "اشتباه تایپی متحمل: شما یک کلمه را تکرار کرده‌اید";
const FA_SHORT: &str = "تکرار کلمه";
const FA_CATEGORY_NAME: &str = "متفرقه";

/// `PersianWordRepeatRule.ignore`: the base list plus the Persian exceptions
/// (exact, case-sensitive `wordRepetitionOf`, unlike the case-insensitive
/// repetition test itself).
fn fa_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    if base_ignore(tokens, position) {
        return true;
    }
    for name in ["لی", "سی", "لک", "ریز", "جز", "کل"] {
        if position > 0
            && tokens[position - 1].surface() == name
            && tokens[position].surface() == name
        {
            return true;
        }
    }
    false
}

/// `PersianWordRepeatRule` (`PERSIAN_WORD_REPEAT_RULE`) over one sentence.
pub fn check_sentence_fa(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized {
            prev_token.clear();
            continue;
        }
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !fa_ignore(&view, i) {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    FA_RULE_ID,
                    Option::<String>::None,
                    FA_MESSAGE,
                    Some(FA_SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "MISC",
                    FA_CATEGORY_NAME,
                )
                .with_metadata(FA_DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}
