//! `org.languagetool.tokenizers.eo.EsperantoWordTokenizer`: the base
//! `WordTokenizer` with Esperanto apostrophe handling. A word such as
//! `dank'` keeps the apostrophe inside the token via the
//! `\u{0001}\u{0001}EO@APOS` placeholders (Java's hack).

use fancy_regex::Regex;
use std::sync::OnceLock;

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

const LETTERS: &str = "a-zA-ZĉĝĥĵŝŭĈĜĤĴŜŬ";
const APOS1: &str = "\u{0001}\u{0001}EO@APOS1\u{0001}\u{0001}";
const APOS2: &str = "\u{0001}\u{0001}EO@APOS2\u{0001}\u{0001}";

fn pattern_1() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(&format!("(?<!')\\b([{LETTERS}]+)'(?![{LETTERS}-])")).unwrap())
}

fn pattern_2() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(&format!("(?<!')\\b([{LETTERS}]+)'(?=[{LETTERS}-])")).unwrap())
}

/// `EsperantoWordTokenizer`.
pub struct EsperantoWordTokenizer;

impl EsperantoWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

impl Default for EsperantoWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `EsperantoWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let replaced = pattern_1()
        .replace_all(text, format!("$1{APOS1}"))
        .into_owned();
    let replaced = pattern_2()
        .replace_all(&replaced, format!("$1{APOS2} "))
        .into_owned();
    let token_list =
        join_emails_and_urls(string_tokenize(&replaced, &base_tokenizing_characters()));

    let mut tokens: Vec<String> = Vec::with_capacity(token_list.len());
    let mut it = token_list.into_iter();
    while let Some(mut word) = it.next() {
        if word.ends_with(APOS2) {
            // Skip the next spurious white space.
            let _ = it.next();
        }
        word = word.replace(APOS1, "'");
        word = word.replace(APOS2, "'");
        tokens.push(word);
    }
    tokens
}
