//! `QuestionWhitespaceRule` and `QuestionWhitespaceStrictRule`: French
//! punctuation spacing (thin no-break space before `? ! ;`, no-break space
//! before `:`, `«`/`»` handling), including the rule's anti-patterns.

use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_pattern::{compile_pattern, PatternToken};
use regex::Regex;

pub const STRICT_RULE_ID: &str = "FRENCH_WHITESPACE_STRICT";
pub const RULE_ID: &str = "FRENCH_WHITESPACE";
const DESCRIPTION: &str = "Insertion des espaces fines insécables";
const SHORT_MESSAGE: &str = "Insérer une espace insécable";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Règles de base";

/// `QuestionWhitespaceRule.urlPattern` (anchored `find()`).
static URL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        "^(?:file|s?ftp|finger|git|gopher|hdl|https?|shttp|imap|mailto|mms|nntp|s?news(post|reply)?|prospero|rsync|rtspu|sips?|svn|svn\\+ssh|telnet|wais)$",
    )
    .unwrap()
});

const ESPACE_FINE_INSECABLE: &str = "\u{202F}";
const NBSP: &str = "\u{00A0}";

/// `QuestionWhitespaceRule.ANTI_PATTERNS` (PatternTokenBuilder terms; text
/// tokens are case-insensitive, `tokenRegex` sets `regexp`,
/// `setIsWhiteSpaceBefore(false)` sets `spacebefore`).
fn anti_patterns() -> Vec<Vec<PatternToken>> {
    let token = |s: &str| PatternToken {
        text: Some(s.to_string()),
        ..Default::default()
    };
    let token_re = |s: &str| PatternToken {
        text: Some(s.to_string()),
        regexp: true,
        ..Default::default()
    };
    let no_space_before = |mut t: PatternToken| {
        t.spacebefore = Some(false);
        t
    };
    vec![
        // ignore smileys, such as :-)
        vec![
            token_re("[:;]"),
            no_space_before(token("-")),
            no_space_before(token_re("[\\(\\)D]")),
        ],
        // ignore smileys, such as :)
        vec![token_re("[:;]"), no_space_before(token_re("[\\(\\)D]"))],
        // times like 23:20
        vec![token_re(".*\\d{1,2}"), token(":"), token_re("\\d{1,2}")],
        // "??"
        vec![token_re("[?!]"), token_re("[?!]")],
        // mac address
        vec![
            token_re("[a-z0-9]{2}"),
            token(":"),
            token_re("[a-z0-9]{2}"),
            token(":"),
            token_re("[a-z0-9]{2}"),
        ],
        // csv markup
        vec![
            token(";"),
            no_space_before(token_re(".+")),
            no_space_before(token(";")),
        ],
        vec![
            no_space_before(token_re(".+")),
            no_space_before(token(";")),
            no_space_before(token_re(".+")),
        ],
    ]
}

pub struct FrenchQuestionWhitespaceRule {
    strict: bool,
    anti_patterns: Vec<Arc<lt_pattern::CompiledPattern>>,
}

impl FrenchQuestionWhitespaceRule {
    pub fn new(strict: bool) -> Self {
        let mut compiled = Vec::new();
        for pattern in anti_patterns() {
            if let Ok(pattern) = compile_pattern(&pattern, None, None) {
                compiled.push(Arc::new(pattern));
            }
        }
        Self {
            strict,
            anti_patterns: compiled,
        }
    }

    pub fn rule_id(&self) -> &str {
        if self.strict {
            STRICT_RULE_ID
        } else {
            RULE_ID
        }
    }

    pub fn strict(&self) -> bool {
        self.strict
    }

    /// `QuestionWhitespaceRule.isAllowedWhitespaceChar`.
    fn is_allowed_whitespace_char(&self, tokens: &[&AnalyzedTokenReadings], i: isize) -> bool {
        if i < 0 {
            return false;
        }
        let idx = i as usize;
        if idx >= tokens.len() {
            return false;
        }
        if !self.strict {
            // Java `tokens[0].isWhitespace()` is true for the SENT_START
            // pseudo-token, so a punctuation mark as the first real token is
            // never reported (`: toutes`, `» Il`, `; en revanche`).
            return tokens[idx].is_whitespace || tokens[idx].is_sentence_start;
        }
        // Strictly speaking, the character before ?!; should be an "espace
        // fine insécable" (U+202f); in practice U+00a0 is accepted too.
        // Covered by FRENCH_WHITESPACE (non strict), which makes both rules
        // mutually exclusive.
        tokens[idx].surface() == "\u{202f}"
            || tokens[idx].surface() == "\u{00a0}"
            || !tokens[idx].is_whitespace
    }

