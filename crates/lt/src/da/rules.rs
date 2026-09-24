//! Danish word-list rules: common typos from the Danish Wikipedia
//! "Almindelige stavefejl" list. Data files live in `data/da/rules/`; each
//! entry is `wrong=right` (`|` in the right side separates suggestions).

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule};

/// All default-on Danish list rules. Danish has no Java rule classes, so this
/// is an owner-approved addition (Rust reports matches Java does not have).
pub fn danish_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules_dir = data_dir.join("da/rules");
    Ok(vec![SimpleReplaceRule::from_files(
        &[rules_dir.join("typos_wikipedia.txt")],
        SimpleReplaceConfig {
            rule_id: "DANISH_TYPOS",
            description: "Almindelige stavefejl ($match)",
            short: "Stavefejl",
            message: "'$match' er en almindelig stavefejl. Mente du $suggestions?",
            suggestions_separator: " eller ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "TYPOS",
            category_name: "Mulige slåfejl",
            issue_type: "misspelling",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: crate::simple_replace::TokenException::None,
        },
    )?])
}
