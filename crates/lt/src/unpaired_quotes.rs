//! Port of `GenericUnpairedQuotesRule` / `EnglishUnpairedQuotesRule`
//! (`EN_UNPAIRED_QUOTES`): finds unpaired quotation marks.

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use regex::Regex;
use std::sync::LazyLock;

/// Per-language symbol lists and strings of `GenericUnpairedQuotesRule`.
pub struct UnpairedQuotesSymbols {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    pub start: &'static [&'static str],
    pub end: &'static [&'static str],
    /// `EnglishUnpairedQuotesRule` overrides the apostrophe checks with the
    /// tag-based ones; German inherits the base `POSSIBLE_APOSTROPHE` logic.
    pub english_apostrophes: bool,
}

pub fn symbols_en() -> UnpairedQuotesSymbols {
    UnpairedQuotesSymbols {
        rule_id: "EN_UNPAIRED_QUOTES",
        description: "Unpaired quotation marks",
        category_id: "PUNCTUATION",
        category_name: "Punctuation",
        start: &["“", "\"", "'", "‘"],
        end: &["”", "\"", "'", "’"],
        english_apostrophes: true,
    }
}

/// `de.GermanUnpairedQuotesRule` (`DE_UNPAIRED_QUOTES`).
pub fn symbols_de() -> UnpairedQuotesSymbols {
    UnpairedQuotesSymbols {
        rule_id: "DE_UNPAIRED_QUOTES",
        description: "Unpaarige Anführungszeichen",
        category_id: "PUNCTUATION",
        category_name: "Zeichensetzung",
        start: &["„", "»", "«", "\"", "'", "‚", "›", "‹"],
        end: &["“", "«", "»", "\"", "'", "‘", "‹", "›"],
        english_apostrophes: false,
    }
}

struct SymbolLocator {
    symbol: String,
    start_pos: usize,
}

fn corresponding_symbol(sym: &UnpairedQuotesSymbols, symbol: &str) -> String {
    if let Some(idx) = sym.start.iter().position(|s| *s == symbol) {
        return sym.end[idx].to_string();
    }
    if let Some(idx) = sym.end.iter().position(|s| *s == symbol) {
        return sym.start[idx].to_string();
    }
    symbol.to_string()
}

/// `GenericUnpairedQuotesRule.PUNCTUATION` (`[\p{Punct}…–—&&[^"'_]]`).
fn is_punctuation_char(c: char) -> bool {
    (c.is_ascii_punctuation() || matches!(c, '…' | '–' | '—')) && !matches!(c, '"' | '_' | '\'')
}

fn is_punctuation(token: &str) -> bool {
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => is_punctuation_char(c),
        _ => false,
    }
}

fn is_punct_mark(token: &str) -> bool {
    matches!(token, "?" | "." | "!" | ",")
}

/// `GenericUnpairedQuotesRule.INCH_PATTERN` (`.*\d".*`, DOTALL).
fn is_inch_quote(text: &str) -> bool {
    text.char_indices().any(|(i, c)| {
        c == '"'
            && text[..i]
                .chars()
                .next_back()
                .is_some_and(|p| p.is_ascii_digit())
    })
}

fn has_apostrophe_tag(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag("_apostrophe_contraction_")
        || token.has_pos_tag("POS")
        || token.has_pos_tag("NNP")
}

