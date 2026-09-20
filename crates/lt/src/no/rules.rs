//! Norwegian Bokmål word-list rules (stage 1): common typos, Nynorsk forms,
//! word division and the capitalization check. Data files live in
//! `data/no/rules/`; each entry is `wrong=right` (`|` separates multiple
//! wrong forms, `|` in the right side separates suggestions).

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule};

/// All default-on Norwegian Bokmål list rules, in `getRelevantRules` order:
/// typos, Nynorsk forms, word division, capitalization.
pub fn norwegian_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules_dir = data_dir.join("no/rules");
    Ok(vec![
        SimpleReplaceRule::from_files(
            &[
                rules_dir.join("typos.txt"),
                rules_dir.join("typos_wikipedia.txt"),
            ],
            SimpleReplaceConfig {
                rule_id: "NB_TYPOS",
                description: "Vanlige stavefeil ($match)",
                short: "Stavefeil",
                message: "'$match' er en vanlig stavefeil. Mente du $suggestions?",
                suggestions_separator: " eller ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "TYPOS",
                category_name: "Mulig skrivefeil",
                issue_type: "misspelling",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
        SimpleReplaceRule::from_files(
            &[rules_dir.join("nynorsk.txt")],
            SimpleReplaceConfig {
                rule_id: "NB_NYNORSK_FORMS",
                description: "Nynorske former i bokmålstekst ($match)",
                short: "Nynorsk form",
                message: "'$match' er en nynorsk form. På bokmål skriver man $suggestions.",
                suggestions_separator: " eller ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "GRAMMAR",
                category_name: "Grammatikk",
                issue_type: "grammar",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
        SimpleReplaceRule::from_files(
            &[rules_dir.join("word_division.txt")],
            SimpleReplaceConfig {
                rule_id: "NB_WORD_DIVISION",
                description: "Feil orddeling ($match)",
                short: "Orddeling",
                message: "'$match' er feil orddeling. Riktig er $suggestions.",
                suggestions_separator: " eller ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "TYPOS",
                category_name: "Mulig skrivefeil",
                issue_type: "misspelling",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
        SimpleReplaceRule::from_files(
            &[rules_dir.join("split_compounds.txt")],
            SimpleReplaceConfig {
                rule_id: "NB_SPLIT_COMPOUND",
                description: "Særskriving ($match)",
                short: "Særskriving",
                message: "'$match' er særskriving. Det skal skrives i ett ord: $suggestions.",
                suggestions_separator: " eller ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "TYPOS",
                category_name: "Mulig skrivefeil",
                issue_type: "misspelling",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
        // `AbstractCheckCaseRule`: the file lists the *correct* forms; any
        // other casing is flagged (sentence start is exempted).
        SimpleReplaceRule::from_files(
            &[rules_dir.join("check_case.txt")],
            SimpleReplaceConfig {
                rule_id: "NB_CAPITALIZATION",
                description: "Stor eller liten forbokstav ($match)",
                short: "Stor/liten bokstav",
                message: "På norsk skrives '$match' med liten forbokstav: $suggestions",
                suggestions_separator: ", ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "CASING",
                category_name: "Stor og liten bokstav",
                issue_type: "typographical",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: true,
                ignore_short_uppercase_words: false,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
    ])
}
