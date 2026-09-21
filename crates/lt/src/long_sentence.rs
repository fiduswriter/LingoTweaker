//! Port of `LongSentenceRule` (`TOO_LONG_SENTENCE`, `tags="picky"`): warns
//! on sentences longer than 40 words.

use lt_core::{AnalyzedSentence, Match, Suggestion, TextRange};

const RULE_ID: &str = "TOO_LONG_SENTENCE";
const MAX_WORDS: usize = 40;
const OPENING_QUOTES: [&str; 8] = ["\"", "“", "„", "«", "(", "[", "{", "—"];
const CLOSING_QUOTES: [&str; 8] = ["\"", "”", "“", "»", ")", "]", "}", "—"];

fn quoted_sent_end_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r#"(?s)[?!.]["“”„»«]"#).unwrap());
    &RE
}

fn sent_end_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[?!.]$").unwrap());
    &RE
}

fn is_word_count(token: &str) -> bool {
    token.chars().next().is_some_and(|c| c.is_alphabetic())
}

fn is_separator(token: &str) -> bool {
    matches!(token, ":" | ";" | "\n" | "\r\n" | "\n\r")
}

fn description() -> String {
    format!("Readability: sentence over {MAX_WORDS} words")
}

fn message() -> String {
    format!(
        "This sentence is over {MAX_WORDS} words long. Consider splitting it up, as shorter sentences make the text easier to read."
    )
}

/// `LongSentenceRule` with the Dutch `MessagesBundle_nl` strings
/// (`long_sentence_rule_desc`/`_msg2`, 40 words like the Dutch rule list).
pub fn check_nl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_impl(
        sentences,
        RULE_ID,
        &format!("Leesbaarheid: zin heeft meer dan {MAX_WORDS} woorden."),
        &format!("Deze zin heeft meer dan {MAX_WORDS} woorden. Overweeg herformulering."),
        ("STYLE", "Stijl"),
        MAX_WORDS,
    )
}

/// `LongSentenceRule.match` over all sentences.
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_impl(
        sentences,
        RULE_ID,
        &description(),
        &message(),
        ("STYLE", "Style"),
        MAX_WORDS,
    )
}

/// `Galician.getRelevantRules`: `new LongSentenceRule(messages, userConfig,
/// 50)` (`MessagesBundle_gl` `long_sentence_rule_desc`/`_msg2`).
pub fn check_gl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const GL_MAX_WORDS: usize = 50;
    check_impl(
        sentences,
        RULE_ID,
        "Lexibilidade: oración de máis de 50 palabras",
        "Esta oración ten máis de 50 palabras. Considere a posibilidade de partila, xa que as oracións máis curtas facilitan a lectura.",
        ("STYLE", "Estilo"),
        GL_MAX_WORDS,
    )
}

/// `Swedish.getRelevantRules`: `new LongSentenceRule(messages, userConfig, 40)`
/// (`MessagesBundle_sv` `long_sentence_rule_desc`/`_msg2`).
pub fn check_sv(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_impl(
        sentences,
        RULE_ID,
        "Läsbarhet: meningen fler än 40 ord",
        "Meningen är över 40 ord lång. Överväg att dela upp den i flera för att göra texten lättare att läsa.",
        ("STYLE", "Stil"),
        MAX_WORDS,
    )
}

/// `Greek.getRelevantRules`: `new LongSentenceRule(messages, userConfig, 50)`
/// (`MessagesBundle_el` `long_sentence_rule_desc`/`_msg2`).
pub fn check_el(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const EL_MAX_WORDS: usize = 50;
    check_impl(
        sentences,
        RULE_ID,
        "Η πρόταση έχει περισσότερες από 50 λέξεις",
        "This sentence is over 50 words long at the marked position, consider revising",
        ("STYLE", "Υφολογικά λάθη"),
        EL_MAX_WORDS,
    )
}

/// `de.LongSentenceRule` (`TOO_LONG_SENTENCE_DE`, 40 words, German messages).
pub fn check_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_impl(
        sentences,
        "TOO_LONG_SENTENCE_DE",
        &format!("Lesbarkeit: Satz mit mehr als {MAX_WORDS} Wörtern"),
        &format!(
            "Dieser Satz hat mehr als {MAX_WORDS} Wörter. Kürzen Sie den Satz oder teilen Sie ihn, um die Lesbarkeit zu verbessern."
        ),
        ("STYLE", "Stil"),
        MAX_WORDS,
    )
}

/// `French.getRelevantRules`: `new LongSentenceRule(messages, userConfig, 40)`
/// (id `TOO_LONG_SENTENCE` like English, French messages).
pub fn check_fr(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_impl(
        sentences,
        RULE_ID,
        "Lisibilité : la phrase contient plus de 40 mots",
        "Cette phrase contient plus de 40 mots. Divisez-la afin d’obtenir une phrase plus concise, et ainsi améliorer la lisibilité de votre texte.",
        ("STYLE", "Style"),
        MAX_WORDS,
    )
}

