//! Norwegian Bokmål context rules that need more than a surface XML pattern:
//! the spurious comma between a fronted prepositional phrase and the finite
//! verb (`NB_COMMA_PP_VERB`, owner decision D12: an error where Språkrådet
//! prescribes it).

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, TextRange};

pub const COMMA_PP_RULE_ID: &str = "NB_COMMA_PP_VERB";

const PREPOSITIONS: &[&str] = &[
    "etter", "i", "på", "med", "for", "ved", "fra", "til", "over", "under", "mellom", "gjennom",
    "uten", "blant", "hos", "rundt", "mot", "av", "om", "inn", "ut", "opp", "ned",
];

const CLAUSE_MARKERS: &[&str] = &[
    "og", "eller", "men", "fordi", "at", "hvis", "når", "da", "som", "mens", "siden", "dersom",
    "ettersom", "jeg", "du", "han", "hun", "vi", "dere", "de", "det", "den",
];

/// Finite-verb heuristic: `-r` endings, the modals and the frequent
/// irregulars (same lists the XML rules use).
fn is_finite_verb_like(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.len() >= 3
        && (lower.ends_with('r')
            || lower.ends_with("et")
            || lower.ends_with("te")
            || lower.ends_with("de"))
    {
        return true;
    }
    matches!(
        lower.as_str(),
        "ville"
            | "kunne"
            | "skulle"
            | "måtte"
            | "burde"
            | "kan"
            | "vil"
            | "skal"
            | "må"
            | "bør"
            | "får"
            | "kom"
            | "gikk"
            | "så"
            | "satt"
            | "lå"
            | "sto"
            | "var"
            | "hadde"
            | "ble"
            | "fikk"
            | "tok"
            | "ga"
            | "sa"
    )
}

/// `NB_COMMA_PP_VERB`: a comma directly before the finite verb is wrong when
/// the fronted element is a prepositional phrase, not a clause.
pub fn check_comma_pp(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut matches = Vec::new();
    for i in 1..view.len() {
        if view[i].surface() != "," {
            continue;
        }
        let Some(next) = view.get(i + 1) else {
            continue;
        };
        if !is_finite_verb_like(next.surface()) {
            continue;
        }
        // Walk back over the fronted phrase; a clause marker or another
        // comma means this is not a plain prepositional phrase. A sentence
        // start ends the phrase.
        let mut words: Vec<String> = Vec::new();
        let mut j = i;
        let mut abort = false;
        while j > 0 && words.len() < 6 {
            j -= 1;
            if view[j].is_sentence_start {
                break;
            }
            let surface = view[j].surface();
            if surface == "," {
                abort = true;
                break;
            }
            if !surface.chars().next().is_some_and(|c| c.is_alphabetic()) {
                abort = true;
                break;
            }
            let lower = surface.to_lowercase();
            if CLAUSE_MARKERS.contains(&lower.as_str()) {
                abort = true;
                break;
            }
            words.push(lower);
        }
        if abort || words.is_empty() {
            continue;
        }
        let first = words.last().expect("non-empty");
        if !PREPOSITIONS.contains(&first.as_str()) {
            continue;
        }
        matches.push(
            Match::new(
                COMMA_PP_RULE_ID,
                Option::<String>::None,
                "Det skal ikke stå komma mellom et foranstilt adverbial og det finitte verbet.",
                Some("Komma".to_string()),
                TextRange::new(
                    sentence_offset + view[i].start_pos,
                    sentence_offset + view[i].end_pos(),
                ),
                Vec::new(),
                "PUNCTUATION",
                "Tegnsetting",
            )
            .with_metadata("Komma mellom adverbial og verb", "typographical", 0),
        );
    }
    matches
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NounGender {
    Common,
    Neuter,
}

/// Curated overrides for nouns whose definite forms are ambiguous in the
/// speller dictionary (`data/no/words/gender_overrides.txt`).
pub type GenderOverrides = HashMap<String, NounGender>;

/// Load the gender overrides (`noun c` / `noun n` per line).
pub fn load_gender_overrides(data_dir: &Path) -> Result<GenderOverrides> {
    let mut overrides = GenderOverrides::new();
    let path = data_dir.join("no/words/gender_overrides.txt");
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Ok(overrides);
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let (Some(noun), Some(gender)) = (parts.next(), parts.next()) else {
            continue;
        };
        let gender = match gender {
            "c" => NounGender::Common,
            "n" => NounGender::Neuter,
            _ => continue,
        };
        overrides.insert(noun.to_lowercase(), gender);
    }
    Ok(overrides)
}

