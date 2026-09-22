//! Port of `org.languagetool.rules.uk.MixedAlphabetsRule` (`UK_MIXED_ALPHABETS`):
//! a Latin letter used inside a Cyrillic word (or vice versa).

use std::collections::HashMap;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "UK_MIXED_ALPHABETS";
const DESCRIPTION: &str = "Змішування кирилиці й латиниці";
const SHORT: &str = "Мішанина розкладок";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";

static LIKELY_LATIN_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[XVIХІ]{2,8}(?:-[а-яіїє]{1,3})?$").unwrap());
static LATIN_NUMBER_WITH_CYRILLICS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:Х{1,3}І{1,3}|І{1,3}Х{1,3}|Х{2,3}|І{2,3})(?:-[а-яіїє]{1,4})?$").unwrap()
});
static MIXED_ALPHABETS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^.*(?:[a-zA-ZïáÁéÉíÍḯḮóÓúýÝ]'?[а-яіїєґА-ЯІЇЄҐ]|[а-яіїєґА-ЯІЇЄҐ]'?[a-zA-ZïáÁéÉíÍḯḮóÓúýÝ]).*$")
        .unwrap()
});
static CYRILLIC_ONLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*[бвгґдєжзийїлнпфцчшщьюяБГҐДЄЖЗИЙЇЛПФЦЧШЩЬЮЯ].*$").unwrap());
static LATIN_ONLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*[bdfghjlqrstvzDFGJLNQRSUVZ].*$").unwrap());
static COMMON_CYR_LETTERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[АВЕІКОРСТУХ]+$").unwrap());
static CYRILLIC_FIRST_LETTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[а-яіїєґА-ЯІЇЄҐ].*$").unwrap());
static FORMULA_LETTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[xbB]$").unwrap());
static INVALID_SUFFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-[а-яіїє]{1,4}").unwrap());
static ROMAN_WITH_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[IVXІХ]+-[а-яіїє]{1,4}$").unwrap());
static FNAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?!.*:abbr).*fname.*$").unwrap());

const CYR_CHARS: &str = "аеіїкморстухАВЕІКМНОРСТУХ";
const LAT_CHARS: &str = "aeiïkmopctyxABEIKMHOPCTYX";
const UMLAUTS: &[&str] = &[
    "á", "Á", "é", "É", "í", "Í", "ḯ", "Ḯ", "ó", "Ó", "ú", "ý", "Ý",
];
const UMLAUTS_REPLACE: &[&str] = &[
    "а́", "А́", "е́", "Е́", "і́", "І́", "ї́", "Ї́", "о́", "О́", "и́", "у́", "У́",
];

fn to_lat_map() -> &'static HashMap<char, char> {
    static MAP: LazyLock<HashMap<char, char>> =
        LazyLock::new(|| CYR_CHARS.chars().zip(LAT_CHARS.chars()).collect());
    &MAP
}

fn to_cyr_map() -> &'static HashMap<char, char> {
    static MAP: LazyLock<HashMap<char, char>> =
        LazyLock::new(|| LAT_CHARS.chars().zip(CYR_CHARS.chars()).collect());
    &MAP
}

fn to_cyrillic(word: &str) -> String {
    let mut out: String = word
        .chars()
        .map(|c| *to_cyr_map().get(&c).unwrap_or(&c))
        .collect();
    for (from, to) in UMLAUTS.iter().zip(UMLAUTS_REPLACE.iter()) {
        out = out.replace(from, to);
    }
    out
}

fn to_latin(word: &str) -> String {
    word.chars()
        .map(|c| *to_lat_map().get(&c).unwrap_or(&c))
        .collect()
}

fn adjust_for_invalid_suffix(token: &str) -> String {
    if token.contains('-') {
        INVALID_SUFFIX_RE.replace(token, "").into_owned()
    } else {
        token.to_string()
    }
}

fn adjust_for_invalid_suffix_msg(token: &str, msg: &str) -> String {
    if token.contains('-') && ROMAN_WITH_SUFFIX.is_match(token).unwrap_or(false) {
        format!("{msg}. Також: до римських цифр букви не дописуються.")
    } else {
        msg.to_string()
    }
}

fn to_latin_left_only(token: &str) -> String {
    match token.split_once('-') {
        Some((left, right)) => format!("{}-{right}", to_latin(left)),
        None => to_latin(token),
    }
}

fn add_match(
    out: &mut Vec<Match>,
    readings: &AnalyzedTokenReadings,
    replacements: Vec<String>,
    msg: &str,
    offset: usize,
) {
    out.push(
        Match::new(
            RULE_ID,
            Option::<String>::None,
            msg,
            Some(SHORT.to_string()),
            TextRange::new(offset + readings.start_pos, offset + readings.end_pos()),
            replacements
                .into_iter()
                .map(|value| Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "misspelling", 0),
    );
}

fn has_tag_start(readings: &[lt_core::AnalyzedToken], prefix: &str) -> bool {
    readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(prefix)))
}

