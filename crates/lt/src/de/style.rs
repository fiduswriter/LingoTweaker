//! German style rules that are not part of a shared core port:
//! `de.DashRule` (`DE_DASH`) and `CompoundCoherencyRule`
//! (`DE_COMPOUND_COHERENCY`).

use std::collections::HashMap;

use lt_core::{
    AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange,
};

/// `de.DashRule` (`Rule`, category COMPOUNDING, default on).
pub const DASH_ID: &str = "DE_DASH";
const DASH_DESCRIPTION: &str =
    "Keine Leerzeichen in Bindestrich-Komposita (wie z.B. in 'Diäten- Erhöhung')";
const DASH_MESSAGE: &str = "Möglicherweise fehlt ein 'und' oder ein Komma, oder es wurde nach dem Wort ein überflüssiges Leerzeichen eingefügt. Eventuell haben Sie auch versehentlich einen Bindestrich statt eines Punktes eingefügt.";
const DASH_SHORT: &str = "Fehlendes 'und' oder Komma oder überflüssiges Leerzeichen?";

/// `de.DashRule.match` over one sentence.
pub fn dash(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut prev_token: Option<&str> = None;
    for i in 0..view.len() {
        let token = view[i].surface();
        if let Some(prev) = prev_token {
            if prev.ends_with('-') && prev != "-" && !prev.contains("--") && !prev.contains("–-")
            {
                let first_char = token.chars().next();
                if first_char.is_some_and(char::is_uppercase)
                    && !matches!(token, "UND" | "ODER" | "BZW")
                {
                    let from_pos = view[i - 1].start_pos;
                    let mut suggestions = vec![Suggestion {
                        value: format!("{prev}{token}"),
                        short_description: None,
                    }];
                    if count_matches(prev, '-') + count_matches(token, '-') <= 1 {
                        suggestions.push(Suggestion {
                            value: format!("{prev}, {token}"),
                            short_description: None,
                        });
                    }
                    rule_matches.push(
                        Match::new(
                            DASH_ID,
                            Option::<String>::None,
                            DASH_MESSAGE,
                            Some(DASH_SHORT.to_string()),
                            TextRange::new(
                                sentence_offset + from_pos,
                                sentence_offset + view[i].end_pos(),
                            ),
                            suggestions,
                            "COMPOUNDING",
                            "Getrennt- und Zusammenschreibung",
                        )
                        .with_metadata(
                            DASH_DESCRIPTION,
                            "uncategorized",
                            0,
                        ),
                    );
                }
            }
        }
        prev_token = Some(token);
    }
    rule_matches
}

fn count_matches(s: &str, needle: char) -> usize {
    s.chars().filter(|c| *c == needle).count()
}

// ---------------------------------------------------------------------------
// `CompoundCoherencyRule`
// ---------------------------------------------------------------------------

/// `CompoundCoherencyRule.getLemma`: `hasSameLemmas` on the raw (nullable)
/// lemmas, with the hyphen fix-up for cases like "Jugend-Fotos".
fn compound_coherency_lemma(atr: &AnalyzedTokenReadings) -> Option<String> {
    if atr.readings.is_empty() || !are_lemmas_same(&atr.readings) {
        return None;
    }
    let lemma = atr.readings[0].stem.clone()?;
    let token = atr.surface();
    if lemma.contains('-') || !token.contains('-') {
        return Some(lemma);
    }
    let lemma_chars: Vec<char> = lemma.chars().collect();
    let token_chars: Vec<char> = token.chars().collect();
    let mut out = String::new();
    let mut lemma_pos = 0usize;
    let mut token_pos = 0usize;
    while lemma_pos < lemma_chars.len() {
        if token_pos >= token_chars.len() {
            break;
        }
        let lemma_char = lemma_chars[lemma_pos];
        let token_char = token_chars[token_pos];
        if lemma_char == token_char {
            out.push(lemma_char);
        } else if token_char == '-' {
            token_pos += 1;
            out.push('-');
            if lemma_pos + 1 < token_chars.len()
                && token_chars.get(token_pos).is_some_and(|c| c.is_uppercase())
            {
                out.extend(lemma_char.to_uppercase());
            } else {
                out.push(lemma_char);
            }
        }
        lemma_pos += 1;
        token_pos += 1;
    }
    Some(out)
}

