//! `org.languagetool.tokenizers.ru.RussianWordTokenizer`: the base
//! `WordTokenizer` with `б/у` and `б/н` kept as single tokens (the `/` is a
//! delimiter, so the pairs are shielded before tokenization) and the
//! ` .`-sentinel dance of the Java override reproduced literally.

use crate::wordtokenizer::{base_tokenizing_characters, join_emails_and_urls, string_tokenize};

const SOCR_BU: &str = "\u{1}\u{1}SOCR_BU\u{1}\u{1}";
const SOCR_BN: &str = "\u{1}\u{1}SOCR_BN\u{1}\u{1}";
const SP_DDOT_SP: &str = "\u{1}\u{1}SP_DDOT_SP\u{1}\u{1}";
const SP_DOT_SP: &str = "\u{1}\u{1}SP_DOT_SP\u{1}\u{1}";
const SP_DOT: &str = "\u{1}\u{1}SP_DOT\u{1}\u{1}";

/// `RussianWordTokenizer`.
pub struct RussianWordTokenizer;

impl RussianWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

impl Default for RussianWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `RussianWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let aux_text = text
        .replace("б/у", SOCR_BU)
        .replace("б/н", SOCR_BN)
        .replace(" .. ", SP_DDOT_SP)
        .replace(" . ", SP_DOT_SP)
        .replace(" .", &format!(" {SP_DOT}"))
        .replace(SP_DDOT_SP, " .. ")
        .replace(SP_DOT_SP, " . ");

    let raw = string_tokenize(&aux_text, &base_tokenizing_characters());
    let l: Vec<String> = raw
        .into_iter()
        .map(|s| {
            s.replace(SOCR_BU, "б/у")
                .replace(SOCR_BN, "б/н")
                .replace(SP_DOT, ".")
        })
        .collect();
    join_emails_and_urls(l)
}