fn likely_bad_latin_i(tokens: &[&AnalyzedTokenReadings], i: usize) -> bool {
    if i == 0 {
        return false;
    }
    let prev = tokens[i - 1].surface();
    let next = tokens.get(i + 1).map(|t| t.surface());
    (lt_tagger::uk_helpers::is_capitalized(prev)
        || (has_tag_start(&tokens[i - 1].readings, "prep")
            && next.is_some_and(|n| !lt_tagger::uk_helpers::is_all_uppercase_uk(n))))
        || next.is_some_and(|n| ["ст.", "тис."].contains(&n))
        || next.is_some_and(|n| ["квартал", "півріччя", "тисячоліття", "половина"].contains(&n))
}

/// `MixedAlphabetsRule.match` over one sentence.
pub fn check_sentence_uk(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens.iter().filter(|t| !t.is_whitespace).collect();
    let mut out = Vec::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        let has_formula = view
            .iter()
            .any(|t| FORMULA_LETTERS.is_match(t.surface()).unwrap_or(false));

        if i < view.len() - 1
            && (token == "i" || token == "y" || token == "a" || (token == "A" && i == 1))
            && CYRILLIC_FIRST_LETTER
                .is_match(view[i + 1].surface())
                .unwrap_or(false)
            && !has_formula
        {
            let msg = format!("Вжито латинську «{token}» замість кириличної");
            add_match(
                &mut out,
                view[i],
                vec![to_cyrillic(&token)],
                &msg,
                sentence_offset,
            );
        } else if (token == "І" && likely_bad_latin_i(&view, i))
            || (token == "І."
                && i > 1
                && view[i - 1].surface() != "Тому"
                && view[i - 1].surface() != "Франко"
                && view[i - 1].readings.iter().any(|r| {
                    r.pos_tag
                        .as_deref()
                        .is_some_and(|t| FNAME.is_match(t).unwrap_or(false))
                }))
        {
            add_match(
                &mut out,
                view[i],
                vec![to_latin(&token)],
                "Вжито кириличну літеру замість латинської",
                sentence_offset,
            );
        } else if COMMON_CYR_LETTERS.is_match(&token).unwrap_or(false) {
            let prev_lemma = view[i - 1]
                .readings
                .first()
                .and_then(|r| r.stem.clone())
                .unwrap_or_default();
            if ["гепатит", "група", "турнір"].contains(&prev_lemma.as_str()) {
                add_match(
                    &mut out,
                    view[i],
                    vec![to_latin(&token)],
                    "Вжито кириличну літеру замість латинської",
                    sentence_offset,
                );
            }
        }

        if token.chars().count() < 2 {
            if token == "°" && i < view.len() - 1 && view[i + 1].surface() == "С" {
                add_match(
                    &mut out,
                    view[i + 1],
                    vec!["C".to_string()],
                    "Вжито кириличну літеру замість латинської",
                    sentence_offset,
                );
            }
            continue;
        }

        if MIXED_ALPHABETS.is_match(&token).unwrap_or(false) {
            let mut msg = "Вжито кириличні й латинські літери в одному слові".to_string();
            let mut replacements = Vec::new();
            let likely_latin_number = LIKELY_LATIN_NUMBER.is_match(&token).unwrap_or(false);
            if !LATIN_ONLY.is_match(&token).unwrap_or(false) && !likely_latin_number {
                replacements.push(to_cyrillic(&token));
            }
            if (token.chars().count() > 2 && !CYRILLIC_ONLY.is_match(&token).unwrap_or(false))
                || likely_latin_number
            {
                let converted = adjust_for_invalid_suffix(&to_latin_left_only(&token));
                replacements.push(converted);
                msg = "Вжито кириличні літери замість латинських".to_string();
                msg = adjust_for_invalid_suffix_msg(&token, &msg);
            }
            if !replacements.is_empty() {
                add_match(&mut out, view[i], replacements, &msg, sentence_offset);
            }
        } else if LATIN_NUMBER_WITH_CYRILLICS
            .is_match(&token)
            .unwrap_or(false)
        {
            let converted = adjust_for_invalid_suffix(&to_latin_left_only(&token));
            let msg = adjust_for_invalid_suffix_msg(
                &token,
                "Вжито кириличні літери замість латинських на позначення римської цифри",
            );
            add_match(&mut out, view[i], vec![converted], &msg, sentence_offset);
        }

        if (token.contains('\u{0306}') || token.contains('\u{0308}'))
            && (token.contains("и\u{0306}") || token.contains("і\u{0308}"))
        {
            let fix = token.replace("и\u{0306}", "й").replace("і\u{0308}", "ї");
            add_match(
                &mut out,
                view[i],
                vec![fix],
                "Вжито комбіновані символи замість українських літер",
                sentence_offset,
            );
        }
    }
    out
}
