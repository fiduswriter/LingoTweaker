//! Port of `WordRepeatBeginningRule` + `EnglishWordRepeatBeginningRule`
//! (`ENGLISH_WORD_REPEAT_BEGINNING_RULE`): three successive sentences
//! beginning with the same word / two successive sentences beginning with
//! the same adverb.

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

const RULE_ID: &str = "ENGLISH_WORD_REPEAT_BEGINNING_RULE";
const DESCRIPTION: &str = "Successive sentences beginning with the same word";
const SHORT_ADV: &str = "Two successive sentences begin with the same adverb.";
const SHORT_WORD: &str = "Three successive sentences begin with the same word.";
const THESAURUS: &str = "Consider rewording the sentence or use a thesaurus to find a synonym.";

/// The Java sets iterate in `HashSet` bucket order; these lists are the
/// iteration order of the corresponding Java `HashSet`s (capacity 16).
const ADD_ADVERBS: [&str; 5] = ["Besides", "Moreover", "Also", "Additionally", "Furthermore"];
const CONTRAST_ADVERBS: [&str; 3] = ["Nonetheless", "Alternatively", "Nevertheless"];
const EMPHASIS_ADVERBS: [&str; 7] = [
    "Absolutely",
    "Clearly",
    "Obviously",
    "Definitely",
    "Indeed",
    "Importantly",
    "Undoubtedly",
];
const EXPLAIN_ADVERBS: [&str; 3] = ["Especially", "Specifically", "Particularly"];
const ADD_EXPRESSIONS: [&str; 2] = ["In addition", "As well as"];
const CONTRAST_EXPRESSIONS: [&str; 2] = ["Even so", "On the other hand"];

fn is_adverb(token: &str) -> bool {
    ADD_ADVERBS.contains(&token)
        || CONTRAST_ADVERBS.contains(&token)
        || EMPHASIS_ADVERBS.contains(&token)
        || EXPLAIN_ADVERBS.contains(&token)
}

/// `WordRepeatBeginningRule.isException` + English override.
fn is_exception(token: &str) -> bool {
    matches!(
        token,
        ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️" | "The" | "A" | "An"
    )
}

fn different_adverbs(adverb: &str, adverbs: &[&str]) -> Vec<String> {
    adverbs
        .iter()
        .filter(|a| **a != adverb)
        .map(|a| (*a).to_string())
        .collect()
}

/// `EnglishWordRepeatBeginningRule.getSuggestions`.
fn suggestions(token: &AnalyzedTokenReadings) -> Vec<String> {
    let tok = token.surface();
    if token.has_pos_tag("PRP") {
        let adapted = if tok == "I" {
            tok.to_string()
        } else {
            tok.to_lowercase()
        };
        return vec![
            format!("Furthermore, {adapted}"),
            format!("Likewise, {adapted}"),
            format!("Not only that, but {adapted}"),
        ];
    }
    if ADD_ADVERBS.contains(&tok) {
        let mut out = different_adverbs(tok, &ADD_ADVERBS);
        out.extend(ADD_EXPRESSIONS.iter().map(|s| (*s).to_string()));
        return out;
    }
    if CONTRAST_ADVERBS.contains(&tok) {
        let mut out = different_adverbs(tok, &CONTRAST_ADVERBS);
        out.extend(CONTRAST_EXPRESSIONS.iter().map(|s| (*s).to_string()));
        return out;
    }
    if EMPHASIS_ADVERBS.contains(&tok) {
        return different_adverbs(tok, &EMPHASIS_ADVERBS);
    }
    if EXPLAIN_ADVERBS.contains(&tok) {
        return different_adverbs(tok, &EXPLAIN_ADVERBS);
    }
    Vec::new()
}

/// `WordRepeatBeginningRule.match` over all sentences.
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
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
                    && !is_exception(&token)
                    && !is_exception(tokens[2].surface())
                    && !is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(|p| trimmed_ends_like_sentence(&p.text))
                {
                    let short_msg = if is_adverb(&token) {
                        Some(SHORT_ADV)
                    } else if before_last_token == token {
                        Some(SHORT_WORD)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {THESAURUS}");
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        let suggestions: Vec<Suggestion> = suggestions(tokens[1])
                            .into_iter()
                            .map(|value| Suggestion {
                                value,
                                short_description: None,
                            })
                            .collect();
                        rule_matches.push(
                            Match::new(
                                RULE_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                suggestions,
                                "REPETITIONS_STYLE",
                                "Repetitions (Style)",
                            )
                            .with_metadata(DESCRIPTION, "style", 0),
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

/// `prevSentence.getText().trim().matches(".+[.?!]$")`.
fn trimmed_ends_like_sentence(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() > 1
        && trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '?' | '!'))
}
