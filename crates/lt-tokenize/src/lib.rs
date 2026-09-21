//! Sentence splitting and tokenization with UTF-8 byte offsets.
//!
//! v0 baseline: simple heuristic splitter and Unicode-alphanumeric tokenizer.
//! Parity with LT's `SRXSentenceTokenizer`/English word tokenizer is the P1.1
//! deliverable; this module defines the offset contract everything else uses.

pub mod breton;
pub mod catalan;
pub mod dutch;
pub mod english;
pub mod esperanto;
pub mod french;
pub mod galician;
pub mod german;
pub mod german_compound;
pub mod greek;
pub mod polish;
pub mod portuguese;
pub mod romanian;
pub mod spanish;
pub mod srx;
pub mod tagalog;
pub mod wordtokenizer;

pub use breton::BretonWordTokenizer;
pub use catalan::CatalanWordTokenizer;
pub use dutch::DutchWordTokenizer;
pub use english::{EnglishWordTokenizer, IsTagged};
pub use esperanto::EsperantoWordTokenizer;
pub use french::FrenchWordTokenizer;
pub use galician::GalicianWordTokenizer;
pub use german::GermanWordTokenizer;
pub use german_compound::GermanCompoundTokenizer;
pub use greek::GreekWordTokenizer;
pub use polish::PolishWordTokenizer;
pub use portuguese::PortugueseWordTokenizer;
pub use romanian::RomanianWordTokenizer;
pub use spanish::SpanishWordTokenizer;
pub use srx::{SrxDocument, SrxTokenizer};
pub use tagalog::TagalogWordTokenizer;

use lt_core::{Sentence, TextRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Word,
    Space,
    Punct,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub range: TextRange,
    pub kind: TokenKind,
}

/// Tokenize `text` into word / space / punctuation tokens with byte offsets.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut iter = text.char_indices().peekable();
    while let Some((idx, ch)) = iter.next() {
        let kind = classify(ch);
        let tok_start = idx;
        let mut tok_end = idx + ch.len_utf8();
        match kind {
            TokenKind::Word => {
                // extend over alphanumerics, intra-word apostrophes, hyphens
                while let Some(&(i, c)) = iter.peek() {
                    if classify(c) == TokenKind::Word || c == '\'' || c == '\u{2019}' || c == '-' {
                        tok_end = i + c.len_utf8();
                        iter.next();
                    } else {
                        break;
                    }
                }
            }
            TokenKind::Other => {
                while let Some(&(i, c)) = iter.peek() {
                    if classify(c) == TokenKind::Other {
                        tok_end = i + c.len_utf8();
                        iter.next();
                    } else {
                        break;
                    }
                }
            }
            TokenKind::Space | TokenKind::Punct => {}
        }
        tokens.push(Token {
            text: text[tok_start..tok_end].to_string(),
            range: TextRange::new(tok_start, tok_end),
            kind,
        });
    }
    tokens
}

fn classify(ch: char) -> TokenKind {
    if ch.is_whitespace() {
        TokenKind::Space
    } else if ch.is_alphanumeric() {
        TokenKind::Word
    } else if ch.is_ascii_punctuation() || is_unicode_punct(ch) {
        TokenKind::Punct
    } else {
        TokenKind::Other
    }
}

fn is_unicode_punct(ch: char) -> bool {
    matches!(ch, '\u{2013}'..='\u{2026}' | '\u{00ab}' | '\u{00bb}')
}

/// v0 sentence splitter: break after `.`, `!`, `?` (and combinations) when
/// followed by whitespace and a sentence-start character, or at newlines.
pub fn split_sentences(text: &str) -> Vec<Sentence> {
    let mut sentences = Vec::new();
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (idx, ch) = chars[i];
        if ch == '\n' {
            let end = idx + 1;
            push_sentence(&mut sentences, text, start, end);
            start = end;
            i += 1;
            continue;
        }
        if matches!(ch, '.' | '!' | '?' | '…') {
            // consume run of terminators
            let mut j = i;
            while j < chars.len() && matches!(chars[j].1, '.' | '!' | '?' | '…' | '"') {
                j += 1;
            }
            // lookahead: whitespace then sentence-start char, or end of text
            let mut k = j;
            while k < chars.len() && chars[k].1.is_whitespace() && chars[k].1 != '\n' {
                k += 1;
            }
            let next = chars.get(k).map(|&(_, c)| c);
            let boundary = match next {
                None => true,
                Some(c) => {
                    c.is_uppercase()
                        || c == '"'
                        || c == '\u{201c}'
                        || c == '('
                        || c == '*'
                        || c == '-'
                }
            };
            if boundary && (j < chars.len() || start < text.len()) {
                let end = chars[j.saturating_sub(1)].0 + chars[j.saturating_sub(1)].1.len_utf8();
                push_sentence(&mut sentences, text, start, end);
                start = end;
                i = j;
                continue;
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    if start < text.len() {
        push_sentence(&mut sentences, text, start, text.len());
    }
    sentences
}

fn push_sentence(out: &mut Vec<Sentence>, text: &str, start: usize, end: usize) {
    let trimmed_start = text[start..end].len() - text[start..end].trim_start().len();
    let s = start + trimmed_start;
    if s < end && !text[s..end].trim().is_empty() {
        out.push(Sentence {
            range: TextRange::new(s, end),
            text: text[s..end].to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_offsets_are_byte_accurate() {
        let text = "Grüße, Müller-Welt!";
        let tokens = tokenize(text);
        for t in &tokens {
            assert_eq!(&text[t.range.start..t.range.end], t.text);
        }
        let words: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Word)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(words, vec!["Grüße", "Müller-Welt"]);
    }

    #[test]
    fn tokenizer_keeps_intra_word_apostrophes() {
        let tokens = tokenize("don't stop");
        let words: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Word)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(words, vec!["don't", "stop"]);
    }

    #[test]
    fn splits_simple_sentences() {
        let text = "This is a test. And another one! Right?";
        let sents = split_sentences(text);
        let texts: Vec<&str> = sents.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["This is a test.", "And another one!", "Right?"]);
        for s in &sents {
            assert_eq!(&text[s.range.start..s.range.end], s.text);
        }
    }

    #[test]
    fn no_split_on_abbreviations_yet_but_offsets_stay_valid() {
        // v0 may over-split; the contract is only that ranges stay valid.
        let text = "Dr. Smith arrived.";
        for s in split_sentences(text) {
            assert_eq!(&text[s.range.start..s.range.end], s.text);
        }
    }

    #[test]
    fn empty_text_yields_no_sentences() {
        assert!(split_sentences("").is_empty());
        assert!(split_sentences("   \n ").is_empty());
    }
}