/// `Catalan.getRelevantRules`: `new LongSentenceRule(messages, userConfig,
/// 60)` (`MessagesBundle_ca` `long_sentence_rule_desc`/`_msg2`).
pub fn check_ca(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const CA_MAX_WORDS: usize = 60;
    check_impl(
        sentences,
        RULE_ID,
        "Llegibilitat: frase amb més de 60 paraules",
        "Aquesta frase té més de 60 paraules. Considereu revisar-la. Les frases més curtes fan el text més llegible.",
        ("STYLE", "Estil"),
        CA_MAX_WORDS,
    )
}

/// `Spanish.getRelevantRules`: `new LongSentenceRule(messages, userConfig, 60)`
/// (id `TOO_LONG_SENTENCE` like English, Spanish messages, 60 words).
pub fn check_es(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const ES_MAX_WORDS: usize = 60;
    check_impl(
        sentences,
        RULE_ID,
        "Legibilidad: oración con más de 60 palabras",
        "Esta frase tiene más de 60 palabras. Considere la posibilidad de revisarla.",
        ("STYLE", "Estilo"),
        ES_MAX_WORDS,
    )
}

/// `Portuguese.getRelevantRules`: `new LongSentenceRule(messages,
/// userConfig, 50)` (id `TOO_LONG_SENTENCE`, 50 words). pt-BR has its own
/// bundle; plain `pt`/`pt-AO`/`pt-MZ` fall back to the pt-PT strings through
/// Java's `getDefaultLanguageVariant()` resolution.
pub fn check_pt(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    const PT_MAX_WORDS: usize = 50;
    let (description, message) = match variant {
        "pt-PT" => (
            "Legibilidade: frase com mais de 50 palavras",
            "Na posição marcada, esta frase excede as 50 palavras. Considere rever.",
        ),
        "pt-BR" => (
            "Legibilidade: frase com mais de 50 palavras",
            "Essa frase possui mais de 50 palavras na posição indicada, considere uma revisão",
        ),
        _ => (
            "Legibilidade: frase com mais de 50 palavras",
            "Na posição marcada, esta frase excede as 50 palavras. Considere rever.",
        ),
    };
    check_impl(
        sentences,
        RULE_ID,
        description,
        message,
        ("STYLE", "Estilo"),
        PT_MAX_WORDS,
    )
}

fn check_impl(
    sentences: &[AnalyzedSentence],
    rule_id: &str,
    description: &str,
    message: &str,
    category: (&str, &str),
    max_words: usize,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    for sentence in sentences {
        let tokens = &sentence.tokens;
        if tokens.len() < max_words {
            continue;
        }
        if quoted_sent_end_re().is_match(&sentence.text) {
            continue;
        }
        let mut i = 0usize;
        let mut positions: Vec<(usize, usize)> = Vec::new();
        let mut from_pos_token: Option<usize> = None;
        let mut to_pos_token: Option<usize> = None;
        let mut index_of_quote: i32 = -1;
        while i < tokens.len() {
            let mut num_words = 0usize;
            while i < tokens.len() && !is_separator(tokens[i].surface()) {
                let token = tokens[i].surface();
                if index_of_quote == -1 {
                    if let Some(q) = OPENING_QUOTES.iter().position(|s| *s == token) {
                        index_of_quote = q as i32;
                    }
                } else if CLOSING_QUOTES
                    .iter()
                    .position(|s| *s == token)
                    .is_some_and(|q| q as i32 == index_of_quote)
                {
                    index_of_quote = -1;
                }
                if is_word_count(token) && index_of_quote == -1 {
                    if from_pos_token.is_none() {
                        from_pos_token = Some(i);
                    }
                    if num_words == max_words {
                        if to_pos_token.is_none() {
                            for j in (0..tokens.len()).rev() {
                                if is_word_count(tokens[j].surface()) {
                                    to_pos_token = if j + 1 < tokens.len()
                                        && sent_end_re().is_match(tokens[j + 1].surface())
                                    {
                                        Some(j + 1)
                                    } else {
                                        Some(j)
                                    };
                                    break;
                                }
                            }
                        }
                        if let (Some(f), Some(t)) = (from_pos_token, to_pos_token) {
                            positions.push((tokens[f].start_pos, tokens[t].end_pos()));
                        } else {
                            positions
                                .push((tokens[0].start_pos, tokens[tokens.len() - 1].end_pos()));
                        }
                        break;
                    }
                    num_words += 1;
                }
                i += 1;
            }
            i += 1;
        }
        for (from, to) in positions {
            rule_matches.push(
                Match::new(
                    rule_id,
                    Option::<String>::None,
                    message,
                    Option::<String>::None,
                    TextRange::new(sentence.offset + from, sentence.offset + to),
                    Vec::<Suggestion>::new(),
                    category.0,
                    category.1,
                )
                .with_metadata(description, "style", 0)
                .with_picky(true),
            );
        }
    }
    rule_matches
}
