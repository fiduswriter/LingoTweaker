//! Portuguese `AbstractSimpleReplaceRule2` instances (barbarisms, clichés,
//! redundancies, wordiness, Wikipedia, diacritics) and the variant-specific
//! additions from `PortugalPortuguese`/`BrazilianPortuguese`.
//!
//! The common rule list of `Portuguese.getRelevantRules` and the variant
//! classes both instantiate these classes with the same rule id, so the
//! instances are kept separate (Java runs both; the overlap filter collapses
//! identical spans). `pt-AO`/`pt-MZ` inherit the base list only; their
//! `ResourceBundle` resolution falls back to `MessagesBundle_pt_PT` through
//! `Portuguese.getDefaultLanguageVariant()` (probed 2026-09-19), so they use
//! the pt-PT category names, not the core English ones.

use std::path::Path;

use lt_core::Result;

use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};

/// The `MessagesBundle_pt_{PT,BR}` category names; plain `pt`, `pt-AO` and
/// `pt-MZ` fall back to pt-PT (`ResourceBundleTools.getMessageBundle` uses
/// `getDefaultLanguageVariant()` when no language-level bundle exists).
pub(crate) struct CategoryNames {
    pub style: &'static str,
    pub redundancy: &'static str,
    pub wikipedia: &'static str,
    pub typos: &'static str,
}

pub(crate) fn categories(variant: &str) -> CategoryNames {
    match variant {
        "pt-BR" => CategoryNames {
            style: "Estilo",
            redundancy: "Redundância",
            wikipedia: "Regras específicas da Wikipédia",
            typos: "Erro de Escrita",
        },
        "pt-PT" => CategoryNames {
            style: "Estilo",
            redundancy: "Frases Redundantes",
            wikipedia: "Wikipédia",
            typos: "Erros Ortográficos",
        },
        _ => CategoryNames {
            style: "Estilo",
            redundancy: "Frases Redundantes",
            wikipedia: "Wikipédia",
            typos: "Erros Ortográficos",
        },
    }
}

