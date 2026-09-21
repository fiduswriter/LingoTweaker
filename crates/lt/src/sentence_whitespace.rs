//! Port of `SentenceWhitespaceRule` (`SENTENCE_WHITESPACE`): repeated or
//! missing whitespace between sentences. English allows two spaces between
//! sentences (`maxSpacesBetweenSentences = 2`).

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "SENTENCE_WHITESPACE";

const REPEATED_MESSAGE: &str = "Possible typo: you repeated a whitespace";
const ADD_SPACE_MESSAGE: &str = "Add a space between sentences.";

/// Per-language strings of the rule (English defaults).
pub(crate) struct Strings {
    description: &'static str,
    repeated_message: &'static str,
    add_space_message: &'static str,
    category_name: &'static str,
    /// `SentenceWhitespaceRule(messages, maxSpacesBetweenSentences)`: English
    /// passes 2, the other languages the default 1.
    max_spaces_between_sentences: usize,
}

const STRINGS_EN: Strings = Strings {
    description: "Missing space between sentences",
    repeated_message: REPEATED_MESSAGE,
    add_space_message: ADD_SPACE_MESSAGE,
    category_name: "Typography",
    max_spaces_between_sentences: 2,
};

/// French `SentenceWhitespaceRule` strings (`MessagesBundle_fr`).
const STRINGS_FR: Strings = Strings {
    description: "Espace manquante entre les phrases",
    repeated_message: "Faute de frappe possible : une espace est répétée",
    add_space_message: "Ajoutez une espace entre les phrases.",
    category_name: "Typographie",
    max_spaces_between_sentences: 2,
};

fn is_only_spaces(token: &str) -> bool {
    token.chars().all(|c| c == ' ')
}

fn is_space_token(token: &AnalyzedTokenReadings) -> bool {
    is_only_spaces(token.surface())
}

fn is_sentence_end_token(token: &str) -> bool {
    matches!(token, "." | "!" | "?")
}

fn follows_sentence_end(tokens: &[AnalyzedTokenReadings], whitespace_pos: usize) -> bool {
    let mut i = whitespace_pos;
    while i > 1 {
        i -= 1;
        if !tokens[i].is_whitespace {
            return is_sentence_end_token(tokens[i].surface());
        }
    }
    false
}

fn is_line_break_token(token: &AnalyzedTokenReadings) -> bool {
    let s = token.surface();
    matches!(s, "\n" | "\r\n" | "\r" | "\n\r") || s.contains('\n') || s.contains('\r')
}

fn get_whitespace_length(tokens: &[AnalyzedTokenReadings], from: usize, to: usize) -> usize {
    (from..=to).map(|i| tokens[i].surface().len()).sum()
}

fn get_leading_spaces_length(tokens: &[AnalyzedTokenReadings]) -> usize {
    let mut length = 0;
    let mut i = 1;
    while i < tokens.len() && is_space_token(&tokens[i]) {
        length += tokens[i].surface().len();
        i += 1;
    }
    length
}

fn get_token_index_after_leading_spaces(
    tokens: &[AnalyzedTokenReadings],
    leading_spaces_length: usize,
) -> usize {
    let mut pos = 1;
    let mut spaces_length = 0;
    while pos < tokens.len() && spaces_length < leading_spaces_length {
        spaces_length += tokens[pos].surface().len();
        pos += 1;
    }
    pos
}

fn has_text_after_leading_spaces(
    tokens: &[AnalyzedTokenReadings],
    leading_spaces_length: usize,
) -> bool {
    let pos = get_token_index_after_leading_spaces(tokens, leading_spaces_length);
    pos < tokens.len() && !tokens[pos].is_whitespace && !is_line_break_token(&tokens[pos])
}

fn is_followed_by_non_whitespace_token(
    tokens: &[AnalyzedTokenReadings],
    whitespace_pos: usize,
) -> bool {
    whitespace_pos + 1 < tokens.len()
        && !tokens[whitespace_pos + 1].is_whitespace
        && !is_line_break_token(&tokens[whitespace_pos + 1])
}

fn starts_with_line_break(tokens: &[AnalyzedTokenReadings]) -> bool {
    tokens.len() > 1 && is_line_break_token(&tokens[1])
}

/// `SentenceWhitespaceRule.addRepeatedWhitespaceMatches`.
fn add_repeated_whitespace_matches(
    sentence: &AnalyzedSentence,
    rule_matches: &mut Vec<Match>,
    strings: &Strings,
) {
    let tokens = &sentence.tokens;
    let mut i = 1usize;
    while i < tokens.len() {
        if is_space_token(&tokens[i]) && follows_sentence_end(tokens, i) {
            let first_whitespace = i;
            let mut last_whitespace = i;
            i += 1;
            while i < tokens.len() && is_space_token(&tokens[i]) {
                last_whitespace = i;
                i += 1;
            }
            if get_whitespace_length(tokens, first_whitespace, last_whitespace)
                > strings.max_spaces_between_sentences
                && is_followed_by_non_whitespace_token(tokens, last_whitespace)
            {
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        strings.repeated_message,
                        Option::<String>::None,
                        TextRange::new(
                            sentence.offset + tokens[first_whitespace].start_pos,
                            sentence.offset + tokens[last_whitespace].end_pos(),
                        ),
                        vec![Suggestion {
                            value: " ".to_string(),
                            short_description: None,
                        }],
                        "TYPOGRAPHY",
                        strings.category_name,
                    )
                    .with_metadata(strings.description, "whitespace", 0),
                );
            }
        } else {
            i += 1;
        }
    }
}

