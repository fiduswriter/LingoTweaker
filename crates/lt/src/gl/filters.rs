//! Galician XML-referenced filter classes. The only class referenced by
//! `gl/rules/grammar.xml` is
//! `org.languagetool.rules.gl.AdvancedSynthesizerFilter`, an empty
//! `AbstractAdvancedSynthesizerFilter` subclass (the base implementation is
//! ported here).

use std::sync::Arc;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Suggestion};
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::gl::GalicianSynthesizerAdapter;

/// Environment for the Galician filters.
pub struct GlFilterEnv {
    pub synth: Arc<GalicianSynthesizerAdapter>,
}

/// `org.languagetool.rules.gl.AdvancedSynthesizerFilter`
/// (`AbstractAdvancedSynthesizerFilter`).
struct AdvancedSynthesizerFilter {
    env: Arc<GlFilterEnv>,
}

impl AdvancedSynthesizerFilter {
    fn analyzed_token<'a>(
        &self,
        token: &'a AnalyzedTokenReadings,
        regexp: &str,
    ) -> Option<&'a AnalyzedToken> {
        let re = regex::Regex::new(&format!("^(?:{regexp})$")).ok()?;
        token
            .readings
            .iter()
            .find(|r| {
                let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
                re.is_match(pos_tag)
            })
            .or_else(|| token.readings.first())
    }

    fn composite_postag(
        &self,
        lemma_select: &str,
        postag_select: &str,
        original_postag: &str,
        desired_postag: &str,
        postag_replace: &str,
    ) -> String {
        let a_pattern = regex::Regex::new(&format!("^(?:{lemma_select})$")).ok();
        let b_pattern = regex::Regex::new(&format!("^(?:{postag_select})$")).ok();
        let mut result = postag_replace.to_string();
        let (Some(a_pattern), Some(b_pattern)) = (a_pattern, b_pattern) else {
            return result;
        };
        let (Some(a_caps), Some(b_caps)) = (
            a_pattern.captures(original_postag),
            b_pattern.captures(desired_postag),
        ) else {
            return result;
        };
        for i in 1..a_caps.len() {
            if let Some(group) = a_caps.get(i) {
                result = result.replace(&format!("\\a{i}"), group.as_str());
            }
        }
        for i in 1..b_caps.len() {
            if let Some(group) = b_caps.get(i) {
                result = result.replace(&format!("\\b{i}"), group.as_str());
            }
        }
        result
    }
}

impl RuleFilter for AdvancedSynthesizerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(postag_select) = ctx.args.get("postagSelect") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_select) = ctx.args.get("lemmaSelect") else {
            return FilterOutcome::reject();
        };
        let Some(postag_from_str) = ctx.args.get("postagFrom") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_from_str) = ctx.args.get("lemmaFrom") else {
            return FilterOutcome::reject();
        };
        let new_lemma = ctx.args.get("newLemma").cloned().unwrap_or_default();
        let resolve = |s: &str| -> Option<usize> {
            if s.starts_with("marker") {
                let mut pos = 0usize;
                while pos < ctx.pattern_tokens.len()
                    && ctx.pattern_tokens[pos].start_pos < ctx.match_range.start
                {
                    pos += 1;
                }
                pos += 1;
                if s.len() > 6 {
                    pos += s.replace("marker", "").parse::<usize>().ok()?;
                }
                Some(pos)
            } else {
                s.parse::<usize>().ok()
            }
        };
        let Some(postag_from) = resolve(postag_from_str) else {
            return FilterOutcome::reject();
        };
        let Some(lemma_from) = resolve(lemma_from_str) else {
            return FilterOutcome::reject();
        };
        if postag_from < 1
            || postag_from > ctx.pattern_tokens.len()
            || lemma_from < 1
            || lemma_from > ctx.pattern_tokens.len()
        {
            return FilterOutcome::reject();
        }
        let postag_replace = ctx.args.get("postagReplace");
        let lemma_token = ctx.pattern_tokens[lemma_from - 1];
        let Some(mut desired_lemma) = self
            .analyzed_token(lemma_token, lemma_select)
            .and_then(|t| t.stem.clone())
        else {
            return FilterOutcome::reject();
        };
        let original_postag = self
            .analyzed_token(lemma_token, lemma_select)
            .and_then(|t| t.pos_tag.clone())
            .unwrap_or_default();
        let Some(desired_postag) = self
            .analyzed_token(ctx.pattern_tokens[postag_from - 1], postag_select)
            .and_then(|t| t.pos_tag.clone())
        else {
            return FilterOutcome::reject();
        };
        if !new_lemma.is_empty() && !new_lemma.starts_with('_') {
            desired_lemma = new_lemma;
        }
        let desired_postag = match postag_replace {
            Some(replace) => self.composite_postag(
                lemma_select,
                postag_select,
                &original_postag,
                &desired_postag,
                replace,
            ),
            None => desired_postag,
        };
        let lemma_surface = lemma_token.surface();
        let is_word_capitalized = lt_tagger::is_capitalized_word(lemma_surface);
        let is_word_allupper = lt_tagger::is_all_uppercase(lemma_surface);
        let token = AnalyzedToken::new("", Some(desired_lemma), None);
        let replacements = self
            .env
            .synth
            .inner()
            .synthesize(&token, &desired_postag, true);
        if replacements.is_empty() {
            return FilterOutcome::accept();
        }
        let mut replacements_list: Vec<String> = Vec::new();
        let mut suggestion_used = false;
        for r in &ctx.suggestions {
            for nr in &replacements {
                if r.value.contains("{suggestion}")
                    || r.value.contains("{Suggestion}")
                    || r.value.contains("{SUGGESTION}")
                {
                    suggestion_used = true;
                }
                let mut nr = nr.clone();
                if is_word_capitalized {
                    nr = lt_tagger::uppercase_first_char(&nr);
                }
                if is_word_allupper {
                    nr = nr.to_uppercase();
                }
                let complete = r
                    .value
                    .replace("{suggestion}", &nr)
                    .replace("{Suggestion}", &lt_tagger::uppercase_first_char(&nr))
                    .replace("{SUGGESTION}", &nr.to_uppercase());
                if !replacements_list.contains(&complete) {
                    replacements_list.push(complete);
                }
            }
        }
        if !suggestion_used {
            replacements_list.extend(replacements);
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(
                replacements_list
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}

/// `galician_filter_registry`: the filter classes referenced by the Galician
/// XML rule data.
pub fn galician_filter_registry(synth: Arc<GalicianSynthesizerAdapter>) -> FilterRegistry {
    let env = Arc::new(GlFilterEnv { synth });
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.gl.AdvancedSynthesizerFilter",
        Arc::new(AdvancedSynthesizerFilter { env }),
    );
    builder.build()
}
