//! Port of `GenericUnpairedBracketsRule` / `EnglishUnpairedBracketsRule`
//! (`EN_UNPAIRED_BRACKETS`): stack-based pairing of `[ ( {` / `] ) }`.

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

/// Per-language symbol lists and strings of `GenericUnpairedBracketsRule`.
pub struct UnpairedSymbols {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    pub start: &'static [&'static str],
    pub end: &'static [&'static str],
    /// `SpanishUnpairedBracketsRule`: symbol suggestions and the Spanish
    /// `isNoException` checks
    pub spanish: bool,
    /// `GenericUnpairedBracketsRule.getMessage` template; `{other}` is the
    /// missing counterpart symbol.
    pub message_template: &'static str,
}

pub fn symbols_en() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "EN_UNPAIRED_BRACKETS",
        description: "Unpaired braces, brackets, quotation marks and similar symbols",
        category_id: "PUNCTUATION",
        category_name: "Punctuation",
        start: &["[", "(", "{"],
        end: &["]", ")", "}"],
        spanish: false,
        message_template: "Unpaired symbol: '{other}' seems to be missing",
    }
}

/// `Italian.getRelevantRules`: the generic rule with `[ ( { » «` / `] ) } « »`
/// (Italian quotes) and the Italian strings.
pub fn symbols_it() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: "Non chiusura di parentesi, virgolette e altra punteggiatura simile",
        category_id: "PUNCTUATION",
        category_name: "Punteggiatura",
        start: &["[", "(", "{", "»", "«"],
        end: &["]", ")", "}", "«", "»"],
        spanish: false,
        message_template: "Manca chiusura: \"{other}\" sembra mancare",
    }
}

/// `French.getRelevantRules`: the generic rule with `[ ( {` / `] ) }` (no
/// quotes; French dialog can span sentences) and the French strings.
pub fn symbols_fr() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description:
            "Guillemet, parenthèse ou autres symboles similaires fermants ou ouvrants manquants",
        category_id: "PUNCTUATION",
        category_name: "Ponctuation",
        start: &["[", "(", "{"],
        end: &["]", ")", "}"],
        spanish: false,
        message_template:
            "Pas de correspondance fermante ou ouvrante pour le caractère « {other} »",
    }
}

/// `de.GermanUnpairedBracketsRule` keeps the id `UNPAIRED_BRACKETS`
/// ("no `DE_` to be compatible with old versions") and the same symbols.
pub fn symbols_de() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: "Unpaarige Anführungszeichen und Klammern",
        category_id: "PUNCTUATION",
        category_name: "Zeichensetzung",
        start: &["[", "(", "{"],
        end: &["]", ")", "}"],
        spanish: false,
        message_template: "Zeichen ohne sein Gegenstück: '{other}' scheint zu fehlen",
    }
}

/// `es.SpanishUnpairedBracketsRule` symbols (quotes included).
pub fn symbols_es() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "ES_UNPAIRED_BRACKETS",
        description:
            "Paréntesis, comillas, signos de exclamación, interrogación y similares desparejados",
        category_id: "PUNCTUATION",
        category_name: "Puntuación",
        start: &["[", "(", "{", "“", "«", "\"", "'", "‘"],
        end: &["]", ")", "}", "”", "»", "\"", "'", "’"],
        spanish: true,
        message_template: "Símbolo desparejado: Parece que falta un ‘{other}’.",
    }
}

fn url_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^https?://.+$").unwrap());
    &RE
}

/// `GenericUnpairedBracketsRule.NUMERALS_EN` (`matches()`, case-insensitive).
fn numerals_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)^(?:\d{1,2}?[a-z']*|M*(?:D?C{0,3}|C[DM])(?:L?X{0,3}|X[LC])(?:V?I{0,3}|I[VX])$)$",
        )
        .unwrap()
    });
    &RE
}

fn context_2_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^\n[a-zA-Z]\)$").unwrap());
    &RE
}

fn context_1_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[a-zA-Z]\)$").unwrap());
    &RE
}

struct SymbolLocator {
    symbol: String,
    closing: bool,
    start_pos: usize,
    sentence_index: usize,
    /// Index into `ruleMatches` (only used on the rule-match stack)
    rule_index: usize,
}

fn corresponding_symbol(sym: &UnpairedSymbols, symbol: &str) -> String {
    if let Some(idx) = sym.start.iter().position(|s| *s == symbol) {
        return sym.end[idx].to_string();
    }
    if let Some(idx) = sym.end.iter().position(|s| *s == symbol) {
        return sym.start[idx].to_string();
    }
    symbol.to_string()
}