/// `GenericUnpairedQuotesRule.POSSIBLE_APOSTROPHE` (`[‘’']`, full match).
static POSSIBLE_APOSTROPHE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[‘’']$").unwrap());
/// `AnalyzedTokenReadings.NON_WORD_REGEX` (single punctuation char, full
/// match).
static NON_WORD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[.?!…:;,~’'"„“”»«‚‘›‹()\[\]\-–—*×∗·+÷/=]$"#).unwrap());

fn is_not_beginning_apostrophe(
    sym: &UnpairedQuotesSymbols,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
) -> bool {
    if sym.english_apostrophes {
        return !has_apostrophe_tag(tokens[i]);
    }
    !POSSIBLE_APOSTROPHE.is_match(tokens[i].surface())
        || i >= tokens.len() - 1
        || NON_WORD_REGEX.is_match(tokens[i + 1].surface())
        || tokens[i + 1].whitespace_before
}

fn is_not_ending_apostrophe(
    sym: &UnpairedQuotesSymbols,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
) -> bool {
    if sym.english_apostrophes {
        return !has_apostrophe_tag(tokens[i]);
    }
    !POSSIBLE_APOSTROPHE.is_match(tokens[i].surface())
        || tokens[i].whitespace_before
        || NON_WORD_REGEX.is_match(tokens[i - 1].surface())
}

fn is_not_quote(
    sym: &UnpairedQuotesSymbols,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    j: usize,
) -> bool {
    if (tokens[i - 1].is_sentence_start || tokens[i].whitespace_before)
        && (i >= tokens.len() - 1 || tokens[i + 1].whitespace_before)
    {
        return true;
    }
    if end_symbol_equals_start(sym, j)
        && i < tokens.len() - 1
        && !tokens[i].whitespace_before
        && !tokens[i + 1].whitespace_before
        && is_punctuation(tokens[i - 1].surface())
        && tokens[i + 1].surface() != "."
        && is_punctuation(tokens[i + 1].surface())
    {
        return true;
    }
    false
}

fn end_symbol_equals_start(sym: &UnpairedQuotesSymbols, j: usize) -> bool {
    sym.end[j] == sym.start[j]
}

fn is_start_symbol_before(
    sym: &UnpairedQuotesSymbols,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
) -> bool {
    let mut j = i - 1;
    while j > 0 {
        if tokens[i].surface() != tokens[j].surface() && sym.start.contains(&tokens[j].surface()) {
            if tokens[j - 1].is_sentence_start || tokens[j].whitespace_before {
                return true;
            }
        } else {
            return false;
        }
        j -= 1;
    }
    true
}

fn is_not_open_symbol(
    sym: &UnpairedQuotesSymbols,
    j: usize,
    opening_quotes: &[SymbolLocator],
) -> bool {
    if end_symbol_equals_start(sym, j) {
        for opening_quote in opening_quotes {
            if sym.end[j] == opening_quote.symbol {
                return false;
            }
        }
    }
    true
}

fn is_opening_quote(
    sym: &UnpairedQuotesSymbols,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
) -> bool {
    for (j, start_symbol) in sym.start.iter().enumerate() {
        if *start_symbol == tokens[i].surface() {
            if is_not_quote(sym, tokens, i, j) {
                return false;
            }
            if sym.end.contains(start_symbol) {
                return tokens[i - 1].is_sentence_start
                    || tokens[i].whitespace_before
                    || (i < tokens.len() - 1
                        && !tokens[i + 1].whitespace_before
                        && ((!is_punct_mark(tokens[i + 1].surface())
                            && is_punctuation(tokens[i - 1].surface()))
                            || tokens[i - 1].surface().ends_with('-')))
                    || is_start_symbol_before(sym, tokens, i);
            }
            return true;
        }
    }
    false
}

fn is_closing_quote(
    sym: &UnpairedQuotesSymbols,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    opening_quotes: &[SymbolLocator],
) -> bool {
    for (j, end_symbol) in sym.end.iter().enumerate() {
        if *end_symbol == tokens[i].surface() {
            if is_not_quote(sym, tokens, i, j) && is_not_open_symbol(sym, j, opening_quotes) {
                return false;
            }
            return true;
        }
    }
    false
}

fn index_of_opening_quote(opening_quotes: &[SymbolLocator], symbol: &str) -> Option<usize> {
    opening_quotes.iter().position(|q| q.symbol == symbol)
}

fn add_match(sym: &UnpairedQuotesSymbols, locator: &SymbolLocator, rule_matches: &mut Vec<Match>) {
    let symbol = &locator.symbol;
    let message = if sym.rule_id == "DE_UNPAIRED_QUOTES" {
        format!(
            "Zeichen ohne sein Gegenstück: '{}' scheint zu fehlen",
            corresponding_symbol(sym, symbol)
        )
    } else {
        format!(
            "Unpaired symbol: '{}' seems to be missing",
            corresponding_symbol(sym, symbol)
        )
    };
    rule_matches.push(
        Match::new(
            sym.rule_id,
            Option::<String>::None,
            message,
            None::<String>,
            TextRange::new(locator.start_pos, locator.start_pos + symbol.len()),
            Vec::<Suggestion>::new(),
            sym.category_id,
            sym.category_name,
        )
        .with_metadata(sym.description, "typographical", 0)
        .with_match_type("Other"),
    );
}

fn remove_all_open_inner_quotes(
    sym: &UnpairedQuotesSymbols,
    index: i64,
    opening_quotes: &mut Vec<SymbolLocator>,
    rule_matches: &mut Vec<Match>,
) {
    let mut i = opening_quotes.len() as i64 - 1;
    while i > index {
        add_match(sym, &opening_quotes[i as usize], rule_matches);
        opening_quotes.remove(i as usize);
        i -= 1;
    }
}

/// `GenericUnpairedQuotesRule.match` over all sentences of the text.
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_en())
}

