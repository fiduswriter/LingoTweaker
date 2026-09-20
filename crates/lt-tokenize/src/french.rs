//! `org.languagetool.tokenizers.fr.FrenchWordTokenizer`: the base
//! `WordTokenizer` with French apostrophe/elision and hyphen handling,
//! decimal/number-space re-joins and the tagger-backed hyphen split.

use regex::Regex;
use std::sync::OnceLock;

use crate::wordtokenizer::{join_emails_and_urls, string_tokenize};

/// The French `wordCharacters` class (Java `\d` is ASCII-only without
/// `UNICODE_CHARACTER_CLASS`, `\p{L}` is Unicode).
fn french_word_characters() -> &'static str {
    "§©@€£$_\\p{L}0-9\\-\u{0300}-\u{036F}\u{00A8}\u{2070}-\u{209F}°%‰‱&\u{FFFD}\u{00AD}\u{00AC}"
}

fn tokenizer_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        let wc = french_word_characters();
        Regex::new(&format!("[{wc}]+|[^{wc}]")).unwrap()
    })
}

/// French no-split words of `FrenchWordTokenizer.doNotSplit` (checked
/// lowercase).
const DO_NOT_SPLIT: [&str; 20] = [
    "mers-cov",
    "mcgraw-hill",
    "sars-cov-2",
    "sars-cov",
    "ph-metre",
    "ph-metres",
    "anti-ivg",
    "anti-uv",
    "anti-vih",
    "al-qaïda",
    "c'est-à-dire",
    "add-on",
    "add-ons",
    "rendez-vous",
    "garde-à-vous",
    "chez-eux",
    "chez-moi",
    "chez-nous",
    "chez-soi",
    "chez-toi",
];

fn patterns() -> &'static [Regex; 7] {
    static PATTERNS: OnceLock<[Regex; 7]> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let pronouns = "-ce|-elle|-t-elle|-elles|-t-telles|-en|-il|-t-il|-ils|-t-ils|-je|-la|-le|-les|-leur|-lui|-moi|-nous|-on|-t-on|-toi|-tu|-vous|-vs|-y";
        let elisions = "([cç]['’]|j['’]|n['’]|m['’]|t['’]|s['’]|l['’]|d['’]|qu['’]|jusqu['’]|lorsqu['’]|puisqu['’]|quoiqu['’])";
        [
            // words not to be split
            compile(
                "^(c['’]te?|m['’]as-tu-vu|c['’]est-à-dire|add-on|add-ons|rendez-vous|garde-à-vous|chez-eux|chez-moi|chez-nous|chez-soi|chez-toi|chez-vous)$",
            ),
            compile(&format!("^{elisions}([^\\-]*)({pronouns})$")),
            // Apostrophe at the beginning of a word (ce, je, ne, …):
            // two tokens, `<token>l'</token><token>homme</token>`
            compile(&format!("^{elisions}([^'’\\-].*)$")),
            compile(&format!("^([^\\-0-9]+)({pronouns})({pronouns})$")),
            compile("^([^\\-]*)(-t|-m)(['’]en|['’]y)$"),
            compile("^(.*)(-t-elle|-t-elles|-t-il|-t-ils|-t-on)$"),
            compile(&format!("^(.*)({pronouns})$")),
        ]
    })
}

fn compile(pattern: &str) -> Regex {
    Regex::new(&format!("(?i){pattern}")).unwrap()
}

fn typewriter_apostrophe() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"(?i)([\p{L}])'([\p{L}1"‘“«])"#).unwrap())
}

fn typographic_apostrophe() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"(?i)([\p{L}])’([\p{L}1"‘“«])"#).unwrap())
}

fn nearby_hyphens() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([\p{L}])-([\p{L}])-([\p{L}])").unwrap())
}

fn hyphens() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([\p{L}])-([\p{L}0-9])").unwrap())
}

fn decimal_point() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([0-9])\.([0-9])").unwrap())
}

fn decimal_comma() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([0-9]),([0-9])").unwrap())
}

fn space_digits0() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([0-9]{4}) ").unwrap())
}

fn space_digits() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)([0-9]) ([0-9][0-9][0-9])\b").unwrap())
}

fn space_digits2() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"(?i)([0-9]) ([0-9][0-9][0-9]) ([0-9][0-9][0-9])\b").unwrap())
}

const APOS_TYPEW: &str = "xxFR_APOS_TYPEWxx";
const APOS_TYPOG: &str = "xxFR_APOS_TYPOGxx";
const HYPHEN: &str = "xxFR_HYPHENxx";
const DECIMAL_POINT_SENTINEL: &str = "xxFR_DECIMALPOINTxx";
const DECIMAL_COMMA_SENTINEL: &str = "xxFR_DECIMALCOMMAxx";
const SPACE_SENTINEL: &str = "xxFR_SPACExx";
const SPACE0_SENTINEL: &str = "xxFR_SPACE0xx";

