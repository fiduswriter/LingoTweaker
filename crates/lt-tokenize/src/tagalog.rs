//! `org.languagetool.language.tokenizers.TagalogWordTokenizer`: the base
//! `WordTokenizer` with `-` added to the tokenizing characters.

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

/// `TagalogWordTokenizer`.
pub struct TagalogWordTokenizer;

impl TagalogWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

impl Default for TagalogWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `TagalogWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut characters = base_tokenizing_characters();
    characters.push('-');
    join_emails_and_urls(string_tokenize(text, &characters))
}
