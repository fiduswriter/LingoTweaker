//! English word tokenizer: port of `EnglishWordTokenizer` (contraction
//! splitting, hyphen handling, email/URL joining). Tokens mirror LT's stream,
//! including single whitespace/delimiter characters as tokens.

use regex::Regex;
use std::sync::OnceLock;

const WORD_CHAR_CLASS: &str =
    "±§©@€£¥$\\p{L}\\d\\-\u{0300}-\u{036F}\u{00A8}°%‰‱&\u{FFFD}\u{00AD}\u{00AC}\u{FF0C}\u{FF1F}";

fn tokenizer_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(&format!("[{}]+|[^{}]", WORD_CHAR_CLASS, WORD_CHAR_CLASS)).unwrap()
    })
}

const CONTRACTION_VERBS: &str = "are|is|were|was|do|does|did|have|has|had|wo|would|ca|could|sha|should|must|ai|ought|might|need|may|am|dare|das|dass|hai|used|use";

struct Patterns {
    p1: Regex,
    p2: Regex,
    p3: Regex,
    p4: Regex,
    email: fancy_regex::Regex,
    url_chars: Regex,
    domain_chars: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        p1: Regex::new("(?i)^(fo['’]c['’]sle|rec['’][ds]|OK['’]d|cc['’][ds]|DJ['’][d]|[pd]m['’]d|rsvp['’]d)$")
            .unwrap(),
        p2: Regex::new(&format!("(?i)^(['’]?)({})(n['’]t)$", CONTRACTION_VERBS)).unwrap(),
        p3: Regex::new("(?i)^(.+)(['’]m|['’]re|['’]ll|['’]ve|['’]d|['’]s)(['’-]?)$").unwrap(),
        p4: Regex::new("(?i)^(['’]t)(was|were|is)$").unwrap(),
        email: fancy_regex::Regex::new(
            "(?<!:)@?\\b[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@((\\[[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\])|(([a-zA-Z\\-0-9]+\\.)+[a-zA-Z]{2,}))\\b",
        )
        .unwrap(),
        // Java URL_CHARS relies on the `$-_` range (includes '=', ':', ...);
        // the pattern is copied verbatim, not with an escaped hyphen
        url_chars: Regex::new("[a-zA-ZÄÖÜäöü0-9/%$-_.+!*'(),?#~]+").unwrap(),
        domain_chars: Regex::new("[a-zA-Z0-9][a-zA-Z0-9-]+").unwrap(),
    })
}

const PROTOCOLS: &[&str] = &[
    "http", "https", "ws", "wss", "ftp", "ftps", "sftp", "file", "mailto", "tel", "sms", "git",
    "ssh", "data", "magnet", "smb", "slack", "spotify",
];

/// The base LT tokenizing characters plus English's underscore.
pub fn english_tokenizing_characters() -> String {
    // (kept for parity documentation; the English tokenizer uses the regex
    // class above rather than a delimiter split)
    let mut s = String::from(
        "\u{0020}\u{00A0}\u{115f}\u{1160}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{200B}\u{200c}\u{200d}\u{200e}\u{200f}\u{2028}\u{2029}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{202f}\u{205F}\u{2060}\u{2061}\u{2062}\u{2063}\u{206A}\u{206b}\u{206c}\u{206d}\u{206E}\u{206F}\u{3000}\u{3164}\u{feff}\u{ffa0}\u{fff9}\u{fffa}\u{fffb}",
    );
    s.push_str(
        "¦‖∣|,.;()[]{}=*#∗+×·÷<>!?:~/\\\"'«»„”“‘’`´‛′›‹…¿¡‼⁇⁈⁉™®\u{203d}\u{00B6}\u{FFEB}\u{2E2E}",
    );
    s.push_str("\u{2012}\u{2013}\u{2014}\u{2015}\u{2500}\u{3161}\u{2713}");
    s.push_str("\u{25CF}\u{25CB}\u{25C6}\u{27A2}\u{25A0}\u{25A1}\u{2605}\u{274F}\u{2794}\u{21B5}\u{2756}\u{25AA}\u{2751}\u{2022}");
    s.push_str("\u{2B9A}\u{2265}\u{2192}\u{21FE}\u{21C9}\u{21D2}\u{21E8}\u{21DB}");
    s.push_str(
        "\u{00b9}\u{00b2}\u{00b3}\u{2070}\u{2071}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}",
    );
    s.push_str("\t\n\r\u{000B}");
    s.push('_');
    s
}

/// Callback answering "does this word have a tagged reading?" (used by the
/// hyphenated-word check; supplied by the tagger at engine assembly time).
pub type IsTagged<'a> = &'a dyn Fn(&str) -> bool;

