//! Ports of the English paragraph rules (all default off except
//! `PUNCTUATION_PARAGRAPH_END`, which is `tags="picky"`):
//! `LongParagraphRule`, `EmptyLineRule`, `WhiteSpaceAtBeginOfParagraph`,
//! `WhiteSpaceBeforeParagraphEnd`, `PunctuationMarkAtParagraphEnd` 1/2 and
//! `ParagraphRepeatBeginningRule`.
//!
//! Paragraph ends follow `Tools.isParagraphEnd` with
//! `singleLineBreaksMarksPara() == false` (`en_two` SRX rules): a sentence is
//! a paragraph end when it the last one, ends with two linebreaks, or the
//! next sentence starts with a linebreak.

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const LONG_PARAGRAPH_ID: &str = "TOO_LONG_PARAGRAPH";
pub const EMPTY_LINE_ID: &str = "EMPTY_LINE";
pub const WHITESPACE_PARAGRAPH_BEGIN_ID: &str = "WHITESPACE_PARAGRAPH_BEGIN";
pub const WHITESPACE_PARAGRAPH_ID: &str = "WHITESPACE_PARAGRAPH";
pub const PUNCTUATION_PARAGRAPH_END_ID: &str = "PUNCTUATION_PARAGRAPH_END";
pub const PUNCTUATION_PARAGRAPH_END2_ID: &str = "PUNCTUATION_PARAGRAPH_END2";
pub const PARAGRAPH_REPEAT_BEGINNING_ID: &str = "PARAGRAPH_REPEAT_BEGINNING_RULE";

const STYLE_CATEGORY: (&str, &str) = ("STYLE", "Style");
const PUNCTUATION_CATEGORY: (&str, &str) = ("PUNCTUATION", "Punctuation");

/// `LongParagraphRule.DEFAULT_MAX_WORDS`
pub const LONG_PARAGRAPH_MAX_WORDS: i32 = 220;

/// `Tools.isParagraphEnd(sentences, nTest, lang)` for English
/// (`singleLineBreaksMarksPara == false`).
pub fn is_paragraph_end(sentences: &[AnalyzedSentence], n_test: usize) -> bool {
    if n_test >= sentences.len().saturating_sub(1) {
        return true;
    }
    let text = &sentences[n_test].text;
    if text.ends_with("\n\n") || text.ends_with("\n\r\n\r") || text.ends_with("\r\n\r\n") {
        return true;
    }
    let next = &sentences[n_test + 1].text;
    next.starts_with('\n') || next.starts_with("\r\n")
}

/// `AnalyzedTokenReadings.NON_WORD_REGEX` (full match on one character).
fn is_non_word(token: &AnalyzedTokenReadings) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"^[.?!…:;,~’'"„“”»«‚‘›‹()\[\]\-–—*×∗·+÷/=]$"#).unwrap()
    });
    RE.is_match(token.surface())
}

fn is_linebreak(token: &AnalyzedTokenReadings) -> bool {
    token.is_linebreak()
}

/// Whitespace a `WhiteSpaceAtBeginOfParagraph`/`WhiteSpaceBeforeParagraphEnd`
/// match can be built on (not the zero-width space).
fn is_deletable_whitespace(token: &AnalyzedTokenReadings) -> bool {
    token.is_whitespace && token.surface() != "\u{200B}"
}

/// `sentence.getCorrectedTextLength()` for the ported data (no soft-hyphen
/// token fixups; equals the sentence text length in UTF-8 bytes).
fn corrected_text_length(sentence: &AnalyzedSentence) -> usize {
    sentence.text.len()
}

/// Per-language strings of the paragraph rules (the core classes are shared
/// with English; the German `MessagesBundle_de` values differ).
pub struct ParagraphStrings {
    pub style_category: (&'static str, &'static str),
    pub punctuation_category: (&'static str, &'static str),
    pub long_desc: fn(i32) -> String,
    pub long_msg: fn(i32) -> String,
    pub empty_line_msg: &'static str,
    pub empty_line_desc: &'static str,
    pub ws_begin_msg: &'static str,
    pub ws_begin_desc: &'static str,
    pub ws_end_msg: &'static str,
    pub ws_end_desc: &'static str,
    pub punct_msg: &'static str,
    pub punct_desc: &'static str,
    /// `repetition_paragraph_beginning_last_msg`
    pub repetition_last_msg: &'static str,
    /// `repetition_paragraph_beginning_desc`
    pub repetition_desc: &'static str,
}

pub fn strings_en() -> ParagraphStrings {
    ParagraphStrings {
        style_category: STYLE_CATEGORY,
        punctuation_category: PUNCTUATION_CATEGORY,
        long_desc: |max| format!("Readability: paragraph over {max} words"),
        long_msg: |max| {
            format!("This paragraph is over {max} words long here, consider revising it.")
        },
        empty_line_msg: "Please delete empty line below and use formatting instead",
        empty_line_desc: "Empty Line",
        ws_begin_msg: "Please delete the space character at the beginning of the paragraph.",
        ws_begin_desc: "Space character at the beginning of paragraph",
        ws_end_msg: "Please delete the space character at the end of the paragraph.",
        ws_end_desc: "Space character at the end of paragraph",
        punct_msg: "Please add a punctuation mark at the end of paragraph.",
        punct_desc: "No punctuation mark at the end of paragraph",
        repetition_last_msg: "Same beginning as last paragraph",
        repetition_desc: "Same beginning of paragraph",
    }
}