/// `SentenceWhitespaceRule.match` over all sentences (English strings).
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &STRINGS_EN)
}

/// `SentenceWhitespaceRule` with the French `MessagesBundle_fr` strings.
pub fn check_fr(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &STRINGS_FR)
}

/// `SentenceWhitespaceRule` with the Swedish `MessagesBundle_sv` strings.
pub fn check_sv(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const SV_STRINGS: Strings = Strings {
        description: "Det saknas ett blanksteg mellan meningar",
        repeated_message: "Möjligt korrekturfel: du upprepade ett blanktecken",
        add_space_message: "Lägg till ett blanksteg mellan meningarna.",
        category_name: "Typografi",
        max_spaces_between_sentences: 1,
    };
    check_with(sentences, &SV_STRINGS)
}

/// `SentenceWhitespaceRule` with the Breton `MessagesBundle_br` strings.
pub fn check_br(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const BR_STRINGS: Strings = Strings {
        description: "Ur spas a vank etre ar frazennoù",
        repeated_message: "Fazi bizskrivañ posupl: daou spas ho peus lakaet",
        add_space_message: "Ouzhpennañ ur spas etre ar frazennoù",
        category_name: "Lizherennerezh",
        max_spaces_between_sentences: 1,
    };
    check_with(sentences, &BR_STRINGS)
}

/// `SentenceWhitespaceRule` with the Esperanto `MessagesBundle_eo` strings
/// (the description and repeated message fall back to the core English
/// bundle; only `addSpaceBetweenSentences` is translated).
pub fn check_eo(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const EO_STRINGS: Strings = Strings {
        description: "Missing space between sentences",
        repeated_message: "Ebla mistajpaĵo: vi ripetis spaceton",
        add_space_message: "Aldoni spaceton inter frazoj",
        category_name: "Tipografio",
        max_spaces_between_sentences: 1,
    };
    check_with(sentences, &EO_STRINGS)
}

/// `SentenceWhitespaceRule` with the Portuguese variant strings
/// (`MessagesBundle_pt_PT` / `_pt_BR`; plain `pt`/`pt-AO`/`pt-MZ` fall back
/// to the core English bundle).
pub fn check_pt(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    check_with(sentences, strings_pt(variant))
}

/// `SentenceWhitespaceRule` with the Dutch `MessagesBundle_nl` strings
/// (`missing_space_between_sentences`/`whitespace_repetition`; the add-space
/// message falls back to the core bundle like Java's ResourceBundle chain).
pub fn check_nl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const NL_STRINGS: Strings = Strings {
        description: "Tussen twee zinnen hoort witruimte",
        repeated_message: "Te veel witruimte",
        add_space_message: "Tussen zinnen hoort witruimte",
        category_name: "Typografie",
        max_spaces_between_sentences: 1,
    };
    check_with(sentences, &NL_STRINGS)
}

/// Catalan `SentenceWhitespaceRule` strings (`MessagesBundle_ca`; the Java
/// constructor default `maxSpacesBetweenSentences` = 1).
const STRINGS_CA: Strings = Strings {
    description: "Falta un espai entre frases",
    repeated_message: "Possible error: heu repetit un espai en blanc",
    add_space_message: "Afegiu un espai entre les frases.",
    category_name: "Tipografia",
    max_spaces_between_sentences: 1,
};

/// `SentenceWhitespaceRule` with the Catalan `MessagesBundle_ca` strings.
pub fn check_ca(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &STRINGS_CA)
}

/// `SentenceWhitespaceRule` with the Galician `MessagesBundle_gl` strings
/// (Java constructor default `maxSpacesBetweenSentences` = 1).
const STRINGS_GL: Strings = Strings {
    description: "Falta un espazo entre oracións",
    repeated_message: "Posíbel erro tipográfico: repetiu un espazo en branco",
    add_space_message: "Engada un espazo entre oracións.",
    category_name: "Tipografía",
    max_spaces_between_sentences: 1,
};

/// `SentenceWhitespaceRule` with the Galician `MessagesBundle_gl` strings.
pub fn check_gl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &STRINGS_GL)
}

/// `SentenceWhitespaceRule` with the Polish `MessagesBundle_pl` strings
/// (Java constructor default `maxSpacesBetweenSentences` = 1).
const STRINGS_PL: Strings = Strings {
    description: "Brak spacji między zdaniami",
    repeated_message: "Prawdopodobna literówka: wiele spacji z rzędu",
    add_space_message: "Dodaj spację między zdaniami",
    category_name: "Błędy typograficzne",
    max_spaces_between_sentences: 1,
};

