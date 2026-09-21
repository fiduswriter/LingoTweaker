//! Greek word tokenizer: port of
//! `org.languagetool.tokenizers.el.GreekWordTokenizer` (the JFlex
//! `GreekWordTokenizerImpl`).
//!
//! The generated scanner splits on a Greek-specific delimiter set (a subset of
//! the base one: no `-`, `?`, `=`, `*`, `%`, `<`, `>`, `|`, `_`, …) and keeps
//! `ό,τι` as a single token. `joinEMailsAndUrls` runs afterwards like the base
//! `WordTokenizer`.

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// `GreekWordTokenizerImpl`'s `Delim` set, verbatim (JFlex longest-match with
/// the `ό,τι` literal handled in [`GreekWordTokenizer::tokenize`]).
pub fn greek_tokenizing_characters() -> String {
    let mut s = String::from(
        "\u{0020}\u{00A0}\u{115f}\u{1160}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{200B}\u{200c}\u{200d}\u{200e}\u{200f}\u{2028}\u{2029}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{202f}\u{205F}\u{2060}\u{2061}\u{2062}\u{2063}\u{206A}\u{206b}\u{206c}\u{206d}\u{206E}\u{206F}\u{3000}\u{3164}\u{feff}\u{ffa0}\u{fff9}\u{fffa}\u{fffb}",
    );
    s.push_str(",.;()[]{}!:\"'·’‘„“”…«»\\/\t\n");
    s
}

/// `ό,τι` kept as one token (the JFlex `Word` alternative).
const OTI: &str = "ό,τι";

pub struct GreekWordTokenizer;

impl GreekWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let delims = greek_tokenizing_characters();
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;
        let starts_oti = |i: usize| -> bool {
            let oti: Vec<char> = OTI.chars().collect();
            i + oti.len() <= n && chars[i..i + oti.len()] == oti[..]
        };
        while i < n {
            if starts_oti(i) {
                out.push(OTI.to_string());
                i += OTI.chars().count();
                continue;
            }
            if delims.contains(chars[i]) {
                out.push(chars[i].to_string());
                i += 1;
                continue;
            }
            let mut word = String::new();
            while i < n && !delims.contains(chars[i]) {
                word.push(chars[i]);
                i += 1;
            }
            out.push(word);
        }
        // The scanner path is exactly `string_tokenize` on the delimiter set
        // apart from the `ό,τι` literal; keep the e-mail/URL joining.
        join_emails_and_urls(out)
    }
}

impl Default for GreekWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience wrapper mirroring the other tokenizers' free function.
pub fn tokenize(text: &str) -> Vec<String> {
    GreekWordTokenizer::new().tokenize(text)
}

/// `string_tokenize` fallback is kept reachable for tests/parity checks.
pub fn tokenize_without_special(text: &str) -> Vec<String> {
    join_emails_and_urls(string_tokenize(text, &greek_tokenizing_characters()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_greek_delimiters() {
        let tok = GreekWordTokenizer::new();
        assert_eq!(
            tok.tokenize("Καλημέρα, κόσμε!"),
            vec!["Καλημέρα", ",", " ", "κόσμε", "!"]
        );
        // `?` is not a Greek delimiter (Greek uses `;`)
        assert_eq!(tok.tokenize("τι?"), vec!["τι?"]);
        assert_eq!(tok.tokenize("τι;"), vec!["τι", ";"]);
    }

    #[test]
    fn keeps_oti_together() {
        let tok = GreekWordTokenizer::new();
        assert_eq!(tok.tokenize("ό,τι αφορά"), vec!["ό,τι", " ", "αφορά"]);
        // a longer non-delimiter run wins over the literal, like JFlex
        assert_eq!(tok.tokenize("αό,τι"), vec!["αό", ",", "τι"]);
    }
}
