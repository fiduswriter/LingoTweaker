//! Ukrainian tagging helpers: `PosTagHelper`, `LetterEndingForNumericHelper`
//! and the `LemmaHelper` text functions used by `UkrainianTagger`.

use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings};

/// `LemmaHelper.IGNORE_CHARS` (`\u00AD\u0301`).
const IGNORE_CHARS: &str = "\u{00AD}\u{0301}";

/// Java `Matcher.matches()`: the match must cover the whole string.
pub(crate) fn full_match(re: &fancy_regex::Regex, s: &str) -> bool {
    match re.find(s) {
        Ok(Some(m)) => m.start() == 0 && m.end() == s.len(),
        _ => false,
    }
}

/// `LemmaHelper.isAllUppercaseUk`.
pub fn is_all_uppercase_uk(word: &str) -> bool {
    for ch in word.chars() {
        if ch != '-'
            && ch != '\u{2013}'
            && ch != '\''
            && ch != '\u{0301}'
            && ch != '\u{00AD}'
            && !ch.is_uppercase()
        {
            return false;
        }
    }
    true
}

/// `LemmaHelper.capitalizeProperName`.
pub fn capitalize_proper_name(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut prev_char = '-';
    for ch in word.chars() {
        if prev_char == '-' {
            out.extend(ch.to_uppercase());
        } else {
            out.extend(ch.to_lowercase());
        }
        prev_char = if ch == '\u{2013}' { '-' } else { ch };
    }
    out
}

/// `LemmaHelper.isCapitalized` (`\u{2013}` is normalized to `-` inside).
pub fn is_capitalized(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 2 {
        return false;
    }
    let char0 = chars[0];
    if !char0.is_uppercase() {
        return false;
    }
    // lax on Latin: EuroGas
    if char0.is_ascii_uppercase() && chars[1].is_lowercase() {
        return true;
    }
    let mut prev_dash = false;
    let sz = chars.len();
    for i in 1..sz {
        let ch = chars[i];
        if IGNORE_CHARS.contains(ch) {
            continue;
        }
        let dash = ch == '-' || ch == '\u{2013}';
        if dash {
            if i == sz - 2 && chars[i + 1].is_ascii_digit() {
                return true;
            }
            prev_dash = true;
            continue;
        }
        if ch != '\'' && ch != '\u{0301}' && ch != '\u{00AD}' && (prev_dash != ch.is_uppercase()) {
            return false;
        }
        prev_dash = false;
    }
    true
}

/// `PosTagHelper.addIfNotContains(tag, addTag)`.
pub fn add_if_not_contains(tag: &str, add_tag: &str) -> String {
    if !add_tag.is_empty() && !tag.contains(add_tag) {
        format!("{tag}{add_tag}")
    } else {
        tag.to_string()
    }
}

/// `LetterEndingForNumericHelper.isPossibleAdjAdjEnding`.
pub fn is_possible_adj_adj_ending(right_word: &str) -> bool {
    matches!(
        right_word,
        "й" | "ий"
            | "ій"
            | "го"
            | "му"
            | "ма"
            | "м"
            | "им"
            | "ім"
            | "а"
            | "ва"
            | "ша"
            | "га"
            | "тя"
            | "я"
            | "та"
            | "ї"
            | "ої"
            | "у"
            | "шу"
            | "гу"
            | "ту"
            | "тю"
            | "ою"
            | "ю"
            | "е"
            | "є"
            | "ше"
            | "ге"
            | "тє"
            | "те"
            | "ме"
            | "і"
            | "ті"
            | "ні"
            | "ми"
            | "х"
            | "их"
            | "ві"
            | "тій"
            | "мій"
            | "мою"
            | "тою"
            | "тої"
            | "того"
            | "тього"
            | "тому"
            | "тьому"
            | "тими"
            | "тім"
            | "мої"
            | "тий"
            | "мий"
            | "тих"
            | "ого"
            | "ому"
            | "тим"
            | "ома"
            | "ший"
            | "гій"
    )
}

/// `LetterEndingForNumericHelper.isPossibleAdjAdjEnding` and the noun map.
pub fn is_possible_noun_noun_ending(right_word: &str) -> bool {
    matches!(
        right_word,
        "ти" | "ці" | "ма" | "ми" | "ох" | "ві" | "ть" | "ка"
    )
}

/// `PosTagHelper.VIDMINKY_MAP` keys in Java `LinkedHashMap` order.
pub const VIDMINKY: &[&str] = &[
    "v_naz", "v_rod", "v_dav", "v_zna", "v_oru", "v_mis", "v_kly",
];

/// `PosTagHelper.BASE_GENDERS`.
pub const BASE_GENDERS: &[&str] = &["m", "f", "n", "p"];

/// `PosTagHelper.generateTokensForNv`.
pub fn generate_tokens_for_nv(
    word: &str,
    genders: &str,
    extra_tags: Option<&str>,
) -> Vec<AnalyzedToken> {
    let mut out = Vec::new();
    for gen in genders.chars() {
        let pos_tag_base = format!("noun:inanim:{gen}:");
        for vidm in VIDMINKY {
            if *vidm == "v_kly" {
                continue;
            }
            let mut pos_tag = format!("{pos_tag_base}{vidm}:nv");
            if let Some(extra) = extra_tags {
                pos_tag.push_str(extra);
            }
            out.push(AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some(pos_tag),
            ));
        }
    }
    out
}

