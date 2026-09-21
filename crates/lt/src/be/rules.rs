//! Belarusian language-specific rule instances that are not XML rules:
//! `SimpleReplaceRule` (`BE_SIMPLE_REPLACE`, `be/rules/replace.txt`) and
//! `BelarusianSpecificCaseRule` (`BE_SPECIFIC_CASE`,
//! `be/words/specific_case.txt`).

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};
use crate::specific_case::{SpecificCaseConfig, SpecificCaseRule};

/// `be.SimpleReplaceRule` (`BE_SIMPLE_REPLACE`) over `be/rules/replace.txt`.
pub fn simple_replace_instance(data_dir: &Path) -> Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("be/rules/replace.txt")],
        SimpleReplaceConfig {
            rule_id: "BE_SIMPLE_REPLACE",
            description: "Пошук прастамоўяў і памылковых фраз",
            short: "Памылка?",
            message:
                "«$match» — памылка, нелітаратурны выраз або прастамоўе, правільна: $suggestions",
            suggestions_separator: ", ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: "Агульныя правілы",
            issue_type: "misspelling",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

/// `be.BelarusianSpecificCaseRule` (`BE_SPECIFIC_CASE`) over
/// `be/words/specific_case.txt`.
pub fn specific_case_instance(data_dir: &Path) -> SpecificCaseRule {
    SpecificCaseRule::from_data_with(
        data_dir,
        SpecificCaseConfig {
            rule_id: "BE_SPECIFIC_CASE",
            description: "Напісанне спецыяльных найменняў у верхнім або ніжнім рэгістры",
            short_message: "Proper noun",
            category_id: "CASING",
            category_name: "Вялікія літары",
            initial_capital_message: "Уласныя імёны і назвы пішуцца з вялікай літары.",
            other_capitalization_message:
                "Калі гэта уласнае імя або назва, выкарыстоўвайце прапанаванае напісанне.",
            phrases_path: "be/words/specific_case.txt",
        },
    )
}