pub fn strings_de() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Stil"),
        punctuation_category: ("PUNCTUATION", "Zeichensetzung"),
        long_desc: |max| format!("Lesbarkeit: Absatz mit mehr als {max} Wörtern"),
        long_msg: |max| {
            format!("Der Absatz hat an dieser Stelle mehr als {max} Wörter und sollte evtl. umformuliert werden.")
        },
        empty_line_msg: "Bitte löschen Sie die nachfolgende leere Zeile und nutzen Sie stattdessen die Formatierung",
        empty_line_desc: "Leere Zeile",
        ws_begin_msg: "Bitte löschen Sie das Leerzeichen am Anfang des Absatzes.",
        ws_begin_desc: "Leerzeichen am Anfang des Absatzes",
        ws_end_msg: "Bitte löschen Sie das Leerzeichen am Ende des Absatzes.",
        ws_end_desc: "Leerzeichen am Absatzende",
        punct_msg: "Bitte ein Satzzeichen am Ende des Absatzes einfügen.",
        punct_desc: "Kein Satzzeichen am Ende des Absatzes",
        repetition_last_msg: "Der Absatz beginnt so wie der vorhergehende Absatz",
        repetition_desc: "Gleicher Anfang von aufeinanderfolgenden Absätzen",
    }
}

/// `MessagesBundle_fr` strings for the paragraph rules.
pub fn strings_fr() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Style"),
        punctuation_category: ("PUNCTUATION", "Ponctuation"),
        long_desc: |max| format!("Lisibilité : le paragraphe contient plus de {max} mots"),
        long_msg: |max| {
            format!(
                "Ce paragraphe contient plus de {max} mots à la position marquée, pensez à le raccourcir."
            )
        },
        empty_line_msg: "Veuillez supprimer la ligne vide suivante et appliquer un formatage",
        empty_line_desc: "Ligne vide",
        ws_begin_msg: "Veuillez supprimer l'espace au début du paragraphe.",
        ws_begin_desc: "Espace au début du paragraphe",
        ws_end_msg: "Veuillez supprimer l'espace à la fin du paragraphe.",
        ws_end_desc: "Caractère espace à la fin du paragraphe",
        punct_msg: "Veuillez insérer un signe de ponctuation à la fin du paragraphe.",
        punct_desc: "Fin de paragraphe sans ponctuation",
        repetition_last_msg: "Même début que le paragraphe précédent",
        repetition_desc: "Même début de paragraphe",
    }
}

/// `MessagesBundle_es` strings for the paragraph rules.
pub fn strings_es() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Estilo"),
        punctuation_category: ("PUNCTUATION", "Puntuación"),
        long_desc: |max| format!("Legibilidad: el párrafo tiene más de {max} palabras"),
        long_msg: |max| {
            format!(
                "El párrafo tiene más de {max} palabras. Considere la posibilidad de revisarlo."
            )
        },
        empty_line_msg: "Elimine la línea vacía siguiente y aplique un formato en su lugar",
        empty_line_desc: "Línea vacía",
        ws_begin_msg: "Elimine el carácter de espacio al principio del párrafo.",
        ws_begin_desc: "Carácter de espacio al principio del párrafo",
        ws_end_msg: "Elimine el carácter de espacio al final del párrafo.",
        ws_end_desc: "Carácter de espacio al final del párrafo",
        punct_msg: "Añada un signo de puntuación al final del párrafo.",
        punct_desc: "Falta un signo de puntuación al final del párrafo",
        repetition_last_msg: "Mismo comienzo que el párrafo anterior",
        repetition_desc: "Mismo comienzo de párrafo",
    }
}

/// Paragraph-rule strings of the Dutch `MessagesBundle_nl` (the paragraph
/// rules other than `LongParagraphRule` are not in Dutch's rule list, so the
/// remaining fields fall back to the core bundle like Java).
pub fn strings_nl() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Stijl"),
        punctuation_category: ("PUNCTUATION", "Interpunctie"),
        long_desc: |max| format!("Leesbaarheid: alinea heeft meer dan {max} woorden"),
        long_msg: |max| format!("Alinea is meer dan {max} woorden lang, overweeg inkorten."),
        empty_line_msg: "Please delete empty line below and use formatting instead",
        empty_line_desc: "Empty Line",
        ws_begin_msg: "Verwijder s.v.p. het spatiekarakter aan het begin van de paragraaf.",
        ws_begin_desc: "Spatie of vergelijkbaar teken aan het begin van de paragraaf",
        ws_end_msg: "Verwijder s.v.p. de spatie aan het einde van de paragraaf.",
        ws_end_desc: "Spatie aan einde van paragraaf",
        punct_msg: "Please add a punctuation mark at the end of paragraph.",
        punct_desc: "No punctuation mark at the end of paragraph",
        repetition_last_msg: "Same beginning as last paragraph",
        repetition_desc: "Same beginning of paragraph",
    }
}

