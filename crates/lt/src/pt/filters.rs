//! Portuguese rule filters.
//!
//! Stage 1 registers the language-independent filters referenced by
//! `rules/pt/**.xml`; stage 2 adds the `MultitokenSpellerFilter` over the
//! Portuguese speller. The Portuguese-specific filter classes
//! (`AdvancedSynthesizerFilter`, the date filters, `ConfusionCheckFilter`,
//! `PortugueseEnclisisFilter`/`PortugueseProclisisFilter`,
//! `RegularIrregularParticipleFilter`,
//! `PortugueseSuppressMisspelledSuggestionsFilter`, `BrazilianToponymFilter`,
//! `RomanNumeralFilter`) are stage-3 work and are listed in
//! internal development notes.

use std::collections::HashMap;
use std::sync::Arc;

use lt_core::Suggestion;
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::Ymd;
use crate::en::filters::{
    DateRangeCheckerFilter, RegexAntiPatternFilter, ShortenedYearRangeCheckerFilter,
    UnderlineSpacesFilter,
};
use crate::wordutil::is_punctuation_mark;

pub struct PtFilterEnv {
    /// `AdvancedSynthesizerFilter` (suggestion tagging), the
    /// suppress-misspelled tag checks
    tagger: Arc<lt_tagger::PortugueseTagger>,
    /// `AdvancedSynthesizerFilter`, enclisis/proclisis, the participle and
    /// Roman-numeral filters
    synth: Arc<crate::pt::PortugueseSynthesizerAdapter>,
    /// `MorfologikPortugueseSpellerRule` (the default spelling rule)
    spelling: Arc<crate::pt::spelling::PortugueseSpellingRule>,
    /// `PortugueseMultitokenSpeller` (`MultitokenSpellerFilter`)
    multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// `ConfusionCheckFilter` data for the engine variant.
    confusion: HashMap<String, Vec<ConfusionReading>>,
    /// `BrazilianToponymFilter` data.
    toponyms: BrazilianToponymMap,
}

pub type Env = Arc<PtFilterEnv>;

fn suggestions_values(values: &[String]) -> Vec<Suggestion> {
    values
        .iter()
        .map(|v| Suggestion {
            value: v.clone(),
            short_description: None,
        })
        .collect()
}

fn is_not_word_string(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| !c.is_alphabetic())
}

/// `MultitokenSpellerFilter.isMisspelled(text, language)`: tokenize with the
/// language tokenizer (`PortugueseWordTokenizer`) and ask the speller.
fn is_misspelled(env: &Env, text: &str) -> bool {
    let is_tagged = |w: &str| env.tagger.is_tagged_word(w);
    let tokens = lt_tokenize::PortugueseWordTokenizer::new(&is_tagged).tokenize(text);
    for token in tokens {
        if token.trim().is_empty() {
            continue;
        }
        if env.spelling.is_misspelled(&token) {
            return true;
        }
    }
    false
}

struct PortugueseMultitokenSpellerFilter {
    env: Env,
}

impl RuleFilter for PortugueseMultitokenSpellerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        if ctx.pattern_tokens.iter().all(|t| t.is_ignore_spelling) {
            return FilterOutcome::reject();
        }
        let underlined_error = &ctx.sentence_text[ctx.match_range.start..ctx.match_range.end];
        let are_tokens_accepted_by_speller = !is_misspelled(&self.env, underlined_error);
        let mut replacements = self
            .env
            .multitoken
            .suggestions(underlined_error, are_tokens_accepted_by_speller);
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        if underlined_error.chars().count() > 4 && lt_tagger::is_all_uppercase(underlined_error) {
            let mut all_upper: Vec<String> = Vec::new();
            for replacement in replacements {
                let new_replacement = replacement.to_uppercase();
                if !all_upper.contains(&new_replacement) && new_replacement != underlined_error {
                    all_upper.push(new_replacement);
                }
            }
            replacements = all_upper;
        } else {
            // capitalize suggestions when the error starts the sentence
            let non_blank: Vec<&lt_core::AnalyzedTokenReadings> = ctx
                .sentence_tokens
                .iter()
                .copied()
                .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
                .collect();
            let mut words_start_pos = 1usize;
            while words_start_pos < non_blank.len()
                && (is_punctuation_mark(non_blank[words_start_pos].surface())
                    || is_not_word_string(non_blank[words_start_pos].surface()))
            {
                words_start_pos += 1;
            }
            if ctx.pattern_token_pos == words_start_pos {
                let mut capitalized: Vec<String> = Vec::new();
                for replacement in replacements {
                    let mut new_replacement = replacement.clone();
                    if replacement == replacement.to_lowercase() {
                        new_replacement = lt_tagger::uppercase_first_char(&replacement);
                    }
                    if !capitalized.contains(&new_replacement)
                        && new_replacement != underlined_error
                    {
                        capitalized.push(new_replacement);
                    }
                }
                replacements = capitalized;
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions_values(&replacements)),
        }
    }
}

