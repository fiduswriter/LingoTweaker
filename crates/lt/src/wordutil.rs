//! Shared word/text helpers used by both language modules: LT `WordTokenizer`
//! URL/e-mail classification (`WordTokenizer.isUrl` / `isEMail`), word
//! classification (`WordRepeatRule.isWord`), case-insensitive comparison and
//! the punctuation/compound split helpers from the spelling rules
//! (`StringTools.isPunctuationMark`, `Pattern.split("-")`).

use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;

/// `SpellingRule.PUNCTUATION_MARK`: a single punctuation char (plus `'`).
static PUNCTUATION_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{P}']$").unwrap());

/// `WordTokenizer.PROTOCOLS` (duplicates in Java, order irrelevant).
const PROTOCOLS: [&str; 18] = [
    "http", "https", "ws", "wss", "ftp", "ftps", "sftp", "file", "mailto", "tel", "sms", "git",
    "ssh", "data", "magnet", "smb", "slack", "spotify",
];

/// `WordTokenizer.NO_PROTOCOL_URL` with `matches()` semantics.
static NO_PROTOCOL_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:([a-zA-Z0-9][a-zA-Z0-9-]+)\.)?([a-zA-Z0-9][a-zA-Z0-9-]+)\.([a-zA-Z0-9][a-zA-Z0-9-]+)/.*$",
    )
    .unwrap()
});

/// `WordTokenizer.E_MAIL` with `matches()` semantics (lookbehind → fancy).
static EMAIL: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(
        r"(?<!:)@?\b[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))\b",
    )
    .unwrap()
});

/// `WordTokenizer.isUrl`.
pub fn is_url(token: &str) -> bool {
    for protocol in PROTOCOLS {
        if token.starts_with(&format!("{protocol}://")) || token.starts_with("www.") {
            return true;
        }
    }
    NO_PROTOCOL_URL.is_match(token)
}

/// `WordTokenizer.isEMail`.
pub fn is_email(token: &str) -> bool {
    EMAIL.is_match(token).unwrap_or(false)
}

/// `StringTools.isEmoji` (approximation: symbols in the emoji ranges).
fn is_emoji(token: &str) -> bool {
    token.chars().any(|c| {
        matches!(c as u32,
            0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F | 0x200D)
    })
}

/// `WordRepeatRule.isWord` (emoji and numeric-space tokens are not words).
pub(crate) fn is_word(token: &str) -> bool {
    if is_emoji(token) {
        return false;
    }
    // `StringUtils.isNumericSpace`: all chars are digits or spaces
    if !token.is_empty() && token.chars().all(|c| c.is_numeric() || c.is_whitespace()) {
        return false;
    }
    if token.chars().count() == 1 {
        return token.chars().next().is_some_and(char::is_alphabetic);
    }
    true
}

/// Case-insensitive equality (LT `String.equalsIgnoreCase`).
pub(crate) fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// `StringTools.isPunctuationMark`: exactly one punctuation character.
pub(crate) fn is_punctuation_mark(s: &str) -> bool {
    PUNCTUATION_MARK.is_match(s)
}

/// Java `Pattern.split("-")`: trailing empty strings are removed.
pub(crate) fn split_compound(word: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = word.split('-').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `<suggestion>` blocks of a rule message, extracted like the Java
/// `RuleMatch` constructor (order-preserving dedup, `<mistake/>` blocks
/// skipped). Filters that rewrite a message containing inline suggestions
/// (`NewYearDateFilter`, `YMDNewYearDateFilter`) build the new `RuleMatch`
/// from that message, so the suggestions have to be re-extracted after the
/// rewrite.
pub(crate) fn suggestion_tags(message: &str) -> Vec<String> {
    const START: &str = "<suggestion>";
    const END: &str = "</suggestion>";
    let mut out: Vec<String> = Vec::new();
    let mut rest = message;
    while let Some(start) = rest.find(START) {
        let after = &rest[start + START.len()..];
        let Some(end) = after.find(END) else {
            break;
        };
        let inner = &after[..end];
        if !inner.contains("<mistake/>") && !out.iter().any(|s| s == inner) {
            out.push(inner.to_string());
        }
        rest = &after[end + END.len()..];
    }
    out
}
