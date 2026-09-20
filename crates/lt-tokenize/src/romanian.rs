//! Romanian word tokenizer: port of
//! `org.languagetool.tokenizers.ro.RomanianWordTokenizer`.
//!
//! Like the base `WordTokenizer`, but the delimiter set is restricted (no
//! `=*#+…` and similar): `StringTokenizer(text, ROMANIAN_DELIMS, true)`
//! followed by the e-mail/URL joining passes.

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// `RomanianWordTokenizer.tokenize`'s delimiter string, verbatim.
pub fn romanian_tokenizing_characters() -> String {
    let mut s = String::from(
        "\u{0020}\u{00A0}\u{115f}\u{1160}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{200B}\u{200c}\u{200d}\u{200e}\u{200f}\u{2028}\u{2029}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{202f}\u{205F}\u{2060}\u{2061}\u{2062}\u{2063}\u{206A}\u{206b}\u{206c}\u{206d}\u{206E}\u{206F}\u{3000}\u{3164}\u{feff}\u{ffa0}\u{fff9}\u{fffa}\u{fffb}",
    );
    s.push_str(",.;()[]{}!?:\"'’‘„“”…\\/\t\n\r«»<>%°-|=");
    s
}

pub struct RomanianWordTokenizer;

impl RomanianWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let tokens = string_tokenize(text, &romanian_tokenizing_characters());
        join_emails_and_urls(tokens)
    }
}

impl Default for RomanianWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience wrapper mirroring `GalicianWordTokenizer::tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    RomanianWordTokenizer::new().tokenize(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_romanian_delimiters() {
        let tok = RomanianWordTokenizer::new();
        assert_eq!(
            tok.tokenize("Bună, lume!"),
            vec!["Bună", ",", " ", "lume", "!"]
        );
        // hyphen is a delimiter in Romanian, unlike the base flag set
        assert_eq!(tok.tokenize("nu-i"), vec!["nu", "-", "i"]);
    }
}
