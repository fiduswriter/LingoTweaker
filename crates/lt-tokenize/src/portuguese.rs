//! `org.languagetool.tokenizers.pt.PortugueseWordTokenizer`: the base
//! `WordTokenizer` currency handling plus the Portuguese decimal
//! comma/date/dotted-number/dotted-ordinal/spaced-decimal/colon/hyphen
//! re-joins and the dictionary-aware hyphen splitting.

use regex::Regex;
use std::sync::OnceLock;

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// `PortugueseWordTokenizer` substitution characters.
const DECIMAL_COMMA_SUBST: char = '\u{E001}';
const NON_BREAKING_SPACE_SUBST: char = '\u{E002}';
const NON_BREAKING_DOT_SUBST: char = '\u{E003}';
const NON_BREAKING_COLON_SUBST: char = '\u{E004}';
const HYPHEN_SUBST_TEXT: &str = "\u{0001}\u{0001}PT_HYPHEN\u{0001}\u{0001}";

fn decimal_comma_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([0-9]),([0-9])").unwrap())
}

/// `DATE_PATTERN` (three alternatives; Java `replaceAll` leaves unmatched
/// `$n` empty).
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
        Regex::new(
            "([0-9])\\.([\u{0061}\u{006F}\u{00AA}\u{00BA}\u{1D43}\u{1D52}][\u{0073}\u{02E2}]?)",
        )
        .unwrap()
    })
}

/// `DECIMAL_SPACE_PATTERN` without the lookarounds (checked manually).
fn decimal_space_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            "[0-9]{{1,3}}( [0-9]{{3}})+(?:[{DECIMAL_COMMA_SUBST}{NON_BREAKING_DOT_SUBST}][0-9]+)?"
        ))
        .unwrap()
    })
}

fn colon_numbers_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([0-9]):([0-9])").unwrap())
}

fn nearby_hyphens_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\p{L})-(\p{L})-(\p{L})").unwrap())
}

fn hyphen_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\p{L})-(\p{L}|[0-9])").unwrap())
}

/// `PortugueseWordTokenizer.wordPattern`.
fn tokenizer_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // \u0300-\u036F combining diacritics, \u00A8 diaeresis,
        // \u2070-\u209F superscripts; ° may appear inside tokens ("30°C").
        let word_chars = format!(
            "\u{00B0}\\^\\-\\p{{L}}0-9\u{0300}-\u{036F}\u{00A8}\u{2070}-\u{209F}{DECIMAL_COMMA_SUBST}{NON_BREAKING_SPACE_SUBST}{NON_BREAKING_DOT_SUBST}{NON_BREAKING_COLON_SUBST}\u{0001}PT_HYPHEN"
        );
        let left = "\u{2212}@\u{20AC}\u{00A3}\\$\u{00A2}\u{00A5}\u{00A4}";
        let right = "\u{20AC}\u{00A3}\\$%\u{2030}\u{2031}\u{00BA}\u{00AA}\u{1D43}\u{1D52}\u{02E2}";
        Regex::new(&format!("(?i)[{left}]?[{word_chars}]+[{right}]?|[^{word_chars}]")).unwrap()
    })
}

/// `WordTokenizer.CURRENCY_SYMBOLS`.
fn currency_symbols() -> &'static str {
    "[A-Z]*[\u{0E3F}\u{20BF}\u{20B5}\u{00A2}\u{20A1}$\u{20AB}\u{058F}\u{20AC}\u{0192}\u{20B2}\u{20B4}\u{20AD}\u{20BE}\u{20BA}\u{20BC}\u{20A6}\u{20B1}\u{00A3}\u{17DB}\u{20BD}\u{20B9}\u{20AA}\u{09F3}\u{20B8}\u{20AE}\u{20A9}\u{00A5}\u{00A4}]"
}

fn currency_expression() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            "(?:({})([0-9]+(?:[.,][0-9]+)*)|([0-9]+(?:[.,][0-9]+)*)({}))",
            currency_symbols(),
            currency_symbols()
        ))
        .unwrap()
    })
}