/// `FrenchWordTokenizer` over a tagger callback
/// (`FrenchTagger.INSTANCE.tag([word]).get(0).isTagged()`).
pub struct FrenchWordTokenizer<'a> {
    is_tagged: crate::english::IsTagged<'a>,
}

impl<'a> FrenchWordTokenizer<'a> {
    pub fn new(is_tagged: crate::english::IsTagged<'a>) -> Self {
        Self { is_tagged }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        // replace hyphen, non-break hyphen -> hyphen-minus
        let mut aux = text.replace(['\u{2010}', '\u{2011}'], "-");
        aux = typewriter_apostrophe()
            .replace_all(&aux, format!("${{1}}{APOS_TYPEW}${{2}}").as_str())
            .into_owned();
        aux = typographic_apostrophe()
            .replace_all(&aux, format!("${{1}}{APOS_TYPOG}${{2}}").as_str())
            .into_owned();
        aux = nearby_hyphens()
            .replace_all(&aux, format!("${{1}}{HYPHEN}${{2}}{HYPHEN}${{3}}").as_str())
            .into_owned();
        aux = hyphens()
            .replace_all(&aux, format!("${{1}}{HYPHEN}${{2}}").as_str())
            .into_owned();
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
        aux = space_digits2()
            .replace_all(
                &aux,
                format!("${{1}}{SPACE_SENTINEL}${{2}}{SPACE_SENTINEL}${{3}}").as_str(),
            )
            .into_owned();
        aux = space_digits0()
            .replace_all(&aux, format!("${{1}}{SPACE0_SENTINEL}").as_str())
            .into_owned();
        aux = space_digits()
            .replace_all(&aux, format!("${{1}}{SPACE_SENTINEL}${{2}}").as_str())
            .into_owned();
        aux = aux.replace(SPACE0_SENTINEL, " ");

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
                .replace(APOS_TYPEW, "'")
                .replace(APOS_TYPOG, "’")
                .replace(HYPHEN, "-")
                .replace(DECIMAL_POINT_SENTINEL, ".")
                .replace(DECIMAL_COMMA_SENTINEL, ",")
                .replace(SPACE_SENTINEL, " ");
            let mut s = s.as_str();
            while s.chars().count() > 1 && s.starts_with('-') {
                tokens.push("-".to_string());
                s = &s[1..];
            }
            let mut hyphens_at_end = 0usize;
            while s.chars().count() > 1 && s.ends_with('-') {
                s = &s[..s.len() - 1];
                hyphens_at_end += 1;
            }
            let mut matched: Option<regex::Captures> = None;
            for pattern in patterns() {
                if let Some(caps) = pattern.captures(s) {
                    matched = Some(caps);
                    break;
                }
            }
            if let Some(caps) = matched {
                for i in 1..caps.len() {
                    let group = caps.get(i).map(|g| g.as_str()).unwrap_or("");
                    tokens.extend(self.words_to_add(group));
                }
            } else {
                tokens.extend(self.words_to_add(s));
            }
            for _ in 0..hyphens_at_end {
                tokens.push("-".to_string());
            }
        }
        join_emails_and_urls(tokens)
    }

    /// `FrenchWordTokenizer.wordsToAdd`: hyphenated words are looked up in
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
        if DO_NOT_SPLIT.iter().any(|c| c.eq_ignore_ascii_case(s)) {
            out.push(s.to_string());
            return out;
        }
        out.extend(string_tokenize(s, "-"));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(text: &str, tagged: impl Fn(&str) -> bool) -> Vec<String> {
        let t = FrenchWordTokenizer::new(&tagged);
        t.tokenize(text)
    }

    #[test]
    fn splits_elisions() {
        assert_eq!(tokenize("l'homme", |_| false), vec!["l'", "homme"]);
        assert_eq!(tokenize("qu'il", |_| false), vec!["qu'", "il"]);
        assert_eq!(tokenize("d’accord", |_| false), vec!["d’", "accord"]);
    }

    #[test]
    fn keeps_no_split_words() {
        assert_eq!(
            tokenize("rendez-vous", |w| w == "rendez-vous"),
            vec!["rendez-vous"]
        );
        assert_eq!(tokenize("c'est-à-dire", |_| false), vec!["c'est-à-dire"]);
    }

    #[test]
    fn re_joins_numbers() {
        assert_eq!(tokenize("3.14", |_| false), vec!["3.14"]);
        assert_eq!(tokenize("1 000 000", |_| false), vec!["1 000 000"]);
    }

    #[test]
    fn splits_unknown_hyphenated_words() {
        assert_eq!(
            tokenize("bien-venido", |_| false),
            vec!["bien", "-", "venido"]
        );
        // `-t-il` is in the dictionary, so the interrogative suffix is kept
        assert_eq!(
            tokenize("donne-t-il", |w| w == "-t-il"),
            vec!["donne", "-t-il"]
        );
    }
}
