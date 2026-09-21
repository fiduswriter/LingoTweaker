//! Greek Java-coded built-in rules (`Greek.getRelevantRules`):
//! `WordRepeatRule` (9, generic base), `GreekWordRepeatBeginningRule` (8),
//! `ReplaceHomonymsRule` (10), `GreekSpecificCaseRule` (11),
//! `NumeralStressRule` (12) and `GreekRedundancyRule` (13).

use std::path::Path;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::simple_replace::{CaseSensitivity, SimpleReplaceConfig, TokenException};

// ---------------------------------------------------------------------------
// WordRepeatRule (`WORD_REPEAT_RULE`), the generic base class
// ---------------------------------------------------------------------------

/// `WordRepeatRule` (`WORD_REPEAT_RULE`) with the Greek strings.
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Επανάληψη λέξης (π.χ. 'και και')",
        message: "Πιθανό λάθος: επαναλάβατε μία λέξη",
        short_message: "Επανάληψη λέξης",
        category_name: "Διάφορα",
    })
}

// ---------------------------------------------------------------------------
// GreekWordRepeatBeginningRule (`GREEK_WORD_REPEAT_BEGINNING_RULE`)
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_BEGINNING_ID: &str = "GREEK_WORD_REPEAT_BEGINNING_RULE";
const WRB_DESCRIPTION: &str = "Συνεχόμενες προτάσεις που ξεκινάνε με την ίδια λέξη";
const WRB_SHORT_ADV: &str = "Δύο συνεχόμενες προτάσεις ξεκινάνε με το ίδιο επίρρημα.";
const WRB_SHORT_WORD: &str = "Τρεις συνεχόμενες προτάσεις ξεκινάνε με την ίδια λέξη.";
const WRB_THESAURUS: &str =
    "Αλλάξτε την σειρά των λέξεων στην πρόταση ή συμβουλευτείτε έναν θησαυρό για να βρείτε μία συνώνυμη λέξη.";

/// `GreekWordRepeatBeginningRule.ADD_ADVERBS`, in Java `HashSet` iteration
/// order (so the suggestion order matches the legacy engine).
const ADD_ADVERBS: [&str; 5] = [
    "Επιπρόσθετα",
    "Επιπλέον",
    "Επίσης",
    "Συμπληρωματικά",
    "Ακόμη",
];
/// `GreekWordRepeatBeginningRule.CONTRAST_ADVERBS`, Java `HashSet` order.
const CONTRAST_ADVERBS: [&str; 4] = ["Αντίθετα", "Εντούτοις", "Ωστόσο", "Εξάλλου"];
/// `GreekWordRepeatBeginningRule.EXPLAIN_ADVERBS`, Java `HashSet` order.
const EXPLAIN_ADVERBS: [&str; 4] = ["Δηλαδή", "Ειδικά", "Ειδικότερα", "Συγκεκριμένα"];

/// `GreekWordRepeatBeginningRule.isException`: the base list plus the Greek
/// articles/numerals.
fn greek_wrb_is_exception(token: &str) -> bool {
    matches!(
        token,
        ":" | "–"
            | "-"
            | "✔️"
            | "➡️"
            | "—"
            | "⭐️"
            | "⚠️"
            | "Ο"
            | "Η"
            | "Το"
            | "Οι"
            | "Τα"
            | "Ένας"
            | "Μία"
            | "Ένα"
    )
}

/// `GreekWordRepeatBeginningRule.getSuggestions`: the other adverbs of the
/// matched token's set, in Java `HashSet` order.
fn greek_wrb_suggestions(token: &str) -> Vec<String> {
    let set: &[&str] = if ADD_ADVERBS.contains(&token) {
        &ADD_ADVERBS
    } else if CONTRAST_ADVERBS.contains(&token) {
        &CONTRAST_ADVERBS
    } else if EXPLAIN_ADVERBS.contains(&token) {
        &EXPLAIN_ADVERBS
    } else {
        return Vec::new();
    };
    set.iter()
        .filter(|adv| **adv != token)
        .map(|s| s.to_string())
        .collect()
}

