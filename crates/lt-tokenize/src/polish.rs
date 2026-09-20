//! Polish word tokenizer: port of
//! `org.languagetool.tokenizers.pl.PolishWordTokenizer`.
//!
//! Like the base `WordTokenizer` (delimiters plus the single low-9 quote
//! `‚`), but with the Polish hyphen handling: a trailing hyphen is split off,
//! a leading hyphen recurses, and interior hyphens split compound words when
//! the parts form an adjective/noun/numeral compound — except for a fixed
//! list of prefixes and when the last part is a digit.

use lt_core::AnalyzedTokenReadings;

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// `PolishWordTokenizer`'s tagger hook (`Tagger.tag(List<String>)`). The
/// tokenizer lives in `lt-tokenize`, which cannot depend on `lt-tagger`, so
/// the caller passes the tagger's `tag` function.
pub type PolishTagFn<'a> = &'a dyn Fn(&[String]) -> Vec<AnalyzedTokenReadings>;

/// The Polish prefixes that must never be split off a hyphenated word
/// (`PolishWordTokenizer.prefixes`).
const PREFIXES: &[&str] = &[
    "arcy", "neo", "pre", "anty", "eks", "bez", "beze", "ekstra", "hiper", "infra", "kontr",
    "maksi", "midi", "między", "mini", "nad", "nade", "około", "ponad", "post", "pro", "przeciw",
    "pseudo", "super", "śród", "ultra", "wice", "wokół", "wokoło",
];

/// `PolishWordTokenizer.tokenize`'s delimiter string: the base set plus `‚`
/// (U+201A; the en dash `–` is already in the base set).
pub fn polish_tokenizing_characters() -> String {
    let mut s = crate::wordtokenizer::base_tokenizing_characters();
    s.push('‚');
    s
}

pub struct PolishWordTokenizer;

impl PolishWordTokenizer {
    pub fn new() -> Self {
        Self
    }

    /// `PolishWordTokenizer.tokenize` without the hybrid hyphen splitting
    /// (`tagger == null`).
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        self.tokenize_with(text, None)
    }

    pub fn tokenize_with(&self, text: &str, tagger: Option<PolishTagFn<'_>>) -> Vec<String> {
        let mut l: Vec<String> = Vec::new();
        for token in string_tokenize(text, &polish_tokenizing_characters()) {
            if token.chars().count() > 1 {
                if token.ends_with('-') {
                    l.push(token[..token.len() - 1].to_string());
                    l.push("-".to_string());
                } else if let Some(stripped) = token.strip_prefix('-') {
                    l.push("-".to_string());
                    l.extend(self.tokenize_with(stripped, tagger));
                } else if token.contains('-') {
                    l.extend(self.split_hyphenated(&token, tagger));
                } else {
                    l.push(token);
                }
            } else {
                l.push(token);
            }
        }
        join_emails_and_urls(l)
    }

    /// The `token.contains("-")` branch of `PolishWordTokenizer.tokenize`.
    fn split_hyphenated(&self, token: &str, tagger: Option<PolishTagFn<'_>>) -> Vec<String> {
        let parts = java_split_hyphen(token);
        let mut l = Vec::new();
        let Some(tagger) = tagger else {
            l.push(token.to_string());
            return l;
        };
        if PREFIXES.contains(&parts[0]) {
            l.push(token.to_string());
            return l;
        }
        let last_first = parts[parts.len() - 1].chars().next();
        if last_first.is_some_and(|c| c.is_numeric()) {
            // split numbers at dash or minus sign, 1-10
            for (i, part) in parts.iter().enumerate() {
                l.push(part.to_string());
                if i != parts.len() - 1 {
                    l.push("-".to_string());
                }
            }
            return l;
        }
        let mut tested: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
        tested.push(token.to_string());
        let tagged = tagger(&tested);
        if tagged.len() == parts.len() + 1 && !tagged[parts.len()].is_tagged {
            let has_pos = |idx: usize, tag: &str| {
                tagged[idx]
                    .readings
                    .iter()
                    .any(|r| r.pos_tag.as_deref() == Some(tag))
            };
            let has_partial = |idx: usize, part: &str| {
                tagged[idx]
                    .readings
                    .iter()
                    .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(part)))
            };
            let is_compound = match parts.len() {
                2 => {
                    (has_pos(0, "adja") && has_partial(1, "adj:"))
                        || (has_partial(0, "subst:") && has_partial(1, "subst:"))
                        || (has_partial(0, "num:") && has_partial(1, "num:"))
                }
                3 => has_pos(0, "adja") && has_pos(1, "adja") && has_partial(2, "adj:"),
                _ => false,
            };
            if is_compound {
                for (i, part) in parts.iter().enumerate() {
                    l.push(part.to_string());
                    if i != parts.len() - 1 {
                        l.push("-".to_string());
                    }
                }
            } else {
                l.push(token.to_string());
            }
        } else {
            l.push(token.to_string());
        }
        l
    }
}

impl Default for PolishWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// `String.split("-")`: drop trailing empty parts, keep interior ones.
fn java_split_hyphen(s: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split('-').collect();
    while parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Convenience wrapper mirroring `GalicianWordTokenizer::tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    PolishWordTokenizer::new().tokenize(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_polish_delimiters() {
        let tok = PolishWordTokenizer::new();
        assert_eq!(
            tok.tokenize("Kot, pies!"),
            vec!["Kot", ",", " ", "pies", "!"]
        );
        // a trailing hyphen is split off
        assert_eq!(tok.tokenize("słowo-"), vec!["słowo", "-"]);
    }
}