    /// `QuestionWhitespaceRule.match` over one sentence (`getTokens()`,
    /// whitespace included; `sentence_offset` is the sentence's byte start).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        // `getSentenceWithImmunization`: mark the tokens matched by the
        // IMMUNIZE anti-patterns (matched over the non-whitespace view).
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut immunized = vec![false; tokens.len()];
        let mut view_to_full: Vec<usize> = Vec::with_capacity(non_blank.len());
        for (full_idx, token) in tokens.iter().enumerate() {
            if !token.is_whitespace || token.is_sentence_start || token.is_sentence_end {
                view_to_full.push(full_idx);
            }
        }
        for ap in &self.anti_patterns {
            for m in lt_pattern::find_matches(ap, &[], &non_blank) {
                for idx in m.start_tok()..=m.end_tok() {
                    if let Some(&full) = view_to_full.get(idx) {
                        immunized[full] = true;
                    }
                }
            }
        }

        let all: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        let mut rule_matches: Vec<Match> = Vec::new();
        let mut prev_prev_token = String::new();
        let mut prev_token = String::new();
        for i in 1..all.len() {
            let token = all[i].surface().to_string();
            if immunized[i] || prev_token == "(" || prev_token == "[" {
                prev_prev_token = prev_token;
                prev_token = token;
                continue;
            }
            let mut msg: Option<String> = None;
            let mut suggestion_text: Option<String> = None;
            let mut i_from = i - 1;
            let mut i_to = i;
            let is_previous_whitespace = all[i - 1].is_whitespace;
            let mut prev_token_to_change = prev_token.clone();
            if is_previous_whitespace {
                prev_token_to_change = String::new();
            }
            if !self.is_allowed_whitespace_char(&all, i as isize - 1) {
                if token == "?" && prev_token != "!" {
                    msg = Some(
                        "Le point d'interrogation est précédé d'une espace fine insécable."
                            .to_string(),
                    );
                    suggestion_text =
                        Some(format!("{prev_token_to_change}{ESPACE_FINE_INSECABLE}?"));
                } else if token == "!" && prev_token != "?" {
                    msg = Some(
                        "Le point d'exclamation est précédé d'une espace fine insécable."
                            .to_string(),
                    );
                    suggestion_text =
                        Some(format!("{prev_token_to_change}{ESPACE_FINE_INSECABLE}!"));
                } else if token == ";" {
                    msg = Some(
                        "Le point-virgule est précédé d'une espace fine insécable.".to_string(),
                    );
                    suggestion_text =
                        Some(format!("{prev_token_to_change}{ESPACE_FINE_INSECABLE};"));
                } else if token == ":" {
                    // Avoid false positive for URL like http://www.languagetool.org.
                    if !URL_PATTERN.is_match(&prev_token) {
                        msg = Some(
                            "Les deux-points sont précédés d'une espace insécable.".to_string(),
                        );
                        suggestion_text = Some(format!("{prev_token_to_change}{NBSP}:"));
                    }
                } else if token == "»" {
                    if prev_prev_token == "«" {
                        msg = Some(
                            "Les guillemets sont toujours accompagnés d'une espace insécable."
                                .to_string(),
                        );
                        suggestion_text = Some(format!("«{NBSP}{prev_token_to_change}{NBSP}»"));
                        i_from = i - 2;
                    } else {
                        msg = Some(
                            "Le guillemet fermant est précédé d'une espace insécable.".to_string(),
                        );
                        suggestion_text = Some(format!("{prev_token_to_change}{NBSP}»"));
                    }
                }
            }

            if prev_token == "«" {
                if token.is_empty() {
                    msg =
                        Some("Le guillemet ouvrant est suivi d'une espace insécable.".to_string());
                    suggestion_text = Some(format!("«{NBSP}"));
                    i_to = i - 1;
                } else if !self.is_allowed_whitespace_char(&all, i as isize) {
                    let next_token = if i + 1 < all.len() {
                        all[i + 1].surface().to_string()
                    } else {
                        String::new()
                    };
                    if next_token != "»" {
                        msg = Some(
                            "Le guillemet ouvrant est suivi d'une espace insécable.".to_string(),
                        );
                        if !all[i].is_whitespace {
                            suggestion_text = Some(format!("«{NBSP}{token}"));
                        } else {
                            suggestion_text = Some(format!("«{NBSP}"));
                        }
                    }
                }
            }

            if let Some(msg) = msg {
                let from_pos = all[i_from].start_pos;
                let to_pos = all[i_to].end_pos();
                let mut m = Match::new(
                    self.rule_id(),
                    Option::<String>::None,
                    &msg,
                    Some(SHORT_MESSAGE.to_string()),
                    TextRange::new(sentence_offset + from_pos, sentence_offset + to_pos),
                    Vec::new(),
                    CATEGORY_ID,
                    CATEGORY_NAME,
                )
                .with_metadata(DESCRIPTION, "typographical", 0)
                .with_match_type("Other");
                if let Some(text) = suggestion_text {
                    m.suggestions.push(Suggestion {
                        value: text,
                        short_description: None,
                    });
                }
                rule_matches.push(m);
            }
            prev_prev_token = prev_token;
            prev_token = token;
        }
        rule_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anti_patterns_compile() {
        let rule = FrenchQuestionWhitespaceRule::new(false);
        assert_eq!(rule.anti_patterns.len(), 7);
    }
}
