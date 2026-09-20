//! Port of `AvsAnRule` (`EN_A_VS_AN`): checks whether the indefinite article
//! before a word should be "a" or "an". Exception word lists come from
//! `/en/det_a.txt` and `/en/det_an.txt` (`AvsAnData`).

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use regex::Regex;

const RULE_ID: &str = "EN_A_VS_AN";
const DESCRIPTION: &str = "Use of 'a' vs. 'an'";
const MESSAGE_A_TO_AN: &str = "Use <suggestion>{suggestion}</suggestion> instead of '{article}' if the following word starts with a vowel sound, e.g. 'an article', 'an hour'.";
const MESSAGE_AN_TO_A: &str = "Use <suggestion>{suggestion}</suggestion> instead of '{article}' if the following word doesn't start with a vowel sound, e.g. 'a sentence', 'a university'.";
const SHORT_MESSAGE: &str = "Wrong article";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Miscellaneous";

static CLEANUP: LazyLock<Regex> = LazyLock::new(|| Regex::new("[^αa-zA-Z0-9.;,':]").unwrap());
static DELIM: LazyLock<Regex> = LazyLock::new(|| Regex::new("^(?:[-\"“'‘()\\[\\]]+)$").unwrap());
static DASH_QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[-']").unwrap());
static AN_PREFIXES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^(?:unidentif|uni[mn])[a-z]+$").unwrap());
static AN_EXCEPTION_PREFIXES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^(?:eu|one|uni|u[rst][aeiou])[a-z]*$").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Determiner {
    A,
    An,
    AOrAn,
    Unknown,
    None,
}

pub struct AvsAnRule {
    requires_a: HashSet<String>,
    requires_an: HashSet<String>,
}

impl AvsAnRule {
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            requires_a: load_words(&data_dir.join("en/rules/det_a.txt")),
            requires_an: load_words(&data_dir.join("en/rules/det_an.txt")),
        })
    }

    /// `AvsAnRule.match` over one sentence's tokens (absolute offsets).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut prev_token_index = 0usize;
        for i in 1..non_blank.len() {
            let token = non_blank[i];
            let prev_token_str = if prev_token_index > 0 {
                non_blank[prev_token_index].surface()
            } else {
                ""
            };
            let is_sentence_start = prev_token_index == 1;
            let (equals_a, equals_an) = if is_sentence_start {
                (
                    prev_token_str.eq_ignore_ascii_case("a"),
                    prev_token_str.eq_ignore_ascii_case("an"),
                )
            } else {
                (prev_token_str == "a", prev_token_str == "an")
            };
            if equals_a || equals_an {
                let determiner = self.correct_determiner(token);
                let message = if equals_a && determiner == Determiner::An {
                    let suggestion = if starts_with_uppercase(prev_token_str) {
                        "An"
                    } else {
                        "an"
                    };
                    Some(
                        MESSAGE_A_TO_AN
                            .replace("{suggestion}", suggestion)
                            .replace("{article}", prev_token_str),
                    )
                } else if equals_an && determiner == Determiner::A {
                    let suggestion = if starts_with_uppercase(prev_token_str) {
                        "A"
                    } else {
                        "a"
                    };
                    Some(
                        MESSAGE_AN_TO_A
                            .replace("{suggestion}", suggestion)
                            .replace("{article}", prev_token_str),
                    )
                } else {
                    None
                };
                if let Some(message) = message {
                    let suggestion = if equals_a { "an" } else { "a" };
                    let suggestion = if starts_with_uppercase(prev_token_str) {
                        uppercase_first(suggestion)
                    } else {
                        suggestion.to_string()
                    };
                    let start = sentence_offset + non_blank[prev_token_index].start_pos;
                    let end = sentence_offset + non_blank[prev_token_index].end_pos();
                    rule_matches.push(
                        Match::new(
                            RULE_ID,
                            Option::<String>::None,
                            message,
                            Some(SHORT_MESSAGE.to_string()),
                            TextRange::new(start, end),
                            vec![Suggestion {
                                value: suggestion,
                                short_description: None,
                            }],
                            CATEGORY_ID,
                            CATEGORY_NAME,
                        )
                        .with_metadata(DESCRIPTION, "misspelling", 1)
                        .with_match_type("Other"),
                    );
                }
            }
            let next_token = if i + 1 < non_blank.len() {
                non_blank[i + 1].surface()
            } else {
                ""
            };
            if token.has_pos_tag("DT") {
                prev_token_index = i;
            } else if next_token.chars().count() > 1 && DELIM.is_match(token.surface()) {
                // skip e.g. the quote in `an "industry party"`
            } else {
                prev_token_index = 0;
            }
        }
        rule_matches
    }

    /// `AvsAnRule.suggestAorAn`: `"a word"` / `"an word"`, or the unchanged
    /// word when the determiner is unknown (used by `+DT`/`+INDT` synthesis).
    pub fn suggest_a_or_an(&self, orig_word: &str) -> String {
        let token = token_readings(orig_word);
        match self.correct_determiner(&token) {
            Determiner::A | Determiner::AOrAn => {
                format!("a {}", lowercase_first_char_if_capitalized(orig_word))
            }
            Determiner::An => format!("an {}", lowercase_first_char_if_capitalized(orig_word)),
            _ => orig_word.to_string(),
        }
    }

    /// `AvsAnRule.getCorrectDeterminerFor`.
    fn correct_determiner(&self, token: &AnalyzedTokenReadings) -> Determiner {
        let mut word = token.surface().to_string();
        let parts: Vec<&str> = DASH_QUOTE.split(&word).collect();
        if !parts.is_empty() && !parts[0].eq_ignore_ascii_case("a") {
            word = parts[0].to_string();
        }
        if token.whitespace_before || word != "-" {
            word = CLEANUP.replace_all(&word, "").into_owned();
            if word.is_empty() {
                return Determiner::Unknown;
            }
        }
        let mut determiner = Determiner::None;
        if self.requires_a.contains(&word.to_lowercase()) || self.requires_a.contains(&word) {
            determiner = Determiner::A;
        }
        if self.requires_an.contains(&word.to_lowercase()) || self.requires_an.contains(&word) {
            determiner = if determiner == Determiner::A {
                Determiner::AOrAn
            } else {
                Determiner::An
            };
        }
        if determiner == Determiner::None {
            let Some(first) = word.chars().next() else {
                return Determiner::Unknown;
            };
            if lt_spell::morfologik::is_all_uppercase(&word)
                || lt_spell::morfologik::is_mixed_case(&word)
            {
                // all-uppercase abbreviations are pronounced unpredictably
                determiner = Determiner::Unknown;
            } else if AN_PREFIXES.is_match(&word)
                || (is_vowel(first) && !AN_EXCEPTION_PREFIXES.is_match(&word))
            {
                determiner = Determiner::An;
            } else {
                determiner = Determiner::A;
            }
        }
        determiner
    }
}