/// Paragraph-rule strings per Portuguese variant: `MessagesBundle_pt_BR`
/// for pt-BR, the `MessagesBundle_pt_PT` values otherwise (plain `pt`,
/// `pt-AO` and `pt-MZ` resolve through `getDefaultLanguageVariant()`).
/// `MessagesBundle_ca` strings of the paragraph rules (`ws_begin`/`ws_end`
/// are not translated in the Catalan bundle, so they fall back to the core
/// English strings like Java's ResourceBundle chain).
pub fn strings_ca() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Estil"),
        punctuation_category: ("PUNCTUATION", "Puntuació"),
        long_desc: |max| format!("Llegibilitat: el paràgraf té més de {max} paraules"),
        long_msg: |max| format!("El paràgraf té més de {max} paraules. Considereu revisar-lo."),
        empty_line_msg: "Esborreu la línia buida i feu servir el format en comptes d'això",
        empty_line_desc: "Línia buida",
        ws_begin_msg: "Please delete the space character at the beginning of the paragraph.",
        ws_begin_desc: "Space character at the beginning of paragraph",
        ws_end_msg: "Please delete the space character at the end of the paragraph.",
        ws_end_desc: "Space character at the end of paragraph",
        punct_msg: "Afegiu un signe de puntuació al final del paràgraf.",
        punct_desc: "Falta un signe de puntuació al final del paràgraf",
        repetition_last_msg: "Principi repetit com en l'últim paràgraf",
        repetition_desc: "Principi de paràgraf repetit",
    }
}

/// Galician `MessagesBundle_gl` paragraph-rule strings (`Galician.java`
/// wires the base `LongParagraphRule`, `EmptyLineRule`,
/// `WhiteSpaceBeforeParagraphEnd`, `WhiteSpaceAtBeginOfParagraph`,
/// `ParagraphRepeatBeginningRule` and `PunctuationMarkAtParagraphEnd`).
pub fn strings_gl() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Estilo"),
        punctuation_category: ("PUNCTUATION", "Puntuación"),
        long_desc: |max| format!("Lexibilidade: parágrafo de máis de {max} palabras"),
        long_msg: |max| {
            format!("O parágrafo sobrepasa as {max} palabras de longo. Considere unha revisión.")
        },
        empty_line_msg: "Elimine a liña baleira seguinte e utilice formatado no seu lugar",
        empty_line_desc: "Liña baleira",
        ws_begin_msg: "Elimine o carácter de espazo ao comezo do parágrafo.",
        ws_begin_desc: "Carácter de espazo ao comezo do parágrafo",
        ws_end_msg: "Elimine o carácter de espazo ao final do parágrafo.",
        ws_end_desc: "Carácter de espazo ao final do parágrafo",
        punct_msg: "Engada un signo de puntuación ao final do parágrafo.",
        punct_desc: "Sen signo de puntuación ao final do parágrafo",
        repetition_last_msg: "O mesmo comezo como derradeiro parágrafo",
        repetition_desc: "O mesmo comezo do parágrafo",
    }
}

/// `MessagesBundle_sv` strings for `Swedish.getRelevantRules`'s
/// `LongParagraphRule(messages, this, userConfig, 150)`.
pub fn strings_sv() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Stil"),
        punctuation_category: ("PUNCTUATION", "Skiljetecken"),
        long_desc: |max| format!("Läsbarhet: stycke längre än {max} ord"),
        long_msg: |max| {
            format!("Stycket är längre än {max} ord. Överväg att skriva om det genom att dela upp i flera stycken om det passar strukturellt och innehållsmässigt.")
        },
        empty_line_msg: "Please delete empty line below and use formatting instead",
        empty_line_desc: "Empty Line",
        ws_begin_msg: "Please delete the space character at the beginning of the paragraph.",
        ws_begin_desc: "Space character at the beginning of paragraph",
        ws_end_msg: "Please delete the space character at the end of the paragraph.",
        ws_end_desc: "Space character at the end of paragraph",
        punct_msg: "Please add a punctuation mark at the end of paragraph.",
        punct_desc: "No punctuation mark at the end of paragraph",
        repetition_last_msg: "Same beginning as last paragraph",
        repetition_desc: "Same beginning of paragraph",
    }
}