// ---------------------------------------------------------------------------
// ConfusionCheckFilter
// ---------------------------------------------------------------------------

/// One reading of a `pt/confusion_pairs.txt` form (`token;postag`).
struct ConfusionReading {
    token: String,
    postag: String,
}

/// `ConfusionPairsDataLoader.loadWords`: the variant's confusion-pair files
/// (`pt/rules/confusion_pairs.txt` + `pt/rules/{pt-PT,pt-BR}/confusion_pairs.txt`).
fn load_confusion_pairs(
    data_dir: &std::path::Path,
    variant: &str,
) -> HashMap<String, Vec<ConfusionReading>> {
    let variant_dir = if variant == "pt-BR" { "pt-BR" } else { "pt-PT" };
    let mut map: HashMap<String, Vec<ConfusionReading>> = HashMap::new();
    for rel in [
        "pt/rules/confusion_pairs.txt",
        &format!("pt/rules/{variant_dir}/confusion_pairs.txt"),
    ] {
        let Ok(text) = lt_data::fs::read_to_string(data_dir.join(rel)) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() != 3 {
                continue;
            }
            map.entry(parts[0].to_string())
                .or_default()
                .push(ConfusionReading {
                    token: parts[1].to_string(),
                    postag: parts[2].to_string(),
                });
        }
    }
    map
}

const MS: &str = "NC[MC][SN]000|A..[MC][SN].|V.P..SM";
const FS: &str = "NC[FC][SN]000|A..[FC][SN].|V.P..SF";
const MP: &str = "NC[MC][PN]000|A..[MC][PN].|V.P..PM";
const FP: &str = "NC[FC][PN]000|A..[FC][PN].|V.P..PF";
const CP: &str = "NC[MFC][PN]000|A..[MFC][PN].|V.P..P.";
const CS: &str = "NC[MFC][SN]000|A..[MFC][SN].|V.P..S.";

/// `org.languagetool.rules.pt.ConfusionCheckFilter`.
struct ConfusionCheckFilter {
    env: Env,
}