/// One `LetterEndingForNumericHelper.RegexToCaseList` entry: an optional
/// full-match regex (empty = always) plus the case tags.
type CaseList = (&'static str, &'static [&'static str]);

fn adj_ending_map(right_word: &str) -> Option<&'static [CaseList]> {
    Some(match right_word {
        "й" => &[(
            "",
            &[":m:v_naz", ":m:v_zna:rinanim", ":f:v_dav", ":f:v_mis"],
        )],
        "ий" => &[("", &[":m:v_naz", ":m:v_zna:rinanim"])],
        "ій" => &[
            (".*([^3]|13)", &[":f:v_dav", ":f:v_mis"]),
            (
                "",
                &[":m:v_naz", ":m:v_zna:rinanim", ":f:v_dav", ":f:v_mis"],
            ),
        ],
        "го" => &[("", &[":m:v_rod", ":m:v_zna:ranim", ":n:v_rod"])],
        "му" => &[
            (
                ".*(?!<1)7",
                &[":m:v_dav", ":m:v_mis", ":n:v_dav", ":n:v_mis", ":f:v_zna"],
            ),
            (
                ".*(?!<1)8",
                &[":f:v_zna", ":m:v_dav", ":m:v_mis", ":n:v_dav", ":n:v_mis"],
            ),
            ("", &[":m:v_dav", ":m:v_mis", ":n:v_dav", ":n:v_mis"]),
        ],
        "ма" => &[(".*(?!<1)[78]", &[":f:v_naz"])],
        "м" => &[("", &[":m:v_oru", ":n:v_oru", ":p:v_dav"])],
        "им" => &[("", &[":m:v_oru", ":n:v_oru", ":p:v_dav"])],
        "ім" => &[
            (
                ".*(?!<1)3",
                &[":m:v_oru", ":m:v_mis", ":n:v_oru", ":n:v_mis"],
            ),
            ("", &[":m:v_mis", ":n:v_oru", ":n:v_mis"]),
        ],
        "а" => &[("", &[":f:v_naz"])],
        "ва" => &[("", &[":f:v_naz"])],
        "ша" => &[("", &[":f:v_naz"])],
        "га" => &[("", &[":f:v_naz"])],
        "тя" => &[("", &[":f:v_naz"])],
        "я" => &[(".*(?!<1)3", &[":f:v_naz"])],
        "та" => &[("", &[":f:v_naz"])],
        "ї" => &[("", &[":f:v_rod"])],
        "ої" => &[("", &[":f:v_rod"])],
        "у" => &[("", &[":f:v_zna"])],
        "шу" => &[("", &[":f:v_zna"])],
        "гу" => &[("", &[":f:v_zna"])],
        "ту" => &[("", &[":f:v_zna"])],
        "тю" => &[("", &[":f:v_zna"])],
        "ою" => &[("", &[":f:v_oru"])],
        "ю" => &[
            (".*([^3]|13)", &[":f:v_oru"]),
            ("", &[":f:v_zna", ":f:v_oru"]),
        ],
        "е" => &[("", &[":n:v_naz", ":n:v_zna"])],
        "є" => &[("", &[":n:v_naz", ":n:v_zna"])],
        "ше" => &[("", &[":n:v_naz", ":n:v_zna"])],
        "ге" => &[("", &[":n:v_naz", ":n:v_zna"])],
        "тє" => &[("", &[":n:v_naz", ":n:v_zna"])],
        "те" => &[("", &[":n:v_naz", ":n:v_zna"])],
        "ме" => &[(".*(?!<1)[78]", &[":n:v_naz", ":n:v_zna"])],
        "і" => &[("", &[":p:v_naz", ":p:v_zna:rinanim"])],
        "ті" => &[("", &[":p:v_naz", ":p:v_zna:rinanim"])],
        "ні" => &[("", &[":p:v_naz", ":p:v_zna:rinanim"])],
        "ми" => &[("", &[":p:v_oru"])],
        "х" => &[("", &[":p:v_rod", ":p:v_zna:ranim", ":p:v_mis"])],
        "их" => &[("", &[":p:v_rod", ":p:v_zna:ranim", ":p:v_mis"])],
        "ві" => &[
            (".*40", &[":p:v_naz", ":p:v_zna:rinanim"]),
            (".*%", &[":p:v_naz", ":p:v_zna:rinanim"]),
        ],
        "тій" => &[
            (".*([^3]|13)", &[":f:v_dav:bad", ":f:v_mis:bad"]),
            (
                "",
                &[
                    ":m:v_naz:bad",
                    ":m:v_zna:rinanim:bad",
                    ":f:v_dav:bad",
                    ":f:v_mis:bad",
                ],
            ),
        ],
        "мій" => &[("", &[":f:v_dav:bad", ":f:v_mis:bad"])],
        "мою" => &[("", &[":f:v_oru:bad"])],
        "тою" => &[("", &[":f:v_oru:bad"])],
        "тої" => &[("", &[":f:v_rod:bad"])],
        "того" => &[("", &[":m:v_rod:bad", ":n:v_rod:bad"])],
        "тього" => &[("", &[":m:v_rod:bad", ":n:v_rod:bad"])],
        "тому" => &[(
            "",
            &[
                ":m:v_dav:bad",
                ":m:v_mis:bad",
                ":n:v_rod:bad",
                ":n:v_mis:bad",
            ],
        )],
        "тьому" => &[(
            "",
            &[
                ":m:v_dav:bad",
                ":m:v_mis:bad",
                ":n:v_rod:bad",
                ":n:v_mis:bad",
            ],
        )],
        "тими" => &[("", &[":p:v_oru:bad"])],
        "тім" => &[("", &[":m:v_mis:bad", ":n:v_mis:bad"])],
        "мої" => &[("", &[":f:v_rod:bad"])],
        "тий" => &[("", &[":m:v_naz:bad", ":m:v_zna:rinanim:bad"])],
        "мий" => &[("", &[":m:v_naz:bad", ":m:v_zna:rinanim:bad"])],
        "тих" => &[("", &[":p:v_rod:bad", ":p:v_mis:bad"])],
        "ого" => &[("", &[":m:v_rod:bad", ":m:v_zna:ranim:bad", ":n:v_rod:bad"])],
        "ому" => &[(
            "",
            &[
                ":m:v_dav:bad",
                ":m:v_mis:bad",
                ":n:v_dav:bad",
                ":n:v_mis:bad",
            ],
        )],
        "тим" => &[("", &[":m:v_oru:bad", ":n:v_oru:bad", ":p:v_dav:bad"])],
        "ома" => &[("", &[":f:v_naz:bad", ":p:v_oru:bad"])],
        "ший" => &[("", &[":m:v_naz:bad", ":m:v_zna:rinanim:bad"])],
        "гій" => &[("", &[":f:v_mis:bad", ":f:v_dav:bad"])],
        _ => return None,
    })
}

fn noun_ending_map(right_word: &str) -> Option<&'static [CaseList]> {
    Some(match right_word {
        "ти" => &[(
            ".*([0569]|1[0-9])",
            &[":p:v_rod:bad", ":p:v_dav:bad", ":p:v_mis:bad"],
        )],
        "ці" => &[(".*([03456789]|1[0-9])", &[":f:v_dav:bad", ":f:v_mis:bad"])],
        "ма" => &[(".*([023456789]|1[0-9])", &[":p:v_oru:bad"])],
        "ми" => &[("", &[":p:v_rod:bad", ":p:v_mis:bad"])],
        "ох" => &[("", &[":p:v_rod:bad", ":p:v_zna:ranim:bad"])],
        "ві" => &[(".*(?!<1)2", &[":p:v_naz:bad", ":p:v_zna:rinanim:bad"])],
        "ть" => &[("", &[":p:v_naz:bad", ":p:v_zna:rinanim:bad"])],
        "ка" => &[("", &[":f:v_naz:bad"])],
        _ => return None,
    })
}

