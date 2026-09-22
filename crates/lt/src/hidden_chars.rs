//! Port of `org.languagetool.rules.uk.HiddenCharacterRule` (`UK_HIDDEN_CHARS`):
//! finds the soft hyphen (`U+00AD`) inside a token.

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "UK_HIDDEN_CHARS";
const HIDDEN_CHAR: char = '\u{00AD}';
const DESCRIPTION: &str = "Приховані символи: знак м’якого перенесення";
const SHORT: &str = "Приховані символи";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";

fn suggestion(word: &str) -> String {
    let highlighted = word.replace(HIDDEN_CHAR, "-");
    format!(" містить невидимий знак м’якого перенесення: «{highlighted}», виправлення: ")
}

/// `HiddenCharacterRule.match` over one sentence.
pub fn check_sentence_uk(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    for tr in tokens.iter().filter(|t| !t.is_whitespace) {
        let token = tr.surface().to_string();
        if token.contains(HIDDEN_CHAR) {
            let replacement = token.replace(HIDDEN_CHAR, "");
            let msg = format!("{token}{}{replacement}", suggestion(&token));
            rule_matches.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    msg,
                    Some(SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + tr.start_pos,
                        sentence_offset + tr.end_pos(),
                    ),
                    vec![Suggestion {
                        value: replacement,
                        short_description: None,
                    }],
                    CATEGORY_ID,
                    CATEGORY_NAME,
                )
                .with_metadata(DESCRIPTION, "typographical", 0),
            );
        }
    }
    rule_matches
}
