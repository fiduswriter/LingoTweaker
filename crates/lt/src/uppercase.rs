//! Port of `UppercaseSentenceStartRule` (`UPPERCASE_SENTENCE_START`): checks
//! that a sentence starts with an uppercase letter.

use std::sync::LazyLock;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use regex::Regex;

use crate::wordutil::{is_email, is_url};

const RULE_ID: &str = "UPPERCASE_SENTENCE_START";
const MESSAGE: &str = "This sentence does not start with an uppercase letter.";
const SHORT_MESSAGE: &str = "Capitalization";
const DESCRIPTION: &str = "Checks that a sentence starts with an uppercase letter";
const CATEGORY_ID: &str = "CASING";
const CATEGORY_NAME: &str = "Upper/Lowercase";

/// `UppercaseSentenceStartRule.NUMERALS_EN` (full match).
static NUMERALS_EN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[a-z]|(?:m{0,4}(?:c[md]|d?c{0,3})(?:x[cl]|l?x{0,3})(?:i[xv]|v?i{0,3})))$")
        .unwrap()
});
static CONTAINS_DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*\d.*$").unwrap());
static ONLY_LOWERCASE_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][A-Z].*$").unwrap());
static WHITESPACE_OR_QUOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[ \"'„«»‘’“”\n]$").unwrap());
static DIGIT_DOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+\. .*$").unwrap());
/// `.*\n\d+\. ` (`.` does not match `\n` by default in Java either).
static LINEBREAK_DIGIT_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*\n\d+\. .*$").unwrap());
static CAMEL_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]+[A-Z][A-Za-z]+$").unwrap());

const EXCEPTIONS: [&str; 8] = ["n", "w", "x86", "ⓒ", "ø", "cc", "pH", "heylogin"];

fn is_sentence_end(word: &str) -> bool {
    matches!(word, "." | "?" | "!" | "…")
}

fn is_quote_start(word: &str) -> bool {
    matches!(word, "\"" | "'" | "„" | "»" | "«" | "“" | "‘" | "¡" | "¿")
}