fn get_case_tags(left_word: &str, lists: &[CaseList]) -> Option<Vec<&'static str>> {
    for (regex, cases) in lists {
        if regex.is_empty()
            || fancy_regex::Regex::new(regex)
                .unwrap()
                .is_match(left_word)
                .unwrap_or(false)
        {
            return Some(cases.to_vec());
        }
    }
    None
}

/// `LetterEndingForNumericHelper.findTagsAdj`.
pub fn find_tags_adj(left_word: &str, right_word: &str) -> Option<Vec<&'static str>> {
    adj_ending_map(right_word).and_then(|lists| get_case_tags(left_word, lists))
}

/// `LetterEndingForNumericHelper.findTagsNoun`.
pub fn find_tags_noun(left_word: &str, right_word: &str) -> Option<Vec<&'static str>> {
    noun_ending_map(right_word).and_then(|lists| get_case_tags(left_word, lists))
}

/// `PosTagHelper.getGender` (group 2 of the gender regex).
pub fn get_gender(pos_tag: &str) -> Option<String> {
    let re = fancy_regex::Regex::new(r"(noun:(?:[iu]n)?anim|numr|adj|adjp.*):(.):v_.*").unwrap();
    re.captures(pos_tag)
        .ok()
        .flatten()
        .and_then(|c| c.get(2).map(|m| m.as_str().to_string()))
}

/// `PosTagHelper.getNum`: `p` stays `p`, anything else becomes `s`.
pub fn get_num(pos_tag: &str) -> Option<String> {
    let re = fancy_regex::Regex::new(r"(noun:(?:[iu]n)?anim|numr|adj|adjp.*):(.):v_.*").unwrap();
    re.captures(pos_tag)
        .ok()
        .flatten()
        .and_then(|c| c.get(2).map(|m| m.as_str().to_string()))
        .map(|g| {
            if g == "p" {
                "p".to_string()
            } else {
                "s".to_string()
            }
        })
}

/// `PosTagHelper.getConj` (`CONJ_REGEX`, group 2).
pub fn get_conj(pos_tag: &str) -> Option<String> {
    let re =
        fancy_regex::Regex::new(r"(noun:(?:[iu]n)?anim|numr|adj|adjp.*):[mfnp]:(v_...).*").unwrap();
    re.captures(pos_tag)
        .ok()
        .flatten()
        .and_then(|c| c.get(2).map(|m| m.as_str().to_string()))
}

/// `PosTagHelper.getGenderConj` (`GENDER_CONJ_REGEX`, group 2).
pub fn get_gender_conj(pos_tag: &str) -> Option<String> {
    let re = fancy_regex::Regex::new(r"(noun:(?:[iu]n)?anim|adj|numr|adjp.*):(.:v_...).*").unwrap();
    re.captures(pos_tag)
        .ok()
        .flatten()
        .and_then(|c| c.get(2).map(|m| m.as_str().to_string()))
}

/// `PosTagHelper.ADJ_COMP_REGEX`.
pub fn adj_comp_regex() -> &'static fancy_regex::Regex {
    static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| fancy_regex::Regex::new(":comp[bcs]").unwrap())
}

/// `PosTagHelper.hasPosTag(taggedWords, pattern)`.
pub fn has_pos_tag2(tagged: &[(String, String)], regex: &fancy_regex::Regex) -> bool {
    tagged.iter().any(|(_, tag)| full_match(regex, tag))
}

/// `PosTagHelper.hasPosTagPart2`.
pub fn has_pos_tag_part2(tagged: &[(String, String)], part: &str) -> bool {
    tagged.iter().any(|(_, tag)| tag.contains(part))
}