fn are_lemmas_same(readings: &[AnalyzedToken]) -> bool {
    match readings.first().and_then(|r| r.stem.as_ref()) {
        Some(first) => readings.iter().all(|r| r.stem.as_ref() == Some(first)),
        None => readings.iter().all(|r| r.stem.is_none()),
    }
}

/// `CompoundCoherencyRule.match` over all sentences.
pub fn compound_coherency(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut norm_to_text_occurrences: HashMap<String, Vec<String>> = HashMap::new();
    for sentence in sentences {
        for atr in sentence.tokens.iter() {
            if atr.is_whitespace
                && !atr.is_sentence_start
                && !atr.is_sentence_end
                && !atr.is_paragraph_end
            {
                continue;
            }
            let token = atr.surface();
            if token.is_empty() {
                continue;
            }
            let lemma = compound_coherency_lemma(atr).unwrap_or_else(|| token.to_string());
            let norm_token = lemma.replace('-', "").to_lowercase();
            if norm_token.chars().all(|c| c.is_numeric()) && !norm_token.is_empty() {
                // avoid messages about "2-3" and "23" both being used
                break;
            }
            let text_occ = norm_to_text_occurrences.get(&norm_token).cloned();
            match text_occ {
                Some(occurrences) => {
                    if !occurrences
                        .iter()
                        .any(|f| f.to_lowercase() == lemma.to_lowercase())
                    {
                        let other = occurrences[0].clone();
                        if contains_hyphen_inside(&other) || contains_hyphen_inside(token) {
                            let msg = format!(
                                "Uneinheitliche Verwendung von Bindestrichen. Der Text enthält sowohl '{token}' als auch '{other}'."
                            );
                            let mut m = Match::new(
                                "DE_COMPOUND_COHERENCY",
                                Option::<String>::None,
                                msg,
                                Option::<String>::None,
                                TextRange::new(
                                    sentence.offset + atr.start_pos,
                                    sentence.offset + atr.end_pos(),
                                ),
                                Vec::<Suggestion>::new(),
                                "STYLE",
                                "Stil",
                            )
                            .with_metadata(
                                "Einheitliche Schreibweise bei Komposita (mit oder ohne Bindestrich)",
                                "style",
                                -1,
                            );
                            if token.replace('-', "").to_lowercase()
                                == other.replace('-', "").to_lowercase()
                            {
                                m.suggestions = vec![Suggestion {
                                    value: other,
                                    short_description: None,
                                }];
                            }
                            rule_matches.push(m);
                        }
                    }
                }
                None => {
                    norm_to_text_occurrences
                        .entry(norm_token)
                        .or_insert_with(|| vec![lemma]);
                }
            }
        }
    }
    rule_matches
}

fn contains_hyphen_inside(token: &str) -> bool {
    token.contains('-') && !token.starts_with('-') && !token.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn token(surface: &str, stem: Option<&str>, pos: Option<&str>) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            surface,
            stem.map(str::to_string),
            pos.map(str::to_string),
        )])
    }

    #[test]
    fn compound_coherency_lemma_fixes_hyphenated_tokens() {
        let atr = token("Jugend-Fotos", Some("Jugendfoto"), Some("SUB:GEN:PLU:NEU"));
        assert_eq!(
            compound_coherency_lemma(&atr).as_deref(),
            Some("Jugend-Foto")
        );
        let atr = token("Helpdesk", Some("Helpdesk"), Some("SUB:NOM:SIN:MAS"));
        assert_eq!(compound_coherency_lemma(&atr).as_deref(), Some("Helpdesk"));
        // different lemmas -> None
        let atr = AnalyzedTokenReadings::new(vec![
            AnalyzedToken::new("das", Some("der".into()), Some("ART".into())),
            AnalyzedToken::new("das", Some("das".into()), Some("PRO".into())),
        ]);
        assert_eq!(compound_coherency_lemma(&atr), None);
    }
}