impl RuleFilter for ConfusionCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(postag) = ctx.args.get("postag") else {
            return FilterOutcome::reject();
        };
        let Some(form) = ctx.args.get("form").map(|f| f.to_lowercase()) else {
            return FilterOutcome::reject();
        };
        let gendernumber_from = ctx.args.get("gendernumberFrom").cloned();
        let desired_gender_number: Option<&str> = if let Some(raw) = &gendernumber_from {
            let Ok(index) = raw.parse::<usize>() else {
                return FilterOutcome::reject();
            };
            if index < 1 || index > ctx.pattern_tokens.len() {
                return FilterOutcome::reject();
            }
            let atr = ctx.pattern_tokens[index - 1];
            let matches =
                |pattern: &str| atr.has_pos_tag_matching(&crate::de::util::anchored(pattern));
            if matches("[NAPD].+MS.*|V.P..SM") {
                Some(MS)
            } else if matches("[NAPD].+MP.*|V.P..PM") {
                Some(MP)
            } else if matches("[NAPD].+FS.*|V.P..SF") {
                Some(FS)
            } else if matches("[NAPD].+FP.*|V.P..PF") {
                Some(FP)
            } else if matches("[NAPD].+CP.*|V.P..P.") {
                Some(CP)
            } else if matches("[NAPD].+CS.*|V.P..S.") {
                Some(CS)
            } else {
                None
            }
        } else {
            None
        };

        let Some(readings) = self.env.confusion.get(&form) else {
            return FilterOutcome::reject();
        };
        let postag_re = crate::de::util::anchored(postag);
        if !readings.iter().any(|r| postag_re.is_match(&r.postag)) {
            return FilterOutcome::reject();
        }
        let replacement = match desired_gender_number {
            Some(pattern) => {
                if !crate::de::util::anchored(pattern).is_match(&readings[0].postag) {
                    return FilterOutcome::reject();
                }
                readings[0].token.clone()
            }
            None if gendernumber_from.is_none() => readings[0].token.clone(),
            // gendernumberFrom was set but no reading matched the pattern
            None => return FilterOutcome::reject(),
        };

        let mut message = ctx.message.clone();
        // Java keeps the "tilde" wording only when the replacement carries a
        // diacritic and the form does not.
        let has_diacritics = |s: &str| crate::multitoken::remove_diacritics(s) != s;
        if !(has_diacritics(&replacement) && !has_diacritics(&form)) {
            message = message.replace("se escribe con tilde", "se escribe de otra manera");
        }
        let Some(template) = ctx.suggestions.first() else {
            return FilterOutcome::reject();
        };
        let suggestion = template
            .value
            .replace("{suggestion}", &replacement)
            .replace(
                "{Suggestion}",
                &lt_tagger::uppercase_first_char(&replacement),
            )
            .replace("{SUGGESTION}", &replacement.to_uppercase());
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: Some(vec![Suggestion {
                value: suggestion,
                short_description: None,
            }]),
        }
    }
}

// ---------------------------------------------------------------------------
// AdvancedSynthesizerFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.pt.AdvancedSynthesizerFilter`
/// (`AbstractAdvancedSynthesizerFilter` with the Portuguese
/// `keepPronoun` postag mode and the `VMIP1P0`/`VMIP2P0` exception).
struct AdvancedSynthesizerFilter {
    env: Env,
}

impl AdvancedSynthesizerFilter {
    fn analyzed_token<'a>(
        &self,
        token: &'a lt_core::AnalyzedTokenReadings,
        regexp: &str,
    ) -> Option<&'a lt_core::AnalyzedToken> {
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
        if postag_replace == "keepPronoun" {
            return move_pronoun_tag(original_postag, desired_postag);
        }
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

    /// `PortugueseAdvancedSynthesizerFilter.isSuggestionException`.
    fn is_suggestion_exception(&self, token: &str, desired_postag: &str) -> bool {
        (desired_postag == "VMIP1P0" || desired_postag == "VMIP2P0") && !token.ends_with('s')
    }
}

/// `AbstractAdvancedSynthesizerFilter.movePronounTag` (Portuguese override).
fn move_pronoun_tag(source_tag: &str, destination_tag: &str) -> String {
    let source_parts: Vec<&str> = source_tag.split(':').collect();
    if source_parts.len() == 2 {
        let destination_parts: Vec<&str> = destination_tag.split(':').collect();
        format!("{}:{}", destination_parts[0], source_parts[1])
    } else {
        destination_tag.to_string()
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
                // Portuguese `getNewLemma` is not overridden and returns null
                return FilterOutcome::reject();
            }
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
        let token = lt_core::AnalyzedToken::new("", Some(desired_lemma), None);
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
                if self.is_suggestion_exception(nr, &desired_postag) {
                    continue;
                }
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
            suggestions: Some(suggestions_values(&replacements_list)),
        }
    }
}

// ---------------------------------------------------------------------------
// PortugueseEnclisisFilter / PortugueseProclisisFilter
// ---------------------------------------------------------------------------

/// `PortugueseEnclisisFilter.convertPronounToAccusative`.
fn convert_pronoun_to_accusative(pronoun_tag: &str) -> String {
    match pronoun_tag.strip_suffix("N00") {
        Some(prefix) => format!("{prefix}A00"),
        None => pronoun_tag.to_string(),
    }
}

