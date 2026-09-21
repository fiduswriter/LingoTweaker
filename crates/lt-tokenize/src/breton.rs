//! `org.languagetool.tokenizers.br.BretonWordTokenizer`: the base
//! `WordTokenizer` with Breton apostrophe handling. `c’h` (the trigraph) is
//! not split, and a word such as `n’eo` becomes `n’` + `eo` (the apostrophe
//! stays inside the token via the `\u{0001}\u{0001}BR@APOS` placeholder).

use fancy_regex::Regex;
use std::sync::OnceLock;

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

const APOS: &str = "\u{0001}\u{0001}BR@APOS\u{0001}\u{0001}";

fn pattern_1() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new("([Cc])['’‘ʼ]([Hh])").unwrap())
}

fn pattern_2() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(\p{L})['’‘ʼ]").unwrap())
}

/// `BretonWordTokenizer`.
pub struct BretonWordTokenizer;

impl BretonWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

impl Default for BretonWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `BretonWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let replaced = pattern_1()
        .replace_all(text, format!("$1{APOS}$2"))
        .into_owned();
    let replaced = pattern_2()
        .replace_all(&replaced, format!("$1{APOS} "))
        .into_owned();
    let token_list =
        join_emails_and_urls(string_tokenize(&replaced, &base_tokenizing_characters()));

    let mut tokens: Vec<String> = Vec::with_capacity(token_list.len());
    let mut it = token_list.into_iter();
    while let Some(word) = it.next() {
        let word = word.replace(APOS, "’");
        // Skip the next spurious white space after an apostrophe-final token.
        if word != "’" && word.ends_with('’') {
            let _ = it.next();
        }
        tokens.push(word);
    }
    tokens
}
