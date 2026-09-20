//! Post-matching filter framework (P1.6), mirroring LT's
//! `RuleFilter` / `RuleFilterEvaluator` contract: after a pattern match, a
//! `<filter class="..." args="..."/>` element invokes a registry
//! implementation with the resolved arguments, the matched token span, and
//! the current match. The filter returns the (possibly modified) match or
//! rejects it.
//!
//! Argument resolution (`RuleFilterEvaluator.getResolvedArguments`):
//! whitespace-separated `key:value` pairs; values starting with `\N` are
//! back-references resolved through the skip-corrected token position
//! (`tokenPositions` = per-pattern-element consumed token counts).

use std::collections::HashMap;
use std::sync::Arc;

use lt_core::{AnalyzedTokenReadings, Suggestion, TextRange};

/// Resolved arguments plus everything a filter may need about the match.
pub struct FilterContext<'a> {
    /// Rule id of the rule the filter is attached to (LT
    /// `match.getRule().getId()`; `""` for synthetic inner contexts).
    pub rule_id: &'a str,
    pub args: HashMap<String, String>,
    /// tokens of the matched span (`tokens[first_match..=last_match]`,
    /// including whitespace and skipped tokens, like LT's `patternTokens`)
    pub pattern_tokens: &'a [&'a AnalyzedTokenReadings],
    /// the full analyzed token stream of the sentence (LT
    /// `match.getSentence().getTokens()`; index 0 is the SENT_START entry)
    pub sentence_tokens: &'a [&'a AnalyzedTokenReadings],
    /// per-pattern-element consumed token counts (LT `tokenPositions`)
    pub token_positions: &'a [usize],
    /// absolute index of the first matched token (LT `patternTokenPos`)
    pub pattern_token_pos: usize,
    /// current match range within the sentence (UTF-8 bytes)
    pub match_range: TextRange,
    pub sentence_text: &'a str,
    pub message: String,
    pub short_message: Option<String>,
    pub suggestions: Vec<Suggestion>,
}

/// Result of a filter invocation (LT returns the match or `null`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterOutcome {
    /// false = reject the match (LT `null`)
    pub accepted: bool,
    pub range: Option<TextRange>,
    pub message: Option<String>,
    pub suggestions: Option<Vec<Suggestion>>,
}

impl FilterOutcome {
    pub fn accept() -> Self {
        Self {
            accepted: true,
            ..Default::default()
        }
    }

    pub fn reject() -> Self {
        Self {
            accepted: false,
            ..Default::default()
        }
    }
}

pub trait RuleFilter: Send + Sync {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome;

    /// Java filters that rebuild the match with the plain `RuleMatch(rule,
    /// sentence, …)` constructor lose the rule's match type (`Hint`): return
    /// the replacement type name (e.g. `"Other"`).
    fn rebuilds_match_type(&self) -> Option<&'static str> {
        None
    }

    /// `RegexRuleFilter` variant: the filter receives the regexp captures of
    /// a `<regexp>` rule (`captures[0]` is the whole match; `None` for
    /// non-participating groups, like Java `Matcher.group(n)`). Plain
    /// `RuleFilter`s keep using [`RuleFilter::accept`].
    fn accept_regexp(&self, ctx: &FilterContext, _captures: &[Option<String>]) -> FilterOutcome {
        self.accept(ctx)
    }
}

/// Registry of filter implementations keyed by the Java class name used in
/// the XML (`<filter class="org.languagetool...">`). An unmapped class is a
/// load error (plan §7).
#[derive(Clone, Default)]
pub struct FilterRegistry {
    filters: Arc<HashMap<String, Arc<dyn RuleFilter>>>,
}

impl FilterRegistry {
    pub fn builder() -> FilterRegistryBuilder {
        FilterRegistryBuilder::default()
    }

    pub fn get(&self, class: &str) -> Option<Arc<dyn RuleFilter>> {
        self.filters.get(class).cloned()
    }

    pub fn len(&self) -> usize {
        self.filters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

#[derive(Default)]
pub struct FilterRegistryBuilder {
    filters: HashMap<String, Arc<dyn RuleFilter>>,
}

impl FilterRegistryBuilder {
    pub fn register(mut self, class: &str, filter: Arc<dyn RuleFilter>) -> Self {
        self.filters.insert(class.to_string(), filter);
        self
    }

    pub fn build(self) -> FilterRegistry {
        FilterRegistry {
            filters: Arc::new(self.filters),
        }
    }
}

/// Skip-corrected reference (LT `RuleFilter.getSkipCorrectedReference`):
/// `ref_number` is a 1-based pattern element number; returns the 0-based
/// index into `pattern_tokens` accounting for skipped tokens.
pub fn skip_corrected_reference(token_positions: &[usize], ref_number: i64) -> i64 {
    if ref_number < 0 {
        return ref_number;
    }
    let mut corrected: i64 = 0;
    for (i, &pos) in token_positions.iter().enumerate() {
        if i as i64 >= ref_number {
            break;
        }
        corrected += pos as i64;
    }
    corrected - 1
}

/// Resolve `<filter args>` (LT `RuleFilterEvaluator.getResolvedArguments`).
pub fn resolve_args(
    args: &str,
    pattern_tokens: &[&AnalyzedTokenReadings],
    token_positions: &[usize],
) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for arg in args.split_whitespace() {
        let Some((key, val)) = arg.split_once(':') else {
            return Err(format!(
                "invalid filter argument syntax, expected 'key:value', got '{arg}'"
            ));
        };
        if let Some(num) = val.strip_prefix('\\') {
            let ref_number: i64 = num
                .parse()
                .map_err(|_| format!("invalid back-reference '{val}'"))?;
            if ref_number < 1 || ref_number as usize > token_positions.len() {
                return Err(format!(
                    "back-reference {ref_number} outside pattern ({})",
                    token_positions.len()
                ));
            }
            let corrected = skip_corrected_reference(token_positions, ref_number);
            let corrected = usize::try_from(corrected).map_err(|_| corrected.to_string())?;
            if corrected >= pattern_tokens.len() {
                return Err(format!(
                    "back-reference {ref_number} resolves past the matched span ({})",
                    pattern_tokens.len()
                ));
            }
            if out
                .insert(
                    key.to_string(),
                    pattern_tokens[corrected].surface().to_string(),
                )
                .is_some()
            {
                return Err(format!("duplicate filter argument key '{key}'"));
            }
        } else if out.insert(key.to_string(), val.to_string()).is_some() {
            return Err(format!("duplicate filter argument key '{key}'"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn tr(surface: &str) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(surface, None, None)],
            chunk_tags: Vec::new(),
            whitespace_before: false,
            start_pos: 0,
            raw_byte_len: 0,
            is_whitespace: surface.chars().all(|c| c.is_whitespace()),
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: false,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        }
    }

    #[test]
    fn resolves_back_references() {
        // element 1 matched "a", one whitespace token skipped, element 2
        // matched "b": tokenPositions = [1, 2]
        let tokens = [tr("a"), tr(" "), tr("b")];
        let refs: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        let args = resolve_args("x:\\1 y:\\2", &refs, &[1, 2]).unwrap();
        assert_eq!(args["x"], "a");
        assert_eq!(args["y"], "b");
    }

    #[test]
    fn rejects_bad_references() {
        let tokens = [tr("a")];
        let refs: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        assert!(resolve_args("x:\\2", &refs, &[1]).is_err());
        assert!(resolve_args("x", &refs, &[1]).is_err());
        assert!(resolve_args("x:lit y:lit", &refs, &[1]).is_ok());
    }
}