/// `PortugueseEnclisisFilter.getPronounTags`.
fn pronoun_tags(
    pronoun_readings: &lt_core::AnalyzedTokenReadings,
    verb_text: &str,
    convert_to_accusative: bool,
) -> Vec<String> {
    let mut tags = Vec::new();
    for pronoun_token in &pronoun_readings.readings {
        if pronoun_token.token == "nos" {
            tags.push("PP1CPO00".to_string());
            if verb_text.ends_with('m') || verb_text.ends_with("ão") || verb_text.ends_with("õe")
            {
                tags.push("PP3MPA00".to_string());
            }
            break;
        }
        if let Some(pos_tag) = pronoun_token.pos_tag.as_deref() {
            if pos_tag.starts_with("PP") {
                tags.push(if convert_to_accusative {
                    convert_pronoun_to_accusative(pos_tag)
                } else {
                    pos_tag.to_string()
                });
            }
        }
    }
    tags
}

/// `PortugueseEnclisisFilter.getVerbForms` (Java `HashSet` iteration order).
fn enclitic_verb_forms(
    env: &Env,
    verb_readings: &lt_core::AnalyzedTokenReadings,
    pronoun_tags: &[String],
) -> Vec<String> {
    let verb_text = verb_readings.surface();
    let is_title_case = lt_tagger::is_capitalized_word(verb_text);
    let is_all_caps = lt_tagger::is_all_uppercase(verb_text);
    let mut forms: Vec<String> = Vec::new();
    for at in &verb_readings.readings {
        let Some(pos_tag) = at.pos_tag.as_deref() else {
            continue;
        };
        if !pos_tag.starts_with('V') {
            continue;
        }
        for pronoun_tag in pronoun_tags {
            let enclisis_tag = format!("{pos_tag}:{pronoun_tag}");
            for mut form in env.synth.inner().synthesize(at, &enclisis_tag, false) {
                if is_title_case {
                    form = lt_tagger::uppercase_first_char(&form);
                } else if is_all_caps {
                    form = form.to_uppercase();
                }
                forms.push(form);
            }
        }
        break;
    }
    crate::de::util::java_hash_set_order(&forms)
}

/// `org.languagetool.rules.pt.PortugueseEnclisisFilter`.
struct PortugueseEnclisisFilter {
    env: Env,
}

impl RuleFilter for PortugueseEnclisisFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(verb_pos) = ctx
            .args
            .get("verbPos")
            .and_then(|v| v.parse::<usize>().ok())
        else {
            return FilterOutcome::reject();
        };
        let Some(pronoun_pos) = ctx
            .args
            .get("pronounPos")
            .and_then(|v| v.parse::<usize>().ok())
        else {
            return FilterOutcome::reject();
        };
        let convert_to_accusative = ctx
            .args
            .get("convertToAccusative")
            .is_some_and(|v| v.eq_ignore_ascii_case("true"));
        let (Some(verb), Some(pronoun)) = (
            ctx.pattern_tokens.get(verb_pos).copied(),
            ctx.pattern_tokens.get(pronoun_pos).copied(),
        ) else {
            return FilterOutcome::reject();
        };
        let tags = pronoun_tags(pronoun, verb.surface(), convert_to_accusative);
        if tags.is_empty() {
            return FilterOutcome::reject();
        }
        let forms = enclitic_verb_forms(&self.env, verb, &tags);
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions_values(&forms)),
        }
    }
}

/// `org.languagetool.rules.pt.PortugueseProclisisFilter`.
struct PortugueseProclisisFilter {
    env: Env,
}