fn uppercase_first_char(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `UppercaseSentenceStartRule.match` (English strings).
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        MESSAGE,
        SHORT_MESSAGE,
        DESCRIPTION,
        CATEGORY_NAME,
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Spanish `MessagesBundle_es` strings.
pub fn check_es(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Esta frase no empieza con mayúscula.",
        "Mayúsculas y minúsculas",
        "Comprobar si la frase empieza con una letra mayúscula",
        "Mayúsculas y minúsculas",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the German `MessagesBundle_de` strings.
pub fn check_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Dieser Satz fängt nicht mit einem großgeschriebenen Wort an.",
        "Groß-/Kleinschreibung",
        "Großschreibung am Satzanfang",
        "Groß-/Kleinschreibung",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the French `MessagesBundle_fr` strings.
pub fn check_fr(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Cette phrase ne commence pas par une majuscule.",
        "Majuscules",
        "Absence de majuscule en début de phrase",
        "Majuscules",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Italian `MessagesBundle_it` strings.
pub fn check_it(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Questa frase non inizia con una maiuscola",
        "Uso delle maiuscole",
        "Controlla che la frase inizi con una maiuscola",
        "Uso delle maiuscole",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Portuguese strings
/// (`MessagesBundle_pt_PT`, or `MessagesBundle_pt_BR` for `pt-BR`).
pub fn check_pt(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    if variant.starts_with("pt-BR") {
        check_with(
            sentences,
            "Esta frase não inicia com um letra maiúscula",
            "Maiúsculo / Minúsculo",
            "Verifica que a frase inicia com uma letra em maiúsculo",
            "Maiúsculo / Minúsculo",
            false,
        )
    } else {
        check_with(
            sentences,
            "Esta frase não começa com maiúscula.",
            "Capitalização",
            "Capitalização da frase",
            "Capitalização",
            false,
        )
    }
}

/// `UppercaseSentenceStartRule` with the Dutch `MessagesBundle_nl` strings.
pub fn check_nl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Deze zin begint niet met een hoofdletter",
        "Hoofdlettergebruik",
        "Controleert of een zin begint met een hoofdletter",
        "Hoofdlettergebruik",
        true,
    )
}

/// `UppercaseSentenceStartRule` with the Catalan `MessagesBundle_ca` strings.
pub fn check_ca(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Aquesta frase no comença amb majúscula.",
        "Majúscules i minúscules",
        "Comproveu que la frase comença amb majúscula",
        "Majúscules i minúscules",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Romanian `MessagesBundle_ro`
/// strings.
pub fn check_ro(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Propoziția nu începe cu literă mare",
        "Capitalizare",
        "Verifică dacă propoziția începe cu literă mare",
        "Capitalizare",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Slovak `MessagesBundle_sk` strings.
pub fn check_sk(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Táto veta nezačína s veľkým písmenom",
        "Veľké a malé písmená",
        "Skontrolujte, či veta začína veľkými počiatočnými písmenami",
        "Veľké a malé písmená",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Slovenian `MessagesBundle_sl` strings.
pub fn check_sl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Ta poved se ne začenja z veliko začetnico",
        "Velike začetnice",
        "Preveri, da se poved začne z veliko začetnico",
        "Velike začetnice",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Galician `MessagesBundle_gl` strings.
pub fn check_gl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Esta oración non comeza cunha letra maiúscula.",
        "Maiúsculas e minúsculas",
        "Comproba que unha oración comece con maiúscula",
        "Maiúsculas e minúsculas",
        false,
    )
}

/// `UppercaseSentenceStartRule` with the Polish `MessagesBundle_pl` strings.
pub fn check_pl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "To zdanie nie zaczyna się wielką literą",
        "Pisownia małą i wielką literą",
        "Test, czy zdanie zaczyna się wielką literą",
        "Pisownia małą i wielką literą",
        false,
    )
}

fn check_with(
    sentences: &[AnalyzedSentence],
    message: &str,
    short_message: &str,
    description: &str,
    category_name: &str,
    dutch_special: bool,
) -> Vec<Match> {
    let mut rule_matches: Vec<Match> = Vec::new();
    if sentences.len() == 1 && sentences[0].tokens.len() == 2 {
        // special case for a single "sentence" with a single word (Java
        // counts the tokens including whitespace)
        return rule_matches;
    }
    let mut last_paragraph_string = String::new();
    let mut is_prev_sentence_numbered_list = false;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        if tokens.len() < 2 {
            return rule_matches;
        }
        let mut match_token_pos = 1usize;
        let first_token_obj = tokens[match_token_pos];
        let first_token = first_token_obj.surface().to_string();
        let mut second_token: Option<String> = None;
        let mut third_token: Option<String> = None;
        // ignore quote characters
        if tokens.len() >= 3 && is_quote_start(&first_token) {
            match_token_pos = 2;
            second_token = Some(tokens[match_token_pos].surface().to_string());
        }
        // `UppercaseSentenceStartRule.dutchSpecialCase`: `'t`, `'s`, `'m` …
        // move the check to the word after the elided pronoun.
        if dutch_special
            && tokens.len() > 3
            && first_token == "'"
            && matches!(
                second_token.as_deref(),
                Some("k" | "m" | "n" | "r" | "s" | "t")
            )
        {
            match_token_pos = 3;
            third_token = Some(tokens[3].surface().to_string());
        }
        let check_token = third_token
            .clone()
            .or_else(|| second_token.clone())
            .unwrap_or_else(|| first_token.clone());

        let mut last_token = tokens[tokens.len() - 1].surface().to_string();
        if WHITESPACE_OR_QUOTE.is_match(&last_token) {
            // ignore trailing whitespace or quote
            last_token = tokens[tokens.len() - 2].surface().to_string();
        }

        let mut prevent_error = false;
        if last_paragraph_string == "," || last_paragraph_string == ";" {
            prevent_error = true;
        }
        if CONTAINS_DIGIT.is_match(tokens[match_token_pos].surface()) {
            prevent_error = true;
        }
        // `SENTENCE_END1` is `[.?!…]|` — the empty alternative makes it
        // match every string, so Java's check
        // `!SENTENCE_END1.matches(lastParagraphString) && !isSentenceEnd(lastToken)`
        // can never trigger. Keep the `isSentenceEnd` helper for clarity.
        let _ = is_sentence_end(&last_token);
        if !sentence.text.replace('\u{a0}', " ").trim().is_empty() {
            last_paragraph_string = last_token.clone();
        }

        // allows enumeration with lowercase letters: a), iv., etc.
        if match_token_pos + 1 < tokens.len()
            && NUMERALS_EN.is_match(tokens[match_token_pos].surface())
            && matches!(tokens[match_token_pos + 1].surface(), "." | ")")
        {
            prevent_error = true;
        }

        if is_prev_sentence_numbered_list
            || is_url(&check_token)
            || is_email(&check_token)
            || first_token_obj.is_immunized
            || tokens[match_token_pos].has_pos_tag("_IS_URL")
        {
            prevent_error = true;
        }

        if !check_token.is_empty() {
            let first_char = check_token.chars().next().unwrap();
            let capitalized = uppercase_first_char(&check_token);
            if capitalized != check_token
                && !prevent_error
                && first_char.is_lowercase()
                && !ONLY_LOWERCASE_START.is_match(&check_token)
                && !EXCEPTIONS.contains(&check_token.as_str())
                && !CAMEL_CASE.is_match(&check_token)
            {
                let token = tokens[match_token_pos];
                let start = sentence.offset + token.start_pos;
                let end = sentence.offset + token.end_pos();
                let rule_match = Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    message,
                    Some(short_message.to_string()),
                    TextRange::new(start, end),
                    vec![Suggestion {
                        value: capitalized,
                        short_description: None,
                    }],
                    CATEGORY_ID,
                    category_name,
                )
                .with_metadata(description, "typographical", 0)
                .with_match_type("Other");
                rule_matches.push(rule_match);
            }
        }

        is_prev_sentence_numbered_list =
            DIGIT_DOT.is_match(&sentence.text) || LINEBREAK_DIGIT_DOT.is_match(&sentence.text);
    }
    rule_matches
}
