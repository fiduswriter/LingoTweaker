//! Spanish sentence- and text-level Java rules that do not fit the shared
//! per-language modules (`SpanishWordRepeatRule`, `QuestionMarkRule`, …).

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_punctuation_mark, is_word};

// ---------------------------------------------------------------------------
// SpanishWordRepeatRule (`SPANISH_WORD_REPEAT_RULE`)
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_RULE_ID: &str = "SPANISH_WORD_REPEAT_RULE";
pub const WORD_REPEAT_DESCRIPTION: &str = "Repetición de una palabra (p. ej. 'soy soy')";
pub const WORD_REPEAT_SHORT: &str = "Repetición de una palabra";
pub const WORD_REPEAT_MESSAGE: &str = "Posible error: repetición de una palabra";

/// `SpanishWordRepeatRule.ignore`: `_allow_repeat` on the token or its
/// predecessor, plus the base `WordRepeatRule` name list.
fn spanish_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    if position > 0
        && (tokens[position].has_pos_tag("_allow_repeat")
            || tokens[position - 1].has_pos_tag("_allow_repeat"))
    {
        return true;
    }
    // `WordRepeatRule.ignore`
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if position > 0
            && tokens[position - 1].surface() == name
            && tokens[position].surface() == name
        {
            return true;
        }
    }
    false
}

