//! `org.languagetool.tokenizers.nl.DutchWordTokenizer`: the base
//! `WordTokenizer` with the apostrophe-like quotes removed from the
//! tokenizing characters (so `oma's` stays one token) plus the quote
//! splitting of the base tokenizer.

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

/// `DutchWordTokenizer.QUOTES`.
const QUOTES: [char; 5] = ['\'', '`', '\u{2019}', '\u{2018}', '\u{00B4}'];

pub struct DutchWordTokenizer;

impl DutchWordTokenizer {
    /// `nlTokenizingChars`: `WordTokenizer.getTokenizingCharacters() + "_"`
    /// with the quote characters removed.
    pub fn tokenizing_characters() -> String {
        let mut chars = base_tokenizing_characters();
        chars.push('_');
        for quote in QUOTES {
            chars = chars.replace(quote, "");
        }
        chars
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let raw = string_tokenize(text, &Self::tokenizing_characters());
        let mut l: Vec<String> = Vec::new();
        for token in raw {
            if token.chars().count() > 1 {
                let starts = starts_with_quote(&token);
                let ends = ends_with_quote(&token);
                if starts && ends && token.chars().count() > 2 {
                    let mut chars = token.chars();
                    let first = chars.next().unwrap();
                    let last = token.chars().next_back().unwrap();
                    let middle: String = token
                        .chars()
                        .skip(1)
                        .take(token.chars().count() - 2)
                        .collect();
                    l.push(first.to_string());
                    l.push(middle);
                    l.push(last.to_string());
                } else if ends {
                    let mut trimmed = token.clone();
                    let mut removed = Vec::new();
                    while ends_with_quote(&trimmed) {
                        let last = trimmed.chars().next_back().unwrap();
                        removed.push(last.to_string());
                        trimmed = trimmed[..trimmed.len() - last.len_utf8()].to_string();
                    }
                    l.push(trimmed);
                    for quote in removed.into_iter().rev() {
                        l.push(quote);
                    }
                } else if starts {
                    let mut rest = token.as_str().to_string();
                    while starts_with_quote(&rest) {
                        let first = rest.chars().next().unwrap();
                        l.push(first.to_string());
                        rest = rest[first.len_utf8()..].to_string();
                    }
                    l.push(rest);
                } else {
                    l.push(token);
                }
            } else {
                l.push(token);
            }
        }
        join_emails_and_urls(l)
    }
}

fn starts_with_quote(token: &str) -> bool {
    token.chars().next().is_some_and(|c| QUOTES.contains(&c))
}

fn ends_with_quote(token: &str) -> bool {
    token
        .chars()
        .next_back()
        .is_some_and(|c| QUOTES.contains(&c))
}
