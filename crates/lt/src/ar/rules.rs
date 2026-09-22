//! Arabic Java rule classes: the `AbstractSimpleReplaceRule2` instances
//! (`ArabicSimpleReplaceRule`, `ArabicDiacriticsRule`, `ArabicDarjaRule`,
//! `ArabicHomophonesRule`, `ArabicRedundancyRule`, `ArabicWordinessRule`).

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};

/// The six `AbstractSimpleReplaceRule2` instances in `Arabic.getRelevantRules`
/// order (8, 12, 13, 14, 15, 17): simple replace, diacritics, darja,
/// homophones, redundancy, wordiness.
#[allow(clippy::vec_init_then_push)]
pub fn simple_replace_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules = data_dir.join("ar/rules");
    let mut instances = Vec::new();
    // `ArabicSimpleReplaceRule` (8): `AR_SIMPLE_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("replaces.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_SIMPLE_REPLACE",
            description: "قاعدة تطابق الكلمات التي يجب تجنبها وتقترح تصويبا لها",
            short: "خطأ، يفضل أن  يقال:",
            message: "قل $suggestions",
            suggestions_separator: " أو  ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "CONFUSED_WORDS",
            category_name: "كلمات ملتبسة شائعة",
            issue_type: "uncategorized",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicDiacriticsRule` (12): `AR_DIACRITICS_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("diacritics.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_DIACRITICS_REPLACE",
            description: "كلمات مشكولة للتوضيح",
            short: "كلمات يستحسن أن تشكّل لتصحيح نطقها",
            message: "'$match' كلمة يشيع نطقها نطقا خاطئا لذا نقترح تشكيلها كالآتي: $suggestions",
            suggestions_separator: " أو ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "الأسلوب",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicDarjaRule` (13): `AR_DARJA_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("darja.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_DARJA_REPLACE",
            description: "كلمات بديلة للكلمات العامية أو الأجنبية",
            short: "كلمات بديلة للكلمات العامية أو الأجنبية",
            message: "الكلمة عامية  أو أجنبية يفضل أن يقال $suggestions",
            suggestions_separator: " أو  ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "الأسلوب",
            issue_type: "locale-violation",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicHomophonesRule` (14): `AR_HOMOPHONES_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("homophones.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_HOMOPHONES_REPLACE",
            description: "كلمات متشابهة لفظا للتوضيح، يرجى التحقق منها مثل تشابه الظاء والضاد.",
            short: "كلمات متشابهة لفظا يرجى التحقق منها",
            message: "قل $suggestions",
            suggestions_separator: " أو  ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "CONFUSED_WORDS",
            category_name: "كلمات ملتبسة شائعة",
            issue_type: "uncategorized",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicRedundancyRule` (15): `AR_REDUNDANCY_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("redundancies.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_REDUNDANCY_REPLACE",
            description: "1. تكرار (عام)",
            short: "تكرار",
            message: "'$match' تعبير فيه تكرار.في بعض الحالات، يستحسن استعمال $suggestions",
            suggestions_separator: " أو ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "العبارات المسهَبة",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicWordinessRule` (17): `AR_WORDINESS_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("wordiness.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_WORDINESS_REPLACE",
            description: "2. حشو(تعبير فيه تكرار)",
            short: "حشو (تعبير فيه تكرار)",
            message: "'$match' تعبير فيه حشو يفضل أن يقال $suggestions",
            suggestions_separator: " أو ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "العبارات المسهَبة",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    Ok(instances)
}
