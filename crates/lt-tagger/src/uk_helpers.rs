//! Ukrainian tagging helpers: `PosTagHelper`, `LetterEndingForNumericHelper`
//! (suffix key lookup) and the `LemmaHelper` text functions used by
//! `UkrainianTagger`.

/// `LemmaHelper.IGNORE_CHARS` (`\u00AD\u0301`).
const IGNORE_CHARS: &str = "\u{00AD}\u{0301}";

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
