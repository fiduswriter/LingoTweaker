//! Catalan stage-3 suggestion filters over the synthesizer/speller:
//! `AdvancedSynthesizerFilter` (60 XML refs, with the Catalan
//! `NounToVerbHelper.getVerbFromNoun` hook) and `FindSuggestionsFilter`
//! (51 refs, `AbstractFindSuggestionsFilter` over the Catalan tagger and the
//! `ca-ES_spelling.dict` `findSimilarWords`).

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Suggestion};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::adapt::adapt_suggestion;
use crate::ca::filters::Env;
use crate::ca::helpers::get_verb_from_noun;

const MAX_SUGGESTIONS: usize = 10;

fn pos_tag_regex(pattern: &str) -> Option<fancy_regex::Regex> {
    fancy_regex::Regex::new(&format!("^(?:{pattern})$")).ok()
}

/// `RuleFilter.getPosition`.
pub(crate) fn get_position(
    from_str: &str,
    pattern_tokens: &[&AnalyzedTokenReadings],
    from_pos: usize,
) -> Option<usize> {
    let i = if let Some(rest) = from_str.strip_prefix("marker") {
        let mut i = 0usize;
        while i < pattern_tokens.len()
            && (pattern_tokens[i].start_pos < from_pos || pattern_tokens[i].is_sentence_start)
        {
            i += 1;
        }
        let mut i = i + 1;
        if !rest.is_empty() {
            i += rest.parse::<usize>().ok()?;
        }
        i
    } else {
        from_str.parse::<usize>().ok()?
    };
    if i < 1 || i > pattern_tokens.len() {
        return None;
    }
    Some(i - 1)
}