/// `GreekWordRepeatBeginningRule.match` over all sentences (text level).
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            token = tokens[1].surface().to_string();
            if tokens.len() > 3 {
                let mut is_word = true;
                if token.chars().count() == 1 {
                    is_word = token.chars().next().is_some_and(char::is_alphabetic);
                }
                if is_word
                    && last_token == token
                    && !greek_wrb_is_exception(&token)
                    && !greek_wrb_is_exception(tokens[2].surface())
                    && !greek_wrb_is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(ends_like_sentence)
                {
                    let is_adverb = ADD_ADVERBS.contains(&token.as_str())
                        || CONTRAST_ADVERBS.contains(&token.as_str())
                        || EXPLAIN_ADVERBS.contains(&token.as_str());
                    let short_msg = if is_adverb {
                        Some(WRB_SHORT_ADV)
                    } else if before_last_token == token {
                        Some(WRB_SHORT_WORD)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {WRB_THESAURUS}");
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        let suggestions: Vec<Suggestion> = greek_wrb_suggestions(&token)
                            .into_iter()
                            .map(|value| Suggestion {
                                value,
                                short_description: None,
                            })
                            .collect();
                        rule_matches.push(
                            Match::new(
                                WORD_REPEAT_BEGINNING_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                suggestions,
                                "REPETITIONS_STYLE",
                                "Repetitions (Style)",
                            )
                            .with_metadata(WRB_DESCRIPTION, "style", 0),
                        );
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}

fn ends_like_sentence(sentence: &AnalyzedSentence) -> bool {
    let trimmed = sentence.text.trim();
    trimmed.len() > 1
        && trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '?' | '!'))
}

// ---------------------------------------------------------------------------
// NumeralStressRule (`GREEK_ORTHOGRAPHY_NUMERAL_STRESS`)
// ---------------------------------------------------------------------------

/// The `(stressed, unstressed)` suffix pairs of `NumeralStressRule`.
const NUMERAL_SUFFIXES: [(&str, &str); 12] = [
    ("ός", "ος"),
    ("ού", "ου"),
    ("ό", "ο"),
    ("όν", "ον"),
    ("οί", "οι"),
    ("ών", "ων"),
    ("ούς", "ους"),
    ("ή", "η"),
    ("ής", "ης"),
    ("ήν", "ην"),
    ("ές", "ες"),
    ("ά", "α"),
];

fn numeral_regex() -> &'static fancy_regex::Regex {
    static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        let mut stressed = String::new();
        for (i, (s, _)) in NUMERAL_SUFFIXES.iter().enumerate() {
            if i > 0 {
                stressed.push('|');
            }
            stressed.push_str(s);
        }
        let mut pattern = format!("([1-9][0-9]*)({stressed}");
        for (_, u) in NUMERAL_SUFFIXES.iter() {
            pattern.push('|');
            pattern.push_str(u);
        }
        pattern.push(')');
        fancy_regex::Regex::new(&pattern).expect("numeral pattern")
    })
}

fn stressed_number_regex() -> &'static fancy_regex::Regex {
    static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| fancy_regex::Regex::new(r"^[0-9]*[0|2-9]0$").expect("stressed number"))
}

fn stressed_suffix_regex() -> &'static fancy_regex::Regex {
    static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        let mut s = String::new();
        for (i, (st, _)) in NUMERAL_SUFFIXES.iter().enumerate() {
            if i > 0 {
                s.push('|');
            }
            s.push_str(st);
        }
        fancy_regex::Regex::new(&format!("^(?:{s})$")).expect("stressed suffix")
    })
}