/// `AnalyzedTokenReadings` for a bare word (`suggestAorAn`; Java builds it
/// from a single reading without a POS tag, so `isPosTagUnknown` is true).
fn token_readings(word: &str) -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: vec![lt_core::AnalyzedToken::new(word.to_string(), None, None)],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: word.len(),
        is_whitespace: false,
        is_sentence_start: false,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: false,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: true,
    }
}

/// `StringTools.lowercaseFirstCharIfCapitalized`.
pub(crate) fn lowercase_first_char_if_capitalized(s: &str) -> String {
    let mut chars = s.chars();
    let is_capitalized = match chars.next() {
        Some(first) if first.is_uppercase() => {
            chars.all(|c| !c.is_alphabetic() || c.is_lowercase())
        }
        _ => false,
    };
    if !is_capitalized {
        return s.to_string();
    }
    let mut out = String::new();
    let mut done = false;
    for c in s.chars() {
        if !done && c.is_alphabetic() {
            out.extend(c.to_lowercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

fn is_vowel(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
}

fn starts_with_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `AvsAnData.loadWords`: trim, `#` comments, `*`-prefixed entries keep
/// their case, everything else is lower-cased.
fn load_words(path: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return set;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('*') {
            set.insert(rest.to_string());
        } else {
            set.insert(line.to_lowercase());
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;
    use lt_data::PathExt as _;

    fn tok(surface: &str) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(surface.to_string(), None, None)],
            chunk_tags: Vec::new(),
            whitespace_before: true,
            start_pos: 0,
            raw_byte_len: surface.len(),
            is_whitespace: false,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: false,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        }
    }

    #[test]
    fn determiner_examples() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.join("en/rules/det_an.txt").lt_exists() {
            return;
        }
        let rule = AvsAnRule::from_data(&dir).unwrap();
        assert_eq!(rule.correct_determiner(&tok("hour")), Determiner::An);
        assert_eq!(rule.correct_determiner(&tok("university")), Determiner::A);
        assert_eq!(
            rule.correct_determiner(&tok("α-evaluating")),
            Determiner::An
        );
        assert_eq!(
            rule.correct_determiner(&tok("over-reaction")),
            Determiner::An
        );
        assert_eq!(rule.correct_determiner(&tok("non-default")), Determiner::A);
        // all-uppercase because it contains no letters (Java `isAllUppercase`)
        assert_eq!(
            rule.correct_determiner(&tok("5-yoctogramme")),
            Determiner::Unknown
        );
    }
}