pub fn strings_pt(variant: &str) -> ParagraphStrings {
    if variant == "pt-BR" {
        return ParagraphStrings {
            style_category: ("STYLE", "Estilo"),
            punctuation_category: ("PUNCTUATION", "Acentuação"),
            long_desc: |max| format!("Legibilidade: parágrafo ultrapassa em {max} palavras"),
            long_msg: |max| {
                format!("O parágrafo ultrapassa em {max} palavras, considere uma revisão")
            },
            empty_line_msg: "Por favor delete a linha vazia abaixo e use formatação em seu lugar.",
            empty_line_desc: "Linha vazia",
            ws_begin_msg: "Por favor delete o caractere de espaço no início do parágrafo.",
            ws_begin_desc: "Caractere de espaço no começo do parágrafo",
            ws_end_msg: "Por favor delete o caractere de espaço no fim do parágrafo.",
            ws_end_desc: "Caractere de espaço no fim do parágrafo",
            punct_msg: "Considere pontuar o final do parágrafo.",
            punct_desc: "Final de parágrafo sem sinal de pontuação",
            repetition_last_msg: "Mesmo começo que o último parágrafo",
            repetition_desc: "Mesmo começo do parágrafo",
        };
    }
    ParagraphStrings {
        style_category: ("STYLE", "Estilo"),
        punctuation_category: ("PUNCTUATION", "Pontuação"),
        long_desc: |max| format!("Legibilidade: parágrafo excede as {max} palavras"),
        long_msg: |max| format!("O parágrafo tem mais de {max} palavras aqui. Considere rever."),
        empty_line_msg:
            "Apague a linha em branco abaixo e utilize formatação para dar o espaçamento",
        empty_line_desc: "Linhas Vazias",
        ws_begin_msg: "Apague o espaço de caractere no início do parágrafo.",
        ws_begin_desc: "Caractere de espaço no início do parágrafo",
        ws_end_msg: "Apague o caractere de espaço no fim do parágrafo.",
        ws_end_desc: "Caracteres de espaço no fim do parágrafo.",
        punct_msg: "Adicione pontuação final ao fim do parágrafo.",
        punct_desc: "Falta de pontuação final no final do parágrafo",
        repetition_last_msg: "O mesmo começo que o parágrafo anterior",
        repetition_desc: "Início de parágrafo repetido",
    }
}

/// `LongParagraphRule.match(List<AnalyzedSentence>)`.
pub fn long_paragraph(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    long_paragraph_with(sentences, &strings_en())
}

pub fn long_paragraph_with(
    sentences: &[AnalyzedSentence],
    strings: &ParagraphStrings,
) -> Vec<Match> {
    long_paragraph_with_max(sentences, strings, LONG_PARAGRAPH_MAX_WORDS)
}

pub fn long_paragraph_with_max(
    sentences: &[AnalyzedSentence],
    strings: &ParagraphStrings,
    max_words: i32,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut pos = 0usize;
    let mut start_pos = 0usize;
    let mut end_pos = 0usize;
    let mut word_count = 0i32;
    let mut para_has_linebreaks = false;
    for (n, sentence) in sentences.iter().enumerate() {
        let paragraph_end = is_paragraph_end(sentences, n);
        if !paragraph_end && sentence.text.trim_start_matches('\n').contains('\n') {
            // e.g. text with manually added line breaks
            para_has_linebreaks = true;
        }
        for token in sentence.tokens_without_whitespace() {
            if !token.is_whitespace && !token.is_sentence_start && !is_non_word(token) {
                word_count += 1;
                if word_count == max_words {
                    end_pos = token.end_pos() + pos;
                } else if word_count == max_words - 1 {
                    start_pos = token.start_pos + pos;
                }
            }
        }
        if paragraph_end {
            if word_count > max_words + 5 && !para_has_linebreaks {
                rule_matches.push(
                    Match::new(
                        LONG_PARAGRAPH_ID,
                        Option::<String>::None,
                        (strings.long_msg)(max_words),
                        Option::<String>::None,
                        TextRange::new(start_pos, end_pos),
                        Vec::new(),
                        strings.style_category.0,
                        strings.style_category.1,
                    )
                    .with_metadata((strings.long_desc)(max_words), "style", -1)
                    .with_picky(true),
                );
            }
            word_count = 0;
            para_has_linebreaks = false;
        }
        pos += corrected_text_length(sentence);
    }
    if word_count > max_words {
        rule_matches.push(
            Match::new(
                LONG_PARAGRAPH_ID,
                Option::<String>::None,
                (strings.long_msg)(max_words),
                Option::<String>::None,
                TextRange::new(start_pos, end_pos),
                Vec::new(),
                strings.style_category.0,
                strings.style_category.1,
            )
            .with_metadata((strings.long_desc)(max_words), "style", -1)
            .with_picky(true),
        );
    }
    rule_matches
}

/// `EmptyLineRule.match(List<AnalyzedSentence>)`.
pub fn empty_line(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    empty_line_with(sentences, &strings_en())
}