impl RuleFilter for PortugueseProclisisFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(enclitic_verb) = ctx.pattern_tokens.last().copied() else {
            return FilterOutcome::reject();
        };
        let mut forms: Vec<String> = Vec::new();
        for at in &enclitic_verb.readings {
            let Some(pos_tag) = at.pos_tag.as_deref() else {
                continue;
            };
            if !pos_tag.starts_with('V') || !pos_tag.contains(':') {
                continue;
            }
            let verb_tag = pos_tag.split(':').next().unwrap_or(pos_tag);
            let Some(new_verb) = self
                .env
                .synth
                .inner()
                .synthesize(at, verb_tag, false)
                .into_iter()
                .next()
            else {
                return FilterOutcome::reject();
            };
            let old_token = at.token.clone();
            let old_token_parts = crate::wordutil::split_compound(&old_token);
            if old_token_parts.len() < 2 {
                continue;
            }
            let old_verb = old_token_parts[0];
            let old_pronoun = old_token_parts[1];
            let mut new_pronoun_forms: Vec<String> = Vec::new();
            match old_pronoun {
                "lo" | "no" => new_pronoun_forms.push("o".to_string()),
                "la" | "na" => new_pronoun_forms.push("a".to_string()),
                "los" => new_pronoun_forms.push("os".to_string()),
                "las" | "nas" => new_pronoun_forms.push("as".to_string()),
                "nos" => {
                    new_pronoun_forms.push("nos".to_string());
                    if old_verb.ends_with('m')
                        || old_verb.ends_with("ão")
                        || old_verb.ends_with("õe")
                    {
                        new_pronoun_forms.push("os".to_string());
                    }
                }
                other => new_pronoun_forms.push(other.to_string()),
            }
            for new_pronoun in new_pronoun_forms {
                forms.push(format!("{new_pronoun} {new_verb}"));
            }
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions_values(&crate::de::util::java_hash_set_order(
                &forms,
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// RegularIrregularParticipleFilter
// ---------------------------------------------------------------------------

/// `RegularIrregularParticipleFilter.isRegular`.
fn is_regular_participle(p: &str) -> bool {
    let lp = p.to_lowercase();
    lp.ends_with("do") || lp.ends_with("dos") || lp.ends_with("da") || lp.ends_with("das")
}

/// `org.languagetool.rules.pt.RegularIrregularParticipleFilter`.
struct RegularIrregularParticipleFilter {
    env: Env,
}

impl RuleFilter for RegularIrregularParticipleFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(direction) = ctx.args.get("direction") else {
            return FilterOutcome::reject();
        };
        let atr = ctx
            .pattern_tokens
            .iter()
            .find(|t| t.start_pos == ctx.match_range.start);
        let Some(atr) = atr else {
            return FilterOutcome::reject();
        };
        let has_vmp = atr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("VMP")));
        if !has_vmp {
            return FilterOutcome::reject();
        }
        // Java keeps the last `VMP` reading
        let Some(selected_at) = atr
            .readings
            .iter()
            .rev()
            .find(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("VMP")))
        else {
            return FilterOutcome::reject();
        };
        let mut desired_postag = selected_at.pos_tag.clone().unwrap_or_default();
        if desired_postag.ends_with('C') {
            desired_postag.truncate(desired_postag.len() - 1);
            desired_postag.push_str("[MC]");
        } else if let Some(last) = desired_postag.pop() {
            desired_postag.push_str(&format!("[{last}C]"));
        }
        let participles = self
            .env
            .synth
            .inner()
            .synthesize(selected_at, &desired_postag, true);
        if participles.len() <= 1 {
            return FilterOutcome::reject();
        }
        let original = atr.surface().to_string();
        let pick = |want_regular: bool| -> Option<String> {
            participles
                .iter()
                .take(2)
                .find(|p| is_regular_participle(p) == want_regular)
                .cloned()
        };
        let replacement = if direction.eq_ignore_ascii_case("RegularToIrregular")
            && is_regular_participle(&original)
        {
            pick(false)
        } else if direction.eq_ignore_ascii_case("IrregularToRegular")
            && !is_regular_participle(&original)
        {
            pick(true)
        } else {
            None
        };
        let Some(replacement) = replacement else {
            return FilterOutcome::reject();
        };
        let Some(template) = ctx.suggestions.first() else {
            return FilterOutcome::reject();
        };
        let suggestion = template
            .value
            .replace("{suggestion}", &replacement)
            .replace(
                "{Suggestion}",
                &lt_tagger::uppercase_first_char(&replacement),
            )
            .replace("{SUGGESTION}", &replacement.to_uppercase());
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(vec![Suggestion {
                value: suggestion,
                short_description: None,
            }]),
        }
    }
}