/// `PosTagHelper.hasPosTagStart2`.
pub fn has_pos_tag_start2(tagged: &[(String, String)], part: &str) -> bool {
    tagged.iter().any(|(_, tag)| tag.starts_with(part))
}

/// `PosTagHelper.filter2` / `filter2Negative`.
pub fn filter2(tagged: Vec<(String, String)>, regex: &fancy_regex::Regex) -> Vec<(String, String)> {
    tagged
        .into_iter()
        .filter(|(_, tag)| full_match(regex, tag))
        .collect()
}

/// `PosTagHelper.filter2Negative`.
pub fn filter2_negative(
    tagged: Vec<(String, String)>,
    regex: &fancy_regex::Regex,
) -> Vec<(String, String)> {
    tagged
        .into_iter()
        .filter(|(_, tag)| !full_match(regex, tag))
        .collect()
}

/// `PosTagHelper.hasPosTagPart(taggedWords, part)`.
pub fn has_pos_tag_part(tagged: &[(String, String)], part: &str) -> bool {
    tagged.iter().any(|(_, tag)| tag.contains(part))
}

/// `PosTagHelper.hasPosTagStart`.
pub fn has_pos_tag_start(tagged: &[(String, String)], part: &str) -> bool {
    tagged.iter().any(|(_, tag)| tag.starts_with(part))
}

/// `PosTagHelper.hasPosTagPart(readings, part)`.
pub fn has_reading_pos_tag_part(readings: &[AnalyzedToken], part: &str) -> bool {
    readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(part)))
}

/// `PosTagHelper.hasPosTag(readings, pattern)`.
pub fn has_reading_pos_tag(readings: &[AnalyzedToken], regex: &fancy_regex::Regex) -> bool {
    readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| full_match(regex, t)))
}

/// `PosTagHelper.filter(readings, pattern)`.
pub fn filter_readings(
    readings: &[AnalyzedToken],
    regex: &fancy_regex::Regex,
) -> Vec<AnalyzedToken> {
    readings
        .iter()
        .filter(|r| r.pos_tag.as_deref().is_some_and(|t| full_match(regex, t)))
        .cloned()
        .collect()
}

/// `PosTagHelper.hasPosTagPartAll`.
pub fn has_pos_tag_part_all(readings: &[AnalyzedToken], part: &str) -> bool {
    let mut found = false;
    for r in readings {
        if let Some(tag) = r.pos_tag.as_deref() {
            if tag == "SENT_END" || tag == "PARA_END" {
                continue;
            }
            if !tag.contains(part) {
                return false;
            }
            found = true;
        }
    }
    found
}

/// `PosTagHelper.getGenders`.
pub fn get_genders(readings: &[AnalyzedToken], regex: &fancy_regex::Regex) -> String {
    let mut out = String::new();
    for r in readings {
        if let Some(tag) = r.pos_tag.as_deref() {
            if full_match(regex, tag) {
                if let Some(g) = get_gender(tag) {
                    if !out.contains(&g) {
                        out.push_str(&g);
                    }
                }
            }
        }
    }
    out
}

/// `LemmaHelper.CITY_AVENU`.
pub const CITY_AVENU: &[&str] = &[
    "сіті",
    "ситі",
    "стріт",
    "стрит",
    "рівер",
    "ривер",
    "авеню",
    "штрасе",
    "штрассе",
    "сьоркл",
    "сквер",
    "плац",
];

/// `LemmaHelper.DAYS_OF_WEEK`.
pub const DAYS_OF_WEEK: &[&str] = &[
    "понеділок",
    "вівторок",
    "середа",
    "четвер",
    "п'ятниця",
    "субота",
    "неділя",
];

/// `LemmaHelper.MONTH_LEMMAS`.
pub const MONTH_LEMMAS: &[&str] = &[
    "січень",
    "лютий",
    "березень",
    "квітень",
    "травень",
    "червень",
    "липень",
    "серпень",
    "вересень",
    "жовтень",
    "листопад",
    "грудень",
];

/// `LemmaHelper.hasLemma(readings, lemmas)`.
pub fn has_lemma(readings: &[AnalyzedToken], lemmas: &[&str]) -> bool {
    readings
        .iter()
        .any(|r| r.stem.as_deref().is_some_and(|l| lemmas.contains(&l)))
}

/// `LemmaHelper.hasLemma(readings, lemmaRegex)`.
pub fn has_lemma_regex(readings: &[AnalyzedToken], regex: &fancy_regex::Regex) -> bool {
    readings
        .iter()
        .any(|r| r.stem.as_deref().is_some_and(|l| full_match(regex, l)))
}

/// `PosTagHelper.hasPosTagStart(readings, part)`.
pub fn has_reading_pos_tag_start(readings: &[AnalyzedToken], part: &str) -> bool {
    readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(part)))
}

/// `LemmaHelper.hasLemma(readings, lemmas, posTagPattern)`.
pub fn has_lemma_with_pattern(
    readings: &[AnalyzedToken],
    lemmas: &[&str],
    regex: &fancy_regex::Regex,
) -> bool {
    readings.iter().any(|r| {
        r.stem.as_deref().is_some_and(|l| lemmas.contains(&l))
            && r.pos_tag.as_deref().is_some_and(|t| full_match(regex, t))
    })
}

/// `LemmaHelper.hasLemma(readings, lemmaRegex, posTagPattern)`.
pub fn has_lemma_regex_with_pattern(
    readings: &[AnalyzedToken],
    lemma_regex: &fancy_regex::Regex,
    pos_regex: &fancy_regex::Regex,
) -> bool {
    readings.iter().any(|r| {
        r.stem
            .as_deref()
            .is_some_and(|l| full_match(lemma_regex, l))
            && r.pos_tag
                .as_deref()
                .is_some_and(|t| full_match(pos_regex, t))
    })
}