/// `SpanishWordRepeatRule.match` over one sentence.
pub fn word_repeat_sentence(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized {
            prev_token.clear();
            continue;
        }
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !spanish_ignore(&view, i) {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    WORD_REPEAT_RULE_ID,
                    Option::<String>::None,
                    WORD_REPEAT_MESSAGE,
                    Some(WORD_REPEAT_SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "MISC",
                    "Varios",
                )
                .with_metadata(WORD_REPEAT_DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// QuestionMarkRule (`ES_QUESTION_MARK`, text level)
// ---------------------------------------------------------------------------

pub const QUESTION_MARK_RULE_ID: &str = "ES_QUESTION_MARK";

/// `QuestionMarkRule.hasTokenAtPos`.
fn has_token_at_pos(ch: &str, tokens: &[&AnalyzedTokenReadings]) -> i32 {
    let mut i = tokens.len() as i64 - 1;
    while i > 0 {
        let idx = i as usize;
        if tokens[idx].surface() == ch {
            let next = tokens.get(idx + 1);
            let ignore = next.is_some_and(|t| {
                !t.whitespace_before && !is_punctuation_mark(t.surface()) && !t.is_whitespace
            });
            if !ignore {
                return i as i32;
            }
        }
        i -= 1;
    }
    -1
}

/// `QuestionMarkRule.match` over all sentences (text-level rule).
pub fn question_mark(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut matches = Vec::new();
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        if tokens.is_empty() {
            continue;
        }
        let needs_inv_question_mark_at = has_token_at_pos("?", &tokens);
        let needs_inv_excl_mark_at = has_token_at_pos("!", &tokens);
        if needs_inv_question_mark_at <= 1 && needs_inv_excl_mark_at <= 1 {
            continue;
        }
        let mut has_inv_question_mark = false;
        let mut has_inv_excl_mark = false;
        let mut first_token: Option<usize> = None;
        for i in 0..tokens.len() {
            if first_token.is_none()
                && !tokens[i].is_sentence_start
                && !is_punctuation_mark(tokens[i].surface())
            {
                first_token = Some(i);
            }
            if tokens[i].surface() == "¿" && (i as i32) < needs_inv_question_mark_at {
                has_inv_question_mark = true;
            } else if tokens[i].surface() == "¡" && (i as i32) < needs_inv_excl_mark_at {
                has_inv_excl_mark = true;
            }
            // possibly a sentence end
            if !tokens[i].is_sentence_end
                && ((tokens[i].surface() == "?" && (i as i32) > needs_inv_question_mark_at)
                    || (tokens[i].surface() == "!" && (i as i32) > needs_inv_excl_mark_at))
            {
                first_token = None;
            }
            // put the question mark in: ¿de qué... ¿para cuál... ¿cómo...
            if i > 2 && i + 2 < tokens.len() {
                if tokens[i - 1].surface() == ","
                    && tokens[i].has_pos_tag("CC")
                    && tokens[i + 1].has_pos_tag("SPS00")
                    && (tokens[i + 2].has_pos_tag_starting_with("PT")
                        || tokens[i + 2].has_pos_tag_starting_with("DT"))
                {
                    first_token = Some(i);
                }
                if tokens[i - 1].surface() == ","
                    && tokens[i].has_pos_tag("SPS00")
                    && (tokens[i + 1].has_pos_tag_starting_with("PT")
                        || tokens[i + 1].has_pos_tag_starting_with("DT"))
                {
                    first_token = Some(i);
                }
                if tokens[i - 1].surface() == ","
                    && tokens[i].has_pos_tag("CC")
                    && (tokens[i + 1].has_pos_tag_starting_with("PT")
                        || tokens[i + 1].has_pos_tag_starting_with("DT"))
                {
                    first_token = Some(i);
                }
                if tokens[i - 1].surface() == ","
                    && (tokens[i].has_pos_tag_starting_with("PT")
                        || tokens[i].has_pos_tag_starting_with("DT"))
                {
                    first_token = Some(i);
                }
                if tokens[i - 1].surface() == ","
                    && tokens[i].has_pos_tag("CC")
                    && (tokens[i + 1].surface() == "no" || tokens[i + 1].surface() == "sí")
                {
                    first_token = Some(i);
                }
            }
            if i > 2
                && i < tokens.len()
                && tokens[i - 1].surface() == ","
                && (tokens[i].surface() == "no"
                    || tokens[i].surface() == "sí"
                    || tokens[i].surface() == "eh")
            {
                first_token = Some(i);
            }
        }
        let Some(first_idx) = first_token else {
            continue;
        };
        if tokens[first_idx].has_pos_tag("_english_ignore_") {
            continue;
        }
        let s = if needs_inv_question_mark_at > 1 && needs_inv_excl_mark_at > 1 {
            // ignore for now, e.g. "¡¿Nunca tienes clases o qué?!"
            None
        } else if needs_inv_question_mark_at > 1 && !has_inv_question_mark {
            Some("¿")
        } else if needs_inv_excl_mark_at > 1 && !has_inv_excl_mark {
            Some("¡")
        } else {
            None
        };
        if let Some(s) = s {
            let message = format!("Símbolo desparejado: Parece que falta un '{s}'");
            let start = sentence.offset + tokens[first_idx].start_pos;
            let end = sentence.offset + tokens[first_idx].end_pos();
            matches.push(
                Match::new(
                    QUESTION_MARK_RULE_ID,
                    Option::<String>::None,
                    &message,
                    Option::<String>::None,
                    TextRange::new(start, end),
                    vec![Suggestion {
                        value: format!("{s}{}", tokens[first_idx].surface()),
                        short_description: None,
                    }],
                    "TYPOGRAPHY",
                    "Tipografía",
                )
                .with_metadata(
                    "Signos de exclamación / interrogación desparejados",
                    "typographical",
                    0,
                ),
            );
        }
    }
    matches
}

// ---------------------------------------------------------------------------
// SpanishWordRepeatBeginningRule (`SPANISH_WORD_REPEAT_BEGINNING_RULE`)
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_BEGINNING_RULE_ID: &str = "SPANISH_WORD_REPEAT_BEGINNING_RULE";
pub const WORD_REPEAT_BEGINNING_DESCRIPTION: &str =
    "Dos frases consecutivas comienzan con la misma palabra.";
const WRB_SHORT_ADV: &str = "Dos frases consecutivas comienzan con el mismo elemento.";
const WRB_SHORT_WORD: &str = "Tres frases consecutivas comienzan con la misma palabra.";
const WRB_THESAURUS: &str = "Considere reescribir la frase o usar un sinónimo.";

/// The Java sets iterate in `HashSet` bucket order (capacity 16); these
/// lists are that iteration order.
const ADD_ADVERBS: [&str; 5] = [
    "Adicionalmente",
    "También",
    "Asimismo",
    "Además",
    "Igualmente",
];
const CONTRAST_CONJ: [&str; 3] = ["Pero", "Empero", "Mas"];
const EMPHASIS_ADVERBS: [&str; 4] = [
    "Claramente",
    "Obviamente",
    "Definitivamente",
    "Absolutamente",
];
const EXPLAIN_ADVERBS: [&str; 4] = [
    "Específicamente",
    "Particularmente",
    "Concretamente",
    "Precisamente",
];
const PERSONAL_PRONOUNS: [&str; 12] = [
    "yo",
    "tú",
    "él",
    "ella",
    "nosostros",
    "nosotras",
    "vosotros",
    "vosotras",
    "ellos",
    "ellas",
    "usted",
    "ustedes",
];
const EXCEPCIONS_START: [&str; 12] = [
    "el",
    "la",
    "los",
    "las",
    "punto",
    "artículo",
    "módulo",
    "parte",
    "sesión",
    "unidad",
    "tema",
    "n",
];
const SENTENCE_EXCEPCIONS: [&str; 4] = ["por un", "por otro", "por otra", "por una"];

fn wrb_is_adverb(tr: &AnalyzedTokenReadings) -> bool {
    if tr.has_pos_tag("RG") || tr.has_pos_tag("LOC_ADV") {
        return true;
    }
    let tok = tr.surface();
    ADD_ADVERBS.contains(&tok)
        || CONTRAST_CONJ.contains(&tok)
        || EMPHASIS_ADVERBS.contains(&tok)
        || EXPLAIN_ADVERBS.contains(&tok)
}

fn wrb_is_exception(token: &str) -> bool {
    matches!(token, ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️")
        || token.chars().next().is_some_and(|c| c.is_ascii_digit())
        || EXCEPCIONS_START.contains(&token.to_lowercase().as_str())
}

fn wrb_suggestions(tr: &AnalyzedTokenReadings) -> Vec<String> {
    let tok = tr.surface();
    let lower = tok.to_lowercase();
    if PERSONAL_PRONOUNS.contains(&lower.as_str()) {
        return vec![
            format!("Además, {lower}"),
            format!("Igualmente, {lower}"),
            format!("No solo eso, sino que {lower}"),
        ];
    }
    if ADD_ADVERBS.contains(&tok) {
        let mut out: Vec<String> = ADD_ADVERBS
            .iter()
            .filter(|a| **a != tok)
            .map(|a| (*a).to_string())
            .collect();
        out.push("Así mismo".to_string());
        return out;
    }
    if CONTRAST_CONJ.contains(&tok) {
        return vec![
            "Aun así".to_string(),
            "Por otra parte".to_string(),
            "Sin embargo".to_string(),
        ];
    }
    if EMPHASIS_ADVERBS.contains(&tok) {
        return EMPHASIS_ADVERBS
            .iter()
            .filter(|a| **a != tok)
            .map(|a| (*a).to_string())
            .collect();
    }
    if EXPLAIN_ADVERBS.contains(&tok) {
        return EXPLAIN_ADVERBS
            .iter()
            .filter(|a| **a != tok)
            .map(|a| (*a).to_string())
            .collect();
    }
    Vec::new()
}

/// `SpanishWordRepeatBeginningRule.match` over all sentences (text level,
/// `tags="picky"`).
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        if SENTENCE_EXCEPCIONS
            .iter()
            .any(|e| sentence.text.to_lowercase().starts_with(e))
        {
            prev_sentence = None;
            continue;
        }
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            token = tokens[1].surface().to_string();
            if tokens.len() > 3 {
                let mut is_word = true;
                if token.chars().count() == 1 {
                    is_word = token.chars().next().is_some_and(char::is_alphabetic);
                }
                if is_word
                    && last_token == token
                    && !wrb_is_exception(&token)
                    && !wrb_is_exception(tokens[2].surface())
                    && !wrb_is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(ends_like_sentence)
                {
                    let short_msg = if wrb_is_adverb(tokens[1]) {
                        Some(WRB_SHORT_ADV)
                    } else if before_last_token == token {
                        Some(WRB_SHORT_WORD)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {WRB_THESAURUS}");
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        rule_matches.push(
                            Match::new(
                                WORD_REPEAT_BEGINNING_RULE_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                wrb_suggestions(tokens[1])
                                    .into_iter()
                                    .map(|value| Suggestion {
                                        value,
                                        short_description: None,
                                    })
                                    .collect(),
                                "REPETITIONS_STYLE",
                                "Repeticiones",
                            )
                            .with_metadata(WORD_REPEAT_BEGINNING_DESCRIPTION, "style", 0)
                            .with_picky(true),
                        );
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}

fn ends_like_sentence(sentence: &AnalyzedSentence) -> bool {
    let trimmed = sentence.text.trim();
    trimmed.len() > 1
        && trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '?' | '!'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_token_at_pos_ignores_urls() {
        let mk = |s: &str| {
            let mut tr = AnalyzedTokenReadings::new(vec![lt_core::AnalyzedToken::new(
                s,
                None::<String>,
                None::<String>,
            )]);
            tr.raw_byte_len = s.len();
            tr
        };
        let mut b = mk("b");
        b.whitespace_before = true;
        let tokens = [mk("a"), mk("?"), b];
        let refs: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        assert_eq!(has_token_at_pos("?", &refs), 1);
        // a question mark directly before a word is ignored (URL style)
        let tokens = [mk("a"), mk("?"), mk("b")];
        let refs: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        assert_eq!(has_token_at_pos("?", &refs), -1);
    }
}