// ---------------------------------------------------------------------------
// PortugueseSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.pt.PortugueseSuppressMisspelledSuggestionsFilter`
/// (`AbstractSuppressMisspelledSuggestionsFilter`). Both XML references are
/// commented out in the legacy engine, so no rule currently instantiates it.
struct PortugueseSuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl PortugueseSuppressMisspelledSuggestionsFilter {
    fn is_misspelled_multiword(&self, word: &str) -> bool {
        let is_tagged = |w: &str| self.env.tagger.is_tagged_word(w);
        let tokens = lt_tokenize::PortugueseWordTokenizer::new(&is_tagged).tokenize(word);
        for token in tokens {
            if token.trim().is_empty() {
                continue;
            }
            if self.env.spelling.is_misspelled(&token) {
                return true;
            }
        }
        false
    }
}

impl RuleFilter for PortugueseSuppressMisspelledSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let suppress_postag = ctx.args.get("SuppressPostag");
        let filter_postag = ctx.args.get("FilterPostag");
        let mut new_replacements: Vec<Suggestion> = Vec::new();
        for replacement in &ctx.suggestions {
            if self.is_misspelled_multiword(&replacement.value) {
                continue;
            }
            let mut add = true;
            if suppress_postag.is_some() || filter_postag.is_some() {
                let readings = self
                    .env
                    .tagger
                    .tag(std::slice::from_ref(&replacement.value));
                let Some(atr) = readings.into_iter().next() else {
                    continue;
                };
                if let Some(re) =
                    suppress_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                {
                    if atr.has_pos_tag_matching(&re) {
                        add = false;
                    }
                }
                if add {
                    if let Some(re) =
                        filter_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                    {
                        if !atr.has_pos_tag_matching(&re) {
                            add = false;
                        }
                    }
                }
            }
            if add {
                new_replacements.push(replacement.clone());
            }
        }
        if new_replacements.is_empty() && suppress_match {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(new_replacements),
        }
    }
}

// ---------------------------------------------------------------------------
// RomanNumeralFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.pt.RomanNumeralFilter`
/// (`PortugueseSynthesizer.getRomanNumber`).
struct RomanNumeralFilter {
    env: Env,
}

impl RuleFilter for RomanNumeralFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(arabic_source) = ctx.args.get("arabicSource") else {
            return FilterOutcome::reject();
        };
        let roman = self.env.synth.inner().get_roman_number(arabic_source);
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(vec![Suggestion {
                value: roman,
                short_description: None,
            }]),
        }
    }
}

// ---------------------------------------------------------------------------
// BrazilianToponymFilter (RegexRuleFilter)
// ---------------------------------------------------------------------------

/// `BrazilianToponymMap`: the Brazilian municipality lists per state
/// (`pt/brazilian_municipalities/*.tsv`), normalized like the Java loader.
struct BrazilianToponymMap {
    municipalities: std::collections::HashSet<String>,
}

impl BrazilianToponymMap {
    fn load(data_dir: &std::path::Path) -> Self {
        const STATES: &[&str] = &[
            "AC", "AL", "AP", "AM", "BA", "CE", "DF", "ES", "GO", "MA", "MT", "MS", "MG", "PA",
            "PB", "PR", "PE", "PI", "RJ", "RN", "RS", "RO", "RR", "SC", "SP", "SE", "TO",
        ];
        let mut municipalities = std::collections::HashSet::new();
        for state in STATES {
            let path = data_dir.join(format!("pt/brazilian_municipalities/{state}.tsv"));
            let Ok(text) = lt_data::fs::read_to_string(path) else {
                continue;
            };
            for line in text.lines() {
                let normalized = line.replace('-', " ").to_lowercase();
                if !normalized.is_empty() {
                    municipalities.insert(normalized);
                }
            }
        }
        Self { municipalities }
    }

    /// `BrazilianToponymMap.isValidToponym`: drop the leftmost element until
    /// a known municipality is found.
    fn is_valid_toponym(&self, toponym: &str) -> bool {
        let normalized = toponym.replace('-', " ").to_lowercase();
        let parts: Vec<&str> = normalized.split(' ').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(" ");
            if self.municipalities.contains(&candidate) {
                return true;
            }
        }
        false
    }
}