/// `PosTagHelper.PREDICT_INSERT_PATTERN`.
pub fn is_predict_or_insert(token: &AnalyzedToken) -> bool {
    token
        .pos_tag
        .as_deref()
        .is_some_and(|t| full_match(&PREDICT_INSERT, t))
}

static PREDICT_INSERT: LazyLock<fancy_regex::Regex> =
    LazyLock::new(|| fancy_regex::Regex::new(r"^noninfl:(?:predic|insert).*$").unwrap());

/// `PosTagHelper.hasMaleUA`.
pub fn has_male_ua(token: &AnalyzedTokenReadings) -> bool {
    static POS: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^noun:inanim:m:v_dav(?!:nv).*$").unwrap());
    static TOK: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^.*[ую]$").unwrap());
    has_pos_tag_and_token(token, &POS, &TOK)
}

/// `PosTagHelper.hasPosTag(AnalyzedTokenReadings, Pattern)`.
pub fn has_pos_tag_re(token: &AnalyzedTokenReadings, re: &fancy_regex::Regex) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| full_match(re, t)))
}

/// `PosTagHelper.hasPosTag(AnalyzedTokenReadings, String)`.
pub fn has_pos_tag_str2(token: &AnalyzedTokenReadings, pattern: &str) -> bool {
    let re = fancy_regex::Regex::new(&format!("^(?:{pattern})$")).unwrap();
    has_pos_tag_re(token, &re)
}

/// `PosTagHelper.filter(AnalyzedTokenReadings, postag, token)`.
pub fn filter_token(
    token: &AnalyzedTokenReadings,
    postag: &fancy_regex::Regex,
    token_re: &fancy_regex::Regex,
) -> Vec<AnalyzedToken> {
    token
        .readings
        .iter()
        .filter(|t| {
            t.pos_tag.as_deref().is_some_and(|p| full_match(postag, p))
                && full_match(token_re, &t.token)
        })
        .cloned()
        .collect()
}

/// `PosTagHelper.hasPosTagAndToken`.
pub fn has_pos_tag_and_token(
    token: &AnalyzedTokenReadings,
    postag: &fancy_regex::Regex,
    token_re: &fancy_regex::Regex,
) -> bool {
    !filter_token(token, postag, token_re).is_empty()
}

/// `PosTagHelper.getGenders(AnalyzedTokenReadings, Pattern)`.
pub fn get_genders_token(token: &AnalyzedTokenReadings, postag: &fancy_regex::Regex) -> String {
    let mut out = String::new();
    for r in &token.readings {
        if let Some(tag) = r.pos_tag.as_deref() {
            if full_match(postag, tag) {
                if let Some(g) = get_gender(tag) {
                    if !out.contains(&g) {
                        out.push_str(&g);
                    }
                }
            }
        }
    }
    out
}

/// `LemmaHelper.QUOTES_PATTERN`.
pub fn quotes_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^[\p{Pi}\p{Pf}]$").unwrap());
    &RE
}

/// `LemmaHelper.TIME_LEMMAS`.
pub const TIME_LEMMAS: &[&str] = &[
    "секунда",
    "хвилина",
    "хвилинка",
    "хвилина-дві",
    "хвилинка-друга",
    "година",
    "годинка",
    "півгодини",
    "година-друга",
    "година-дві",
    "час",
    "день",
    "день-другий",
    "півдня",
    "ніч",
    "ніченька",
    "вечір",
    "ранок",
    "тиждень",
    "тиждень-два",
    "тиждень-другий",
    "місяць",
    "місяць-два",
    "місяць-другий",
    "місяць-півтора",
    "доба",
    "мить",
    "хвилька",
    "рік",
    "рік-два",
    "рік-півтора",
    "півроку",
    "півроку-рік",
    "десятиліття",
    "десятиріччя",
    "століття",
    "півстоліття",
    "сторіччя",
    "півсторіччя",
    "тисячоліття",
    "півтисячоліття",
    "квартал",
    "годочок",
    "літо",
    "зима",
    "весна",
    "осінь",
    "тайм",
    "період",
    "термін",
    "сезон",
    "декада",
    "каденція",
    "раунд",
];

/// `LemmaHelper.TIME_PLUS_LEMMAS`.
pub const TIME_PLUS_LEMMAS: &[&str] = &[
    "секунда",
    "хвилина",
    "хвилинка",
    "хвилина-дві",
    "хвилинка-друга",
    "година",
    "годинка",
    "півгодини",
    "година-друга",
    "година-дві",
    "час",
    "день",
    "день-другий",
    "півдня",
    "ніч",
    "ніченька",
    "вечір",
    "ранок",
    "тиждень",
    "тиждень-два",
    "тиждень-другий",
    "місяць",
    "місяць-два",
    "місяць-другий",
    "місяць-півтора",
    "доба",
    "мить",
    "хвилька",
    "рік",
    "рік-два",
    "рік-півтора",
    "півроку",
    "півроку-рік",
    "десятиліття",
    "десятиріччя",
    "століття",
    "півстоліття",
    "сторіччя",
    "півсторіччя",
    "тисячоліття",
    "півтисячоліття",
    "квартал",
    "годочок",
    "літо",
    "зима",
    "весна",
    "осінь",
    "тайм",
    "період",
    "термін",
    "сезон",
    "декада",
    "каденція",
    "раунд",
    "міліметр",
    "сантиметр",
    "метр",
    "кілометр",
    "кілограм",
    "кілограм–півтора",
    "гектар",
    "миля",
    "аршин",
    "дециметр",
    "верства",
    "верста",
    "грам",
    "літр",
    "фунт",
    "тонна",
    "центнер",
    "десяток",
    "десяток-другий",
    "сотня",
    "сотка",
    "тисяча",
    "п'ятірка",
    "пара",
    "третина",
    "чверть",
    "половина",
    "дюжина",
    "жменя",
    "жменька",
    "купа",
    "купка",
    "парочка",
    "оберемок",
    "безліч",
    "гривня",
    "копійка",
    "вихідний",
    "уїк-енд",
    "уїкенд",
    "вікенд",
    "відсоток",
    "раз",
    "крок",
];