/// `GenericUnpairedBracketsRule.isNoException` (English: no subclass override).
fn is_no_exception(tokens: &[&AnalyzedTokenReadings], i: usize, token: &str) -> bool {
    if i > 0 && url_re().is_match(tokens[i - 1].surface()) && tokens[i - 1].surface().contains('(')
    {
        return false;
    }
    if i >= 2 {
        let prev_prev = tokens[i - 2].surface();
        let prev = tokens[i - 1].surface();
        // Smiley ":-)" and ":-(", ";-)" and ";-("
        if (prev_prev == ":" || prev_prev == ";") && prev == "-" && (token == ")" || token == "(") {
            return false;
        }
    }
    if i >= 1 {
        let prev = tokens[i - 1].surface();
        // Smiley ":)" and ":(", ";)" and ";("
        if (prev == ":" || prev == ";")
            && !tokens[i].whitespace_before
            && (token == ")" || token == "(")
        {
            return false;
        }
    }
    true
}

/// `SpanishUnpairedBracketsRule.isNoException` (after the generic checks).
fn is_no_exception_es(tokens: &[&AnalyzedTokenReadings], i: usize, token: &str) -> bool {
    if i < 1 {
        return true;
    }
    if !is_no_exception(tokens, i, token) {
        return false;
    }
    let is_quote = |t: &str| t == "'" || t == "’";
    if (token == "’" || token == "'")
        && (tokens[i].has_pos_tag_starting_with("N")
            || tokens[i].has_pos_tag_starting_with("A")
            || tokens[i].has_pos_tag("_allow_apostrophe"))
    {
        return false;
    }
    // Exception for English plural Saxon genitive
    if i + 1 < tokens.len() && is_quote(token) && tokens[i + 1].surface().eq_ignore_ascii_case("s")
    {
        return false;
    }
    // degrees, minutes, seconds...
    if token == "\"" || token == "'" {
        static NUMBER: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:\d[\d., ]+\d|\d{1,2})$").unwrap());
        if NUMBER.is_match(tokens[i - 1].surface())
            && !tokens[i].whitespace_before
            && ((i > 2
                && (tokens[i - 2].surface().contains('º')
                    || tokens[i - 2].surface().contains('°')))
                || (i > 4
                    && (tokens[i - 4].surface().contains('º')
                        || tokens[i - 4].surface().contains('°'))))
        {
            return false;
        }
    }
    if i == 1 && token == "»" {
        return false;
    }
    if i > 1 && token == ")" {
        static VALID: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:\d+|[a-zA-Z])$").unwrap());
        let mut is_there_opening_parenthesis = false;
        let mut k = 1usize;
        while i > k {
            if tokens[i - k].surface() == ")" {
                break;
            }
            if tokens[i - k].surface() == "(" {
                is_there_opening_parenthesis = true;
                break;
            }
            k += 1;
        }
        if !is_there_opening_parenthesis && VALID.is_match(tokens[i - 1].surface()) {
            return false;
        }
    }
    true
}

/// `GenericUnpairedBracketsRule.getPrecededByWhitespace`: symmetric symbols
/// (quote-like) only open when preceded by whitespace, sentence start,
/// punctuation-without-dot or another opening symbol.
fn preceded_by_whitespace(
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    j: usize,
    sym: &UnpairedSymbols,
) -> bool {
    if sym.start[j] != sym.end[j] {
        return true;
    }
    static PUNCTUATION_NO_DOT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"[ldmnstLDMNST]'|[–—[:punct:]&&[^.]]").unwrap()
    });
    tokens[i - 1].is_sentence_start
        || tokens[i].whitespace_before
        || PUNCTUATION_NO_DOT.is_match(tokens[i - 1].surface())
        || sym.start.contains(&tokens[i - 1].surface())
}

/// `GenericUnpairedBracketsRule.getSpecialCase` (true means "not the one
/// exception case" for symmetric symbols followed by a normal token).
fn special_case(
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    j: usize,
    sym: &UnpairedSymbols,
) -> bool {
    if i < tokens.len() - 1 && sym.start[j] == sym.end[j] {
        static PUNCTUATION: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r"[[:punct:]…–—]").unwrap());
        let next = tokens[i + 1].surface();
        return tokens[i + 1].whitespace_before
            || PUNCTUATION.is_match(next)
            || sym.end.contains(&next)
            || (i >= 1 && tokens[i - 1].surface().ends_with('-'))
            || next.starts_with('-')
            || next == "s";
    }
    true
}

