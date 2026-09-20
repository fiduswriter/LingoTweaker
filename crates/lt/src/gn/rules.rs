//! Guaraní word-list rules (stage 1): curated orthography corrections
//! (nasal tilde, acute accent, puso, g̃, adapted loans) and attached
//! postpositions. Data files live in `data/gn/rules/`; the list is curated
//! until the morphology-aware rules (ALG nasal harmony, accent placement)
//! land in stage 3.

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule};

/// All default-on Guaraní list rules.
pub fn guarani_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules_dir = data_dir.join("gn/rules");
    Ok(vec![
        SimpleReplaceRule::from_files(
            &[rules_dir.join("orthography.txt")],
            SimpleReplaceConfig {
                rule_id: "GN_ORTHOGRAPHY",
                description: "Guaraní orthography ($match)",
                short: "Orthography",
                message: "'$match' does not follow the standard orthography. Write $suggestions.",
                suggestions_separator: " or ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "TYPOS",
                category_name: "Possible Typo",
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
            &[rules_dir.join("jopara.txt")],
            SimpleReplaceConfig {
                rule_id: "GN_JOPARA",
                description: "Spanish connector in Guaraní text ($match)",
                short: "Jopara",
                message: "Spanish connector in Guaraní text; consider: $suggestions.",
                suggestions_separator: " or ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: "Style",
                issue_type: "style",
                default_off: true,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
        SimpleReplaceRule::from_files(
            &[rules_dir.join("check_case.txt")],
            SimpleReplaceConfig {
                rule_id: "GN_CAPITALIZATION",
                description: "Guaraní capitalization ($match)",
                short: "Capitalization",
                message: "In Guaraní the language name is lowercase: '$suggestions', not '$match'.",
                suggestions_separator: ", ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "CASING",
                category_name: "Capitalization",
                issue_type: "typographical",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: true,
                ignore_short_uppercase_words: false,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
        SimpleReplaceRule::from_files(
            &[rules_dir.join("postpositions.txt")],
            SimpleReplaceConfig {
                rule_id: "GN_POSTPOSITIONS",
                description: "Guaraní postpositions are written attached ($match)",
                short: "Postposition",
                message: "'$match' is written attached in Guaraní: $suggestions.",
                suggestions_separator: " or ",
                sub_rule_specific_ids: false,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "GRAMMAR",
                category_name: "Grammar",
                issue_type: "grammar",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: crate::simple_replace::TokenException::None,
            },
        )?,
    ])
}
