//! Romanian Java-coded built-in rules (`Romanian.getRelevantRules`):
//! `WordRepeatRule` (6, generic base), `RomanianWordRepeatBeginningRule` (8),
//! `SimpleReplaceRule` (9) and `CompoundRule` (10).

use std::path::Path;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::simple_replace::{CaseSensitivity, SimpleReplaceConfig, TokenException};
use crate::wordutil::{eq_ignore_case, is_word};

// ---------------------------------------------------------------------------
// WordRepeatRule (`WORD_REPEAT_RULE`), the generic base class
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_RULE_ID: &str = "WORD_REPEAT_RULE";
pub const WORD_REPEAT_DESCRIPTION: &str = "Cuvânt repetat (ex: „voi voi”)";
pub const WORD_REPEAT_SHORT: &str = "Cuvânt repetat";
pub const WORD_REPEAT_MESSAGE: &str = "Posibilă greșeală: ați repetat un cuvânt";

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

/// `WordRepeatRule.match` over one sentence. The generic base class has no
/// language override, so this is the `WordRepeatRule` behaviour unchanged.
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
                        "Diverse",
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

// ---------------------------------------------------------------------------
// RomanianWordRepeatBeginningRule
// (`ROMANIAN_WORD_REPEAT_BEGINNING_RULE`)
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_BEGINNING_ID: &str = "ROMANIAN_WORD_REPEAT_BEGINNING_RULE";
pub const WORD_REPEAT_BEGINNING_DESCRIPTION: &str = "Propoziții succesive încep cu același cuvânt.";
const WRB_SHORT_ADV: &str = "Două propoziții succesive încep cu același adverb.";
const WRB_SHORT_WORD: &str = "Trei propoziții succesive încep cu același cuvânt.";
const WRB_THESAURUS: &str = "Consider rewording the sentence or use a thesaurus to find a synonym.";

/// `RomanianWordRepeatBeginningRule.isAdverb`: `allowAmbiguousAdverbs()` is
/// false, so any non-`null` POS tag not starting with `G` makes the token a
/// non-adverb.
fn romanian_is_adverb(tr: &AnalyzedTokenReadings) -> bool {
    let mut is_adverb = false;
    for reading in &tr.readings {
        if let Some(tag) = &reading.pos_tag {
            if tag.starts_with('G') {
                is_adverb = true;
            } else {
                return false;
            }
        }
    }
    is_adverb
}

/// `WordRepeatBeginningRule.isException` (base, not overridden by Romanian).
fn wrb_is_exception(token: &str) -> bool {
    matches!(token, ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️")
}

fn ends_like_sentence(sentence: &AnalyzedSentence) -> bool {
    let trimmed = sentence.text.trim();
    trimmed.len() > 1
        && trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '?' | '!'))
}

/// `RomanianWordRepeatBeginningRule.match` over all sentences (text level).
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            token = tokens[1].surface().to_string();
            if tokens.len() > 3 {
                let mut is_word = true;
                if token.chars().count() == 1 {
                    is_word = token.chars().next().is_some_and(char::is_alphabetic);
                }
                if is_word
                    && last_token == token
                    && !wrb_is_exception(&token)
                    && !wrb_is_exception(tokens[2].surface())
                    && !wrb_is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(ends_like_sentence)
                {
                    let short_msg = if romanian_is_adverb(tokens[1]) {
                        Some(WRB_SHORT_ADV)
                    } else if before_last_token == token {
                        Some(WRB_SHORT_WORD)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {WRB_THESAURUS}");
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        rule_matches.push(
                            Match::new(
                                WORD_REPEAT_BEGINNING_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                Vec::new(),
                                "REPETITIONS_STYLE",
                                "Repetitions (Style)",
                            )
                            .with_metadata(
                                WORD_REPEAT_BEGINNING_DESCRIPTION,
                                "style",
                                0,
                            ),
                        );
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// SimpleReplaceRule (`RO_SIMPLE_REPLACE`)
// ---------------------------------------------------------------------------

/// The `AbstractSimpleReplaceRule2` instance for `/ro/replace.txt`.
pub fn simple_replace_instance(
    data_dir: &Path,
) -> Result<crate::simple_replace::SimpleReplaceRule> {
    crate::simple_replace::SimpleReplaceRule::from_files(
        &[data_dir.join("ro/rules/replace.txt")],
        SimpleReplaceConfig {
            rule_id: "RO_SIMPLE_REPLACE",
            description: "Cuvinte sau grupuri de cuvinte incorecte sau ieșite din uz",
            short: "Cuvânt incorect sau ieșit din uz",
            message: "'$match' este incorect sau ieșit din uz, folosiți $suggestions",
            suggestions_separator: " sau ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: "Diverse",
            issue_type: "Other",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}