/// German `DE_UNPAIRED_QUOTES` (`GermanUnpairedQuotesRule`).
pub fn check_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_de())
}

pub fn check_with(sentences: &[AnalyzedSentence], sym: &UnpairedQuotesSymbols) -> Vec<Match> {
    let mut opening_quotes: Vec<SymbolLocator> = Vec::new();
    let mut rule_matches: Vec<Match> = Vec::new();
    let mut last_apostrophe_symbol: Option<String> = None;
    let mut was_inch = false;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for i in 1..tokens.len() {
            if is_opening_quote(sym, &tokens, i) {
                let symbol = tokens[i].surface().to_string();
                if !is_not_beginning_apostrophe(sym, &tokens, i) {
                    last_apostrophe_symbol = Some(symbol);
                    continue;
                }
                if symbol == "\"" {
                    was_inch = false;
                }
                if last_apostrophe_symbol.as_deref() == Some(symbol.as_str()) {
                    last_apostrophe_symbol = None;
                }
                let index = index_of_opening_quote(&opening_quotes, &symbol);
                if let Some(index) = index {
                    remove_all_open_inner_quotes(
                        sym,
                        index as i64 - 1,
                        &mut opening_quotes,
                        &mut rule_matches,
                    );
                }
                opening_quotes.push(SymbolLocator {
                    symbol,
                    start_pos: sentence.offset + tokens[i].start_pos,
                });
            } else if is_closing_quote(sym, &tokens, i, &opening_quotes) {
                let symbol = tokens[i].surface().to_string();
                if !is_not_beginning_apostrophe(sym, &tokens, i) {
                    last_apostrophe_symbol = Some(symbol);
                    continue;
                }
                let is_inch_symb = symbol == "\"";
                let is_inch = if is_inch_symb {
                    is_inch_quote(&sentence.text)
                } else {
                    false
                };
                let start_symbol = corresponding_symbol(sym, &symbol);
                let index = index_of_opening_quote(&opening_quotes, &start_symbol);
                if let Some(index) = index {
                    remove_all_open_inner_quotes(
                        sym,
                        index as i64,
                        &mut opening_quotes,
                        &mut rule_matches,
                    );
                    opening_quotes.remove(index);
                    if last_apostrophe_symbol.as_deref() == Some(start_symbol.as_str()) {
                        last_apostrophe_symbol = None;
                    }
                    if is_inch {
                        was_inch = true;
                    }
                } else if is_not_ending_apostrophe(sym, &tokens, i) {
                    if !is_inch && (!is_inch_symb || !was_inch) {
                        if last_apostrophe_symbol.as_deref() != Some(symbol.as_str()) {
                            add_match(
                                sym,
                                &SymbolLocator {
                                    symbol,
                                    start_pos: sentence.offset + tokens[i].start_pos,
                                },
                                &mut rule_matches,
                            );
                        } else {
                            last_apostrophe_symbol = None;
                        }
                    } else {
                        was_inch = false;
                    }
                }
            }
        }
    }
    remove_all_open_inner_quotes(sym, -1, &mut opening_quotes, &mut rule_matches);
    rule_matches
}
