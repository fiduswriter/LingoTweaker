//! Catalan `AbstractSimpleReplaceRule2` instances: `SimpleReplaceMultiwordsRule`
//! (16) and `SimpleReplaceAnglicism` (19). The anglicism rule keeps the Java
//! `isRuleMatchException` (English-context words) and post-processes its
//! matches through `ConvertToGenderAndNumberFilter` unless the underlined
//! error spans several tokens.

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match};

use crate::ca::filters::Env;
use crate::ca::legacy_simple_replace::apply_filter;
use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};

pub const MULTIWORDS_ID: &str = "CA_SIMPLE_REPLACE_MULTIWORDS";
pub const ANGLICISM_ID: &str = "CA_SIMPLE_REPLACE_ANGLICISM";
pub const CHECK_CASE_ID: &str = "CA_CHECKCASE";

/// `SimpleReplaceMultiwordsRule` (16): `/ca/replace_multiwords.txt`, GRAMMAR.
pub fn catalan_multiwords_instance(data_dir: &Path) -> lt_core::Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("ca/rules/replace_multiwords.txt")],
        SimpleReplaceConfig {
            rule_id: MULTIWORDS_ID,
            description: "Expressions inadequades: $match",
            short: "Expressió inadequada",
            message: "Expressió incorrecta.",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "GRAMMAR",
            category_name: "Gramàtica",
            issue_type: "grammar",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

/// `SimpleReplaceAnglicism` (19): `/ca/replace_anglicism.txt`, STYLE.
pub fn catalan_anglicism_instance(data_dir: &Path) -> lt_core::Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("ca/rules/replace_anglicism.txt")],
        SimpleReplaceConfig {
            rule_id: ANGLICISM_ID,
            description: "Anglicismes innecessaris: $match",
            short: "Anglicisme innecessari",
            message: "Anglicisme innecessari. Considereu fer servir una altra paraula.",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "Estil",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::CatalanAnglicism,
        },
    )
}

/// `CheckCaseRule` (21) via `AbstractCheckCaseRule` (`/ca/check_case.txt`).
pub fn catalan_check_case_instance(data_dir: &Path) -> lt_core::Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("ca/rules/check_case.txt")],
        SimpleReplaceConfig {
            rule_id: CHECK_CASE_ID,
            description: "Comprova majúscules i minúscules: $match",
            short: "Majúscules i minúscules",
            message: "Majúscules i minúscules recomanades. Alguns llibres d'estil poden suggerir solucions diferents en alguns casos.",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "CASING",
            category_name: "Capitalization",
            issue_type: "typographical",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: true,
            ignore_short_uppercase_words: false,
            is_token_exception: TokenException::None,
        },
    )
}

/// `SimpleReplaceAnglicism.isRuleMatchException`: accept English words in
/// English sentences (two adjacent `_english_ignore_` tokens around the
/// match).
fn is_rule_match_exception(m: &Match, tokens: &[&AnalyzedTokenReadings]) -> bool {
    let mut start_index = 0usize;
    while start_index < tokens.len() && tokens[start_index].start_pos < m.range.start {
        start_index += 1;
    }
    let mut end_index = start_index;
    while end_index < tokens.len() && tokens[end_index].end_pos() < m.range.end {
        end_index += 1;
    }
    let english = |i: usize| {
        tokens
            .get(i)
            .is_some_and(|t| t.has_pos_tag("_english_ignore_"))
    };
    if start_index > 1 && english(start_index) && english(start_index - 1) {
        return true;
    }
    if end_index + 1 < tokens.len() && english(end_index) && english(end_index + 1) {
        return true;
    }
    false
}

/// Number of non-whitespace tokens the match range covers
/// (`RuleMatch.isUnderlinedErrorSingleToken`).
fn covered_token_count(m: &Match, tokens: &[&AnalyzedTokenReadings]) -> usize {
    tokens
        .iter()
        .filter(|t| !t.is_whitespace && t.start_pos < m.range.end && t.end_pos() > m.range.start)
        .count()
}

/// `SimpleReplaceAnglicism.match`: base matches → English-context exception →
/// `ConvertToGenderAndNumberFilter(lemmaSelect:[NA].*)` for single-token
/// matches.
pub fn check_anglicism(
    rule: &SimpleReplaceRule,
    env: &Env,
    tokens: &[AnalyzedTokenReadings],
    sentence: &AnalyzedSentence,
    sentence_offset: usize,
) -> Vec<Match> {
    let non_blank: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut out = Vec::new();
    for m in rule.check_sentence(tokens, sentence_offset) {
        if is_rule_match_exception(&m, &non_blank) {
            continue;
        }
        if covered_token_count(&m, &non_blank) > 1 {
            out.push(m);
            continue;
        }
        let filter = crate::ca::gender_number::ConvertToGenderAndNumberFilter {
            env: std::sync::Arc::clone(env),
        };
        let mut args = HashMap::new();
        args.insert("lemmaSelect".to_string(), "[NA].*".to_string());
        if let Some(m) = apply_filter(&filter, m, sentence, tokens, args) {
            out.push(m);
        }
    }
    out
}
