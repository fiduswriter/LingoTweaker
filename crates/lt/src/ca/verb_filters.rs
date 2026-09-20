//! Catalan stage-3 verb/pronoun suggestion filters built on
//! `VerbSynthesizer` + `PronomsFeblesHelper`:
//! `AdjustVerbSuggestionsFilter` (93 XML refs) and `AdjustPronounsFilter`
//! (43 refs).

use lt_core::{AnalyzedTokenReadings, Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::adapt::{adapt_suggestion, preserve_case};
use crate::ca::filters::Env;
use crate::ca::helpers::{
    apostrophe_needed, convert_pronouns_for_intransitive_verb, do_add_pronoun_en,
    do_add_pronoun_reflexive, do_add_pronoun_reflexive_en, do_add_pronoun_reflexive_imperative,
    do_remove_pronoun_reflexive, do_replace_em_en, fix_apostrophes, get_dative_pronoun,
    get_reflexive_pronoun, has_partial_pos_tag, transform, transform_darrere, transform_davant,
    PronounPosition, VerbSynthesizer,
};

/// LT `AnalyzedSentence.getTokensWithoutWhitespace()` (index 0 is
/// SENT_START).
pub(crate) fn tokens_without_whitespace<'a>(
    ctx: &FilterContext<'a>,
) -> Vec<&'a AnalyzedTokenReadings> {
    ctx.sentence_tokens
        .iter()
        .copied()
        .filter(|t| !t.is_whitespace)
        .collect()
}

/// The `posWord` search of both filters.
pub(crate) fn pos_word_start(tokens: &[&AnalyzedTokenReadings], match_start: usize) -> usize {
    let mut pos = 0usize;
    while pos < tokens.len()
        && (tokens[pos].start_pos < match_start || tokens[pos].is_sentence_start)
    {
        pos += 1;
    }
    pos
}

fn get_optional<'a>(
    args: &'a std::collections::HashMap<String, String>,
    key: &str,
    default: &'a str,
) -> &'a str {
    args.get(key).map(|s| s.as_str()).unwrap_or(default)
}

// ---------------------------------------------------------------------------
// AdjustVerbSuggestionsFilter
// ---------------------------------------------------------------------------

pub struct AdjustVerbSuggestionsFilter {
    pub(crate) env: Env,
}

impl AdjustVerbSuggestionsFilter {
    fn any_change_vowel_consonant(original_verb: &str, replacements: &[String]) -> bool {
        let original_needs_apostrophe = apostrophe_needed().is_match(original_verb);
        replacements
            .iter()
            .any(|r| original_needs_apostrophe != apostrophe_needed().is_match(r))
    }
}

