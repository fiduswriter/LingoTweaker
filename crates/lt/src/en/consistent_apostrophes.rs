//! Port of `ConsistentApostrophesRule` (`EN_CONSISTENT_APOS`, default
//! `temp_off`): mixed typewriter/typographic apostrophes in a text.

use lt_core::{AnalyzedSentence, Match, Suggestion, TextRange};

const RULE_ID: &str = "EN_CONSISTENT_APOS";
const DESCRIPTION: &str =
    "Checks if the two types of apostrophes (' and ’) are used consistently in a text.";
const TYPEWRITER_MESSAGE: &str = "You used a typewriter-style apostrophe here, but a typographic apostrophe elsewhere in this text.";
const TYPOGRAPHIC_MESSAGE: &str = "You used a typographic apostrophe here, but a typewriter-style apostrophe elsewhere in this text.";
const CONCESSION: &str =
    " Both are correct, but consider using the same type everywhere in your text.";

fn has_two_apostrophe_types(sentences: &[AnalyzedSentence]) -> bool {
    let mut typewriter = false;
    let mut typographic = false;
    for sentence in sentences {
        for token in &sentence.tokens {
            let surface = token.surface();
            if surface.contains('\'') && !token.has_typographic_apostrophe {
                typewriter = true;
            } else if surface.contains('\'') && token.has_typographic_apostrophe {
                typographic = true;
            }
            if typewriter && typographic {
                return true;
            }
        }
    }
    false
}

/// `ConsistentApostrophesRule.match` over all sentences.
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut matches = Vec::new();
    if !has_two_apostrophe_types(sentences) {
        return matches;
    }
    for sentence in sentences {
        for token in &sentence.tokens {
            let surface = token.surface();
            let (message, replacement) =
                if surface.contains('\'') && !token.has_typographic_apostrophe {
                    (TYPEWRITER_MESSAGE, surface.replace('\'', "\u{2019}"))
                } else if surface.contains('\'') && token.has_typographic_apostrophe {
                    (TYPOGRAPHIC_MESSAGE, surface.to_string())
                } else {
                    continue;
                };
            matches.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    format!("{message}{CONCESSION}"),
                    Option::<String>::None,
                    TextRange::new(
                        sentence.offset + token.start_pos,
                        sentence.offset + token.end_pos(),
                    ),
                    vec![Suggestion {
                        value: replacement,
                        short_description: None,
                    }],
                    "TYPOGRAPHY",
                    "Typography",
                )
                .with_metadata(DESCRIPTION, "typographical", 0),
            );
        }
    }
    matches
}