/// `NumeralStressRule.match` over one sentence.
pub fn numeral_stress(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = Vec::new();
    for token in tokens {
        let surface = token.surface();
        let Some(caps) = numeral_regex().captures(surface).ok().flatten() else {
            continue;
        };
        let number = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let suffix = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let needs_stress = stressed_number_regex().is_match(number).unwrap_or(false);
        let has_stress = stressed_suffix_regex().is_match(suffix).unwrap_or(false);
        if needs_stress == has_stress {
            continue;
        }
        let mapped = NUMERAL_SUFFIXES
            .iter()
            .find_map(|(st, un)| {
                if *st == suffix {
                    Some(*un)
                } else if *un == suffix {
                    Some(*st)
                } else {
                    None
                }
            })
            .unwrap_or(suffix);
        let suggestion = format!("{number}{mapped}");
        let msg = format!("<suggestion>{suggestion}</suggestion>");
        matches.push(
            Match::new(
                "GREEK_ORTHOGRAPHY_NUMERAL_STRESS",
                Option::<String>::None,
                msg,
                Some("Πρόβλημα ορθογραφίας".to_string()),
                TextRange::new(
                    sentence_offset + token.start_pos,
                    sentence_offset + token.end_pos(),
                ),
                vec![Suggestion {
                    value: suggestion,
                    short_description: None,
                }],
                "ORTHOGRAPHY",
                "Orthography",
            )
            .with_metadata("Έλεγχος τονισμού αριθμητικών", "misspelling", 0),
        );
    }
    matches
}

// ---------------------------------------------------------------------------
// AbstractSimpleReplaceRule2 instances (`GreekRedundancyRule`,
// `ReplaceHomonymsRule`)
// ---------------------------------------------------------------------------

/// `GreekRedundancyRule` (`EL_REDUNDANCY_REPLACE`, `/el/redundancies.txt`).
pub fn redundancy_instance(data_dir: &Path) -> Result<crate::simple_replace::SimpleReplaceRule> {
    crate::simple_replace::SimpleReplaceRule::from_files(
        &[data_dir.join("el/rules/redundancies.txt")],
        SimpleReplaceConfig {
            rule_id: "EL_REDUNDANCY_REPLACE",
            description: "Έλεγχος για χρήση πλεονασμού σε μια πρόταση.",
            short: "Πλεονασμός",
            message: "'$match' είναι πλεονασμός. Γενικά, είναι προτιμότερο το: $suggestions",
            suggestions_separator: ", ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "Πλεονασμός",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

/// `ReplaceHomonymsRule` (`GREEK_HOMONYMS_REPLACE`, `/el/replace.txt`).
pub fn homonyms_instance(data_dir: &Path) -> Result<crate::simple_replace::SimpleReplaceRule> {
    crate::simple_replace::SimpleReplaceRule::from_files(
        &[data_dir.join("el/rules/replace.txt")],
        SimpleReplaceConfig {
            rule_id: "GREEK_HOMONYMS_REPLACE",
            description: "Έλεγχος για λανθασμένη χρήση ομόηχων λέξεων σε μια πρόταση",
            short: "Λανθασμένη χρήση της λέξης",
            message: "Μήπως εννοούσατε $suggestions?",
            suggestions_separator: ", ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: "Διάφορα",
            issue_type: "Other",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

// ---------------------------------------------------------------------------
// GreekSpecificCaseRule (`EL_SPECIFIC_CASE`)
// ---------------------------------------------------------------------------

/// `GreekSpecificCaseRule` over `el/words/specific_case.txt`.
pub fn specific_case_instance(data_dir: &Path) -> crate::specific_case::SpecificCaseRule {
    crate::specific_case::SpecificCaseRule::from_data_with(
        data_dir,
        crate::specific_case::SpecificCaseConfig {
            rule_id: "EL_SPECIFIC_CASE",
            description: "Ελέγχει αν κάποιες λέξεις χρειάζονται κεφαλαίο το πρώτο τους γράμμα",
            short_message: "Ειδική κεφαλαιοποίηση",
            category_id: "CASING",
            category_name: "Χρήση κεφαλαίων",
            initial_capital_message:
                "Οι λέξεις της συγκεκριμένης έκφρασης χρείαζεται να ξεκινούν με κεφαλαία γράμματα.",
            other_capitalization_message:
                "Η συγκεκριμένη έκφραση γράφεται σύμφωνα με την προτεινόμενη κεφαλαιοποίηση.",
            phrases_path: "el/words/specific_case.txt",
        },
    )
}
