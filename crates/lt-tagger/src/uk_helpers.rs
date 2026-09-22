//! Ukrainian tagging helpers: `PosTagHelper`, `LetterEndingForNumericHelper`
//! and the `LemmaHelper` text functions used by `UkrainianTagger`.

use lt_core::AnalyzedToken;

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