/// Find the first matching contraction pattern and return its capture
/// groups (skipping group 0).
fn contraction_groups(s: &str) -> Option<Vec<String>> {
    let p = patterns();
    for pattern in [&p.p1, &p.p2, &p.p3, &p.p4] {
        if let Some(caps) = pattern.captures(s) {
            let mut groups = Vec::new();
            for i in 1..caps.len() {
                groups.push(
                    caps.get(i)
                        .map(|g| g.as_str().to_string())
                        .unwrap_or_default(),
                );
            }
            return Some(groups);
        }
    }
    None
}

pub struct EnglishWordTokenizer<'a> {
    tagged: IsTagged<'a>,
}

impl<'a> EnglishWordTokenizer<'a> {
    pub fn new(tagged: IsTagged<'a>) -> Self {
        Self { tagged }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut l: Vec<String> = Vec::new();
        let aux = text
            .replace('\'', "xxAPOSTYPEWxx")
            .replace('’', "xxAPOSTYPOGxx");
        for m in tokenizer_pattern().find_iter(&aux) {
            let mut s = m.as_str().to_string();
            if !l.is_empty()
                && s.chars().count() == 1
                && s.chars()
                    .next()
                    .is_some_and(|c| ('\u{FE00}'..='\u{FE0F}').contains(&c))
            {
                let last = l.len() - 1;
                l[last].push_str(&s);
                continue;
            }
            s = s
                .replace("xxAPOSTYPEWxx", "'")
                .replace("xxAPOSTYPOGxx", "’");
            let groups: Option<Vec<String>> = if s.contains('\'') || s.contains('’') {
                contraction_groups(&s)
            } else {
                None
            };
            match groups {
                Some(groups) => {
                    for g in groups {
                        l.extend(self.words_to_add(&g));
                    }
                }
                None => l.extend(self.words_to_add(&s)),
            }
        }
        self.join_emails_and_urls(l, text)
    }