pub fn empty_line_with(sentences: &[AnalyzedSentence], strings: &ParagraphStrings) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut pos = 0usize;
    for n in 0..sentences.len().saturating_sub(1) {
        let sentence = &sentences[n];
        if is_paragraph_end(sentences, n) && is_second_paragraph_end_mark(&sentence.text) {
            let tokens = sentence.tokens_without_whitespace();
            if tokens.len() > 1 {
                let last = tokens[tokens.len() - 1];
                rule_matches.push(
                    Match::new(
                        EMPTY_LINE_ID,
                        Option::<String>::None,
                        strings.empty_line_msg,
                        Option::<String>::None,
                        TextRange::new(pos + last.start_pos, pos + last.end_pos()),
                        Vec::new(),
                        STYLE_CATEGORY.0,
                        STYLE_CATEGORY.1,
                    )
                    .with_metadata(strings.empty_line_desc, "style", -1),
                );
            }
        }
        pos += corrected_text_length(sentence);
    }
    rule_matches
}

/// `EmptyLineRule.isSecondParagraphEndMark` for `en_two`.
fn is_second_paragraph_end_mark(text: &str) -> bool {
    text.ends_with("\n\n\n\n")
        || text.ends_with("\n\r\n\r\n\r\n\r")
        || text.ends_with("\r\n\r\n\r\n\r\n")
}

/// `WhiteSpaceAtBeginOfParagraph.match(AnalyzedSentence)` (sentence-level).
pub fn whitespace_at_begin_of_paragraph(sentence: &AnalyzedSentence) -> Vec<Match> {
    whitespace_at_begin_of_paragraph_with(sentence, &strings_en())
}

pub fn whitespace_at_begin_of_paragraph_with(
    sentence: &AnalyzedSentence,
    strings: &ParagraphStrings,
) -> Vec<Match> {
    let tokens = &sentence.tokens;
    let mut i = 1usize;
    while i < tokens.len() && is_deletable_whitespace(&tokens[i]) && !is_linebreak(&tokens[i]) {
        i += 1;
    }
    if i > 1 && i < tokens.len() && !is_linebreak(&tokens[i]) {
        return vec![Match::new(
            WHITESPACE_PARAGRAPH_BEGIN_ID,
            Option::<String>::None,
            strings.ws_begin_msg,
            Option::<String>::None,
            TextRange::new(
                sentence.offset + tokens[1].start_pos,
                sentence.offset + tokens[i].end_pos(),
            ),
            vec![Suggestion {
                value: tokens[i].surface().to_string(),
                short_description: None,
            }],
            strings.style_category.0,
            strings.style_category.1,
        )
        .with_metadata(strings.ws_begin_desc, "style", 0)];
    }
    Vec::new()
}

/// `WhiteSpaceBeforeParagraphEnd.match(List<AnalyzedSentence>)`.
pub fn whitespace_before_paragraph_end(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    whitespace_before_paragraph_end_with(sentences, &strings_en())
}

pub fn whitespace_before_paragraph_end_with(
    sentences: &[AnalyzedSentence],
    strings: &ParagraphStrings,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut pos = 0usize;
    for (n, sentence) in sentences.iter().enumerate() {
        if is_paragraph_end(sentences, n) {
            let tokens = &sentence.tokens;
            let mut lb = tokens.len().saturating_sub(1);
            while lb > 0 && is_linebreak(&tokens[lb]) {
                lb -= 1;
            }
            let mut lw = lb;
            while lw > 0 && tokens[lw].is_whitespace && tokens[lw].surface() != "\u{200B}" {
                lw -= 1;
            }
            if lw < lb {
                let from_pos = if tokens[lw].is_whitespace {
                    pos + tokens[lw + 1].start_pos
                } else {
                    pos + tokens[lw].start_pos
                };
                let to_pos = pos + tokens[lb].end_pos();
                let suggestion = if lw > 0 && !tokens[lw].is_whitespace {
                    tokens[lw].surface().to_string()
                } else {
                    String::new()
                };
                rule_matches.push(
                    Match::new(
                        WHITESPACE_PARAGRAPH_ID,
                        Option::<String>::None,
                        strings.ws_end_msg,
                        Option::<String>::None,
                        TextRange::new(from_pos, to_pos),
                        vec![Suggestion {
                            value: suggestion,
                            short_description: None,
                        }],
                        strings.style_category.0,
                        strings.style_category.1,
                    )
                    .with_metadata(strings.ws_end_desc, "style", -1),
                );
            }
        }
        pos += corrected_text_length(sentence);
    }
    rule_matches
}

const PUNCTUATION_MARKS: [&str; 6] = [".", "!", "?", ":", ",", ";"];
const QUOTATION_MARKS: [&str; 13] = [
    "„", "»", "«", "\"", "”", "″", "’", "‚", "‘", "›", "‹", "′", "'",
];
/// `PunctuationMarkAtParagraphEnd.MAX_URL_LENGTH`
const MAX_URL_LENGTH: usize = 30;

fn string_equals_any(token: &str, any: &[&str]) -> bool {
    any.contains(&token)
}

fn is_punctuation_mark(token: &AnalyzedTokenReadings) -> bool {
    string_equals_any(token.surface(), &PUNCTUATION_MARKS)
}

fn is_quotation_mark(token: &AnalyzedTokenReadings) -> bool {
    string_equals_any(token.surface(), &QUOTATION_MARKS)
}