/// `PosTagHelper.VIDMINKY_MAP` display name.
pub fn case_name(case_: &str) -> String {
    match case_ {
        "v_naz" => "називний",
        "v_rod" => "родовий",
        "v_dav" => "давальний",
        "v_zna" => "знахідний",
        "v_oru" => "орудний",
        "v_mis" => "місцевий",
        "v_kly" => "кличний",
        "v_inf" => "інфінітив",
        other => other,
    }
    .to_string()
}

/// `PosTagHelper.GENDER_MAP` display name.
pub fn gender_name(gender: &str) -> String {
    match gender {
        "m" => "ч.р.",
        "f" => "ж.р.",
        "n" => "с.р.",
        "p" => "мн.",
        "s" => "одн.",
        "i" => "інф.",
        "o" => "безос. форма",
        other => other,
    }
    .to_string()
}

/// `PosTagHelper.PERSON_MAP`.
pub fn person_name(person: &str) -> String {
    match person {
        "1" => "1-а особа",
        "2" => "2-а особа",
        "3" => "3-я особа",
        "s" => "одн.",
        "p" => "мн.",
        other => other,
    }
    .to_string()
}

/// `PosTagHelper.GEN_ORDER` (sorting key; unknown -> 0).
pub fn gen_order(gender: &str) -> i32 {
    match gender {
        "m" => 0,
        "f" => 1,
        "n" => 3,
        "s" => 4,
        "p" => 5,
        "i" => 6,
        "o" => 7,
        _ => 0,
    }
}

/// `PosTagHelper.VIDM_ORDER` (sorting key; unknown -> 0).
pub fn vidm_order(case_: &str) -> i32 {
    match case_ {
        "v_naz" => 10,
        "v_rod" => 20,
        "v_dav" => 30,
        "v_zna" => 40,
        "v_oru" => 50,
        "v_mis" => 60,
        "v_kly" => 70,
        _ => 0,
    }
}

/// `PosTagHelper.isUnknownWord`.
pub fn is_unknown_word(tr: &AnalyzedTokenReadings) -> bool {
    static WORD_PATTERN: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"(?i)^[а-яіїєґa-z'-]+$").unwrap());
    tr.readings.first().is_some_and(|r| r.pos_tag.is_none())
        && WORD_PATTERN.is_match(tr.surface()).unwrap_or(false)
}

