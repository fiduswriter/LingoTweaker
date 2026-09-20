//! Port of `org.languagetool.rules.nl.Tools` (`glueParts` + the spelled-word
//! table) used by `SpaceInCompoundRule` and `DutchTagger`.

use std::sync::OnceLock;

const SPELLED_WORDS: &str = "abc|adv|aed|apk|b2b|bh|bhv|bso|btw|bv|cao|cd|cfk|ckv|cv|dc|dj|dtp|dvd|fte|gft|ggo|ggz|gm|gmo|gps|gsm|hbo|\
hd|hiv|hr|hrm|hst|ic|ivf|kmo|lcd|lp|lpg|lsd|mbo|mdf|mkb|mms|msn|mt|ngo|nv|ob|ov|ozb|p2p|pc|pcb|pdf|pk|pps|\
pr|pvc|roc|rvs|sms|tbc|tbs|tl|tv|uv|vbo|vj|vmbo|vsbo|vwo|wc|wo|xtc|zzp";

fn spelled_words_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| SPELLED_WORDS.split('|').collect())
}

fn regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).unwrap()
}

// PROTOTYPE: previously these four patterns were compiled on every
// `glue_parts` call (i.e. per line of multipartcompounds.txt).
fn digits_end() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r".*[0-9]$"))
}

fn digits_start() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"^[0-9].*"))
}

fn hyphen_lower_end() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r".+-[a-z]$"))
}

fn lower_hyphen_start() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"^[a-z]-.+"))
}

/// `Tools.glueParts(List<String>)`: join parts, inserting `-` at vowel
/// collisions and around case changes, numbers and known abbreviations.
pub fn glue_parts(parts: &[&str]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut compound = parts[0].to_string();
    for word2 in &parts[1..] {
        let word2 = *word2;
        if word2.is_empty() {
            continue;
        }
        let glue = compound.chars().count() > 2 || spelled_words_set().contains(compound.as_str());
        let (last_char, first_char) = (
            compound.chars().next_back().unwrap(),
            word2.chars().next().unwrap(),
        );
        let connection: String = [last_char, first_char].iter().collect();
        let hyphen = connection_is_hard(&connection)
            || (first_char.is_uppercase() && last_char.is_lowercase())
            || (last_char.is_uppercase() && first_char.is_lowercase())
            || (last_char.is_uppercase() && first_char.is_uppercase())
            || digits_end().is_match(&compound)
            || digits_start().is_match(word2)
            || hyphen_chars().is_match(&compound)
            || chars_hyphen().is_match(word2)
            || hyphen_lower_end().is_match(&compound)
            || lower_hyphen_start().is_match(word2);
        if glue && hyphen {
            compound.push('-');
            compound.push_str(word2);
        } else {
            compound.push_str(word2);
        }
    }
    compound
}

fn hyphen_chars() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    // Java uses `matcher.matches()` (anchored): the whole string is an
    // optional prefix plus one spelled word.
    RE.get_or_init(|| regex(&format!(r"^(^|.+-)?({SPELLED_WORDS})$")))
}

fn chars_hyphen() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    // Java uses `matcher.matches()` (anchored).
    RE.get_or_init(|| regex(&format!(r"^({SPELLED_WORDS})(-.+|$)?$")))
}

fn connection_is_hard(connection: &str) -> bool {
    const PAIRS: [&str; 20] = [
        "aa", "ae", "ai", "ao", "au", "ee", "ei", "eu", "ée", "éi", "éu", "ie", "ii", "oe", "oi",
        "oo", "ou", "ui", "uu", "ij",
    ];
    PAIRS.contains(&connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glue_parts_matches_java_probes() {
        // pinned from the Dutch corpus / SpaceInCompound messages
        assert_eq!(glue_parts(&["aanvaar", "ding"]), "aanvaarding");
        assert_eq!(
            glue_parts(&["prikkelbare", "darmsyndroom"]),
            "prikkelbaredarmsyndroom"
        );
        assert_eq!(
            glue_parts(&["lange", "afstand", "loper"]),
            "langeafstandloper"
        );
        // hyphen cases from Tools: vowel collision, case change; a 2-char
        // first part never gets the connection checks (Java's outer `if`)
        assert_eq!(glue_parts(&["zee", "egel"]), "zee-egel");
        assert_eq!(glue_parts(&["IRA", "akkoord"]), "IRA-akkoord");
        assert_eq!(glue_parts(&["06", "nummer"]), "06nummer");
    }
}