fn fill_symbol_stack(
    sym: &UnpairedSymbols,
    start_pos_base: usize,
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    j: usize,
    symbol_stack: &mut Vec<SymbolLocator>,
    sentence_index: usize,
) -> bool {
    let token = tokens[i].surface();
    let start_pos = start_pos_base + tokens[i].start_pos;
    if token == sym.start[j] || token == sym.end[j] {
        let no_exception = if sym.spanish {
            is_no_exception_es(tokens, i, token)
        } else {
            is_no_exception(tokens, i, token)
        };
        let preceded = preceded_by_whitespace(tokens, i, j, sym);
        let special = special_case(tokens, i, j, sym);
        if no_exception && preceded && token == sym.start[j] {
            symbol_stack.push(SymbolLocator {
                symbol: sym.start[j].to_string(),
                closing: false,
                start_pos,
                sentence_index,
                rule_index: 0,
            });
            return true;
        } else if no_exception && (special || tokens[i].is_sentence_end) && token == sym.end[j] {
            let top_is_open_paren = symbol_stack.last().is_some_and(|s| s.symbol == "(");
            let numeral_suppressed = (i > 2
                && sym.end[j] == ")"
                && (tokens[i - 3].has_pos_tag("SENT_START") || tokens[i - 2].whitespace_before)
                && tokens[i - 1].surface() == "."
                && numerals_re().is_match(tokens[i - 2].surface())
                && !top_is_open_paren)
                || (i > 1
                    && sym.end[j] == ")"
                    && numerals_re().is_match(tokens[i - 1].surface())
                    && !top_is_open_paren);
            if !numeral_suppressed {
                if symbol_stack.is_empty() {
                    symbol_stack.push(SymbolLocator {
                        symbol: sym.end[j].to_string(),
                        closing: true,
                        start_pos,
                        sentence_index,
                        rule_index: 0,
                    });
                    return true;
                } else if symbol_stack
                    .last()
                    .is_some_and(|s| s.symbol == sym.start[j])
                {
                    symbol_stack.pop();
                    return true;
                } else {
                    // `isEndSymbolUnique` is always true for the English
                    // symbol lists; the `j == last` fallback also pushes.
                    symbol_stack.push(SymbolLocator {
                        symbol: sym.end[j].to_string(),
                        closing: true,
                        start_pos,
                        sentence_index,
                        rule_index: 0,
                    });
                    return true;
                }
            }
        }
    }
    false
}

fn ends_like_real_sentence(text: &str) -> bool {
    let s = text.trim();
    s.ends_with('.') || s.ends_with('?') || s.ends_with('!')
}

#[allow(clippy::too_many_arguments)]
fn create_match(
    sym: &UnpairedSymbols,
    rule_matches: &mut Vec<Match>,
    rule_match_stack: &mut Vec<SymbolLocator>,
    start_pos: usize,
    symbol: &str,
    sentence_index: usize,
    full_text: &str,
) -> Option<Match> {
    if let Some(rloc) = rule_match_stack.last() {
        if let Some(index) = sym.end.iter().position(|s| *s == symbol) {
            if rloc.symbol == sym.start[index] && rule_matches.len() > rloc.rule_index {
                rule_matches.remove(rloc.rule_index);
                rule_match_stack.pop();
                return None;
            }
        }
    }
    rule_match_stack.push(SymbolLocator {
        symbol: symbol.to_string(),
        closing: false,
        start_pos,
        sentence_index,
        rule_index: rule_matches.len(),
    });
    let other_symbol = corresponding_symbol(sym, symbol);
    let message = sym.message_template.replace("{other}", &other_symbol);
    let symbol_len = symbol.len();
    if start_pos + symbol_len < full_text.len() {
        // Java `substring(startPos - 2, …)` counts UTF-16 units; slicing by
        // bytes can split a multibyte character (e.g. `ó»`). The context
        // regexes are ASCII-only, so char-based contexts are equivalent.
        let prefix_chars: Vec<char> = full_text[..start_pos].chars().collect();
        let prefix_len = prefix_chars.len();
        if prefix_len >= 2 {
            let mut context: String = prefix_chars[prefix_len - 2..].iter().collect();
            context.push_str(symbol);
            if context_2_re().is_match(&context) {
                return None;
            }
        } else if prefix_len >= 1 {
            let mut context: String = prefix_chars[prefix_len - 1..].iter().collect();
            context.push_str(symbol);
            if context_1_re().is_match(&context) {
                return None;
            }
        }
    }
    Some(
        Match::new(
            sym.rule_id,
            Option::<String>::None,
            message,
            Option::<String>::None,
            TextRange::new(start_pos, start_pos + symbol_len),
            if sym.spanish {
                // `SpanishUnpairedBracketsRule.getSuggestions`: an opening
                // symbol suggests `symbol + other`, a closing one
                // `other + symbol`; removing the symbol is the second option.
                let combined = if sym.start.contains(&symbol) {
                    format!("{symbol}{other_symbol}")
                } else {
                    format!("{other_symbol}{symbol}")
                };
                vec![
                    Suggestion {
                        value: combined,
                        short_description: None,
                    },
                    Suggestion {
                        value: String::new(),
                        short_description: None,
                    },
                ]
            } else {
                Vec::<Suggestion>::new()
            },
            sym.category_id,
            sym.category_name,
        )
        .with_metadata(sym.description, "typographical", 0)
        .with_match_type("Other"),
    )
}

