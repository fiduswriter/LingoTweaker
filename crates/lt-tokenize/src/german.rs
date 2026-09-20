//! German word tokenizer: port of `org.languagetool.tokenizers.de.
//! GermanWordTokenizer`, which adds `_` and `‚` to the base tokenizing
//! characters and otherwise inherits `WordTokenizer` (StringTokenizer split +
//! e-mail/URL joining).

use crate::wordtokenizer::{german_tokenizing_characters, join_emails_and_urls, string_tokenize};

pub struct GermanWordTokenizer;

impl Default for GermanWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl GermanWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        join_emails_and_urls(string_tokenize(text, &german_tokenizing_characters()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_the_java_base_tokenizer() {
        let t = GermanWordTokenizer::new();
        assert_eq!(
            t.tokenize("Hallo, Welt!"),
            vec!["Hallo", ",", " ", "Welt", "!"]
        );
    }

    #[test]
    fn underscore_is_a_delimiter_in_german() {
        let t = GermanWordTokenizer::new();
        assert_eq!(t.tokenize("a_b"), vec!["a", "_", "b"]);
    }

    #[test]
    fn single_quotation_mark_is_a_delimiter() {
        let t = GermanWordTokenizer::new();
        // ‚ (U+201A) is not a comma but a single quotation mark
        assert_eq!(t.tokenize("‚Hallo‘"), vec!["‚", "Hallo", "‘"]);
    }

    #[test]
    fn joins_url_with_protocol() {
        let t = GermanWordTokenizer::new();
        assert_eq!(
            t.tokenize("http://example.org/test"),
            vec!["http://example.org/test"]
        );
    }
}