/// Infer a singular noun's gender: the curated override first, then the
/// speller dictionary's definite forms (`jenten`/`jentet`, `eplet`). `None`
/// when both or neither form is known — the rules then stay silent.
pub fn infer_singular_gender(
    is_known: &dyn Fn(&str) -> bool,
    overrides: &GenderOverrides,
    lower: &str,
) -> Option<NounGender> {
    if let Some(gender) = overrides.get(lower) {
        return Some(*gender);
    }
    if lower.chars().count() < 3 || !lower.chars().all(char::is_alphabetic) {
        return None;
    }
    // `-e` nouns drop the vowel (`jente` → `jenten`, `eple` → `eplet`).
    let stem = lower
        .strip_suffix('e')
        .filter(|stem| stem.chars().count() >= 2);
    let common_defs: Vec<String> = match stem {
        Some(stem) => vec![format!("{stem}en")],
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

pub const SIN_HANS_RULE_ID: &str = "NB_SIN_HANS";

/// `NB_SIN_HANS` (suggestion): a third-person possessive (`hans`/`hennes`/
/// `deres`) that refers to the clause subject should normally be the
/// reflexive `sin/sitt`. The gender/number of the following noun decides the
/// form; singular common/neuter is inferred from the speller's definite
/// forms, anything else stays silent.
pub fn check_sin_hans(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    is_known: &dyn Fn(&str) -> bool,
    overrides: &GenderOverrides,
) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut matches = Vec::new();
    for i in 1..view.len().saturating_sub(1) {
        let possessive = view[i].surface().to_lowercase();
        let subject = match possessive.as_str() {
            "hans" => "han",
            "hennes" => "hun",
            "deres" => "de",
            _ => continue,
        };
        let noun = view[i + 1].surface().to_lowercase();
        let Some(gender) = infer_singular_gender(is_known, overrides, &noun) else {
            continue;
        };
        if !clause_subject_is(&view, i, subject) {
            continue;
        }
        let suggestion = match gender {
            NounGender::Common => "sin",
            NounGender::Neuter => "sitt",
        };
        matches.push(
            Match::new(
                SIN_HANS_RULE_ID,
                Option::<String>::None,
                "Når eieren er setningens subjekt, brukes «sin/sitt/sine»: «bilen sin».",
                Some("sin/hans".to_string()),
                TextRange::new(
                    sentence_offset + view[i].start_pos,
                    sentence_offset + view[i].end_pos(),
                ),
                vec![lt_core::Suggestion {
                    value: suggestion.to_string(),
                    short_description: None,
                }],
                "GRAMMAR",
                "Grammatikk",
            )
            .with_metadata("Refleksivt eiendomsord", "grammar", 0),
        );
    }
    matches
}

pub const SEG_RULE_ID: &str = "NB_SEG_REFLEX";

/// `NB_SEG_REFLEX` (suggestion): `ham/henne/dem selv` referring back to the
/// clause subject is written `seg selv`.
pub fn check_seg_reflex(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut matches = Vec::new();
    for i in 1..view.len().saturating_sub(1) {
        if view[i].surface().to_lowercase() != "selv" {
            continue;
        }
        let object = view[i - 1].surface().to_lowercase();
        let subject = match object.as_str() {
            "ham" => "han",
            "henne" => "hun",
            "dem" => "de",
            _ => continue,
        };
        if !clause_subject_is(&view, i - 1, subject) {
            continue;
        }
        matches.push(
            Match::new(
                SEG_RULE_ID,
                Option::<String>::None,
                "Når objektet viser tilbake på subjektet, brukes «seg»: «seg selv».",
                Some("seg".to_string()),
                TextRange::new(
                    sentence_offset + view[i - 1].start_pos,
                    sentence_offset + view[i - 1].end_pos(),
                ),
                vec![lt_core::Suggestion {
                    value: "seg".to_string(),
                    short_description: None,
                }],
                "GRAMMAR",
                "Grammatikk",
            )
            .with_metadata("Refleksivt pronomen", "grammar", 0),
        );
    }
    matches
}

/// True when the clause containing `position` has `subject` as its subject
/// pronoun (scan stops at sentence start, punctuation or conjunctions, and at
/// another person's subject pronoun).
fn clause_subject_is(view: &[&AnalyzedTokenReadings], position: usize, subject: &str) -> bool {
    let mut j = position;
    while j > 0 {
        j -= 1;
        if view[j].is_sentence_start {
            return false;
        }
        let surface = view[j].surface().to_lowercase();
        if matches!(
            surface.as_str(),
            "," | "." | ";" | ":" | "!" | "?" | "og" | "men" | "eller"
        ) {
            return false;
        }
        if matches!(
            surface.as_str(),
            "at" | "fordi" | "hvis" | "om" | "når" | "da" | "som"
        ) {
            return false;
        }
        if surface == subject {
            return true;
        }
        if matches!(
            surface.as_str(),
            "jeg" | "du" | "han" | "hun" | "vi" | "dere" | "de" | "det" | "den"
        ) {
            return false;
        }
    }
    false
}

pub const GENDER_RULE_ID: &str = "NB_EN_ET_GENDER";

/// `NB_EN_ET_GENDER`: the indefinite article must match the noun's gender.
/// The gender is inferred from the speller dictionary's definite forms
/// (`jenten` vs `jentet`), so no POS dictionary is needed; the rule only
/// fires when exactly one of the two definite forms is known.
pub fn check_en_et_gender(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    is_known: &dyn Fn(&str) -> bool,
    overrides: &GenderOverrides,
) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut matches = Vec::new();
    for i in 1..view.len().saturating_sub(1) {
        let article = view[i].surface().to_lowercase();
        if article != "en" && article != "et" {
            continue;
        }
        let lower = view[i + 1].surface().to_lowercase();
        if lower.chars().count() < 3 || !lower.chars().all(char::is_alphabetic) {
            continue;
        }
        let Some(gender) = infer_singular_gender(is_known, overrides, &lower) else {
            continue;
        };
        let suggestion = match (article.as_str(), gender) {
            ("en", NounGender::Neuter) => "et",
            ("et", NounGender::Common) => "en",
            _ => continue,
        };
        matches.push(
            Match::new(
                GENDER_RULE_ID,
                Option::<String>::None,
                "Artikkelen må samsvare med substantivets kjønn: «et hus», «en bil».",
                Some("Kjønn".to_string()),
                TextRange::new(
                    sentence_offset + view[i].start_pos,
                    sentence_offset + view[i].end_pos(),
                ),
                vec![lt_core::Suggestion {
                    value: suggestion.to_string(),
                    short_description: None,
                }],
                "GRAMMAR",
                "Grammatikk",
            )
            .with_metadata("Feil artikkel for substantivets kjønn", "grammar", 0),
        );
    }
    matches
}