/// `GenericUnpairedBracketsRule.match` over all sentences of the text.
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_en())
}

/// German `UNPAIRED_BRACKETS` (`GermanUnpairedBracketsRule`).
pub fn check_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_de())
}

/// Spanish `ES_UNPAIRED_BRACKETS` (`SpanishUnpairedBracketsRule`).
pub fn check_es(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_es())
}

/// French `UNPAIRED_BRACKETS` (generic rule, French strings).
pub fn check_fr(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_fr())
}

/// Italian `UNPAIRED_BRACKETS` (generic rule, Italian strings).
pub fn check_it(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_it())
}

/// `Dutch.getRelevantRules`: the generic rule with the Dutch quote lists
/// `[ ( { “ ‹ “ „ "` / `] ) } ” › ” ” "` and the Dutch strings.
pub fn symbols_nl() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: "Niet-gecombineerde haakjes, aanhalingstekens of andere symbolen",
        category_id: "PUNCTUATION",
        category_name: "Interpunctie",
        start: &[
            "[", "(", "{", "\u{201C}", "\u{2039}", "\u{201C}", "\u{201E}", "\"",
        ],
        end: &[
            "]", ")", "}", "\u{201D}", "\u{203A}", "\u{201D}", "\u{201D}", "\"",
        ],
        spanish: false,
        message_template: "Niet-gecombineerd symbool: \"{other}\" lijkt te ontbreken",
    }
}

/// Dutch `UNPAIRED_BRACKETS` (generic rule, `MessagesBundle_nl` strings).
pub fn check_nl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_nl())
}

/// `Catalan.getRelevantRules`: `CatalanUnpairedBracketsRule` with
/// `[ ( { “ « " ' ‘` / `] ) } ” » " ' ’`, the hardcoded `getMessage()`
/// override (no `{0}` placeholder) and the combined-symbol suggestion list
/// (`symbol + other` for openings, `other + symbol` for closings, then the
/// empty "remove" option). The class's extra `isNoException` overrides
/// (proper nouns, `'`/`’` after N/A tags, degrees/minutes/seconds,
/// unmatched `)`, `»` at position 1) are stage 3.
pub fn symbols_ca() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: "Claus, cometes, parèntesis i símbols semblants desaparellats",
        category_id: "PUNCTUATION",
        category_name: "Puntuació",
        start: &[
            "[", "(", "{", "\u{201C}", "\u{00AB}", "\"", "'", "\u{2018}",
        ],
        end: &[
            "]", ")", "}", "\u{201D}", "\u{00BB}", "\"", "'", "\u{2019}",
        ],
        spanish: true,
        message_template: "Símbol sense parella. Afegiu-lo i situeu-lo manualment en el lloc adequat, o bé esborreu-lo.",
    }
}

/// Catalan `UNPAIRED_BRACKETS` (`CatalanUnpairedBracketsRule`, the generic
/// algorithm with the Catalan symbol lists and strings).
pub fn check_ca(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_ca())
}

/// `Portuguese.getRelevantRules`: the generic rule with `[ ( { " “` /
/// `] ) } " ”` and the Portuguese strings (`MessagesBundle_pt_PT`, or
/// `MessagesBundle_pt_BR` for `pt-BR`; its punctuation category is named
/// `Acentuação`).
pub fn symbols_pt(variant: &str) -> UnpairedSymbols {
    let br = variant.starts_with("pt-BR");
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: if br {
            "Símbolos de chaves, parênteses, colchetes, aspas, etc. sem par"
        } else {
            "Chavetas, parênteses, pontos de exclamação e símbolos semelhantes sem par"
        },
        category_id: "PUNCTUATION",
        category_name: if br { "Acentuação" } else { "Pontuação" },
        start: &["[", "(", "{", "\"", "“"],
        end: &["]", ")", "}", "\"", "”"],
        spanish: false,
        message_template: if br {
            "Símbolo sem par: \"{other}\" aparentemente está ausente"
        } else {
            "Símbolo sem par: parece faltar '{other}'"
        },
    }
}