/// `TokenAgreementPrepNounRule.hasVidmPosTag(Collection, readings)`.
pub fn has_vidm_pos_tag(cases: &[String], readings: &[AnalyzedToken]) -> bool {
    let mut vidminok_found = false;
    for token in readings {
        match token.pos_tag.as_deref() {
            None => {
                if readings.len() == 1 {
                    return true;
                }
                continue;
            }
            Some(pos_tag) => {
                if pos_tag.contains(":nv") {
                    return true;
                }
                if pos_tag.contains(":v_") {
                    vidminok_found = true;
                    for case in cases {
                        if pos_tag.contains(case.as_str()) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    !vidminok_found
}

/// `TokenAgreementPrepNounRule.hasVidmPosTag(Collection, AnalyzedTokenReadings)`.
pub fn has_vidm_pos_tag_token(cases: &[String], tr: &AnalyzedTokenReadings) -> bool {
    has_vidm_pos_tag(cases, &tr.readings)
}

#[derive(Clone, Copy)]
pub enum Dir {
    Forward,
    Reverse,
}

/// `LemmaHelper.reverseSearchIdx`.
pub fn reverse_search_idx(
    tokens: &[&AnalyzedTokenReadings],
    pos: i64,
    depth: i64,
    lemma: Option<&fancy_regex::Regex>,
    postag: Option<&fancy_regex::Regex>,
) -> i64 {
    let mut i = pos;
    while i > pos - depth && i >= 0 {
        let tr = tokens[i as usize];
        let lemma_ok = lemma.is_none_or(|re| has_lemma_regex(&tr.readings, re));
        let pos_ok = postag.is_none_or(|re| has_pos_tag_re(tr, re));
        if lemma_ok && pos_ok {
            return i;
        }
        i -= 1;
    }
    -1
}

/// `LemmaHelper.reverseSearch`.
pub fn reverse_search(
    tokens: &[&AnalyzedTokenReadings],
    pos: i64,
    depth: i64,
    lemma: Option<&fancy_regex::Regex>,
    postag: Option<&fancy_regex::Regex>,
) -> bool {
    reverse_search_idx(tokens, pos, depth, lemma, postag) >= 0
}

/// `LemmaHelper.revSearchIdx`.
pub fn rev_search_idx(
    tokens: &[&AnalyzedTokenReadings],
    start_pos: i64,
    lemma: Option<&fancy_regex::Regex>,
    postag_regex: Option<&str>,
) -> i64 {
    let mut start_pos = start_pos;
    if start_pos > 0 && has_pos_tag_str2(tokens[start_pos as usize], "part.*") {
        start_pos -= 1;
    }
    if start_pos > 0 && has_pos_tag_str2(tokens[start_pos as usize], "adv(:.*)?|.*pron.*") {
        start_pos -= 1;
    }
    if start_pos > 0 && has_pos_tag_str2(tokens[start_pos as usize], "part.*") {
        start_pos -= 1;
    }
    if start_pos > 0 {
        let tr = tokens[start_pos as usize];
        if lemma.is_some_and(|re| !has_lemma_regex(&tr.readings, re)) {
            return -1;
        }
        if postag_regex.is_some_and(|p| !has_pos_tag_str2(tr, p)) {
            return -1;
        }
        return start_pos;
    }
    -1
}

/// `LemmaHelper.revSearch`.
pub fn rev_search(
    tokens: &[&AnalyzedTokenReadings],
    start_pos: i64,
    lemma: Option<&fancy_regex::Regex>,
    postag_regex: Option<&str>,
) -> bool {
    rev_search_idx(tokens, start_pos, lemma, postag_regex) != -1
}

/// `LemmaHelper.tokenSearch`.
pub fn token_search(
    tokens: &[&AnalyzedTokenReadings],
    pos: i64,
    pos_tag: Option<&str>,
    token: Option<&fancy_regex::Regex>,
    pos_tags_to_ignore: Option<&fancy_regex::Regex>,
    dir: Dir,
) -> i64 {
    static QUOTES: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^[«»„“\u{201C}]$").unwrap());
    let step: i64 = match dir {
        Dir::Forward => 1,
        Dir::Reverse => -1,
    };
    let mut i = pos;
    while i < tokens.len() as i64 && i > 0 {
        let curr = tokens[i as usize];
        let pos_ok = pos_tag.is_none_or(|p| {
            curr.readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(p)))
        });
        let token_ok = token.is_none_or(|re| re.is_match(curr.surface()).unwrap_or(false));
        if pos_ok && token_ok {
            return i;
        }
        if let Some(ignore) = pos_tags_to_ignore {
            if !has_pos_tag_re(curr, ignore) && !QUOTES.is_match(curr.surface()).unwrap_or(false) {
                break;
            }
        }
        i += step;
    }
    -1
}

/// `PosTagHelper.hasPosTagAll`.
pub fn has_pos_tag_all(readings: &[AnalyzedToken], re: &fancy_regex::Regex) -> bool {
    for r in readings {
        match r.pos_tag.as_deref() {
            Some(tag) => {
                if !full_match(re, tag) {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

/// `LemmaHelper.isDash`.
pub fn is_dash(tr: &AnalyzedTokenReadings) -> bool {
    static DASHES: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^[\u{2010}-\u{2015}-]$").unwrap());
    full_match(&DASHES, tr.surface())
}

/// `LemmaHelper.ADV_QUANT_PATTERN`.
pub fn adv_quant_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
        fancy_regex::Regex::new(concat!(
            "^(?:більше|менше|чимало|багато|мало|забагато|замало|немало|багатенько|чималенько|стільки|обмаль|вдосталь|удосталь|трохи|трошки|досить|достатньо|недостатньо|предостатньо",
            "|багацько|чимбільше|побільше|порівну|більшість|трішки|предосить|повно|повнісінько",
            "|мільйон|тисяча|сотня|мільярд|трильйон|десяток|нуль|безліч",
            "|кілька|декілька|пара|парочка|купа|купка|безліч|мінімум|максимум)$"
        ))
        .unwrap()
    });
    &RE
}

/// `LemmaHelper.TIME_PLUS_LEMMAS_PATTERN` (alternation of the lemmas).
pub fn time_plus_lemmas_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
        fancy_regex::Regex::new(&format!("^(?:{})$", TIME_PLUS_LEMMAS.join("|"))).unwrap()
    });
    &RE
}

/// `LemmaHelper.PLUS_MINUS`.
pub const PLUS_MINUS: &[&str] = &["плюс", "мінус", "максимум", "мінімум"];

/// `PosTagHelper.VERB_PATTERN`.
pub fn verb_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^verb.*$").unwrap());
    &RE
}

/// `PosTagHelper.VERB_ADVP_PATTERN`.
pub fn verb_advp_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^(?:verb|advp).*$").unwrap());
    &RE
}

/// `PosTagHelper.ADJ_V_NAZ_PATTERN`.
pub fn adj_v_naz_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^adj:.:v_naz.*$").unwrap());
    &RE
}

/// `PosTagHelper.NOUN_V_NAZ_PATTERN`.
pub fn noun_v_naz_pattern() -> &'static fancy_regex::Regex {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^noun.*:v_naz.*$").unwrap());
    &RE
}