impl RuleFilter for AdjustVerbSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        const NEEDS_APOSTROPHE_CHANGE: [&str; 5] = ["de", "d'", "l", "l'", "el"];
        const NEEDS_CONTRACTION_CHANGE: [&str; 4] = ["a", "de", "per", "pe"];
        let number_from_next_words =
            get_optional(&ctx.args, "numberFromNextWords", "false").eq_ignore_ascii_case("true");
        let actions_arg = get_optional(&ctx.args, "actions", "removePronounReflexive");
        let actions: Vec<&str> = actions_arg.split(',').collect();
        let force_number = ctx.args.get("forceNumber").cloned().unwrap_or_default();

        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let mut verb_synth = VerbSynthesizer::new(&tokens, pos_word, self.env.synth.inner(), false);
        // verb found out of bounds
        if verb_synth.is_undefined() {
            return FilterOutcome::reject();
        }
        if tokens[verb_synth.get_last_verb_index() as usize].end_pos() > ctx.match_range.end {
            return FilterOutcome::reject();
        }

        let mut replacements: Vec<String> = Vec::new();
        for suggestion in &ctx.suggestions {
            let mut original_suggestion = suggestion.value.to_lowercase();
            let mut make_intransitive = false;
            let mut desired_number = String::new();
            let mut desired_persona = String::new();
            let mut action = actions.first().copied().unwrap_or_default().to_string();
            if original_suggestion.ends_with(" [intr]") {
                original_suggestion.truncate(original_suggestion.len() - 7);
                make_intransitive = true;
            }
            if original_suggestion.ends_with(" [3s]") {
                original_suggestion.truncate(original_suggestion.len() - 5);
                desired_number = "S".to_string();
                desired_persona = "3".to_string();
            }
            if original_suggestion.starts_with("[datiu] ") {
                original_suggestion = original_suggestion[8..].to_string();
                action = "addPronounDative".to_string();
            }
            let mut new_lemma = original_suggestion.clone();
            let mut after_lemma = String::new();
            if let Some(first_space) = original_suggestion.find(' ') {
                new_lemma = original_suggestion[..first_space].to_string();
                after_lemma = original_suggestion[first_space + 1..].to_string();
                if number_from_next_words {
                    if let Some(sentence) = self.env.analyze_text(&after_lemma) {
                        let toks: Vec<&AnalyzedTokenReadings> = sentence
                            .tokens
                            .iter()
                            .filter(|t| !t.is_whitespace)
                            .collect();
                        if let Some(token) = toks.get(1) {
                            desired_number = if has_partial_pos_tag(token, "S") {
                                "S".to_string()
                            } else {
                                "P".to_string()
                            };
                        }
                    }
                }
            }
            if new_lemma.contains("haver-hi") {
                desired_number = "S".to_string();
            }
            if !force_number.is_empty() {
                desired_number = force_number.clone();
            }
            if new_lemma.ends_with("-se'n") {
                new_lemma.truncate(new_lemma.len() - 5);
                action = "addPronounReflexiveEn".to_string();
            } else if new_lemma.ends_with("-se") {
                new_lemma.truncate(new_lemma.len() - 3);
                action = "addPronounReflexive".to_string();
            } else if new_lemma.ends_with("'s") {
                new_lemma.truncate(new_lemma.len() - 2);
                action = "addPronounReflexive".to_string();
            } else if new_lemma.ends_with("-hi") {
                new_lemma.truncate(new_lemma.len() - 3);
                action = "addPronounHi".to_string();
            } else if new_lemma.ends_with("-s'ho") {
                new_lemma.truncate(new_lemma.len() - 5);
                action = "addPronounReflexiveHo".to_string();
            } else if new_lemma.ends_with("-se-les") {
                new_lemma.truncate(new_lemma.len() - 7);
                action = "addPronounReflexiveLes".to_string();
            } else if new_lemma.ends_with("-s'hi") {
                new_lemma.truncate(new_lemma.len() - 5);
                action = "addPronounReflexiveHi".to_string();
            }
            // synthesize with new lemma
            let mut postags: Vec<String> = Vec::new();
            for reading in &tokens[verb_synth.get_first_verb_index() as usize].readings {
                let Some(tag) = reading.pos_tag.as_deref().filter(|p| p.starts_with('V')) else {
                    continue;
                };
                let mut postag = tag.to_string();
                if !desired_number.is_empty()
                    && postag.get(2..3) != Some("P")
                    && matches!(postag.get(5..6), Some("S") | Some("P"))
                {
                    postag = format!("{}{}{}", &postag[..5], desired_number, &postag[6..]);
                }
                if !desired_persona.is_empty()
                    && postag.get(2..3) != Some("P")
                    && postag
                        .get(4..5)
                        .is_some_and(|c| matches!(c, "1" | "2" | "3"))
                {
                    postag = format!("{}{}{}", &postag[..4], desired_persona, &postag[5..]);
                }
                postags.push(postag);
            }
            let target_postag = self.env.synth.inner().target_pos_tag(&postags, "");
            let mut verb_str = String::new();
            if !target_postag.is_empty() {
                verb_synth.set_lemma_and_postag(&new_lemma, &target_postag);
                verb_str = verb_synth.synthesize();
            }
            let mut pronouns_str = if verb_synth.get_num_pronouns_before() > 0 {
                verb_synth.get_pronouns_str_before()
            } else if verb_synth.get_num_pronouns_after() > 0 {
                verb_synth.get_pronouns_str_after()
            } else {
                String::new()
            };
            pronouns_str = pronouns_str.to_lowercase();
            let is_pronouns_after =
                verb_synth.get_num_pronouns_after() > 0 || !verb_synth.is_first_verb_is();
            let first_verb_persona_number = if action == "addPronounDative" {
                verb_synth.get_first_verb_persona_number()
            } else {
                target_postag.get(4..6).unwrap_or("").to_string()
            };
            let replacement = match action.as_str() {
                "addPronounEn" => {
                    let new_pronoun =
                        do_add_pronoun_en(&pronouns_str, &verb_str, !verb_synth.is_first_verb_is());
                    if new_pronoun.is_empty() {
                        String::new()
                    } else if verb_synth.is_first_verb_is() {
                        format!("{new_pronoun}{verb_str}")
                    } else {
                        format!("{verb_str}{new_pronoun}")
                    }
                }
                "removePronounReflexive" => {
                    do_remove_pronoun_reflexive(&pronouns_str, &verb_str, is_pronouns_after)
                }
                "addPronounReflexiveEn" => do_add_pronoun_reflexive_en(
                    &pronouns_str,
                    &verb_str,
                    &first_verb_persona_number,
                    is_pronouns_after,
                ),
                "replaceEmEn" => do_replace_em_en(&pronouns_str, &verb_str, is_pronouns_after),
                "addPronounReflexive" => do_add_pronoun_reflexive(
                    &pronouns_str,
                    &verb_str,
                    &first_verb_persona_number,
                    is_pronouns_after,
                ),
                "addPronounReflexiveHi" => do_add_pronoun_reflexive(
                    "",
                    &format!("hi {verb_str}"),
                    &first_verb_persona_number,
                    is_pronouns_after,
                ),
                "addPronounReflexiveLes" => do_add_pronoun_reflexive(
                    &format!(
                        "{} les",
                        transform(&pronouns_str.to_lowercase(), PronounPosition::Normalized)
                    ),
                    &verb_str,
                    &first_verb_persona_number,
                    is_pronouns_after,
                ),
                "addPronounDative" => {
                    let dative_pronoun = get_dative_pronoun(&first_verb_persona_number);
                    if is_pronouns_after {
                        format!(
                            "{verb_str}{}",
                            transform_darrere(&dative_pronoun, &verb_str)
                        )
                    } else {
                        format!("{}{verb_str}", transform_davant(&dative_pronoun, &verb_str))
                    }
                }
                "addPronounReflexiveHo" => {
                    let mut reflexive_pronoun = get_reflexive_pronoun(&first_verb_persona_number);
                    if reflexive_pronoun.is_empty() && !pronouns_str.is_empty() {
                        let rp = transform(&pronouns_str, PronounPosition::Normalized);
                        if ["em", "et", "es", "ens", "us", "vos"].contains(&rp.as_str()) {
                            reflexive_pronoun = rp;
                        }
                    }
                    if reflexive_pronoun.is_empty() {
                        reflexive_pronoun = "es".to_string();
                    }
                    let pronouns_normalized = format!("{reflexive_pronoun} ho");
                    if is_pronouns_after {
                        format!(
                            "{verb_str}{}",
                            transform_darrere(&pronouns_normalized, &verb_str)
                        )
                    } else {
                        format!(
                            "{}{verb_str}",
                            transform_davant(&pronouns_normalized, &verb_str)
                        )
                    }
                }
                "addPronounHi" => format!("hi {verb_str}"),
                "addPronounReflexiveImperative" => do_add_pronoun_reflexive_imperative(
                    &pronouns_str,
                    &verb_str,
                    &verb_synth.get_first_verb_persona_number_imperative(),
                ),
                "None" => {
                    if is_pronouns_after {
                        format!("{verb_str}{}", transform_darrere(&pronouns_str, &verb_str))
                    } else {
                        format!("{}{verb_str}", transform_davant(&pronouns_str, &verb_str))
                    }
                }
                _ => String::new(),
            };
            if !replacement.is_empty() {
                let mut replacement = replacement;
                if make_intransitive {
                    replacement = convert_pronouns_for_intransitive_verb(&replacement);
                }
                replacement = fix_apostrophes(&replacement);
                replacement = format!("{} {}", replacement, after_lemma)
                    .trim()
                    .to_string();
                replacements.push(preserve_case(&replacement, &verb_synth.get_casing_model()));
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let mut pos_start_underline =
            (verb_synth.get_first_verb_index() - verb_synth.get_num_pronouns_before()) as usize;
        if verb_synth.get_num_pronouns_before() == 0
            && pos_start_underline > 1
            && NEEDS_APOSTROPHE_CHANGE.contains(
                &tokens[pos_start_underline - 1]
                    .surface()
                    .to_lowercase()
                    .as_str(),
            )
            && Self::any_change_vowel_consonant(&verb_synth.get_verb_str(), &replacements)
        {
            let mut prefix = String::new();
            if pos_start_underline > 2
                && NEEDS_CONTRACTION_CHANGE.contains(
                    &tokens[pos_start_underline - 2]
                        .surface()
                        .to_lowercase()
                        .as_str(),
                )
            {
                prefix.push_str(
                    &verb_synth
                        .get_string_from_to(
                            pos_start_underline as isize - 2,
                            pos_start_underline as isize - 1,
                        )
                        .to_lowercase(),
                );
                if tokens[pos_start_underline].whitespace_before {
                    prefix.push(' ');
                }
                pos_start_underline -= 2;
            } else {
                prefix.push_str(&tokens[pos_start_underline - 1].surface().to_lowercase());
                if tokens[pos_start_underline].whitespace_before {
                    prefix.push(' ');
                }
                pos_start_underline -= 1;
            }
            for replacement in &mut replacements {
                *replacement = format!("{prefix}{replacement}");
            }
        }
        // extend the ending position if there are pronouns after the verb
        let ending_pos = ctx
            .match_range
            .end
            .max(tokens[verb_synth.get_last_index() as usize].end_pos());
        let range = TextRange::new(tokens[pos_start_underline].start_pos, ending_pos);
        let original_str =
            &ctx.sentence_text[tokens[pos_start_underline].start_pos..ctx.match_range.end];
        let suggestions = replacements
            .into_iter()
            .map(|value| Suggestion {
                value: adapt_suggestion(&value, original_str),
                short_description: None,
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: Some(range),
            message: None,
            suggestions: Some(suggestions),
        }
    }
}

// ---------------------------------------------------------------------------
// AdjustPronounsFilter
// ---------------------------------------------------------------------------

pub struct AdjustPronounsFilter {
    pub(crate) env: Env,
}

impl RuleFilter for AdjustPronounsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(actions_arg) = ctx.args.get("actions") else {
            return FilterOutcome::reject();
        };
        let actions: Vec<&str> = actions_arg.split(',').collect();
        let new_lemma = ctx.args.get("newLemma").cloned();
        let new_only_lemma = ctx.args.get("newOnlyLemma").cloned();

        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let mut verb_synth = VerbSynthesizer::new(&tokens, pos_word, self.env.synth.inner(), false);
        if verb_synth.is_undefined() {
            return FilterOutcome::reject();
        }
        let mut verb_str = verb_synth.get_verb_str();
        if let Some(new_lemma) = &new_lemma {
            verb_synth.set_lemma(new_lemma);
            verb_str = verb_synth.synthesize();
        }
        let mut verb_str2 = verb_str.clone();
        if let Some(new_only_lemma) = &new_only_lemma {
            verb_synth.set_lemma(new_only_lemma);
            verb_str2 = verb_synth.synthesize();
        }
        let first_verb_persona_number = verb_synth.get_first_verb_persona_number();
        let mut pronouns_str = if verb_synth.get_num_pronouns_before() > 0 {
            verb_synth.get_pronouns_str_before()
        } else if verb_synth.get_num_pronouns_after() > 0 {
            verb_synth.get_pronouns_str_after()
        } else {
            String::new()
        };
        let start_underline_index =
            verb_synth.get_first_verb_index() - verb_synth.get_num_pronouns_before();
        let end_underline_index =
            verb_synth.get_last_verb_index() + verb_synth.get_num_pronouns_after();
        let mut replacements: Vec<String> = Vec::new();
        for action in &actions {
            let replacement = match *action {
                "removePronounEn" => {
                    let pr = pronouns_str
                        .replace("en", "")
                        .replace("n'", "")
                        .replace("'n", "")
                        .trim()
                        .to_string();
                    format!("{}{verb_str}", transform_davant(&pr, &verb_str))
                }
                "addPronounEn" => {
                    let new_pronoun =
                        do_add_pronoun_en(&pronouns_str, &verb_str, !verb_synth.is_first_verb_is());
                    if new_pronoun.is_empty() {
                        String::new()
                    } else if verb_synth.is_first_verb_is() {
                        format!("{new_pronoun}{verb_str}")
                    } else {
                        format!("{verb_str}{new_pronoun}")
                    }
                }
                "removePronounReflexive" => {
                    do_remove_pronoun_reflexive(&pronouns_str, &verb_str, false)
                }
                "replaceEmEn" => do_replace_em_en(&pronouns_str, &verb_str, false),
                "replaceHiEn" => {
                    let without_hi = transform(
                        pronouns_str.replace("hi", "").trim(),
                        PronounPosition::Normalized,
                    );
                    format!(
                        "{}{verb_str}",
                        do_add_pronoun_en(&without_hi, &verb_str, false)
                    )
                }
                "addPronounReflexive" => do_add_pronoun_reflexive(
                    &pronouns_str,
                    &verb_str,
                    &first_verb_persona_number,
                    !verb_synth.is_first_verb_is(),
                ),
                "addPronounReflexiveHi" => do_add_pronoun_reflexive(
                    &pronouns_str,
                    &format!("hi {verb_str}"),
                    &first_verb_persona_number,
                    false,
                ),
                "addPronounReflexiveImperative" => do_add_pronoun_reflexive_imperative(
                    &pronouns_str,
                    &verb_str,
                    &verb_synth.get_first_verb_persona_number_imperative(),
                ),
                "changeOnlyLemma" => {
                    if actions.contains(&"replaceHiEn") {
                        pronouns_str = pronouns_str.replace("hi", "").trim().to_string();
                    }
                    pronouns_str = transform_davant(&pronouns_str, &verb_str2);
                    format!("{pronouns_str}{verb_str2}")
                }
                _ => String::new(),
            };
            if !replacement.is_empty()
                && !replacement.eq_ignore_ascii_case(&verb_synth.get_whole_original_str())
            {
                replacements.push(preserve_case(&replacement, &verb_synth.get_casing_model()));
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let range = TextRange::new(
            tokens[start_underline_index as usize].start_pos,
            tokens[end_underline_index as usize].end_pos(),
        );
        FilterOutcome {
            accepted: true,
            range: Some(range),
            message: None,
            suggestions: Some(
                replacements
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