fn is_word(token: &AnalyzedTokenReadings) -> bool {
    token
        .surface()
        .chars()
        .next()
        .is_some_and(char::is_alphabetic)
}

fn is_numeric(s: &str) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[0-9.]+$").unwrap());
    RE.is_match(s.trim())
}

/// `WordTokenizer.isUrl`.
fn is_url(token: &str) -> bool {
    static NO_PROTOCOL_URL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"^([a-zA-Z0-9][a-zA-Z0-9-]+\.)?([a-zA-Z0-9][a-zA-Z0-9-]+)\.([a-zA-Z0-9][a-zA-Z0-9-]+)/.*$",
        )
        .unwrap()
    });
    const PROTOCOLS: [&str; 19] = [
        "http", "https", "ws", "wss", "ftp", "ftps", "sftp", "file", "mailto", "tel", "sms", "git",
        "ssh", "data", "magnet", "smb", "slack", "spotify", "magnet",
    ];
    PROTOCOLS
        .iter()
        .any(|p| token.starts_with(&format!("{p}://")))
        || token.starts_with("www.")
        || NO_PROTOCOL_URL.is_match(token)
}

/// `PunctuationMarkAtParagraphEnd.match(List<AnalyzedSentence>)`.
pub fn punctuation_at_paragraph_end(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    punctuation_at_paragraph_end_with(sentences, &strings_en())
}

pub fn punctuation_at_paragraph_end_with(
    sentences: &[AnalyzedSentence],
    strings: &ParagraphStrings,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_para: i64 = -1;
    let mut pos = 0usize;
    for (n, sentence) in sentences.iter().enumerate() {
        if is_paragraph_end(sentences, n) {
            let tokens = sentence.tokens_without_whitespace();
            if tokens.len() > 2 {
                let is_first_word = (is_word(tokens[1]) && !is_punctuation_mark(tokens[2]))
                    || (tokens.len() > 3
                        && is_quotation_mark(tokens[1])
                        && is_word(tokens[2])
                        && !is_punctuation_mark(tokens[3]));
                let mut ignore_sentence = false;
                if n == 1 && is_numeric(&sentences[0].text) {
                    ignore_sentence = true;
                }
                if n > 0 && is_numeric(&sentences[n - 1].text) {
                    ignore_sentence = true;
                }
                if (n as i64) - last_para > 1 && is_first_word && !ignore_sentence {
                    let mut last_nw_token = tokens.len() - 1;
                    while is_linebreak(tokens[last_nw_token]) {
                        last_nw_token -= 1;
                    }
                    if tokens[tokens.len() - 2].surface().eq_ignore_ascii_case(":")
                        && is_url(tokens[tokens.len() - 1].surface())
                    {
                        // e.g. "find it at: http://example.com"
                        last_para = n as i64;
                        pos += sentence.text.len();
                        continue;
                    }
                    let last_token = tokens[last_nw_token].surface();
                    if last_token.len() > MAX_URL_LENGTH
                        && (last_token.to_lowercase().starts_with("http")
                            || last_token.to_lowercase().starts_with("ftp"))
                    {
                        continue;
                    }
                    if is_word(tokens[last_nw_token])
                        || (is_quotation_mark(tokens[last_nw_token])
                            && is_word(tokens[last_nw_token - 1]))
                    {
                        let from_pos = pos + tokens[last_nw_token].start_pos;
                        let to_pos = pos + tokens[last_nw_token].end_pos();
                        let replacements: Vec<Suggestion> = PUNCTUATION_MARKS
                            .iter()
                            .map(|mark| Suggestion {
                                value: format!("{}{mark}", tokens[last_nw_token].surface()),
                                short_description: None,
                            })
                            .collect();
                        rule_matches.push(
                            Match::new(
                                PUNCTUATION_PARAGRAPH_END_ID,
                                Option::<String>::None,
                                strings.punct_msg,
                                Option::<String>::None,
                                TextRange::new(from_pos, to_pos),
                                replacements,
                                strings.punctuation_category.0,
                                strings.punctuation_category.1,
                            )
                            .with_metadata(strings.punct_desc, "grammar", -1)
                            .with_picky(true),
                        );
                    }
                }
            }
            last_para = n as i64;
        }
        pos += corrected_text_length(sentence);
    }
    rule_matches
}

/// `PunctuationMarkAtParagraphEnd2.TOKEN_THRESHOLD`
const TOKEN_THRESHOLD: usize = 10;

/// `PunctuationMarkAtParagraphEnd2.match(List<AnalyzedSentence>)`.
pub fn punctuation_at_paragraph_end2(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    punctuation_at_paragraph_end2_with(sentences, &strings_en())
}