/// `org.languagetool.rules.pt.BrazilianToponymFilter` (`RegexRuleFilter`).
struct BrazilianToponymFilter {
    env: Env,
}

impl RuleFilter for BrazilianToponymFilter {
    /// Only meaningful for `<regexp>` rules (`accept_regexp`).
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }

    fn accept_regexp(&self, _ctx: &FilterContext, captures: &[Option<String>]) -> FilterOutcome {
        let group = |n: usize| captures.get(n).and_then(|g| g.clone()).unwrap_or_default();
        let toponym = group(1);
        let underlined = group(2);
        let state = group(3);
        let suggestion = format!("\u{2013}{state}");
        if suggestion == underlined {
            return FilterOutcome::reject();
        }
        if !self.env.toponyms.is_valid_toponym(&toponym) {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(vec![Suggestion {
                value: suggestion,
                short_description: None,
            }]),
        }
    }
}

/// The Portuguese filter registry. `today` feeds the date filters
/// (`DateCheckFilter`, `FutureDateFilter`, `NewYearDateFilter`,
/// `YMDNewYearDateFilter`); `variant` selects the confusion-pair file set.
/// `org.languagetool.rules.IsEnglishWordFilter`: the pinned Portuguese module
/// only depends on `languagetool-core`, so Java's
/// `Languages.getLanguageForShortCode("en-US")` returns null and the filter
/// constructor leaves `tagger == null` — every match is rejected (the
/// `IGNORE_PROBABLE_ENGLISH_TOPONYMS` disambiguation rules therefore never
/// add `_english_ignore_`).
struct IsEnglishWordFilter;

impl RuleFilter for IsEnglishWordFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }
}

pub fn portuguese_filter_registry(
    today: Ymd,
    tagger: Arc<lt_tagger::PortugueseTagger>,
    synth: Arc<crate::pt::PortugueseSynthesizerAdapter>,
    spelling: Arc<crate::pt::spelling::PortugueseSpellingRule>,
    multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    data_dir: &std::path::Path,
    variant: &str,
) -> FilterRegistry {
    let env: Env = Arc::new(PtFilterEnv {
        tagger,
        synth,
        spelling,
        multitoken,
        confusion: load_confusion_pairs(data_dir, variant),
        toponyms: BrazilianToponymMap::load(data_dir),
    });
    let builder = crate::pt::date_filters::register(FilterRegistry::builder(), today);
    builder
        .register(
            "org.languagetool.rules.IsEnglishWordFilter",
            Arc::new(IsEnglishWordFilter),
        )
        .register(
            "org.languagetool.rules.DateRangeChecker",
            Arc::new(DateRangeCheckerFilter),
        )
        .register(
            "org.languagetool.rules.ShortenedYearRangeChecker",
            Arc::new(ShortenedYearRangeCheckerFilter),
        )
        .register(
            "org.languagetool.rules.UnderlineSpacesFilter",
            Arc::new(UnderlineSpacesFilter),
        )
        .register(
            "org.languagetool.rules.patterns.RegexAntiPatternFilter",
            Arc::new(RegexAntiPatternFilter),
        )
        .register(
            "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter",
            Arc::new(PortugueseMultitokenSpellerFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.ConfusionCheckFilter",
            Arc::new(ConfusionCheckFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.AdvancedSynthesizerFilter",
            Arc::new(AdvancedSynthesizerFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.PortugueseEnclisisFilter",
            Arc::new(PortugueseEnclisisFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.PortugueseProclisisFilter",
            Arc::new(PortugueseProclisisFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.RegularIrregularParticipleFilter",
            Arc::new(RegularIrregularParticipleFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.PortugueseSuppressMisspelledSuggestionsFilter",
            Arc::new(PortugueseSuppressMisspelledSuggestionsFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.RomanNumeralFilter",
            Arc::new(RomanNumeralFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.pt.BrazilianToponymFilter",
            Arc::new(BrazilianToponymFilter { env }),
        )
        .build()
}
