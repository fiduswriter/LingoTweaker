//! Norwegian Nynorsk word-list rules: Bokmål-forms in Nynorsk text,
//! word division, common typos and the capitalization check. Data files live
//! in `data/nn/rules/`; each entry is `wrong=right` (`|` separates multiple
//! wrong forms or multiple suggestions).

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule};

/// All default-on Nynorsk list rules, in `getRelevantRules` order.
pub fn nynorsk_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules_dir = data_dir.join("nn/rules");
    Ok(vec![
        SimpleReplaceRule::from_files(
            &[rules_dir.join("typos.txt")],
            SimpleReplaceConfig {
                rule_id: "NN_TYPOS",
                description: "Vanlege skrivefeil ($match)",
                short: "Skrivefeil",
                message: "'$match' er ein vanleg skrivefeil. Meinte du $suggestions?",
                suggestions_separator: " eller ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "TYPOS",
                category_name: "Mogeleg skrivefeil",
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
            &[rules_dir.join("bokmaal_forms.txt")],
            SimpleReplaceConfig {
                rule_id: "NN_BOKMAAL_FORMS",
                description: "Bokmål-former i nynorsktekst ($match)",
                short: "Bokmålsform",
                message: "'$match' er ei bokmålsform. På nynorsk skriv ein $suggestions.",
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
                rule_id: "NN_WORD_DIVISION",
                description: "Feil orddeling ($match)",
                short: "Orddeling",
                message: "'$match' er feil orddeling. Rett er $suggestions.",
                suggestions_separator: " eller ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "TYPOS",
                category_name: "Mogeleg skrivefeil",
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
                rule_id: "NN_CAPITALIZATION",
                description: "Stor eller liten bokstav ($match)",
                short: "Stor/liten bokstav",
                message: "På norsk skriv ein '$match' med liten bokstav: $suggestions",
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
