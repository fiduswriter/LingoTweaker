//! Port of `WhitespaceBeforePunctuationRule` (`WHITESPACE_PUNCTUATION`):
//! `:`/`;`/`%` preceded by whitespace. `Italian.getRelevantRules` is the only
//! registered user in the ported language set; the rule id/messages come from
//! the Italian `MessagesBundle_it` strings.

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "WHITESPACE_PUNCTUATION";

const DESCRIPTION: &str = "Utilizzo dello spazio prima di : ; %";
const NO_SPACE_BEFORE_COLON: &str = "Non inserire uno spazio prima dei due punti";
const NO_SPACE_BEFORE_SEMICOLON: &str = "Non inserire uno spazio prima del punto e virgola";
const NO_SPACE_BEFORE_PERCENTAGE: &str = "Non inserire uno spazio prima del segno di percentuale";

/// `AnalyzedTokenReadings.isWhitespace() || StringTools.isNonBreakingWhitespace`.
fn is_whitespace(token: &AnalyzedTokenReadings) -> bool {
    token.is_whitespace || token.surface() == "\u{00A0}"
}

/// `WhitespaceBeforePunctuationRule.match` over one sentence.
pub fn check_sentence_it(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    check_sentence_with(
        tokens,
        sentence_offset,
        DESCRIPTION,
        NO_SPACE_BEFORE_COLON,
        NO_SPACE_BEFORE_SEMICOLON,
        NO_SPACE_BEFORE_PERCENTAGE,
        "Tipografia",
    )
}

fn check_sentence_with(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    description: &str,
    no_space_before_colon: &str,
    no_space_before_semicolon: &str,
    no_space_before_percentage: &str,
    category_name: &str,
) -> Vec<Match> {
    let mut rule_matches: Vec<Match> = Vec::new();
    let mut prev_white = false;
    let mut prev_len = 0usize;
    for i in 0..tokens.len() {
        let token = tokens[i].surface();
        let is_ws = is_whitespace(&tokens[i]);
        let mut msg: Option<&str> = None;
        let mut suggestion_text: Option<&str> = None;
        if prev_white {
            if token == ":" {
                msg = Some(no_space_before_colon);
                suggestion_text = Some(":");
                // exception case for figures such as " : 0"
                if i + 2 < tokens.len()
                    && tokens[i + 1].is_whitespace
                    && tokens[i + 2]
                        .surface()
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_numeric())
                {
                    msg = None;
                }
            } else if token == ";" {
                msg = Some(no_space_before_semicolon);
                suggestion_text = Some(";");
            } else if i > 1 && token == "%" {
                let prev_prev_token = tokens[i - 2].surface();
                if prev_prev_token
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_numeric())
                {
                    msg = Some(no_space_before_percentage);
                    suggestion_text = Some("%");
                }
            }
        }
        if let Some(message) = msg {
            let from_pos = tokens[i - 1].start_pos;
            let to_pos = from_pos + 1 + prev_len;
            rule_matches.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    message,
                    Option::<String>::None,
                    TextRange::new(sentence_offset + from_pos, sentence_offset + to_pos),
                    vec![Suggestion {
                        value: suggestion_text.unwrap_or("").to_string(),
                        short_description: None,
                    }],
                    "TYPOGRAPHY",
                    category_name,
                )
                .with_metadata(description, "whitespace", 0),
            );
        }
        prev_white = is_ws;
        prev_len = token.len();
    }
    rule_matches
}
