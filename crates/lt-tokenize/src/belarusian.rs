//! `org.languagetool.tokenizers.be.BelarusianWordTokenizer`: the base
//! `WordTokenizer` with the apostrophes `'`, `’` and `ʼ` removed from the
//! tokenizing characters (they are part of the word); after
//! `joinEMailsAndUrls`, every token longer than one character has its
//! typographic apostrophe `’` normalized to `'`.

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

/// `BelarusianWordTokenizer`.
pub struct BelarusianWordTokenizer;

impl BelarusianWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

impl Default for BelarusianWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `BelarusianWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let characters = base_tokenizing_characters().replace(['\'', '’', 'ʼ'], "");
    let raw = string_tokenize(text, &characters);
    let mut output = Vec::with_capacity(raw.len());
    for token in join_emails_and_urls(raw) {
        if token.chars().count() > 1 {
            output.push(token.replace('’', "'"));
        } else {
            output.push(token);
        }
    }
    output
}