/// All `AbstractSimpleReplaceRule2` instances of the requested variant, in
/// Java's order (base list first, then the variant additions).
#[allow(clippy::vec_init_then_push)]
pub fn pt_simple_replace_instances(
    data_dir: &Path,
    variant: &str,
) -> Result<Vec<SimpleReplaceRule>> {
    let rules = data_dir.join("pt/rules");
    let c = categories(variant);
    let mut instances = Vec::new();

    // `Portuguese.getRelevantRules` (common list)
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("barbarisms.txt")],
        SimpleReplaceConfig {
            rule_id: "PT_BARBARISMS_REPLACE",
            description: "Palavras de origem estrangeira evitáveis: $match",
            short: "Estrangeirismo",
            message: "\"$match\" é um estrangeirismo. É preferível dizer $suggestions.",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: c.style,
            issue_type: "locale-violation",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::ProperNounImmunizedOrEnglishIgnore,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("cliches.txt")],
        SimpleReplaceConfig {
            rule_id: "PT_CLICHE_REPLACE",
            description: "Frases-feitas e expressões idiomáticas: $match",
            short: "Frase-feita",
            message: "\"$match\" é uma frase-feita. É preferível dizer $suggestions.",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: c.style,
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("redundancies.txt")],
        SimpleReplaceConfig {
            rule_id: "PT_REDUNDANCY_REPLACE",
            description: "1. Pleonasmos e redundâncias: $match",
            short: "Pleonasmo",
            message: "\"$match\" é um pleonasmo. É preferível dizer $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: c.redundancy,
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("wordiness.txt")],
        SimpleReplaceConfig {
            rule_id: "PT_WORDINESS_REPLACE",
            description: "2. Expressões prolixas: $match",
            short: "Expressão prolixa",
            message: "\"$match\" é uma expressão prolixa. É preferível dizer $suggestions.",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: c.redundancy,
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("wikipedia.txt")],
        SimpleReplaceConfig {
            rule_id: "PT_WIKIPEDIA_COMMON_ERRORS",
            description: "Erros frequentes nos artigos da Wikipédia: $match",
            short: "Erro gramatical ou de normativa",
            message: "Possível erro em \"$match\". Prefira $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "WIKIPEDIA",
            category_name: c.wikipedia,
            issue_type: "grammar",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("diacritics.txt")],
        SimpleReplaceConfig {
            rule_id: "PT_DIACRITICS_REPLACE",
            description: "Palavras estrangeiras com diacríticos: $match",
            short: "A palavra estrangeira original tem diacrítico",
            message: "'$match' é uma expressão estrangeira importada cuja grafia tem diacríticos. É preferível escrever $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "TYPOS",
            category_name: c.typos,
            issue_type: "misspelling",
            default_off: true,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);

    // Variant additions in Java's rule-list order (`PortugalPortuguese`:
    // replace (65) → barbarisms (66) → archaisms (67) → cliché (68) →
    // redundancy (69) → wordiness (70) → wikipedia (71); `BrazilianPortuguese`
    // the same without archaisms' position change).
    if variant == "pt-PT" || variant == "pt-BR" {
        let dir = rules.join(variant);
        // `PortugalPortugueseReplaceRule` / `BrazilianPortugueseReplaceRule`.
        let (rule_id, description, short, message) = if variant == "pt-PT" {
            (
                "PT_PT_SIMPLE_REPLACE",
                "Brasileirismo: 1. palavras confundidas com as de Portugal",
                "Palavra de português do Brasil",
                "'$match' é uma expressão brasileira, em português de Portugal utiliza-se: $suggestions",
            )
        } else {
            (
                "PT_BR_SIMPLE_REPLACE",
                "Palavras portuguesas facilmente confundidas com as do Brasil",
                "Palavra do português de Portugal",
                "\"$match\" é uma expressão usada sobretudo em Portugal. No português brasileiro diz-se $suggestions",
            )
        };
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("replace.txt")],
            SimpleReplaceConfig {
                rule_id,
                description,
                short,
                message,
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: c.style,
                issue_type: "locale-violation",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::ProperNounOrImmunized,
            },
        )?);
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("barbarisms.txt")],
            SimpleReplaceConfig {
                rule_id: "PT_BARBARISMS_REPLACE",
                description: "Palavras de origem estrangeira evitáveis: $match",
                short: "Estrangeirismo",
                message: "\"$match\" é um estrangeirismo. É preferível dizer $suggestions.",
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: c.style,
                issue_type: "locale-violation",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::ProperNounImmunizedOrEnglishIgnore,
            },
        )?);
        // `PortugueseArchaismsRule` (variant-only; the class is commented
        // out in the base `Portuguese` list).
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("archaisms.txt")],
            SimpleReplaceConfig {
                rule_id: "PT_ARCHAISMS_REPLACE",
                description: "Palavras arcaicas evitáveis",
                short: "Arcaísmo",
                message: "\"$match\" é um arcaísmo. É preferível dizer $suggestions.",
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: c.style,
                issue_type: "locale-violation",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::None,
            },
        )?);
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("cliches.txt")],
            SimpleReplaceConfig {
                rule_id: "PT_CLICHE_REPLACE",
                description: "Frases-feitas e expressões idiomáticas: $match",
                short: "Frase-feita",
                message: "\"$match\" é uma frase-feita. É preferível dizer $suggestions.",
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: c.style,
                issue_type: "style",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::None,
            },
        )?);
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("redundancies.txt")],
            SimpleReplaceConfig {
                rule_id: "PT_REDUNDANCY_REPLACE",
                description: "1. Pleonasmos e redundâncias: $match",
                short: "Pleonasmo",
                message: "\"$match\" é um pleonasmo. É preferível dizer $suggestions",
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "REDUNDANCY",
                category_name: c.redundancy,
                issue_type: "style",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::None,
            },
        )?);
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("wordiness.txt")],
            SimpleReplaceConfig {
                rule_id: "PT_WORDINESS_REPLACE",
                description: "2. Expressões prolixas: $match",
                short: "Expressão prolixa",
                message: "\"$match\" é uma expressão prolixa. É preferível dizer $suggestions.",
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "REDUNDANCY",
                category_name: c.redundancy,
                issue_type: "style",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::None,
            },
        )?);
        instances.push(SimpleReplaceRule::from_files(
            &[dir.join("wikipedia.txt")],
            SimpleReplaceConfig {
                rule_id: "PT_WIKIPEDIA_COMMON_ERRORS",
                description: "Erros frequentes nos artigos da Wikipédia: $match",
                short: "Erro gramatical ou de normativa",
                message: "Possível erro em \"$match\". Prefira $suggestions",
                suggestions_separator: " ou ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "WIKIPEDIA",
                category_name: c.wikipedia,
                issue_type: "grammar",
                default_off: false,
                picky: false,
                has_suggestions: true,
                checking_case: false,
                ignore_short_uppercase_words: true,
                is_token_exception: TokenException::None,
            },
        )?);
    }

    Ok(instances)
}
