//! Port of `DoublePunctuationRule` (`DOUBLE_PUNCTUATION`): matches ".." (but
//! not "...") and ",,".

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

const TWO_DOTS: &str = "Two consecutive dots";
const TWO_COMMAS: &str = "Two consecutive commas";
const DOUBLE_DOTS_SHORT: &str = "Two consecutive dots";
const DOUBLE_COMMAS_SHORT: &str = "Two consecutive commas";

/// `GermanDoublePunctuationRule.match` (id `DE_DOUBLE_PUNCTUATION`,
/// German dot message; the comma message comes from the German bundle).
pub fn check_sentence_de(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    const TWO_DOTS_DE: &str = "Zwei aufeinander folgende Punkte. Auch wenn ein Satz mit einer Abkürzung endet, endet er nur mit einem Punkt (§103 Regelwerk).";
    const TWO_COMMAS_DE: &str = "Zwei aufeinanderfolgende Kommas";
    const DOUBLE_DOTS_SHORT_DE: &str = "Zwei aufeinanderfolgende Punkte";
    const DOUBLE_COMMAS_SHORT_DE: &str = "Zwei aufeinanderfolgende Kommas";
    const DESCRIPTION_DE: &str = "Zwei aufeinanderfolgende Kommas oder Punkte";
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.rule_id = "DE_DOUBLE_PUNCTUATION".to_string();
        m.category_name = "Zeichensetzung".to_string();
        m.description = DESCRIPTION_DE.to_string();
        if m.message == TWO_DOTS {
            m.message = TWO_DOTS_DE.to_string();
            m.short_message = Some(DOUBLE_DOTS_SHORT_DE.to_string());
        } else if m.message == TWO_COMMAS {
            m.message = TWO_COMMAS_DE.to_string();
            m.short_message = Some(DOUBLE_COMMAS_SHORT_DE.to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Spanish `MessagesBundle_es` strings.
pub fn check_sentence_es(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Puntuación".to_string();
        m.description = "Dos puntos o comas consecutivos".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dos puntos consecutivos".to_string();
            m.short_message = Some("Dos puntos consecutivos".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dos comas consecutivas".to_string();
            m.short_message = Some("Dos comas consecutivas".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the French `MessagesBundle_fr` strings.
pub fn check_sentence_fr(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Ponctuation".to_string();
        m.description = "Deux virgules ou points consécutifs".to_string();
        if m.message == TWO_DOTS {
            m.message = "Deux points consécutifs".to_string();
            m.short_message = Some("Deux points consécutifs".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Deux virgules consécutives".to_string();
            m.short_message = Some("Deux virgules consécutives".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Italian `MessagesBundle_it` strings.
pub fn check_sentence_it(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Punteggiatura".to_string();
        m.description = "Doppia battitura di punti o di virgole".to_string();
        if m.message == TWO_DOTS {
            m.message = "Due punti consecutivi".to_string();
            m.short_message = Some("Due punti consecutivi".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Due virgole consecutive".to_string();
            m.short_message = Some("Due virgole consecutive".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Portuguese strings
/// (`MessagesBundle_pt_PT`, or `MessagesBundle_pt_BR` for `pt-BR`; the BR
/// bundle renames the punctuation category to `Acentuação`).
pub fn check_sentence_pt(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    variant: &str,
) -> Vec<Match> {
    let br = variant.starts_with("pt-BR");
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = if br { "Acentuação" } else { "Pontuação" }.to_string();
        m.description = if br {
            "Uso de 2 pontos ou vírgulas consecutivos"
        } else {
            "Pontuação duplicada"
        }
        .to_string();
        if m.message == TWO_DOTS {
            m.message = "Dois pontos consecutivos".to_string();
            m.short_message = Some("Dois pontos consecutivos".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Duas vírgulas consecutivas".to_string();
            m.short_message = Some("Duas vírgulas consecutivas".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Dutch `MessagesBundle_nl` strings.
pub fn check_sentence_nl(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Interpunctie".to_string();
        m.description = "Twee komma's of punten".to_string();
        if m.message == TWO_DOTS {
            m.message = "Twee of meer opeenvolgende punten; 1 of 3 is gebruikelijk.".to_string();
            m.short_message = Some("Te veel punten".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Twee opeenvolgende komma's".to_string();
            m.short_message = Some("Te veel komma's".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Catalan `MessagesBundle_ca` strings.
pub fn check_sentence_ca(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Puntuació".to_string();
        m.description = "Dos punts o dues comes consecutives".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dos punts consecutius.".to_string();
            m.short_message = Some("Dos punts consecutius".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dues comes consecutives".to_string();
            m.short_message = Some("Dues comes consecutives".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Galician `MessagesBundle_gl` strings.
pub fn check_sentence_gl(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Puntuación".to_string();
        m.description = "Uso de dous puntos ou comas consecutivos".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dous puntos consecutivos".to_string();
            m.short_message = Some("Dous puntos consecutivos".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dúas comas consecutivas".to_string();
            m.short_message = Some("Dúas comas consecutivas".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule.match` over one sentence.
pub fn check_sentence(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut rule_matches: Vec<Match> = Vec::new();
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut start_pos = 0usize;
    let mut dot_count = 0i32;
    let mut comma_count = 0i32;
    for i in 1..view.len() {
        let token = view[i].surface();
        let next_token = if i < view.len() - 1 {
            Some(view[i + 1].surface())
        } else {
            None
        };
        let prev_prev_token = if i > 1 {
            Some(view[i - 2].surface())
        } else {
            None
        };
        if token == "." {
            dot_count += 1;
            comma_count = 0;
            start_pos = view[i].start_pos;
        } else if token == "," {
            comma_count += 1;
            dot_count = 0;
            start_pos = view[i].start_pos;
        }

        if dot_count == 2
            && next_token != Some(".")
            && next_token != Some("…")
            && token != "/"
            && next_token != Some("/")
            && token != "\\"
            && next_token != Some("\\")
            && prev_prev_token != Some("?")
            && prev_prev_token != Some("!")
            && prev_prev_token != Some("…")
            && prev_prev_token != Some(".")
        {
            let from_pos = start_pos.saturating_sub(1);
            rule_matches.push(
                Match::new(
                    "DOUBLE_PUNCTUATION",
                    Option::<String>::None,
                    TWO_DOTS,
                    Some(DOUBLE_DOTS_SHORT.to_string()),
                    TextRange::new(sentence_offset + from_pos, sentence_offset + start_pos + 1),
                    vec![
                        Suggestion {
                            value: ".".to_string(),
                            short_description: None,
                        },
                        Suggestion {
                            value: "…".to_string(),
                            short_description: None,
                        },
                    ],
                    "PUNCTUATION",
                    "Punctuation",
                )
                .with_metadata(
                    "Use of two consecutive dots or commas",
                    "typographical",
                    0,
                ),
            );
            dot_count = 0;
        } else if comma_count == 2 && next_token != Some(",") {
            let from_pos = start_pos.saturating_sub(1);
            rule_matches.push(
                Match::new(
                    "DOUBLE_PUNCTUATION",
                    Option::<String>::None,
                    TWO_COMMAS,
                    Some(DOUBLE_COMMAS_SHORT.to_string()),
                    TextRange::new(sentence_offset + from_pos, sentence_offset + start_pos + 1),
                    vec![Suggestion {
                        value: ",".to_string(),
                        short_description: None,
                    }],
                    "PUNCTUATION",
                    "Punctuation",
                )
                .with_metadata(
                    "Use of two consecutive dots or commas",
                    "typographical",
                    0,
                ),
            );
            comma_count = 0;
        }
        if token != "." && token != "," {
            dot_count = 0;
            comma_count = 0;
        }
    }
    rule_matches
}
