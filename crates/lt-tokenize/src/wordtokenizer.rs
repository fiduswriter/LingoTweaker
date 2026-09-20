//! Base `org.languagetool.tokenizers.WordTokenizer` port: `StringTokenizer`
//! split on the tokenizing characters (returning delimiters as single-char
//! tokens) followed by the e-mail and URL joining passes.
//!
//! `EnglishWordTokenizer` overrides `tokenize` with a regex-based splitter
//! (see `english.rs`); the German and most other tokenizers inherit this base
//! implementation.

use regex::Regex;
use std::sync::OnceLock;

/// The base tokenizing characters (Java `TOKENIZING_CHARACTERS`), minus the
/// `_` that `EnglishWordTokenizer`/`GermanWordTokenizer` add.
pub fn base_tokenizing_characters() -> String {
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
    s
}

pub fn german_tokenizing_characters() -> String {
    let mut s = base_tokenizing_characters();
    s.push_str("_‚");
    s
}

/// `java.util.StringTokenizer(text, delimiters, true)`.
pub fn string_tokenize(text: &str, delimiters: &str) -> Vec<String> {
    let is_delim = |c: char| delimiters.contains(c);
    let mut out = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if is_delim(c) {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            out.push(c.to_string());
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

struct Patterns {
    email: fancy_regex::Regex,
    url_chars: Regex,
    domain_chars: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        email: fancy_regex::Regex::new(
            "(?<!:)@?\\b[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@((\\[[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\])|(([a-zA-Z\\-0-9]+\\.)+[a-zA-Z]{2,}))\\b",
        )
        .unwrap(),
        url_chars: Regex::new("[a-zA-ZÄÖÜäöü0-9/%$-_.+!*'(),?#~]+").unwrap(),
        domain_chars: Regex::new("[a-zA-Z0-9][a-zA-Z0-9-]+").unwrap(),
    })
}

const PROTOCOLS: &[&str] = &[
    "http", "https", "ws", "wss", "ftp", "ftps", "sftp", "file", "mailto", "tel", "sms", "git",
    "ssh", "data", "magnet", "smb", "slack", "spotify",
];

/// `WordTokenizer.joinEMailsAndUrls`.
pub fn join_emails_and_urls(tokens: Vec<String>) -> Vec<String> {
    join_urls(join_emails(tokens))
}

fn join_emails(tokens: Vec<String>) -> Vec<String> {
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

fn join_urls(tokens: Vec<String>) -> Vec<String> {
    let p = patterns();
    let mut out: Vec<String> = Vec::new();
    let mut in_url = false;
    let mut url = String::new();
    let mut url_quote: Option<String> = None;
    let mut i = 0usize;
    while i < tokens.len() {
        let starts = !in_url && url_starts_at(i, &tokens, p);
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
        if in_url && url_ends_at(i, &tokens, url_quote.as_deref(), p) {
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

fn url_starts_at(i: usize, tokens: &[String], p: &Patterns) -> bool {
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

fn url_ends_at(i: usize, tokens: &[String], url_quote: Option<&str>, p: &Patterns) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_tokenizer_keeps_delimiters() {
        let tokens = string_tokenize("Hello, world!", ",! ");
        assert_eq!(tokens, vec!["Hello", ",", " ", "world", "!"]);
    }

    #[test]
    fn joins_urls_like_java() {
        let tokens = string_tokenize("http://example.org/x y", " ");
        let joined = join_emails_and_urls(tokens);
        assert_eq!(joined, vec!["http://example.org/x", " ", "y"]);
    }
}