/// `TokenAgreementNounVerbExceptionHelper.hasMascFemLemma`.
pub fn has_masc_fem_lemma(
    readings: &[AnalyzedToken],
    masc_fem_set: &std::collections::HashSet<String>,
) -> bool {
    let Some(token) = readings.first().map(|r| r.token.as_str()) else {
        return false;
    };
    if token.ends_with("олог") || token.ends_with("знавець") {
        return true;
    }
    for at in readings {
        if let Some(pos_tag) = at.pos_tag.as_deref() {
            if pos_tag.contains("noun:anim:m:v_naz") {
                if let Some(lemma) = at.stem.as_deref() {
                    let lemma = lemma.replace('\u{2018}', "-");
                    let base = lemma.split('-').next().unwrap_or("");
                    if masc_fem_set.contains(&lemma) || masc_fem_set.contains(base) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// `ExtraDictionaryLoader.loadSet` + `extendSet(..., "екс-")` for
/// `masc_fem.txt`.
pub fn load_masc_fem_set(words_dir: &std::path::Path) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let Ok(text) = lt_data::fs::read_to_string(words_dir.join("masc_fem.txt")) else {
        return set;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        set.insert(line.to_string());
    }
    let extra: Vec<String> = set.iter().map(|l| format!("екс-{l}")).collect();
    set.extend(extra);
    set
}

/// `LemmaHelper.tokenSearch` with a Pattern posTag.
pub fn token_search_re(
    tokens: &[&AnalyzedTokenReadings],
    pos: i64,
    pos_tag: Option<&fancy_regex::Regex>,
    token: Option<&fancy_regex::Regex>,
    pos_tags_to_ignore: Option<&fancy_regex::Regex>,
    dir: Dir,
) -> i64 {
    static QUOTES: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^[«»„“\u{201C}]$").unwrap());
    let step: i64 = match dir {
        Dir::Forward => 1,
        Dir::Reverse => -1,
    };
    let mut i = pos;
    while i < tokens.len() as i64 && i > 0 {
        let curr = tokens[i as usize];
        let pos_ok = pos_tag.is_none_or(|re| has_pos_tag_re(curr, re));
        let token_ok = token.is_none_or(|re| re.is_match(curr.surface()).unwrap_or(false));
        if pos_ok && token_ok {
            return i;
        }
        if let Some(ignore) = pos_tags_to_ignore {
            if !has_pos_tag_re(curr, ignore) && !QUOTES.is_match(curr.surface()).unwrap_or(false) {
                break;
            }
        }
        i += step;
    }
    -1
}

/// `PosTagHelper.VERB_INF_PATTERN`.
pub fn verb_inf_pattern() -> &'static LazyLock<fancy_regex::Regex> {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^verb.*:inf.*$").unwrap());
    &RE
}

/// `PosTagHelper.NOUN_NON_PRON_V_NAZ_PATTERN`.
pub fn noun_non_pron_v_naz_pattern() -> &'static LazyLock<fancy_regex::Regex> {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^noun.*:v_naz(?!.*pron).*$").unwrap());
    &RE
}

/// `LemmaHelper.isPossiblyProperNoun`.
pub fn is_possibly_proper_noun(tr: &AnalyzedTokenReadings) -> bool {
    is_capitalized(tr.surface())
}

/// `LemmaHelper.isInitial`.
pub fn is_initial(tr: &AnalyzedTokenReadings) -> bool {
    static RE: LazyLock<fancy_regex::Regex> =
        LazyLock::new(|| fancy_regex::Regex::new(r"^[А-ЯІЇЄҐA-Z]\.$").unwrap());
    tr.surface().ends_with('.') && RE.is_match(tr.surface()).unwrap_or(false)
}

/// `StringUtils.isAllUpperCase`.
pub fn is_all_upper(s: &str) -> bool {
    s.chars().all(|c| !c.is_lowercase()) && s.chars().any(|c| c.is_uppercase())
}

/// `LemmaHelper.forwardLemmaSearchIdx`.
pub fn forward_lemma_search_idx(
    tokens: &[&AnalyzedTokenReadings],
    pos: i64,
    depth: i64,
    lemma: Option<&fancy_regex::Regex>,
    postag: Option<&fancy_regex::Regex>,
) -> i64 {
    let mut i = pos;
    while i < pos + depth && i < tokens.len() as i64 {
        let tr = tokens[i as usize];
        let lemma_ok = lemma.is_none_or(|re| has_lemma_regex(&tr.readings, re));
        let pos_ok = postag.is_none_or(|re| has_pos_tag_re(tr, re));
        if lemma_ok && pos_ok {
            return i;
        }
        i += 1;
    }
    -1
}

/// `LemmaHelper.CONJ_FOR_PLURAL` as a regex.
pub fn conj_for_plural_pattern() -> &'static LazyLock<fancy_regex::Regex> {
    static RE: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
        fancy_regex::Regex::new(r"^(?:і|а|й|та|чи|або|ані|також|то|a|i)$").unwrap()
    });
    &RE
}

/// `LemmaHelper.CONJ_FOR_PLURAL_WITH_COMMA`.
pub const CONJ_FOR_PLURAL_WITH_COMMA: &[&str] = &[
    "і",
    "а",
    "й",
    "та",
    "чи",
    "або",
    "ані",
    "також",
    "плюс",
    "то",
    "a",
    "i",
    ",",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase_and_capitalize() {
        assert!(is_all_uppercase_uk("УКРАЇНА"));
        assert!(is_all_uppercase_uk("ІВАН-ФРАНКІВСЬК"));
        assert!(!is_all_uppercase_uk("Україна"));
        assert_eq!(capitalize_proper_name("УКРАЇНА"), "Україна");
        assert_eq!(capitalize_proper_name("ІВАН-ФРАНКІВСЬК"), "Іван-Франківськ");
        assert!(is_capitalized("Іван-Франківськ"));
        assert!(!is_capitalized("іван"));
    }

    #[test]
    fn letter_ending_keys() {
        assert!(is_possible_adj_adj_ending("й"));
        assert!(is_possible_adj_adj_ending("го"));
        assert!(!is_possible_adj_adj_ending("xyz"));
        assert!(is_possible_noun_noun_ending("ти"));
    }

    #[test]
    fn pos_tag_fields() {
        assert_eq!(get_gender("noun:inanim:m:v_naz").as_deref(), Some("m"));
        assert_eq!(get_num("noun:inanim:p:v_naz").as_deref(), Some("p"));
        assert_eq!(get_num("noun:inanim:f:v_naz").as_deref(), Some("s"));
    }
}
