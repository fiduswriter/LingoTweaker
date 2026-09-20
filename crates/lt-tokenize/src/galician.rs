//! `org.languagetool.tokenizers.gl.GalicianWordTokenizer`: the base
//! `WordTokenizer` e-mail/URL joining plus the Galician decimal-comma,
//! date/dotted-number/dotted-ordinal/spaced-decimal/colon re-joins and the
//! `StringTokenizer(SPLIT_CHARS)` split.
//!
//! Galician's tokenizer is rule-based (no dictionary lookup), so unlike the
//! Spanish/Portuguese tokenizers it takes no tagger callback.

use regex::Regex;
use std::sync::OnceLock;

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// `GalicianWordTokenizer.SPLIT_CHARS`.
const SPLIT_CHARS: &str = " -\u{A0}\u{115F}\u{1160}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{2013}\u{2014}\u{2015}\u{200A}\u{200B}\u{200C}\u{200D}\u{200E}\u{200F}\u{2028}\u{2029}\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}\u{202F}\u{205F}\u{2060}\u{2061}\u{2062}\u{2063}\u{206A}\u{206B}\u{206C}\u{206D}\u{206E}\u{206F}\u{3000}\u{3164}\u{FEFF}\u{FFA0}\u{FFF9}\u{FFFA}\u{FFFB}*+\u{D7}\u{2217}\u{B7}\u{F7}:=\u{2260}\u{2242}\u{2243}\u{2244}\u{2245}\u{2246}\u{2247}\u{2248}\u{2249}\u{2264}\u{2265}\u{226A}\u{226B}\u{2227}\u{2228}\u{2229}\u{222A}\u{2208}\u{2209}\u{220A}\u{220B}\u{220C}\u{220D},.;<>()[]{}\u{BF}\u{A1}!?:\"\u{AB}\u{BB}`'\u{2019}\u{2018}\u{201E}\u{201C}\u{201D}\u{2026}\\/\t\r\n";

const DECIMAL_COMMA_SUBST: char = '\u{E001}';
const NON_BREAKING_SPACE_SUBST: char = '\u{E002}';
const NON_BREAKING_DOT_SUBST: char = '\u{E003}';
const NON_BREAKING_COLON_SUBST: char = '\u{E004}';

fn decimal_comma_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([0-9]),([0-9])").unwrap())
}

/// `DATE_PATTERN`: three alternatives. Java's `replaceAll` builds the
/// replacement from groups 1–3 only, so the `yyyy.mm.dd` / `yyyy-mm-dd`
/// alternatives lose their content (legacy bug reproduced faithfully).
fn date_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"([0-9]{2})\.([0-9]{2})\.([0-9]{4})|([0-9]{4})\.([0-9]{2})\.([0-9]{2})|([0-9]{4})-([0-9]{2})-([0-9]{2})",
        )
        .unwrap()
    })
}

fn dotted_numbers_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([0-9])\.([0-9])").unwrap())
}

fn dotted_ordinals_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new("([0-9])\\.([\u{0061}\u{006F}\u{00AA}\u{00BA}][\u{0073}\u{02E2}]?)").unwrap()
    })
}

/// `DECIMAL_SPACE_PATTERN` with its Java lookarounds (`\s` is the ASCII
/// class without `UNICODE_CHARACTER_CLASS`).
fn decimal_space_pattern() -> &'static fancy_regex::Regex {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        fancy_regex::Regex::new(
            "(?<=^|[ \\t\\n\\x0B\\f\\r(])[0-9]{1,3}( [0-9]{3})+(?=[ \\t\\n\\x0B\\f\\r(]|$)",
        )
        .unwrap()
    })
}

fn colon_numbers_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([0-9]):([0-9])").unwrap())
}

/// `GalicianWordTokenizer` (rule-based; no tagger callback).
#[derive(Debug, Default, Clone, Copy)]
pub struct GalicianWordTokenizer;

impl GalicianWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }
}

/// `GalicianWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut text = text.to_string();

    if text.contains(',') {
        text = decimal_comma_pattern()
            .replace_all(&text, format!("${{1}}{DECIMAL_COMMA_SUBST}${{2}}").as_str())
            .into_owned();
    }

    // "if period is not the last character in the sentence"
    let dot_inside_sentence = text
        .find('.')
        .is_some_and(|byte| text[byte + 1..].chars().next().is_some());
    if dot_inside_sentence {
        text = date_pattern()
            .replace_all(&text, |caps: &regex::Captures| {
                let g1 = caps.get(1).map_or("", |m| m.as_str());
                let g2 = caps.get(2).map_or("", |m| m.as_str());
                let g3 = caps.get(3).map_or("", |m| m.as_str());
                format!("{g1}{NON_BREAKING_DOT_SUBST}{g2}{NON_BREAKING_DOT_SUBST}{g3}")
            })
            .into_owned();
        text = dotted_numbers_pattern()
            .replace_all(
                &text,
                format!("${{1}}{NON_BREAKING_DOT_SUBST}${{2}}").as_str(),
            )
            .into_owned();
        text = dotted_ordinals_pattern()
            .replace_all(
                &text,
                format!("${{1}}{NON_BREAKING_DOT_SUBST}${{2}}").as_str(),
            )
            .into_owned();
    }

    // "2 000 000": Java `find()` + `appendReplacement` replaces every match.
    text = decimal_space_pattern()
        .replace_all(&text, |caps: &fancy_regex::Captures| {
            caps.get(0)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .replace(' ', &NON_BREAKING_SPACE_SUBST.to_string())
                .replace('\u{00A0}', &NON_BREAKING_SPACE_SUBST.to_string())
        })
        .into_owned();

    if text.contains(':') {
        text = colon_numbers_pattern()
            .replace_all(
                &text,
                format!("${{1}}{NON_BREAKING_COLON_SUBST}${{2}}").as_str(),
            )
            .into_owned();
    }

    let tokens: Vec<String> = string_tokenize(&text, SPLIT_CHARS)
        .into_iter()
        .map(|token| {
            token
                .replace(DECIMAL_COMMA_SUBST, ",")
                .replace(NON_BREAKING_COLON_SUBST, ":")
                .replace(NON_BREAKING_SPACE_SUBST, " ")
                .replace(NON_BREAKING_DOT_SUBST, ".")
        })
        .collect();
    join_emails_and_urls(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_decimals_and_dates() {
        assert_eq!(tokenize("3,14"), vec!["3,14"]);
        assert_eq!(tokenize("12:25"), vec!["12:25"]);
        assert_eq!(tokenize("1.234"), vec!["1.234"]);
        // alternative 2 loses its content in Java (groups 1–3 only)
        assert_eq!(tokenize("2024.05.12"), vec![".."]);
    }

    #[test]
    fn splits_punctuation_like_string_tokenizer() {
        assert_eq!(tokenize("Ola, mundo!"), vec!["Ola", ",", " ", "mundo", "!"]);
    }
}
