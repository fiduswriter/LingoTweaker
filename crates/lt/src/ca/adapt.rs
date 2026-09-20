//! `org.languagetool.rules.AdaptSuggestionsFilter` with the Catalan
//! `Catalan.adaptSuggestion` override (the contraction/apostrophe rewriting
//! applied to every suggestion of the 197 XML rules that use the filter).

use lt_core::Suggestion;
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

/// `Catalan.adaptSuggestion(s, originalErrorStr)`.
pub fn adapt_suggestion(s: &str, original_error_str: &str) -> String {
    // Exceptions: Digues-me alguna cosa, urbi et orbi, Guns N' Roses
    let capitalized = lt_tagger::is_capitalized_word(s);
    let mut s = s.replace("gens traça", "gens de traça");
    s = s.replace("gens facilitat", "gens de facilitat");
    s = ca_contractions().replace_all(&s, "${1}${2}").into_owned();
    s = ca_contractions2().replace_all(&s, "${1}${2}").into_owned();
    s = ca_apostrophes1().replace_all(&s, "${1}").into_owned();
    s = ca_apostrophes2().replace_all(&s, "e${1} ${2}").into_owned();
    if !s.contains("en el") && !s.contains("-se") {
        s = ca_apostrophes3().replace_all(&s, "${1}'${2}").into_owned();
    }
    s = ca_apostrophes4().replace_all(&s, "${1}'${2}").into_owned();
    s = ca_apostrophes5().replace_all(&s, "${1}${2}").into_owned();
    s = ca_apostrophes6().replace_all(&s, "se'${1}").into_owned();
    s = ca_apostrophes7()
        .replace_all(&s, "${1} l'${2}")
        .into_owned();
    // T'comença -> Et comença
    s = ca_apostrophes8()
        .replace_all(&s, |caps: &regex::Captures| {
            let group1 = caps
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            let group2 = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            format!("E{group1} {group2}")
        })
        .into_owned();
    s = ca_apostrophes9().replace_all(&s, "${1}e ${2}").into_owned();
    s = ca_remove_spaces().replace_all(&s, "${1}${2}").into_owned();
    if capitalized {
        s = lt_tagger::uppercase_first_char(&s);
    }
    s = s.replace(" ,", ",");
    preserve_case(&s, original_error_str)
}

/// `StringTools.preserveCase`.
pub(crate) fn preserve_case(input: &str, model: &str) -> String {
    if model.is_empty() {
        return input.to_string();
    }
    if lt_tagger::is_capitalized_word(model) {
        return lt_tagger::uppercase_first_char(&input.to_lowercase());
    }
    if lt_tagger::is_all_uppercase(model) {
        return input.to_uppercase();
    }
    input.to_string()
}

fn ca_contractions() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\b([Aa]|[DdPp]e)r? e(ls?)\b").unwrap())
}

fn ca_contractions2() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\b([Dd])[' ]+(els?)\b").unwrap())
}

fn ca_apostrophes1() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\b([LDNSTMldnstm]['’]) ").unwrap())
}

fn ca_apostrophes2() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"\b([mtlsn])['’]([^1haeiouáàèéíòóúA-ZÀÈÉÍÒÓÚ“«"])"#).unwrap()
    })
}

fn ca_apostrophes3() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\be?([mtsldn])e? (h[aeio]|h?[aeiouàèéíòóú][a-zàèéíòóúïüç])")
            .unwrap()
    })
}

fn ca_apostrophes4() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\b(l)a ([aeoàúèéí][^ ])").unwrap())
}

fn ca_apostrophes5() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\b([mts]e) (['’])").unwrap())
}

fn ca_apostrophes6() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\bs'e(ns|ls)\b").unwrap())
}

fn ca_apostrophes7() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\b(de|a)l (h?[aeoàúèéí][^ ])").unwrap())
}

fn ca_apostrophes8() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"\b([MTLSN])['’]([^1haeiouáàèéíòóúA-ZÀÈÉÍÒÓÚ“«"])"#).unwrap()
    })
}

fn ca_apostrophes9() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"\b([Dd])['’]([^1haeiouáàèéíòóúA-ZÀÈÉÍÒÓÚ“«"])"#).unwrap()
    })
}

/// The negative lookahead needs `fancy_regex` (the `regex` crate has no
/// lookaround).
fn ca_remove_spaces() -> &'static fancy_regex::Regex {
    static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| fancy_regex::Regex::new(r"(?i)\b(a|de|pe) (ls?)(?!['’])\b").unwrap())
}

/// `org.languagetool.rules.AdaptSuggestionsFilter` (Catalan: every
/// suggestion is passed through `Catalan.adaptSuggestion` with the match
/// text as the case model).
pub struct AdaptSuggestionsFilter;

impl RuleFilter for AdaptSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let original_error_str = ctx
            .sentence_text
            .get(ctx.match_range.start..ctx.match_range.end)
            .unwrap_or_default();
        let suggestions: Vec<Suggestion> = ctx
            .suggestions
            .iter()
            .map(|s| Suggestion {
                value: adapt_suggestion(&s.value, original_error_str),
                short_description: s.short_description.clone(),
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions),
        }
    }
}