/// Java `AnalyzedSentence.getText()` concatenates the *token* texts (which
/// are normalized, e.g. `’` → `'`); `RuleMatch.getOriginalErrorStr()` is a
/// substring of that text, not of the raw sentence.
fn matched_text(ctx: &FilterContext) -> String {
    let mut out = String::new();
    for t in ctx.sentence_tokens {
        if t.start_pos >= ctx.match_range.start && t.start_pos < ctx.match_range.end {
            out.push_str(t.surface());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// AdvancedSynthesizerFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.AdvancedSynthesizerFilter`
/// (`AbstractAdvancedSynthesizerFilter` with the Catalan
/// `getNewLemma` override).
pub struct AdvancedSynthesizerFilter {
    pub(crate) env: Env,
}

impl AdvancedSynthesizerFilter {
    /// `getAnalyzedToken`: the first reading whose POS tag fully matches
    /// `regexp` (null tags are tested as `UNKNOWN`).
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

    /// `getCompositePostag`.
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
        if !new_lemma.is_empty() {
            if new_lemma.starts_with('_') {
                // Catalan `getNewLemma` override: `getVerbFromNoun`.
                match get_verb_from_noun(&desired_lemma) {
                    Some(verb) => desired_lemma = verb.to_string(),
                    None => return FilterOutcome::reject(),
                }
            } else {
                desired_lemma = new_lemma.clone();
            }
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
        // `AbstractAdvancedSynthesizerFilter`: `language.adaptSuggestion(s, "")`
        let replacements_list: Vec<String> = replacements_list
            .into_iter()
            .map(|s| adapt_suggestion(&s, ""))
            .collect();
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

// ---------------------------------------------------------------------------
// FindSuggestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.FindSuggestionsFilter`.
pub struct FindSuggestionsFilter {
    pub(crate) env: Env,
}

impl FindSuggestionsFilter {
    /// `FindSuggestionsFilter.getSpellingSuggestions`:
    /// `MorfologikSpeller.findSimilarWords(atr.getToken())`.
    pub(crate) fn spelling_suggestions(&self, word: &str) -> Vec<String> {
        self.env
            .spelling
            .dict_speller()
            .speller()
            .find_similar_word_candidates(word)
            .into_iter()
            .map(|c| c.word)
            .collect()
    }

    /// `FindSuggestionsFilter.isSuggestionException`.
    fn is_suggestion_exception(&self, atr: &AnalyzedTokenReadings) -> bool {
        const LEMMAS_TO_IGNORE: [&str; 6] =
            ["enterar", "sentar", "conseguir", "alcançar", "liar", "vore"];
        const LEMMAS_TO_ALLOW: [&str; 2] = ["enter", "sentir"];
        atr.readings.iter().any(|r| {
            r.stem
                .as_deref()
                .is_some_and(|l| LEMMAS_TO_IGNORE.contains(&l))
        }) && !atr.readings.iter().any(|r| {
            r.stem
                .as_deref()
                .is_some_and(|l| LEMMAS_TO_ALLOW.contains(&l))
        })
    }

    /// `FindSuggestionsFilter.preProcessWrongWord`: remove spaces and
    /// normalize ela geminada separators.
    fn pre_process_wrong_word(word: &str) -> String {
        static ELA_GEMINADA: std::sync::LazyLock<fancy_regex::Regex> =
            std::sync::LazyLock::new(|| {
                fancy_regex::Regex::new(r"(?i)(l)[\.\u{2022}\u{22C5}\u{2219}\u{F0D7}\-](l)")
                    .unwrap()
            });
        let word = word.replace(' ', "");
        ELA_GEMINADA.replace_all(&word, "$1·$2").into_owned()
    }
}

impl RuleFilter for FindSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(word_from) = ctx.args.get("wordFrom") else {
            return FilterOutcome::reject();
        };
        let Some(desired_postag) = ctx.args.get("desiredPostag") else {
            return FilterOutcome::reject();
        };
        let priority_postag = ctx.args.get("priorityPostag");
        let remove_suggestions_regexp = ctx.args.get("removeSuggestionsRegexp");
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let mode = ctx.args.get("Mode");
        let diacritics_mode = mode.map(|m| m == "diacritics").unwrap_or(false);
        let Some(desired_re) = pos_tag_regex(desired_postag) else {
            return FilterOutcome::reject();
        };
        let priority_re = priority_postag.and_then(|p| pos_tag_regex(p));
        let regexp_pattern = remove_suggestions_regexp
            .and_then(|r| fancy_regex::Regex::new(&format!("^(?:{r})$")).ok());

        let atr_word_token: String = if word_from == "inmarker" {
            Self::pre_process_wrong_word(&matched_text(ctx))
        } else {
            let Some(pos) = get_position(word_from, ctx.pattern_tokens, ctx.match_range.start)
            else {
                return FilterOutcome::reject();
            };
            ctx.pattern_tokens[pos].surface().to_string()
        };
        let is_word_capitalized = lt_tagger::is_capitalized_word(&atr_word_token);
        let is_word_all_upper = lt_tagger::is_all_uppercase(&atr_word_token);

        // if the original token already meets the requirements, nothing to do
        for atr in self.env.tagger.tag_word(&atr_word_token) {
            if let Some(tag) = &atr.pos_tag {
                if desired_re.is_match(tag).unwrap_or(false) && diacritics_mode {
                    return FilterOutcome::reject();
                }
            }
        }

        let mut replacements: Vec<String> = Vec::new();
        let mut used_priority_pos = 0usize;
        for suggestion in self.spelling_suggestions(&atr_word_token) {
            let analyzed_suggestion =
                AnalyzedTokenReadings::new(self.env.tagger.tag_word(&suggestion));
            if self.is_suggestion_exception(&analyzed_suggestion) {
                continue;
            }
            if replacements.len() >= 2 * MAX_SUGGESTIONS {
                break;
            }
            if suggestion != atr_word_token
                && analyzed_suggestion.has_pos_tag_matching_fancy(&desired_re)
                && !replacements.contains(&suggestion)
                && !replacements.contains(&suggestion.to_lowercase())
            {
                let diacritics_ok = !diacritics_mode
                    || crate::multitoken::remove_diacritics(&suggestion).to_lowercase()
                        == crate::multitoken::remove_diacritics(&atr_word_token).to_lowercase();
                if diacritics_ok {
                    let regexp_ok = regexp_pattern
                        .as_ref()
                        .map(|re| !re.is_match(&suggestion).unwrap_or(false))
                        .unwrap_or(true);
                    if regexp_ok {
                        let mut replacement = suggestion.clone();
                        if is_word_all_upper {
                            replacement = replacement.to_uppercase();
                        }
                        if is_word_capitalized {
                            replacement = lt_tagger::uppercase_first_char(&replacement);
                        }
                        let is_priority = priority_re
                            .as_ref()
                            .map(|re| analyzed_suggestion.has_pos_tag_matching_fancy(re))
                            .unwrap_or(false);
                        if is_priority {
                            replacements
                                .insert(used_priority_pos.min(replacements.len()), replacement);
                            used_priority_pos += 1;
                        } else {
                            replacements.push(replacement);
                        }
                    }
                }
            }
        }

        let match_contains_finished_suggestion = ctx
            .suggestions
            .iter()
            .any(|s| !s.value.contains("{suggestion}"));
        if diacritics_mode && replacements.is_empty() && !match_contains_finished_suggestion {
            return FilterOutcome::reject();
        }
        if replacements.is_empty() && suppress_match && !match_contains_finished_suggestion {
            return FilterOutcome::reject();
        }

        let mut definitive: Vec<String> = Vec::new();
        let mut replacements_used = false;
        for s in &ctx.suggestions {
            if s.value.contains("{suggestion}")
                || s.value.contains("{Suggestion}")
                || s.value.contains("{SUGGESTION}")
            {
                replacements_used = true;
                for s2 in &replacements {
                    if definitive.len() >= MAX_SUGGESTIONS {
                        break;
                    }
                    if s.value.contains("{suggestion}") {
                        let candidate = s.value.replace("{suggestion}", s2);
                        if !definitive.contains(&candidate) {
                            definitive.push(candidate);
                        }
                    } else if s.value.contains("{Suggestion}") {
                        let candidate = s
                            .value
                            .replace("{Suggestion}", &lt_tagger::uppercase_first_char(s2));
                        if !definitive.contains(&candidate) {
                            definitive.push(candidate);
                        }
                    } else {
                        let candidate = s.value.replace("{SUGGESTION}", &s2.to_uppercase());
                        if !definitive.contains(&candidate) {
                            definitive.push(candidate);
                        }
                    }
                }
            } else if !definitive.contains(&s.value) {
                definitive.push(s.value.clone());
            }
        }
        if !replacements_used {
            for replacement in &replacements {
                if definitive.len() >= MAX_SUGGESTIONS {
                    break;
                }
                if !definitive.contains(replacement) {
                    definitive.push(replacement.clone());
                }
            }
        }
        let original_error: String = if word_from == "inmarker" {
            matched_text(ctx)
        } else {
            String::new()
        };
        if let Some(pos) = definitive.iter().position(|s| *s == original_error) {
            definitive.remove(pos);
        }
        if !definitive.is_empty() {
            let mut seen: Vec<String> = Vec::new();
            let mut out: Vec<Suggestion> = Vec::new();
            for value in definitive {
                if !seen.contains(&value) {
                    seen.push(value.clone());
                    out.push(Suggestion {
                        value,
                        short_description: None,
                    });
                }
            }
            return FilterOutcome {
                accepted: true,
                range: None,
                message: None,
                suggestions: Some(out),
            };
        }
        FilterOutcome::accept()
    }
}
