//! `org.languagetool.tokenizers.es.SpanishWordTokenizer`: the base
//! `WordTokenizer` dictionary-aware hyphen handling plus the Spanish
//! decimal-point/comma and ordinal re-joins.

use regex::Regex;
use std::sync::OnceLock;

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// The Spanish `wordCharacters` class (Java `\d` is ASCII-only without
/// `UNICODE_CHARACTER_CLASS`, `\p{L}` is Unicode).
fn spanish_word_characters() -> &'static str {
    "§©@€£$_\\p{L}0-9·\\-\u{0300}-\u{036F}\u{00A8}\u{2070}-\u{209F}°%‰‱&\u{FFFD}\u{00AD}\u{00AC}"
}

fn tokenizer_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        let wc = spanish_word_characters();
        Regex::new(&format!("[{wc}]+|[^{wc}]")).unwrap()
    })
}

fn decimal_point() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"([0-9])\.([0-9])").unwrap())
}

fn decimal_comma() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"([0-9]),([0-9])").unwrap())
}

fn ordinal_point() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b([0-9]+)\.(º|ª|o|a|er|os|as)\b").unwrap())
}

const DECIMAL_POINT_SENTINEL: &str = "xxES_DECIMAL_POINTxx";
const DECIMAL_COMMA_SENTINEL: &str = "xxES_DECIMAL_COMMAxx";
const ORDINAL_POINT_SENTINEL: &str = "xxES_ORDINAL_POINTxx";

/// `SpanishWordTokenizer` over a tagger callback
/// (`SpanishTagger.INSTANCE.tag([word]).get(0).isTagged()`).
pub struct SpanishWordTokenizer<'a> {
    is_tagged: crate::english::IsTagged<'a>,
}

impl<'a> SpanishWordTokenizer<'a> {
    pub fn new(is_tagged: crate::english::IsTagged<'a>) -> Self {
        Self { is_tagged }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        // replace hyphen, non-break hyphen -> hyphen-minus
        let mut aux = text.replace(['\u{2010}', '\u{2011}'], "-");
        aux = decimal_point()
            .replace_all(
                &aux,
                format!("${{1}}{DECIMAL_POINT_SENTINEL}${{2}}").as_str(),
            )
            .into_owned();
        aux = decimal_comma()
            .replace_all(
                &aux,
                format!("${{1}}{DECIMAL_COMMA_SENTINEL}${{2}}").as_str(),
            )
            .into_owned();
        // Java `\b` + `\d` with UNICODE_CHARACTER_CLASS: the ordinal branch
        // returns group 1 + sentinel + group 2.
        aux = ordinal_point()
            .replace_all(&aux, |caps: &regex::Captures| {
                format!(
                    "{}{}{}",
                    caps.get(1).unwrap().as_str(),
                    ORDINAL_POINT_SENTINEL,
                    caps.get(2).unwrap().as_str()
                )
            })
            .into_owned();

        let mut tokens: Vec<String> = Vec::new();
        for m in tokenizer_pattern().find_iter(&aux) {
            let s = m.as_str();
            if !tokens.is_empty()
                && s.chars().count() == 1
                && s.chars()
                    .next()
                    .is_some_and(|c| ('\u{FE00}'..='\u{FE0F}').contains(&c))
            {
                let last = tokens.last_mut().unwrap();
                last.push_str(s);
                continue;
            }
            let s = s
                .replace(DECIMAL_POINT_SENTINEL, ".")
                .replace(DECIMAL_COMMA_SENTINEL, ",")
                .replace(ORDINAL_POINT_SENTINEL, ".");
            tokens.extend(self.words_to_add(&s));
        }
        join_emails_and_urls(tokens)
    }

    /// `SpanishWordTokenizer.wordsToAdd`: hyphenated words are looked up in
    /// the dictionary; unknown ones are split at the hyphen.
    fn words_to_add(&self, s: &str) -> Vec<String> {
        let mut out = Vec::new();
        if s.is_empty() {
            return out;
        }
        if !s.contains('-') {
            out.push(s.to_string());
            return out;
        }
        let normalized = s.replace('\u{00AD}', "").replace('’', "'");
        if (self.is_tagged)(&normalized) {
            out.push(s.to_string());
            return out;
        }
        const CAMEL_CASE: [&str; 6] = [
            "mers-cov",
            "mcgraw-hill",
            "sars-cov-2",
            "sars-cov",
            "ph-metre",
            "ph-metres",
        ];
        if CAMEL_CASE.iter().any(|c| c.eq_ignore_ascii_case(s)) {
            out.push(s.to_string());
            return out;
        }
        // Java `StringTokenizer(s, "-", true)`: parts with the delimiters
        out.extend(string_tokenize(s, "-"));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_decimals_and_ordinals() {
        let is_tagged = |w: &str| w == "mcgraw-hill";
        let t = SpanishWordTokenizer::new(&is_tagged);
        assert_eq!(t.tokenize("3.14"), vec!["3.14"]);
        assert_eq!(t.tokenize("3,14"), vec!["3,14"]);
        assert_eq!(t.tokenize("1º"), vec!["1º"]);
        assert_eq!(t.tokenize("2.º"), vec!["2.º"]);
    }

    #[test]
    fn splits_unknown_hyphenated_words() {
        let is_tagged = |w: &str| w == "mcgraw-hill";
        let t = SpanishWordTokenizer::new(&is_tagged);
        assert_eq!(t.tokenize("bien-venido"), vec!["bien", "-", "venido"]);
        assert_eq!(t.tokenize("McGraw-Hill"), vec!["McGraw-Hill"]);
    }
}
