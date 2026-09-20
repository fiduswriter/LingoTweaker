//! Port of `AbstractStyleTooOftenUsedWordRule` and the English
//! `StyleTooOftenUsedVerb/Noun/AdjectiveRule` (all default off): stylistic
//! hints when a lemma exceeds a share of all counted words.

use std::collections::HashMap;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

const MIN_WORD_COUNT: usize = 100;
const DEFAULT_MIN_PERCENT: u32 = 5;
const CATEGORY_ID: &str = "CREATIVE_WRITING";
const CATEGORY_NAME: &str = "Stylistic hints for creative writing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    Verb,
    Noun,
    Adjective,
}

/// Language dimension of the shared core rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleLang {
    En,
    De,
}

impl StyleKind {
    pub fn rule_id(self, lang: StyleLang) -> &'static str {
        match (self, lang) {
            (StyleKind::Verb, StyleLang::En) => "TOO_OFTEN_USED_VERB_EN",
            (StyleKind::Noun, StyleLang::En) => "TOO_OFTEN_USED_NOUN_EN",
            (StyleKind::Adjective, StyleLang::En) => "TOO_OFTEN_USED_ADJECTIVE_EN",
            (StyleKind::Verb, StyleLang::De) => "TOO_OFTEN_USED_VERB_DE",
            (StyleKind::Noun, StyleLang::De) => "TOO_OFTEN_USED_NOUN_DE",
            (StyleKind::Adjective, StyleLang::De) => "TOO_OFTEN_USED_ADJECTIVE_DE",
        }
    }

    fn description(self, lang: StyleLang) -> &'static str {
        match (self, lang) {
            (StyleKind::Verb, StyleLang::En) => "Statistical Style Analysis: Overused Verb",
            (StyleKind::Noun, StyleLang::En) => "Statistical Style Analysis: Overused Noun",
            (StyleKind::Adjective, StyleLang::En) => {
                "Statistical Style Analysis: Overused Adjective"
            }
            (StyleKind::Verb, StyleLang::De) => {
                "Statistische Stilanalyse: Zu häufig genutztes Verb"
            }
            (StyleKind::Noun, StyleLang::De) => {
                "Statistische Stilanalyse: Zu häufig genutztes Substantiv"
            }
            (StyleKind::Adjective, StyleLang::De) => {
                "Statistische Stilanalyse: Zu häufig genutztes Adjektiv"
            }
        }
    }

    fn limit_message(self, limit: u32, lang: StyleLang) -> String {
        match (self, lang) {
            (StyleKind::Verb, StyleLang::En) => format!(
                "The verb is used more than {limit}% times of all verbs. It may be better to replace it with a synonym."
            ),
            (StyleKind::Noun, StyleLang::En) => format!(
                "The noun is used more than {limit}% times of all nouns. It may be better to replace it with a synonym."
            ),
            (StyleKind::Adjective, StyleLang::En) => format!(
                "The adjective is used more than {limit}% times of all adjectives. It may be better to replace it with a synonym."
            ),
            (StyleKind::Verb, StyleLang::De) => format!(
                "Das Verb wird häufiger verwendet als {limit}% aller Verben. Möglicherweise ist es besser es durch ein Synonym zu ersetzen."
            ),
            (StyleKind::Noun, StyleLang::De) => format!(
                "Das Substantiv wird häufiger verwendet als {limit}% aller Substantive. Möglicherweise ist es besser es durch ein Synonym zu ersetzen."
            ),
            (StyleKind::Adjective, StyleLang::De) => format!(
                "Das Adjektiv wird häufiger verwendet als {limit}% aller Adjektive . Möglicherweise ist es besser es durch ein Synonym zu ersetzen."
            ),
        }
    }

    fn lemma_prefix(self, lang: StyleLang) -> &'static str {
        match (self, lang) {
            (StyleKind::Verb, StyleLang::En) => "VB",
            (StyleKind::Noun, StyleLang::En) => "NN",
            (StyleKind::Adjective, StyleLang::En) => "JJ",
            (StyleKind::Verb, StyleLang::De) => "VER:",
            (StyleKind::Noun, StyleLang::De) => "SUB:",
            (StyleKind::Adjective, StyleLang::De) => "ADJ:",
        }
    }

    fn is_to_counted_word(self, token: &AnalyzedTokenReadings, lang: StyleLang) -> bool {
        token.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .is_some_and(|t| t.starts_with(self.lemma_prefix(lang)))
        })
    }

    fn is_exception(self, tokens: &[&AnalyzedTokenReadings], n: usize, lang: StyleLang) -> bool {
        let token = tokens[n];
        let starts = |prefix: &str| {
            token
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(prefix)))
        };
        if lang == StyleLang::De {
            return match self {
                StyleKind::Verb => {
                    starts("VER:MOD") || starts("VER:AUX") || starts("ART") || starts("ADJ")
                }
                StyleKind::Noun => {
                    starts("PRO:")
                        || token.surface() == "Ich"
                        || token.surface() == "Aber"
                        || token.surface() == "Ja"
                        || (n < tokens.len() - 1
                            && (token.surface() == "Frau" || token.surface() == "Herr")
                            && (tokens[n + 1].has_pos_tag_starting_with("EIG:")
                                || is_pos_tag_unknown(tokens[n + 1])))
                }
                StyleKind::Adjective => starts("PRO:") || starts("ADV:") || starts("ZUS"),
            };
        }
        match self {
            StyleKind::Verb => {
                let has_lemma = token.readings.iter().any(|r| {
                    r.stem
                        .as_deref()
                        .is_some_and(|l| ["be", "have", "do"].contains(&l))
                });
                has_lemma || starts("IN") || starts("NN")
            }
            StyleKind::Noun => {
                starts("NNP") || starts("IN") || starts("JJ") || starts("RB") || starts("VB")
            }
            StyleKind::Adjective => {
                starts("RB") || starts("IN") || starts("CD") || starts("DT") || starts("NN")
            }
        }
    }

    fn to_added_lemma(self, token: &AnalyzedTokenReadings, lang: StyleLang) -> Option<String> {
        token
            .readings
            .iter()
            .find(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| t.starts_with(self.lemma_prefix(lang)))
            })
            .and_then(|r| r.stem.clone())
    }
}

