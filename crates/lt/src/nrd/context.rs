//! Nordum context rules: possessive agreement
//! (`NDM_PRON_POSS`). The noun's gender is inferred from the speller
//! dictionary's definite forms (`bilen` vs `bilet`), no POS dictionary is
//! needed.

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const POSSESSIVE_RULE_ID: &str = "NDM_PRON_POSS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NounGender {
    Common,
    Neuter,
}

/// Infer a singular noun's gender from its definite forms; `None` when both
/// or neither form is known.
fn infer_singular_gender(is_known: &dyn Fn(&str) -> bool, lower: &str) -> Option<NounGender> {
    if lower.chars().count() < 3 || !lower.chars().all(char::is_alphabetic) {
        return None;
    }
    let stem = lower
        .strip_suffix('e')
        .filter(|stem| stem.chars().count() >= 2);
    let common_defs: Vec<String> = match stem {
        Some(stem) => vec![format!("{stem}en"), format!("{lower}en")],
        None => vec![format!("{lower}en")],
    };
    let mut neuter_defs = vec![format!("{lower}et")];
    if let Some(stem) = stem {
        neuter_defs.push(format!("{stem}et"));
    }
    let common_known = common_defs.iter().any(|form| is_known(form));
    let neuter_known = neuter_defs.iter().any(|form| is_known(form));
    match (common_known, neuter_known) {
        (true, false) => Some(NounGender::Common),
        (false, true) => Some(NounGender::Neuter),
        _ => None,
    }
}

/// `NDM_PRON_POSS`: `min/din/vår/er` take `-t` before neuter nouns
/// (`mitt hus`), the `-t` forms drop it before common-gender nouns
/// (`min bil`).
pub fn check_possessives(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    is_known: &dyn Fn(&str) -> bool,
) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut matches = Vec::new();
    for i in 1..view.len().saturating_sub(1) {
        let possessive = view[i].surface().to_lowercase();
        let (replacement_common, replacement_neuter) = match possessive.as_str() {
            "min" => ("min", "mitt"),
            "mitt" => ("min", "mitt"),
            "din" => ("din", "ditt"),
            "ditt" => ("din", "ditt"),
            "vår" => ("vår", "vårt"),
            "vårt" => ("vår", "vårt"),
            "er" => ("er", "ert"),
            "ert" => ("er", "ert"),
            _ => continue,
        };
        let noun = view[i + 1].surface().to_lowercase();
        let Some(gender) = infer_singular_gender(is_known, &noun) else {
            continue;
        };
        let suggestion = match gender {
            NounGender::Common => replacement_common,
            NounGender::Neuter => replacement_neuter,
        };
        if suggestion == possessive {
            continue;
        }
        matches.push(
            Match::new(
                POSSESSIVE_RULE_ID,
                Option::<String>::None,
                "The possessive agrees with the noun: «min bil», «mitt hus».",
                Some("Possessive".to_string()),
                TextRange::new(
                    sentence_offset + view[i].start_pos,
                    sentence_offset + view[i].end_pos(),
                ),
                vec![Suggestion {
                    value: if view[i]
                        .surface()
                        .chars()
                        .next()
                        .is_some_and(char::is_uppercase)
                    {
                        let mut chars = suggestion.chars();
                        match chars.next() {
                            Some(first) => {
                                first.to_uppercase().collect::<String>() + chars.as_str()
                            }
                            None => String::new(),
                        }
                    } else {
                        suggestion.to_string()
                    },
                    short_description: None,
                }],
                "GRAMMAR",
                "Grammar",
            )
            .with_metadata("Possessive agreement", "grammar", 0),
        );
    }
    matches
}

pub const SPLIT_LEX_RULE_ID: &str = "NDM_COMPOUND_LEX";

const SPLIT_LEX_STOPWORDS: &[&str] = &[
    "jei", "du", "han", "hun", "vi", "ni", "de", "det", "den", "en", "ett", "er", "har", "går",
    "kan", "vil", "skal", "må", "bør", "og", "eller", "men", "som", "att", "om", "hvis", "når",
    "da", "til", "med", "på", "fra", "ved", "over", "under", "mot", "uten", "i", "av", "for",
    "ikke", "bare", "også", "nå", "her", "der", "mye", "mange", "noe", "noen", "alle", "min",
    "mitt", "mina", "din", "ditt", "dina", "vår", "vårt", "våra", "hans", "hennas", "deras", "seg",
    "mei", "dei", "sei", "oss", "dem", "første", "andre", "neste", "siste", "samme", "ganske",
    "veldig", "helt",
];

const SPLIT_LEX_WHITELIST: &[(&str, &str)] = &[("god", "morgen"), ("god", "kveld")];

/// `NDM_COMPOUND_LEX` (default-off, owner decision): a lowercase pair whose
/// concatenation is a known Nordum word is a likely særskriving. Same guards
/// as the Norwegian rule: both words known, the first not a stopword, length
/// limits and a whitelist.
pub fn check_split_compound_lex(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    is_known: &dyn Fn(&str) -> bool,
) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut matches = Vec::new();
    for i in 1..view.len().saturating_sub(1) {
        let first = view[i].surface().to_lowercase();
        let second = view[i + 1].surface().to_lowercase();
        if first.chars().count() < 3
            || second.chars().count() < 4
            || !first.chars().all(char::is_alphabetic)
            || !second.chars().all(char::is_alphabetic)
            || !first.chars().all(char::is_lowercase)
            || !second.chars().all(char::is_lowercase)
        {
            continue;
        }
        if SPLIT_LEX_STOPWORDS.contains(&first.as_str())
            || SPLIT_LEX_WHITELIST.contains(&(first.as_str(), second.as_str()))
        {
            continue;
        }
        let joined = format!("{first}{second}");
        if joined.chars().count() < 7 {
            continue;
        }
        if !(is_known(&joined) && is_known(&first) && is_known(&second)) {
            continue;
        }
        matches.push(
            Match::new(
                SPLIT_LEX_RULE_ID,
                Option::<String>::None,
                "Possible split compound: write it as one word.",
                Some("Compound".to_string()),
                TextRange::new(
                    sentence_offset + view[i].start_pos,
                    sentence_offset + view[i + 1].end_pos(),
                ),
                vec![Suggestion {
                    value: joined,
                    short_description: None,
                }],
                "TYPOS",
                "Possible Typo",
            )
            .with_metadata(
                "Possible split compound (dictionary-driven)",
                "misspelling",
                0,
            ),
        );
    }
    matches
}