pub const SPLIT_LEX_RULE_ID: &str = "NB_SPLIT_COMPOUND_LEX";

/// Words that never start a split-compound candidate (finite verbs, function
/// words, pronouns, determiners); without this guard pairs like
/// «har en» → «haren» would be flagged.
const SPLIT_LEX_STOPWORDS: &[&str] = &[
    "har", "er", "var", "kan", "skal", "vil", "må", "bør", "fikk", "tok", "ga", "sa", "kom",
    "gikk", "ble", "hadde", "satt", "lå", "sto", "så", "og", "men", "eller", "som", "at", "om",
    "for", "til", "med", "fra", "ved", "over", "under", "mot", "uten", "inn", "ut", "opp", "ned",
    "jeg", "du", "han", "hun", "vi", "dere", "de", "det", "den", "en", "et", "ei", "seg", "meg",
    "deg", "oss", "dem", "min", "din", "sin", "vår", "deres", "hans", "hennes", "ikke", "bare",
    "også", "nå", "da", "her", "der", "mye", "mange", "noe", "noen", "alle", "hver", "annen",
    "denne", "disse", "sånn", "slik", "hva", "hvem", "hvor", "når", "hvordan", "hvorfor", "første",
    "andre", "neste", "siste", "samme", "egen", "eneste", "ganske", "veldig", "svært", "helt",
    "bare",
];

/// Correct two-word phrases whose concatenation happens to be a dictionary
/// word.
const SPLIT_LEX_WHITELIST: &[(&str, &str)] = &[
    ("god", "morgen"),
    ("god", "kveld"),
    ("god", "natt"),
    ("god", "dag"),
    ("den", "samme"),
    ("det", "samme"),
    ("til", "stede"),
    ("til", "sammen"),
];

/// `NB_SPLIT_COMPOUND_LEX` (default-off, owner decision): a lowercase pair
/// whose concatenation is a known word is a likely særskriving. Guards: both
/// words known, the first not a stopword, length limits, and a whitelist of
/// correct phrases. False positives are expected without POS, hence
/// default-off.
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
                "Særskriving: dette skal sannsynligvis skrives i ett ord.",
                Some("Særskriving".to_string()),
                TextRange::new(
                    sentence_offset + view[i].start_pos,
                    sentence_offset + view[i + 1].end_pos(),
                ),
                vec![lt_core::Suggestion {
                    value: joined,
                    short_description: None,
                }],
                "TYPOS",
                "Mulig skrivefeil",
            )
            .with_metadata("Mulig særskriving (leksikondrevet)", "misspelling", 0),
        );
    }
    matches
}