/// `AnalyzedTokenReadings.isPosTagUnknown`.
fn is_pos_tag_unknown(token: &AnalyzedTokenReadings) -> bool {
    token.is_pos_tag_unknown
}

/// `AnalyzedTokenReadings.isNonWord` (`NON_WORD_REGEX.matcher(token).matches()`).
pub(crate) fn is_non_word(token: &str) -> bool {
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => matches!(
            c,
            '.' | '?'
                | '!'
                | '…'
                | ':'
                | ';'
                | ','
                | '~'
                | '’'
                | '\''
                | '"'
                | '„'
                | '“'
                | '”'
                | '»'
                | '«'
                | '‚'
                | '‘'
                | '›'
                | '‹'
                | '('
                | ')'
                | '['
                | ']'
                | '-'
                | '–'
                | '—'
                | '*'
                | '×'
                | '∗'
                | '·'
                | '+'
                | '÷'
                | '/'
                | '='
        ),
        _ => false,
    }
}

/// `AbstractStyleTooOftenUsedWordRule.match` for one kind (`minPercent` = 5).
pub fn check(sentences: &[AnalyzedSentence], kind: StyleKind) -> Vec<Match> {
    check_with(sentences, kind, StyleLang::En)
}

/// German `StyleTooOftenUsed*Rule` variants.
pub fn check_de(sentences: &[AnalyzedSentence], kind: StyleKind) -> Vec<Match> {
    check_with(sentences, kind, StyleLang::De)
}

pub fn check_with(sentences: &[AnalyzedSentence], kind: StyleKind, lang: StyleLang) -> Vec<Match> {
    let mut word_map: HashMap<String, u32> = HashMap::new();
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for (token_index, token) in tokens.iter().enumerate().skip(1) {
            let token = *token;
            if !token.is_whitespace
                && !is_non_word(token.surface())
                && kind.is_to_counted_word(token, lang)
                && !kind.is_exception(&tokens, token_index, lang)
            {
                if let Some(lemma) = kind.to_added_lemma(token, lang) {
                    *word_map.entry(lemma).or_insert(0) += 1;
                }
            }
        }
    }
    let num_words: u32 = word_map.values().sum();
    if (num_words as usize) < MIN_WORD_COUNT {
        return Vec::new();
    }
    let mut too_often: Vec<String> = word_map
        .iter()
        .filter(|(_, count)| (**count * 100) / num_words >= DEFAULT_MIN_PERCENT)
        .map(|(lemma, _)| lemma.clone())
        .collect();
    too_often.sort();
    if too_often.is_empty() {
        return Vec::new();
    }
    let mut rule_matches = Vec::new();
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for (token_index, token) in tokens.iter().enumerate().skip(1) {
            let token = *token;
            if !token.is_whitespace
                && !is_non_word(token.surface())
                && kind.is_to_counted_word(token, lang)
                && !kind.is_exception(&tokens, token_index, lang)
            {
                if let Some(lemma) = kind.to_added_lemma(token, lang) {
                    if too_often.contains(&lemma) {
                        rule_matches.push(
                            Match::new(
                                kind.rule_id(lang),
                                Option::<String>::None,
                                kind.limit_message(DEFAULT_MIN_PERCENT, lang),
                                Option::<String>::None,
                                TextRange::new(
                                    sentence.offset + token.start_pos,
                                    sentence.offset + token.end_pos(),
                                ),
                                Vec::<Suggestion>::new(),
                                CATEGORY_ID,
                                if lang == StyleLang::De {
                                    "Stiltipps für kreatives Schreiben"
                                } else {
                                    CATEGORY_NAME
                                },
                            )
                            .with_metadata(
                                kind.description(lang),
                                "style",
                                0,
                            ),
                        );
                        break;
                    }
                }
            }
        }
    }
    rule_matches
}