/// `SentenceWhitespaceRule` with the Polish `MessagesBundle_pl` strings.
pub fn check_pl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(sentences, &STRINGS_PL)
}

fn strings_pt(variant: &str) -> &'static Strings {
    const PT_PT_STRINGS: Strings = Strings {
        description: "Falta um espaço entre as frases",
        repeated_message: "Possível erro: repetiu um espaço",
        add_space_message: "Adicione um espaço entre as frases",
        category_name: "Tipografia",
        max_spaces_between_sentences: 2,
    };
    const PT_BR_STRINGS: Strings = Strings {
        description: "Espaço ausente entre as frases",
        repeated_message: "Possível erro de escrita: você repetiu um espaço em branco",
        add_space_message: "Adicionar um espaço entre as frases",
        category_name: "Tipografia",
        max_spaces_between_sentences: 2,
    };
    match variant {
        "pt-BR" => &PT_BR_STRINGS,
        _ => &PT_PT_STRINGS,
    }
}

/// `SentenceWhitespaceRule.match` over all sentences.
pub(crate) fn check_with(sentences: &[AnalyzedSentence], strings: &Strings) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut is_first_sentence = true;
    let mut prev_sentence_ending_whitespace = String::new();
    let mut prev_sentence_ends_with_line_break = false;
    let mut _prev_sentence_ends_with_number = false;
    for sentence in sentences {
        let tokens = &sentence.tokens;
        add_repeated_whitespace_matches(sentence, &mut rule_matches, strings);
        if is_first_sentence {
            is_first_sentence = false;
        } else if !prev_sentence_ends_with_line_break && !starts_with_line_break(tokens) {
            let leading_spaces_length = get_leading_spaces_length(tokens);
            if is_only_spaces(&prev_sentence_ending_whitespace)
                && prev_sentence_ending_whitespace.len() + leading_spaces_length
                    > strings.max_spaces_between_sentences
                && (!prev_sentence_ending_whitespace.is_empty() || leading_spaces_length > 0)
                && has_text_after_leading_spaces(tokens, leading_spaces_length)
            {
                let start = sentence
                    .offset
                    .saturating_sub(prev_sentence_ending_whitespace.len());
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        strings.repeated_message,
                        Option::<String>::None,
                        TextRange::new(start, sentence.offset + leading_spaces_length),
                        vec![Suggestion {
                            value: " ".to_string(),
                            short_description: None,
                        }],
                        "TYPOGRAPHY",
                        strings.category_name,
                    )
                    .with_metadata(strings.description, "whitespace", 0),
                );
            } else if prev_sentence_ending_whitespace.is_empty() && tokens.len() > 1 {
                let first_token = tokens[1].surface().to_string();
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        strings.add_space_message,
                        Option::<String>::None,
                        TextRange::new(sentence.offset, sentence.offset + first_token.len()),
                        vec![Suggestion {
                            value: format!(" {first_token}"),
                            short_description: None,
                        }],
                        "TYPOGRAPHY",
                        strings.category_name,
                    )
                    .with_metadata(strings.description, "whitespace", 0),
                );
            }
        }
        if !tokens.is_empty() {
            let last_token = tokens[tokens.len() - 1].surface();
            prev_sentence_ending_whitespace =
                if last_token.replace('\u{00A0}', " ").trim().is_empty() {
                    last_token.to_string()
                } else {
                    String::new()
                };
            prev_sentence_ends_with_line_break = is_line_break_token(&tokens[tokens.len() - 1]);
        }
        if tokens.len() > 1 {
            _prev_sentence_ends_with_number = tokens[tokens.len() - 2]
                .surface()
                .chars()
                .all(|c| c.is_numeric())
                && !tokens[tokens.len() - 2].surface().is_empty();
        }
    }
    rule_matches
}

/// `SentenceWhitespaceRule` with the Belarusian `MessagesBundle_be` strings.
pub fn check_be(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const BE_STRINGS: Strings = Strings {
        description: "Адсутнічае прабел паміж сказамі",
        repeated_message: "Магчымая памылка друку: вы паўтарылі прабел",
        add_space_message: "Дадайце прабел паміж сказамі",
        category_name: "Тыпаграфіка",
        max_spaces_between_sentences: 1,
    };
    check_with(sentences, &BE_STRINGS)
}

/// `SentenceWhitespaceRule` with the Russian `MessagesBundle_ru` strings.
pub fn check_ru(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const RU_STRINGS: Strings = Strings {
        description: "Отсутствуют пробелы между предложениями",
        repeated_message: "Повтор пробела",
        add_space_message: "Добавьте пробел между предложениями.",
        category_name: "Типографика",
        max_spaces_between_sentences: 1,
    };
    check_with(sentences, &RU_STRINGS)
}