/// `PortugueseWordTokenizer` over a tagger callback
/// (`PortugueseTagger.tag([word]).get(0).isTagged()`).
pub struct PortugueseWordTokenizer<'a> {
    is_tagged: crate::english::IsTagged<'a>,
}

impl<'a> PortugueseWordTokenizer<'a> {
    pub fn new(is_tagged: crate::english::IsTagged<'a>) -> Self {
        Self { is_tagged }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokenised = text.to_string();

        if tokenised.contains(',') {
            tokenised = decimal_comma_pattern()
                .replace_all(
                    &tokenised,
                    format!("${{1}}{DECIMAL_COMMA_SUBST}${{2}}").as_str(),
                )
                .into_owned();
        }

        let dot_index = tokenised.find('.');
        let dot_inside_sentence = dot_index.is_some_and(|i| i < tokenised.len() - 1);
        if dot_inside_sentence {
            tokenised = date_pattern()
                .replace_all(&tokenised, |caps: &regex::Captures| {
                    let group = |n: usize| caps.get(n).map(|g| g.as_str()).unwrap_or("");
                    format!(
                        "{}{}{}{}{}{}",
                        group(1),
                        NON_BREAKING_DOT_SUBST,
                        group(2),
                        NON_BREAKING_DOT_SUBST,
                        group(3),
                        ""
                    )
                })
                .into_owned();
            tokenised = dotted_numbers_pattern()
                .replace_all(
                    &tokenised,
                    format!("${{1}}{NON_BREAKING_DOT_SUBST}${{2}}").as_str(),
                )
                .into_owned();
            tokenised = dotted_ordinals_pattern()
                .replace_all(&tokenised, |caps: &regex::Captures| {
                    format!(
                        "{}{}{}",
                        caps.get(1).map(|g| g.as_str()).unwrap_or(""),
                        NON_BREAKING_DOT_SUBST,
                        caps.get(2).map(|g| g.as_str()).unwrap_or("")
                    )
                })
                .into_owned();
        }

        // "2 000 000": hide the spaces between digit groups
        if decimal_space_pattern().is_match(&tokenised) {
            let mut out = String::with_capacity(tokenised.len());
            let mut last = 0usize;
            for m in decimal_space_pattern().find_iter(&tokenised) {
                let before_ok = tokenised[..m.start()].chars().next_back().is_none_or(|c| {
                    matches!(c, ' ' | '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | '(')
                });
                let after_ok = tokenised[m.end()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_ascii_digit());
                if !(before_ok && after_ok) {
                    continue;
                }
                out.push_str(&tokenised[last..m.start()]);
                out.push_str(
                    &m.as_str()
                        .replace(' ', &NON_BREAKING_SPACE_SUBST.to_string())
                        .replace('\u{00A0}', &NON_BREAKING_SPACE_SUBST.to_string()),
                );
                last = m.end();
            }
            out.push_str(&tokenised[last..]);
            tokenised = out;
        }

        if tokenised.contains(':') {
            tokenised = colon_numbers_pattern()
                .replace_all(
                    &tokenised,
                    format!("${{1}}{NON_BREAKING_COLON_SUBST}${{2}}").as_str(),
                )
                .into_owned();
        }
        if tokenised.contains('-') {
            tokenised = nearby_hyphens_pattern()
                .replace_all(&tokenised, |caps: &regex::Captures| {
                    format!(
                        "{}{HYPHEN_SUBST_TEXT}{}{HYPHEN_SUBST_TEXT}{}",
                        caps.get(1).map(|g| g.as_str()).unwrap_or(""),
                        caps.get(2).map(|g| g.as_str()).unwrap_or(""),
                        caps.get(3).map(|g| g.as_str()).unwrap_or("")
                    )
                })
                .into_owned();
            tokenised = hyphen_pattern()
                .replace_all(&tokenised, |caps: &regex::Captures| {
                    format!(
                        "{}{HYPHEN_SUBST_TEXT}{}",
                        caps.get(1).map(|g| g.as_str()).unwrap_or(""),
                        caps.get(2).map(|g| g.as_str()).unwrap_or("")
                    )
                })
                .into_owned();
        }

        let mut tokens: Vec<String> = Vec::new();
        for m in tokenizer_pattern().find_iter(&tokenised) {
            let token = m.as_str();
            // \uFE00-\uFE0F non-spacing marks attach to the previous token
            if !tokens.is_empty()
                && token.chars().count() == 1
                && token
                    .chars()
                    .next()
                    .is_some_and(|c| ('\u{FE00}'..='\u{FE0F}').contains(&c))
            {
                let last = tokens.last_mut().unwrap();
                last.push_str(token);
                continue;
            }
            let restored = token
                .replace(DECIMAL_COMMA_SUBST, ",")
                .replace(NON_BREAKING_COLON_SUBST, ":")
                .replace(NON_BREAKING_SPACE_SUBST, " ")
                .replace(NON_BREAKING_DOT_SUBST, ".")
                .replace(HYPHEN_SUBST_TEXT, "-");
            tokens.extend(self.words_to_add(&restored));
        }
        join_emails_and_urls(tokens)
    }

    /// `PortugueseWordTokenizer.wordsToAdd`.
    fn words_to_add(&self, s: &str) -> Vec<String> {
        let mut out = Vec::new();
        if s.is_empty() {
            return out;
        }
        let kept_as_one = !s.contains('-')
            || (self.is_tagged)(&s.replace('\u{2019}', "'"))
            || [
                "mers-cov",
                "mcgraw-hill",
                "sars-cov-2",
                "sars-cov",
                "ph-metre",
                "ph-metres",
                "anti-ivg",
                "anti-uv",
                "anti-vih",
                "al-qa\u{00EF}da",
            ]
            .iter()
            .any(|c| c.eq_ignore_ascii_case(s));
        if self.is_currency_expression(s) {
            out.extend(self.split_currency_expression(s));
        } else if kept_as_one {
            out.push(s.to_string());
        } else {
            // Java `StringTokenizer(s, "-", true)`: parts with the delimiters
            out.extend(string_tokenize(s, "-"));
        }
        out
    }

    /// `WordTokenizer.isCurrencyExpression`.
    fn is_currency_expression(&self, token: &str) -> bool {
        currency_expression()
            .find(token)
            .is_some_and(|m| m.start() == 0 && m.end() == token.len())
    }

    /// `WordTokenizer.splitCurrencyExpression`.
    fn split_currency_expression(&self, token: &str) -> Vec<String> {
        let mut out = Vec::new();
        for caps in currency_expression().captures_iter(token) {
            let g = |n: usize| caps.get(n).map(|g| g.as_str());
            if let (Some(sym), Some(val)) = (g(1), g(2)) {
                out.push(sym.to_string());
                out.push(val.to_string());
            } else if let (Some(val), Some(sym)) = (g(3), g(4)) {
                out.push(val.to_string());
                out.push(sym.to_string());
            }
        }
        if out.is_empty() {
            out.push(token.to_string());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_numbers_dates_and_hyphens() {
        let is_tagged = |w: &str| w == "mcgraw-hill" || w == "guarda-chuva";
        let t = PortugueseWordTokenizer::new(&is_tagged);
        assert_eq!(t.tokenize("3,14"), vec!["3,14"]);
        assert_eq!(t.tokenize("12.03.2020"), vec!["12.03.2020"]);
        assert_eq!(t.tokenize("12:25"), vec!["12:25"]);
        assert_eq!(t.tokenize("2 000 000"), vec!["2 000 000"]);
        assert_eq!(t.tokenize("1º"), vec!["1º"]);
        assert_eq!(t.tokenize("guarda-chuva"), vec!["guarda-chuva"]);
        assert_eq!(t.tokenize("mcgraw-hill"), vec!["mcgraw-hill"]);
        assert_eq!(
            t.tokenize("tinham-o"),
            vec!["tinham", "-", "o"],
            "unknown hyphenated words split"
        );
        assert_eq!(t.tokenize("R$ 10,50"), vec!["R$", " ", "10,50"]);
    }
}