/// `PunctuationMarkAtParagraphEnd2.match(List<AnalyzedSentence>)` with
/// per-language strings.
pub fn punctuation_at_paragraph_end2_with(
    sentences: &[AnalyzedSentence],
    strings: &ParagraphStrings,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut pos = 0usize;
    let mut token_count = 0usize;
    for (sent_pos, sentence) in sentences.iter().enumerate() {
        let tokens = &sentence.tokens;
        for token in tokens {
            // Java's `AnalyzedTokenReadings.isWhitespace` is true for the empty
            // SENT_START token, so it is not counted
            if !is_non_word(token) && !token.is_whitespace && !token.is_sentence_start {
                token_count += 1;
            }
        }
        let last_non_space = tokens.iter().rev().find(|t| !t.is_whitespace);
        let para_end = is_paragraph_end(sentences, sent_pos);
        if let Some(last) = last_non_space {
            if para_end
                && token_count > TOKEN_THRESHOLD
                && !single_punct_or_ellipsis(last.surface())
                && !is_non_word(last)
            {
                rule_matches.push(
                    Match::new(
                        PUNCTUATION_PARAGRAPH_END2_ID,
                        Option::<String>::None,
                        strings.punct_msg,
                        Option::<String>::None,
                        TextRange::new(pos + last.start_pos, pos + last.end_pos()),
                        vec![Suggestion {
                            value: format!("{}.", last.surface()),
                            short_description: None,
                        }],
                        strings.punctuation_category.0,
                        strings.punctuation_category.1,
                    )
                    .with_metadata(strings.punct_desc, "grammar", -1),
                );
            }
        }
        if para_end {
            token_count = 0;
        }
        pos += corrected_text_length(sentence);
    }
    rule_matches
}

/// `token.matches("[:.?!…]")`: exactly one character of that set.
fn single_punct_or_ellipsis(token: &str) -> bool {
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => matches!(c, ':' | '.' | '?' | '!' | '…'),
        _ => false,
    }
}

fn quotes_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r#"^[’'"„“”»«‚‘›‹()\[\]]$"#).unwrap());
    &RE
}

fn is_article(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag_starting_with("DT")
}

/// `GermanParagraphRepeatBeginningRule.isArticle`.
fn is_article_de(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag_starting_with("ART")
}

/// `ParagraphRepeatBeginningRule.numCharEqualBeginning`.
fn num_char_equal_beginning(
    last_tokens: &[&AnalyzedTokenReadings],
    next_tokens: &[&AnalyzedTokenReadings],
    is_article: &dyn Fn(&AnalyzedTokenReadings) -> bool,
) -> usize {
    if last_tokens.len() < 2
        || next_tokens.len() < 2
        || last_tokens[1].is_whitespace
        || next_tokens[1].is_whitespace
    {
        return 0;
    }
    let mut n_token = 1usize;
    let mut last_token = last_tokens[n_token].surface();
    let mut next_token = next_tokens[n_token].surface();
    if quotes_re().is_match(last_token) && last_token == next_token {
        if last_tokens.len() <= n_token + 1 || next_tokens.len() <= n_token + 1 {
            return 0;
        }
        n_token += 1;
        last_token = last_tokens[n_token].surface();
        next_token = next_tokens[n_token].surface();
    }
    if !last_token.chars().next().is_some_and(char::is_alphabetic) {
        return 0;
    }
    if last_tokens.len() > n_token + 1
        && is_article(last_tokens[n_token])
        && last_token == next_token
    {
        if next_tokens.len() <= n_token + 1 {
            return 0;
        }
        n_token += 1;
        last_token = last_tokens[n_token].surface();
        next_token = next_tokens[n_token].surface();
    }
    if !last_token.chars().next().is_some_and(char::is_alphabetic) {
        return 0;
    }
    if last_token == next_token {
        return last_tokens[n_token].end_pos();
    }
    0
}

/// `ParagraphRepeatBeginningRule.match(List<AnalyzedSentence>)`; the
/// language subclasses only change `isArticle`, id, message and category.
fn paragraph_repeat_beginning_impl(
    sentences: &[AnalyzedSentence],
    id: &str,
    msg: &str,
    description: &str,
    category: (&str, &str),
    is_article: &dyn Fn(&AnalyzedTokenReadings) -> bool,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    if sentences.is_empty() {
        return rule_matches;
    }
    let mut next_pos = 0usize;
    let mut last_pos = 0usize;
    let mut last_tokens = sentences[0].tokens_without_whitespace();
    for n in 0..sentences.len() - 1 {
        next_pos += sentences[n].text.len();
        if is_paragraph_end(sentences, n) {
            let next_sentence = &sentences[n + 1];
            let next_tokens = next_sentence.tokens_without_whitespace();
            let end_pos = num_char_equal_beginning(&last_tokens, &next_tokens, is_article);
            if end_pos > 0 {
                let start_pos = last_pos + last_tokens[1].start_pos;
                if start_pos < last_pos + end_pos {
                    rule_matches.push(
                        Match::new(
                            id,
                            Option::<String>::None,
                            msg,
                            Option::<String>::None,
                            TextRange::new(start_pos, last_pos + end_pos),
                            Vec::new(),
                            category.0,
                            category.1,
                        )
                        .with_metadata(description, "style", -1),
                    );
                    let start_pos = next_pos + next_tokens[1].start_pos;
                    rule_matches.push(
                        Match::new(
                            id,
                            Option::<String>::None,
                            msg,
                            Option::<String>::None,
                            TextRange::new(start_pos, next_pos + end_pos),
                            Vec::new(),
                            category.0,
                            category.1,
                        )
                        .with_metadata(description, "style", -1),
                    );
                }
            }
            last_tokens = next_tokens;
            last_pos = next_pos;
        }
    }
    rule_matches
}

