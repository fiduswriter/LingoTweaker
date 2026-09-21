//! `org.languagetool.tokenizers.crh.CrimeanTatarWordTokenizer`: the base
//! `WordTokenizer` with the en-dash `–` added to the tokenizing characters;
//! a trailing hyphen on a multi-character token is split off as its own
//! token, then e-mails/URLs are rejoined.

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

/// `CrimeanTatarWordTokenizer`.
pub struct CrimeanTatarWordTokenizer;

impl CrimeanTatarWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

impl Default for CrimeanTatarWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `CrimeanTatarWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut characters = base_tokenizing_characters();
    characters.push('–'); // n-dash
    let raw = string_tokenize(text, &characters);
    let mut l: Vec<String> = Vec::with_capacity(raw.len());
    for token in raw {
        if token.chars().count() > 1 && token.ends_with('-') {
            let head: String = token.chars().take(token.chars().count() - 1).collect();
            l.push(head);
            l.push("-".to_string());
        } else {
            l.push(token);
        }
    }
    join_emails_and_urls(l)
}
