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

/// `DoublePunctuationRule` with the Romanian `MessagesBundle_ro` strings.
pub fn check_sentence_ro(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Punctuation".to_string();
        m.description = "S-au folosit două puncte sau virgule consecutive".to_string();
        if m.message == TWO_DOTS {
            m.message = "Două puncte consecutive".to_string();
            m.short_message = Some("Două puncte consecutive".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Două virgule consecutive".to_string();
            m.short_message = Some("Două virgule consecutive".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Slovak `MessagesBundle_sk` strings.
pub fn check_sentence_sk(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Interpunkcia".to_string();
        m.description = "Použitie dvoch za sebou idúcich bodiek alebo čiarok".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dve po sebe idúce bodky".to_string();
            m.short_message = Some("Dve za sebou idúce bodky".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dve po sebe idúce čiarky".to_string();
            m.short_message = Some("Dve za sebou idúce čiarky".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Slovenian `MessagesBundle_sl` strings.
pub fn check_sentence_sl(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Postavitev ločil".to_string();
        m.description = "Uporaba dveh zaporednih pik ali vejic".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dve zaporedni piki".to_string();
            m.short_message = Some("Zaporedni piki".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dve zaporedni vejici".to_string();
            m.short_message = Some("Zaporedni vejici".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Greek `MessagesBundle_el` strings.
pub fn check_sentence_el(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Στίξη".to_string();
        m.description = "Χρήση δύο συνεχόμενων κομμάτων ή τελειών".to_string();
        if m.message == TWO_DOTS {
            m.message = "Δύο συνεχόμενες τελείες".to_string();
            m.short_message = Some("Δύο συνεχόμενες τελείες".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Δύο συνεχόμενα κόμματα".to_string();
            m.short_message = Some("Δύο συνεχόμενα κόμματα".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Danish `MessagesBundle_da` strings.
pub fn check_sentence_da(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Tegnsætning".to_string();
        m.description = "To på hinanden følgende punktummer eller kommaer".to_string();
        if m.message == TWO_DOTS {
            m.message = "To på hinanden følgende punktummer".to_string();
            m.short_message = Some("To på hinanden følgende punktummer".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "To på hinanden følgende kommaer".to_string();
            m.short_message = Some("To på hinanden følgende kommaer".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Swedish `MessagesBundle_sv` strings.
pub fn check_sentence_sv(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Skiljetecken".to_string();
        m.description = "Dubbla punkter eller kommatecken".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dubbla punkter".to_string();
            m.short_message = Some("Två punkter i följd".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dubbla kommatecken".to_string();
            m.short_message = Some("Två kommatecken i följd".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Asturian `MessagesBundle_ast` strings.
pub fn check_sentence_ast(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Puntuación".to_string();
        m.description = "Usu de dos puntos o comes seguíos".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dos puntos siguíos".to_string();
            m.short_message = Some("Dos puntos siguíos".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dos comes siguíes".to_string();
            m.short_message = Some("Dos comes siguíes".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Breton `MessagesBundle_br` strings.
pub fn check_sentence_br(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Poentadur".to_string();
        m.description = "Daou skej pe daou bik diouzh renk".to_string();
        if m.message == TWO_DOTS {
            m.message = "Daou bik diouzh renk".to_string();
            m.short_message = Some("Daou bik diouzh renk".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Daou skej diouzh renk".to_string();
            m.short_message = Some("Daou skej diouzh renk".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Tagalog `MessagesBundle_tl` strings.
pub fn check_sentence_tl(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Punctuation".to_string();
        m.description = "Paggamit ng dalawang magkasunod na tuldok o kuwit".to_string();
        if m.message == TWO_DOTS {
            m.message = "Dalawang magkasunod na tuldok".to_string();
            m.short_message = Some("Dalawang magkasunod na tuldok".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Dalawang magkasunod na kuwit".to_string();
            m.short_message = Some("Dalawang magkasunod na kuwit".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Lithuanian `MessagesBundle_lt` strings.
pub fn check_sentence_lt(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Skyryba".to_string();
        m.description = "Ar nėra dviejų pasikartojančių taškų ar kablelių".to_string();
        if m.message == TWO_DOTS {
            m.message = "Du iš eilės einantys taškai".to_string();
            m.short_message = Some("Du iš eilės einantys taškai".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Du iš eilės einantys kableliai".to_string();
            m.short_message = Some("Du iš eilės einantys kableliai".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Serbian `MessagesBundle_sr` strings.
pub fn check_sentence_sr(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Интерпункција".to_string();
        m.description = "Употребљене две узастопне тачке или запете".to_string();
        if m.message == TWO_DOTS {
            m.message = "Две узастопне тачке".to_string();
            m.short_message = Some("Две узастопне тачке".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Две узастопне запете".to_string();
            m.short_message = Some("Две узастопне запете".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Esperanto `MessagesBundle_eo` strings.
pub fn check_sentence_eo(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Interpunkcio".to_string();
        m.description = "Uzo de sinsekvaj punktoj aŭ komoj".to_string();
        if m.message == TWO_DOTS {
            m.message = "Du sinsekvaj punktoj".to_string();
            m.short_message = Some("Du sinsekvaj punktoj".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Du sinsekvaj komoj".to_string();
            m.short_message = Some("Du sinsekvaj komoj".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Icelandic `MessagesBundle_is` strings.
pub fn check_sentence_is(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Punctuation".to_string();
        m.description = "Tvítekinn punktur eða komma".to_string();
        if m.message == TWO_DOTS {
            m.message = "Tveir punktar í röð".to_string();
            m.short_message = Some("Tveir punktar í röð".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Tvær kommur í röð".to_string();
            m.short_message = Some("Tvær kommur í röð".to_string());
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
    check_sentence_with_comma(tokens, sentence_offset, ",", "DOUBLE_PUNCTUATION")
}

/// `DoublePunctuationRule.match` parameterized by the rule's comma character
/// (`getCommaCharacter()`) and rule id (the Arabic `ARABIC_DOUBLE_PUNCTUATION`
/// uses `،`).
pub fn check_sentence_with_comma(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    comma: &str,
    rule_id: &str,
) -> Vec<Match> {
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
        } else if token == comma {
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
                    rule_id,
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
        } else if comma_count == 2 && next_token != Some(comma) {
            let from_pos = start_pos.saturating_sub(1);
            rule_matches.push(
                Match::new(
                    rule_id,
                    Option::<String>::None,
                    TWO_COMMAS,
                    Some(DOUBLE_COMMAS_SHORT.to_string()),
                    TextRange::new(sentence_offset + from_pos, sentence_offset + start_pos + 1),
                    vec![Suggestion {
                        value: comma.to_string(),
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
        if token != "." && token != comma {
            dot_count = 0;
            comma_count = 0;
        }
    }
    rule_matches
}

/// `DoublePunctuationRule` with the Belarusian `MessagesBundle_be` strings.
pub fn check_sentence_be(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "Пунктуацыя".to_string();
        m.description = "Дзве коскі або кропкі запар".to_string();
        if m.message == TWO_DOTS {
            m.message = "Дзве крокі запар".to_string();
            m.short_message = Some("Дзве кропкі запар".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "Дзве коскі запар".to_string();
            m.short_message = Some("Дзве коскі запар".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` parameterized by the comma character and rule id,
/// with the `MessagesBundle_fa` strings: the generic `DoublePunctuationRule`
/// (`DOUBLE_PUNCTUATION`, comma `,`) and `PersianDoublePunctuationRule`
/// (`PERSIAN_DOUBLE_PUNCTUATION`, comma `،`).
pub fn check_sentence_fa(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    comma: &str,
    rule_id: &str,
) -> Vec<Match> {
    let mut matches = check_sentence_with_comma(tokens, sentence_offset, comma, rule_id);
    for m in &mut matches {
        m.category_name = "Punctuation".to_string();
        m.description = "استفاده از دو نقطهٔ پشت‌سر هم یا کاماها".to_string();
        if m.message == TWO_DOTS {
            m.message = "دو نقطهٔ پشت سر هم".to_string();
            m.short_message = Some("دو نقطهٔ پشت‌سرهم".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "دو کامای پشت سر هم".to_string();
            m.short_message = Some("دو کامای پشت‌سرهم".to_string());
        }
    }
    matches
}

/// `DoublePunctuationRule` with the Tamil `MessagesBundle_ta` strings
/// (`desc_double_punct` / `two_dots` / `two_commas` / `double_*_short` /
/// `category_punctuation`).
pub fn check_sentence_ta(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_offset);
    for m in &mut matches {
        m.category_name = "நிறுத்தக்குறியீடு".to_string();
        m.description = "இரண்டு அடுத்தடுத்த புள்ளிகளையோ காற்புள்ளிகளையோ பயன்படுத்து".to_string();
        if m.message == TWO_DOTS {
            m.message = "இரு அடுத்தடுத்த புள்ளிகள்".to_string();
            m.short_message = Some("இரு அடுத்தடுத்த புள்ளிகள்".to_string());
        } else if m.message == TWO_COMMAS {
            m.message = "இரு அடுத்தடுத்த காற்புள்ளிகள்".to_string();
            m.short_message = Some("இரு அடுத்தடுத்த காற்புள்ளிகள்".to_string());
        }
    }
    matches
}
