//! Italian sentence-level Java rules that do not fit the shared modules
//! (`ItalianWordRepeatRule`).

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

pub const WORD_REPEAT_RULE_ID: &str = "ITALIAN_WORD_REPEAT_RULE";
pub const WORD_REPEAT_DESCRIPTION: &str = "Parola ripetuta (es. 'casa casa')";
pub const WORD_REPEAT_SHORT: &str = "Ripetizione";
pub const WORD_REPEAT_MESSAGE: &str = "Possibile errore di battitura: parola ripetuta";

/// `ItalianWordRepeatRule.ignore`: the Italian fixed pairs plus the base
/// `WordRepeatRule` name list.
fn italian_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    for word in ["così", "passo", "piano", "via"] {
        if position > 0
            && tokens[position - 1].surface() == word
            && tokens[position].surface() == word
        {
            return true;
        }
    }
    // `WordRepeatRule.ignore`
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

/// `ItalianWordRepeatRule.match` over one sentence.
pub fn word_repeat_sentence(
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
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !italian_ignore(&view, i) {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    WORD_REPEAT_RULE_ID,
                    Option::<String>::None,
                    WORD_REPEAT_MESSAGE,
                    Some(WORD_REPEAT_SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "MISC",
                    "Altri",
                )
                .with_metadata(WORD_REPEAT_DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}