/// Portuguese `UNPAIRED_BRACKETS` (generic rule, Portuguese strings).
pub fn check_pt(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    check_with(sentences, &symbols_pt(variant))
}

/// `Romanian.getRelevantRules`: the generic rule with
/// `[ ( { „ « »` / `] ) } ” » «` and the `MessagesBundle_ro` strings.
pub fn symbols_ro() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: "Acolade, paranteze, ghilimele sau alte simboluri similare desperecheate",
        category_id: "PUNCTUATION",
        category_name: "Punctuation",
        start: &["[", "(", "{", "\u{201E}", "\u{00AB}", "\u{00BB}"],
        end: &["]", ")", "}", "\u{201D}", "\u{00BB}", "\u{00AB}"],
        spanish: false,
        message_template: "Unpaired symbol: '{other}' seems to be missing",
    }
}

/// Romanian `UNPAIRED_BRACKETS` (generic rule, Romanian strings).
pub fn check_ro(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_ro())
}

/// `Galician.getRelevantRules`: the generic rule with
/// `[ ( { “ « » ‘ " '` / `] ) } ” » « ’ " '` and the `MessagesBundle_gl`
/// strings.
pub fn symbols_gl() -> UnpairedSymbols {
    UnpairedSymbols {
        rule_id: "UNPAIRED_BRACKETS",
        description: "Parénteses, comiñas e símbolos similares desemparellados",
        category_id: "PUNCTUATION",
        category_name: "Puntuación",
        start: &[
            "[", "(", "{", "\u{201C}", "\u{00AB}", "\u{00BB}", "\u{2018}", "\"", "'",
        ],
        end: &[
            "]", ")", "}", "\u{201D}", "\u{00BB}", "\u{00AB}", "\u{2019}", "\"", "'",
        ],
        spanish: false,
        message_template: "Símbolo desemparellado: Parece que falta «{other}»",
    }
}

/// Galician `UNPAIRED_BRACKETS` (generic rule, Galician strings).
pub fn check_gl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &symbols_gl())
}

pub fn check_with(sentences: &[AnalyzedSentence], sym: &UnpairedSymbols) -> Vec<Match> {
    let mut symbol_stack: Vec<SymbolLocator> = Vec::new();
    let mut rule_match_stack: Vec<SymbolLocator> = Vec::new();
    let mut rule_matches: Vec<Match> = Vec::new();
    let mut start_pos_base = 0usize;
    for (sentence_index, sentence) in sentences.iter().enumerate() {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for i in 1..tokens.len() {
            for j in 0..sym.start.len() {
                if fill_symbol_stack(
                    sym,
                    start_pos_base,
                    &tokens,
                    i,
                    j,
                    &mut symbol_stack,
                    sentence_index,
                ) {
                    break;
                }
            }
        }
        // `AnalyzedSentence.getCorrectedTextLength`
        start_pos_base += sentence
            .tokens
            .iter()
            .map(|t| t.raw_byte_len)
            .sum::<usize>();
    }
    let full_text: String = sentences.iter().map(|s| s.text.as_str()).collect();

    let ss_size = symbol_stack.len();
    let mut is_symmetric = false;
    if ss_size > 2 && ss_size % 2 == 1 {
        is_symmetric = true;
        for i in 0..ss_size / 2 {
            let a = sym.start.iter().position(|s| *s == symbol_stack[i].symbol);
            let b = sym
                .end
                .iter()
                .position(|s| *s == symbol_stack[ss_size - 1].symbol);
            if a != b {
                is_symmetric = false;
                break;
            }
        }
    }
    if is_symmetric {
        let loc = &symbol_stack[ss_size / 2];
        let sentence_index = loc.sentence_index;
        if let Some(m) = create_match(
            sym,
            &mut rule_matches,
            &mut rule_match_stack,
            loc.start_pos,
            &loc.symbol.clone(),
            sentence_index,
            &full_text,
        ) {
            rule_matches.push(m);
        }
    } else {
        for s_loc in &symbol_stack {
            let start_pos = s_loc.start_pos;
            let symbol = s_loc.symbol.clone();
            let sentence_index = s_loc.sentence_index;
            let closing = s_loc.closing;
            let m = create_match(
                sym,
                &mut rule_matches,
                &mut rule_match_stack,
                start_pos,
                &symbol,
                sentence_index,
                &full_text,
            );
            if let Some(m) = m {
                if closing
                    || ends_like_real_sentence(&sentences[sentence_index].text)
                    || sentences.len() - 1 > sentence_index
                {
                    rule_matches.push(m);
                }
            }
        }
    }
    rule_matches
}
