//! Polish Java-coded built-in rules (`Polish.getRelevantRules`). Stage 1
//! carries the generic `WordRepeatRule`; the Polish-specific classes
//! (`PolishWordRepeatRule`, `CompoundRule`, `SimpleReplaceRule`,
//! `WordCoherencyRule`, `DashRule`) follow in stage 3.

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

// ---------------------------------------------------------------------------
// WordRepeatRule (`WORD_REPEAT_RULE`), the generic base class
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_RULE_ID: &str = "WORD_REPEAT_RULE";
pub const WORD_REPEAT_DESCRIPTION: &str = "Powtórzenie wyrazu (np. „jest jest”)";
pub const WORD_REPEAT_SHORT: &str = "Powtórzenie wyrazu";
pub const WORD_REPEAT_MESSAGE: &str = "Prawdopodobna literówka: powtórzony wyraz";

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

/// `WordRepeatRule.match` over one sentence with the Polish
/// `MessagesBundle_pl` strings. The generic base class has no language
/// override, so the detection is the `WordRepeatRule` behaviour unchanged.
pub struct WordRepeatSentenceRule;

impl WordRepeatSentenceRule {
    pub fn new() -> Self {
        Self
    }

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
                        "Błędy różne",
                    )
                    .with_metadata(WORD_REPEAT_DESCRIPTION, "duplication", 1),
                );
            }
            prev_token = token;
        }
        rule_matches
    }
}

impl Default for WordRepeatSentenceRule {
    fn default() -> Self {
        Self::new()
    }
}