    fn words_to_add(&self, s: &str) -> Vec<String> {
        let mut l = Vec::new();
        let mut s = s.to_string();
        let mut hyphens_at_end = 0usize;
        while s.starts_with('-') {
            l.push("-".to_string());
            s = s[1..].to_string();
        }
        while s.ends_with('-') {
            s.pop();
            hyphens_at_end += 1;
        }
        if !s.is_empty() {
            if !s.contains('-') && !s.contains('\'') && !s.contains('’') {
                l.push(s);
            } else {
                let normalized: String = s.replace('\u{00AD}', "").replace('’', "'");
                if (self.tagged)(&normalized)
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
                        "al-qaida",
                    ]
                    .iter()
                    .any(|w| s.eq_ignore_ascii_case(w))
                {
                    l.push(s);
                } else {
                    let mut current = String::new();
                    for c in s.chars() {
                        if c == '\'' || c == '’' {
                            if !current.is_empty() {
                                l.push(std::mem::take(&mut current));
                            }
                            l.push(c.to_string());
                        } else {
                            current.push(c);
                        }
                    }
                    if !current.is_empty() {
                        l.push(current);
                    }
                }
            }
        }
        for _ in 0..hyphens_at_end {
            l.push("-".to_string());
        }
        l
    }

    fn join_emails_and_urls(&self, tokens: Vec<String>, _text: &str) -> Vec<String> {
        self.join_urls(self.join_emails(tokens))
    }

    fn join_emails(&self, tokens: Vec<String>) -> Vec<String> {
        let text: String = tokens.join("");
        if !text.contains('@') {
            return tokens;
        }
        if !patterns().email.is_match(&text).unwrap_or(false) {
            return tokens;
        }
        // byte spans of each token in the joined text
        let mut spans: Vec<(usize, usize)> = Vec::with_capacity(tokens.len());
        let mut pos = 0usize;
        for t in &tokens {
            let len = t.len();
            spans.push((pos, pos + len));
            pos += len;
        }
        let find_at = |from: usize| -> Option<(usize, usize)> {
            for m in patterns().email.find_iter(&text).flatten() {
                if m.start() >= from {
                    return Some((m.start(), m.end()));
                }
            }
            None
        };

        let mut out: Vec<String> = Vec::new();
        let mut token_idx = 0usize;
        let mut search_from = 0usize;
        while token_idx < tokens.len() {
            match find_at(search_from) {
                Some((start, end)) => {
                    // copy tokens fully before the email
                    while token_idx < tokens.len() && spans[token_idx].1 <= start {
                        out.push(tokens[token_idx].clone());
                        token_idx += 1;
                    }
                    if token_idx >= tokens.len() {
                        break;
                    }
                    if spans[token_idx].0 != start {
                        // boundary token straddles the match start; keep it
                        out.push(tokens[token_idx].clone());
                        token_idx += 1;
                        search_from = end;
                        continue;
                    }
                    out.push(text[start..end].to_string());
                    while token_idx < tokens.len() && spans[token_idx].1 <= end {
                        token_idx += 1;
                    }
                    search_from = end;
                }
                None => {
                    out.extend(tokens[token_idx..].iter().cloned());
                    break;
                }
            }
        }
        out
    }

    fn join_urls(&self, tokens: Vec<String>) -> Vec<String> {
        let p = patterns();
        let mut out: Vec<String> = Vec::new();
        let mut in_url = false;
        let mut url = String::new();
        let mut url_quote: Option<String> = None;
        let mut i = 0usize;
        while i < tokens.len() {
            let starts = !in_url && self.url_starts_at(i, &tokens, p);
            if starts {
                in_url = true;
                url.clear();
                if i > 0 {
                    url_quote = Some(tokens[i - 1].clone());
                }
                url.push_str(&tokens[i]);
                i += 1;
                continue;
            }
            if in_url && self.url_ends_at(i, &tokens, url_quote.as_deref(), p) {
                out.push(std::mem::take(&mut url));
                in_url = false;
                continue;
            }
            if in_url {
                url.push_str(&tokens[i]);
                i += 1;
                continue;
            }
            out.push(tokens[i].clone());
            i += 1;
        }
        if in_url && !url.is_empty() {
            out.push(url);
        }
        out
    }

    fn url_starts_at(&self, i: usize, tokens: &[String], p: &Patterns) -> bool {
        let token = &tokens[i];
        if PROTOCOLS.contains(&token.as_str())
            && tokens.len() > i + 3
            && tokens[i + 1] == ":"
            && tokens[i + 2] == "/"
            && tokens[i + 3] == "/"
        {
            return true;
        }
        if tokens.len() > i + 1 && token == "www" && tokens[i + 1] == "." {
            return true;
        }
        if tokens.len() > i + 3
            && tokens[i + 1] == "."
            && tokens[i + 3] == "/"
            && p.domain_chars.is_match(token)
            && p.domain_chars.is_match(&tokens[i + 2])
        {
            return true;
        }
        if tokens.len() > i + 5
            && tokens[i + 1] == "."
            && tokens[i + 3] == "."
            && tokens[i + 5] == "/"
            && p.domain_chars.is_match(token)
            && p.domain_chars.is_match(&tokens[i + 2])
            && p.domain_chars.is_match(&tokens[i + 4])
        {
            return true;
        }
        false
    }

    fn url_ends_at(
        &self,
        i: usize,
        tokens: &[String],
        url_quote: Option<&str>,
        p: &Patterns,
    ) -> bool {
        let token = &tokens[i];
        if token.chars().all(|c| c.is_whitespace()) || token == ")" || token == "]" {
            return true;
        }
        if tokens.len() > i + 1 {
            let next = &tokens[i + 1];
            let next_is_boundary = next.chars().all(|c| c.is_whitespace())
                || ["\"", "»", "«", "‘", "’", "“", "”", "'", "."].contains(&next.as_str());
            if next_is_boundary
                && ([".", ",", ";", ":", "!", "?"].contains(&token.as_str())
                    || Some(token.as_str()) == url_quote)
            {
                return true;
            }
            if !p.url_chars.is_match(token) {
                return true;
            }
            false
        } else {
            !p.url_chars.is_match(token) || token == "." || Some(token.as_str()) == url_quote
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    fn tagged_all_true() -> impl Fn(&str) -> bool {
        |_| false
    }

    #[test]
    fn splits_contractions_like_lt() {
        // with the real tagger, "n't" is a dictionary word and stays intact
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/dictionaries");
        if !dir.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let info = lt_tagger::DictionaryInfo::load(&dir.join("english.info")).unwrap();
        let dict = lt_tagger::Dictionary::load(&dir.join("english.dict"), &info).unwrap();
        let tagger = lt_tagger::EnglishTagger::new(dict);
        let callback = |w: &str| tagger.is_tagged(w);
        let tok = EnglishWordTokenizer::new(&callback);
        assert_eq!(tok.tokenize("don't"), vec!["do", "n't"]);
        assert_eq!(tok.tokenize("it's fine"), vec!["it", "'s", " ", "fine"]);
    }

    #[test]
    fn delimiters_are_single_tokens() {
        let f = tagged_all_true();
        let tok = EnglishWordTokenizer::new(&f);
        assert_eq!(
            tok.tokenize("Hello, world!"),
            vec!["Hello", ",", " ", "world", "!"]
        );
    }

    #[test]
    fn variation_selectors_join_previous() {
        let f = tagged_all_true();
        let tok = EnglishWordTokenizer::new(&f);
        assert_eq!(tok.tokenize("ok\u{FE0F}"), vec!["ok\u{FE0F}"]);
    }
}