/// `ParagraphRepeatBeginningRule` with per-language strings (English uses
/// the base `DT` article check).
pub fn paragraph_repeat_beginning_with(
    sentences: &[AnalyzedSentence],
    strings: &ParagraphStrings,
) -> Vec<Match> {
    paragraph_repeat_beginning_impl(
        sentences,
        PARAGRAPH_REPEAT_BEGINNING_ID,
        strings.repetition_last_msg,
        strings.repetition_desc,
        strings.style_category,
        &is_article,
    )
}

/// `ParagraphRepeatBeginningRule` (English).
pub fn paragraph_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    paragraph_repeat_beginning_with(sentences, &strings_en())
}

/// `GermanParagraphRepeatBeginningRule` (`GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE`,
/// `ART` articles, German messages, default off).
pub fn paragraph_repeat_beginning_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    paragraph_repeat_beginning_impl(
        sentences,
        "GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE",
        "Der Absatz beginnt so wie der vorhergehende Absatz",
        "Gleicher Anfang von aufeinanderfolgenden Absätzen",
        ("STYLE", "Stil"),
        &is_article_de,
    )
}

/// `MessagesBundle_be` strings for the paragraph rules.
pub fn strings_be() -> ParagraphStrings {
    ParagraphStrings {
        style_category: ("STYLE", "Стыль"),
        punctuation_category: ("PUNCTUATION", "Пунктуацыя"),
        long_desc: |max| format!("Чытэльнасць: абзац даўжынёй больш за {max} слоў"),
        long_msg: |max| format!("У абзацы больш {max} слоў, неабходна перабудаваць"),
        empty_line_msg: "Выдаліце пусты радок ніжэй і скарыстайцеся фарматаваннем",
        empty_line_desc: "Пусты радок",
        ws_begin_msg: "Выдаліце прабел у пачатку абзаца",
        ws_begin_desc: "Прабел у пачатку абзаца",
        ws_end_msg: "Выдаліце прабел у канцы абзаца",
        ws_end_desc: "Прабел у канцы абзаца",
        punct_msg: "Дадайце знак прыпынку ў канцы абзаца",
        punct_desc: "Прапушчаны знак прыпынку ў канцы абзаца",
        repetition_last_msg: "Супадае з апошнім абзацам",
        repetition_desc: "Супадае з пачаткам абзаца",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn sentence(text: &str) -> AnalyzedSentence {
        AnalyzedSentence {
            text: text.to_string(),
            offset: 0,
            tokens: vec![
                AnalyzedTokenReadings {
                    readings: vec![AnalyzedToken::new("", None, Some("SENT_START".into()))],
                    chunk_tags: Vec::new(),
                    whitespace_before: false,
                    start_pos: 0,
                    raw_byte_len: 0,
                    is_whitespace: false,
                    is_sentence_start: true,
                    is_sentence_end: false,
                    is_paragraph_end: false,
                    is_tagged: false,
                    is_immunized: false,
                    is_ignore_spelling: false,
                    has_typographic_apostrophe: false,
                    is_pos_tag_unknown: false,
                },
                AnalyzedTokenReadings {
                    readings: vec![AnalyzedToken::new(text, None, None)],
                    chunk_tags: Vec::new(),
                    whitespace_before: false,
                    start_pos: 0,
                    raw_byte_len: text.len(),
                    is_whitespace: false,
                    is_sentence_start: false,
                    is_sentence_end: text == ".",
                    is_paragraph_end: false,
                    is_tagged: false,
                    is_immunized: false,
                    is_ignore_spelling: false,
                    has_typographic_apostrophe: false,
                    is_pos_tag_unknown: false,
                },
            ],
            pre_disambig_tokens: Vec::new(),
            pre_disambig_detached: Vec::new(),
        }
    }

    #[test]
    fn paragraph_end_follows_java() {
        let sentences = vec![sentence("one. "), sentence("two.")];
        assert!(!is_paragraph_end(&sentences, 0));
        assert!(is_paragraph_end(&sentences, 1));
        let sentences = vec![sentence("one.\n\n"), sentence("two.")];
        assert!(is_paragraph_end(&sentences, 0));
        let sentences = vec![sentence("one. "), sentence("\ntwo.")];
        assert!(is_paragraph_end(&sentences, 0));
    }

    #[test]
    fn detects_numeric_and_urls() {
        assert!(is_numeric("2.2.2."));
        assert!(is_numeric(" 42 "));
        assert!(!is_numeric("4a"));
        assert!(is_url("http://example.com"));
        assert!(is_url("www.example.com"));
        assert!(is_url("example.com/path"));
        assert!(!is_url("example.com"));
    }
}
