//! Remaining Catalan stage-3 XML-referenced filters (small classes):
//! `RemoveSuggestionsFilter`, `CheckPostagsInSuggestionFilter`,
//! `ConvertToSentenceCaseFilter`, the number filters
//! (`CatalanNumberSpellerFilter`, `CatalanNumberInWordFilter`,
//! `TextToNumberFilter`), `FindSuggestionsEsFilter`,
//! `SynthesizeWithDAFilter`, `SynthesizeWithAnyDeterminerFilter`,
//! `EnNoInfinitiuSuggestionFilter`, `PossessiusRedundantsFilter` and
//! `OblidarseSugestionsFilter`.
//!
//! Documented stage-3 triage stubs (behavior not yet ported):
//! `SuppressIfAnyRuleMatchesFilter` (accepts: the ruleIDs reference Catalan
//! rules that need a compiled-rule back-reference),
//! `CatalanRemoteRewriteFilter` (accepts: Java returns the match unchanged
//! when the remote service is unavailable), `DateCheckFilter` /
//! `NewYearDateFilter`, `DonarseliBeFilter`,
//! `PostponedAdjectiveConcordanceFilter`, `DonarTempsSuggestionsFilter`,
//! `AnarASuggestionsFilter`, `PortarGerundiSuggestionsFilter` and
//! `PortarTempsSuggestionsFilter` (reject) — see `ca-rule-port.md`.

use lt_core::{AnalyzedToken, Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::adapt::{adapt_suggestion, preserve_case};
use crate::ca::filters::Env;
use crate::ca::gender_number::get_preposition_and_determiner;
use crate::ca::helpers::{transform_darrere, transform_davant, VerbSynthesizer};
use crate::ca::synth_filters::{get_position, FindSuggestionsFilter};
use crate::ca::verb_filters::{pos_word_start, tokens_without_whitespace};

fn suggestions(values: Vec<String>) -> Vec<Suggestion> {
    values
        .into_iter()
        .map(|value| Suggestion {
            value,
            short_description: None,
        })
        .collect()
}

fn accept_with(suggestions_values: Vec<String>) -> FilterOutcome {
    FilterOutcome {
        accepted: true,
        range: None,
        message: None,
        suggestions: Some(suggestions(suggestions_values)),
    }
}

/// `RuleFilter.isMatchAtSentenceStart`.
fn is_match_at_sentence_start(tokens: &[&lt_core::AnalyzedTokenReadings], from_pos: usize) -> bool {
    let mut i = 0usize;
    while i < tokens.len() && tokens[i].start_pos < from_pos {
        i += 1;
    }
    while i > 0 && i < tokens.len() && crate::wordutil::is_punctuation_mark(tokens[i].surface()) {
        i -= 1;
    }
    i == 0
}

// ---------------------------------------------------------------------------
// RemoveSuggestionsFilter
// ---------------------------------------------------------------------------

pub struct CaRemoveSuggestionsFilter;

impl RuleFilter for CaRemoveSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(pattern) = ctx.args.get("removeSuggestionsRegexp") else {
            return FilterOutcome::reject();
        };
        let Ok(re) = regex::Regex::new(&format!("^(?:{pattern})$")) else {
            return FilterOutcome::reject();
        };
        let kept: Vec<Suggestion> = ctx
            .suggestions
            .iter()
            .filter(|s| !re.is_match(&s.value))
            .cloned()
            .collect();
        if kept.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(kept),
        }
    }
}

// ---------------------------------------------------------------------------
// CheckPostagsInSuggestionFilter
// ---------------------------------------------------------------------------

pub struct CheckPostagsInSuggestionFilter {
    pub(crate) env: Env,
}

impl RuleFilter for CheckPostagsInSuggestionFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(postags_list_str) = ctx.args.get("PostagsList") else {
            return FilterOutcome::reject();
        };
        let postags_list: Vec<&str> = postags_list_str.split(',').collect();
        if postags_list.is_empty() {
            return FilterOutcome::reject();
        }
        let mut new_replacements: Vec<Suggestion> = Vec::new();
        for replacement in &ctx.suggestions {
            let tokens_in_suggestion: Vec<String> = replacement
                .value
                .split_whitespace()
                .map(str::to_string)
                .collect();
            if tokens_in_suggestion.len() != postags_list.len() {
                // Java throws IOException here; skip the suggestion instead.
                continue;
            }
            let atrs = self.env.tagger.tag(&tokens_in_suggestion);
            let mut postags_match = true;
            for (i, postag) in postags_list.iter().enumerate() {
                let Some(atr) = atrs.get(i) else {
                    postags_match = false;
                    break;
                };
                let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{postag})$")) else {
                    postags_match = false;
                    break;
                };
                if !atr.has_pos_tag_matching_fancy(&re) {
                    postags_match = false;
                    break;
                }
            }
            if postags_match {
                new_replacements.push(replacement.clone());
            }
        }
        if new_replacements.is_empty() {
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
// ConvertToSentenceCaseFilter
// ---------------------------------------------------------------------------

pub struct ConvertToSentenceCaseFilter;

impl ConvertToSentenceCaseFilter {
    fn normalized_case(token: &lt_core::AnalyzedTokenReadings) -> String {
        let mut token_lowercase = token.surface().to_lowercase();
        if token.has_typographic_apostrophe {
            token_lowercase = token_lowercase.replace('\'', "’");
        }
        if token_lowercase.is_empty() {
            return token_lowercase;
        }
        let token_capitalized = lt_tagger::uppercase_first_char(&token_lowercase);
        let mut lemma_is_capitalized = false;
        let mut lemma_is_lowercase = false;
        for at in &token.readings {
            let Some(lemma) = &at.stem else {
                return token_capitalized;
            };
            let lemma = lemma.split(' ').next().unwrap_or("");
            lemma_is_capitalized = lemma_is_capitalized || lt_tagger::is_capitalized_word(lemma);
            lemma_is_lowercase = lemma_is_lowercase || lemma.to_lowercase() == lemma;
        }
        if lemma_is_lowercase {
            return token_lowercase;
        }
        if lemma_is_capitalized {
            return token_capitalized;
        }
        token.surface().to_string()
    }
}

impl RuleFilter for ConvertToSentenceCaseFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let mut first_done = false;
        let mut replacement = String::new();
        let mut original_str = String::new();
        for i in ctx.pattern_token_pos..ctx.pattern_tokens.len() {
            let token = ctx.pattern_tokens[i];
            if token.start_pos < ctx.match_range.start || token.end_pos() > ctx.match_range.end {
                continue;
            }
            let mut normalized_case = Self::normalized_case(token);
            if i + 1 < ctx.pattern_tokens.len() && ctx.pattern_tokens[i + 1].surface() == "." {
                if normalized_case.chars().count() == 1 {
                    normalized_case = normalized_case.to_uppercase();
                } else if normalized_case == "corp" {
                    normalized_case = "Corp".to_string();
                }
            }
            let token_string = token.surface();
            let token_capitalized = lt_tagger::uppercase_first_char(&normalized_case);
            if !first_done
                && !crate::wordutil::is_punctuation_mark(token_string)
                && !token_string.is_empty()
            {
                first_done = true;
                replacement.push_str(&token_capitalized);
                original_str.push_str(token_string);
            } else {
                if token.whitespace_before {
                    replacement.push(' ');
                    original_str.push(' ');
                }
                replacement.push_str(&normalized_case);
                original_str.push_str(token_string);
            }
        }
        if replacement == original_str {
            return FilterOutcome::reject();
        }
        accept_with(vec![replacement])
    }
}

// ---------------------------------------------------------------------------
// Number filters
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.CatalanNumberSpellerFilter`.
pub struct CatalanNumberSpellerFilter {
    pub(crate) env: Env,
}

impl RuleFilter for CatalanNumberSpellerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(number_to_spell) = ctx.args.get("number_to_spell") else {
            return FilterOutcome::reject();
        };
        let mut str_to_spell = number_to_spell.replace('.', "");
        if ctx
            .args
            .get("gender")
            .map(|g| g == "feminine")
            .unwrap_or(false)
        {
            str_to_spell = format!("feminine {str_to_spell}");
        }
        let mut spelled_number = self.env.synth.inner().get_spelled_number(&str_to_spell);
        let tokens = &ctx.sentence_tokens;
        if ctx.pattern_token_pos <= 1
            || tokens
                .get(ctx.pattern_token_pos.saturating_sub(1))
                .is_some_and(|t| t.has_pos_tag_starting_with("SENT_START"))
        {
            spelled_number = lt_tagger::uppercase_first_char(&spelled_number);
        }
        if !spelled_number.is_empty()
            && spelled_number
                .replace("-i-", " ")
                .replace('-', " ")
                .split(' ')
                .count()
                < 4
        {
            return accept_with(vec![spelled_number]);
        }
        FilterOutcome::reject()
    }
}

/// `org.languagetool.rules.ca.CatalanNumberInWordFilter`
/// (`AbstractNumberInWordFilter` with the Catalan speller).
pub struct CatalanNumberInWordFilter {
    pub(crate) env: Env,
}

impl RuleFilter for CatalanNumberInWordFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(word) = ctx.args.get("word") else {
            return FilterOutcome::reject();
        };
        let word_replacing_zero_o = word.replace('0', "o");
        let word_without_number: String = word.chars().filter(|c| !c.is_ascii_digit()).collect();
        let mut replacements: Vec<String> = Vec::new();
        if !self.env.spelling.is_misspelled(&word_replacing_zero_o)
            && *word != word_replacing_zero_o
        {
            replacements.push(word_replacing_zero_o);
        }
        if !self.env.spelling.is_misspelled(&word_without_number) {
            replacements.push(word_without_number.clone());
        }
        if replacements.is_empty() {
            replacements.extend(self.env.spelling.suggestions(&word_without_number));
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        accept_with(replacements)
    }
}

/// `org.languagetool.rules.ca.TextToNumberFilter`
/// (`AbstractTextToNumberFilter` with the Catalan number words and
/// `tokenize(s.split("-"))`).
pub struct TextToNumberFilter;

fn number_words() -> &'static std::collections::HashMap<&'static str, f32> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, f32>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        std::collections::HashMap::from([
            ("zero", 0.0),
            ("mig", 0.5),
            ("un", 1.0),
            ("u", 1.0),
            ("una", 1.0),
            ("dos", 2.0),
            ("dues", 2.0),
            ("tres", 3.0),
            ("quatre", 4.0),
            ("cinc", 5.0),
            ("sis", 6.0),
            ("set", 7.0),
            ("vuit", 8.0),
            ("huit", 8.0),
            ("nou", 9.0),
            ("deu", 10.0),
            ("onze", 11.0),
            ("dotze", 12.0),
            ("tretze", 13.0),
            ("catorze", 14.0),
            ("quinze", 15.0),
            ("setze", 16.0),
            ("disset", 17.0),
            ("desset", 17.0),
            ("dèsset", 17.0),
            ("divuit", 18.0),
            ("devuit", 18.0),
            ("díhuit", 18.0),
            ("dinou", 19.0),
            ("denou", 19.0),
            ("dènou", 19.0),
            ("dèneu", 19.0),
            ("vint", 20.0),
            ("trenta", 30.0),
            ("quaranta", 40.0),
            ("cinquanta", 50.0),
            ("seixanta", 60.0),
            ("setanta", 70.0),
            ("vuitanta", 80.0),
            ("huitanta", 80.0),
            ("noranta", 90.0),
        ])
    })
}

fn number_multipliers() -> &'static std::collections::HashMap<&'static str, f32> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, f32>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        std::collections::HashMap::from([
            ("cent", 100.0),
            ("cents", 100.0),
            ("mil", 1000.0),
            ("milió", 1_000_000.0),
            ("milions", 1_000_000.0),
            ("bilió", 10e12),
            ("bilions", 10e12),
            ("trilió", 10e18),
            ("trilions", 10e18),
        ])
    })
}

impl TextToNumberFilter {
    fn format(total: f32, percentage: bool) -> String {
        let mut result = if total == (total as i64) as f32 {
            format!("{}", total as i64)
        } else {
            format!("{total}")
        };
        if percentage {
            result.push('\u{202F}');
            result.push('%');
        }
        // `TextToNumberFilter.formatResult` (`AbstractTextToNumberFilter`
        // applies it after the percentage suffix): Catalan writes decimals
        // with a comma.
        result.replace('.', ",")
    }
}

impl RuleFilter for TextToNumberFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let mut pos_word = 0usize;
        let mut total: f32 = 0.0;
        let mut current: f32 = 0.0;
        let mut total_decimal: f32 = 0.0;
        let mut current_decimal: f32 = 0.0;
        let mut added_zeros = 0u32;
        let mut percentage = false;
        let mut decimal = false;
        while pos_word < ctx.pattern_tokens.len()
            && ctx.pattern_tokens[pos_word].end_pos() <= ctx.match_range.end
        {
            let token = ctx.pattern_tokens[pos_word];
            if token.start_pos >= ctx.match_range.start && token.end_pos() <= ctx.match_range.end {
                let form = token.surface().to_lowercase();
                if pos_word > 0
                    && form == "cent"
                    && ctx.pattern_tokens[pos_word - 1].surface().to_lowercase() == "per"
                {
                    percentage = true;
                    break;
                }
                if form == "comma" || form == "coma" {
                    decimal = true;
                    pos_word += 1;
                    continue;
                }
                for sub_form in form.split('-') {
                    if !decimal {
                        if let Some(value) = number_words().get(sub_form) {
                            current += value;
                        } else if let Some(multiplier) = number_multipliers().get(sub_form) {
                            if current == 0.0 {
                                current = 1.0;
                            }
                            total += current * multiplier;
                            current = 0.0;
                        }
                    } else if let Some(value) = number_words().get(sub_form) {
                        let formatted = if *value == (*value as i64) as f32 {
                            format!("{}", *value as i64)
                        } else {
                            format!("{value}")
                        };
                        let zeros_to_add = formatted.chars().count() as u32;
                        current_decimal += value / 10f32.powi((added_zeros + zeros_to_add) as i32);
                        added_zeros += 1;
                    }
                }
            }
            pos_word += 1;
        }
        total += current;
        total_decimal += current_decimal;
        let total = total + total_decimal;
        accept_with(vec![Self::format(total, percentage)])
    }
}

// ---------------------------------------------------------------------------
// FindSuggestionsEsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.FindSuggestionsEsFilter` (the `es`/`és`
/// confusion helper).
pub struct FindSuggestionsEsFilter {
    pub(crate) env: Env,
    pub(crate) base: FindSuggestionsFilter,
}

impl RuleFilter for FindSuggestionsEsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = &ctx.pattern_tokens;
        let mut pos_word = 0usize;
        while pos_word < tokens.len()
            && (tokens[pos_word].start_pos < ctx.match_range.start
                || tokens[pos_word].is_sentence_start)
        {
            pos_word += 1;
        }
        pos_word += 1;
        let Some(atr_word) = tokens.get(pos_word) else {
            return FilterOutcome::reject();
        };
        let spelling_suggestions = self.base.spelling_suggestions(atr_word.surface());
        let mut replacements: Vec<String> = Vec::new();
        let mut used_es_accent = false;
        let mut used_es = false;
        let Ok(apostrophe_needed) = regex::Regex::new("(?i)^h?[aeiouàèéíòóú].*$") else {
            return FilterOutcome::reject();
        };
        for suggestion in spelling_suggestions {
            let analyzed_suggestion =
                lt_core::AnalyzedTokenReadings::new(self.env.tagger.tag_word(&suggestion));
            let Ok(nominal) = fancy_regex::Regex::new(
                "^(?:NP..[^0].*|NC.[SN].*|A...[SN].|V.P..S..|V.[NG].*|RG|PX..S...)$",
            ) else {
                continue;
            };
            let Ok(verb3) = fancy_regex::Regex::new("^(?:V...3.*)$") else {
                continue;
            };
            if analyzed_suggestion.has_pos_tag_matching_fancy(&nominal) {
                replacements.push(format!("és {}", analyzed_suggestion.surface()));
                used_es_accent = true;
            }
            if analyzed_suggestion.has_pos_tag_matching_fancy(&verb3)
                && !apostrophe_needed.is_match(analyzed_suggestion.surface())
            {
                replacements.push(format!(
                    "es {}",
                    analyzed_suggestion.surface().to_lowercase()
                ));
                used_es = true;
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let first_ch = tokens[pos_word - 1]
            .surface()
            .chars()
            .next()
            .unwrap_or(' ')
            .to_string();
        let definitive_replacements: Vec<String> = if first_ch.to_uppercase() == first_ch {
            replacements
                .iter()
                .map(|r| lt_tagger::uppercase_first_char(r))
                .collect()
        } else {
            replacements
        };
        // Java `equalsIgnoreCase` is Unicode-aware (Rust's
        // `eq_ignore_ascii_case` would miss `É`).
        let is_first_es_accent = tokens[pos_word - 1].surface().to_lowercase() == "és";
        if is_first_es_accent && used_es_accent && !used_es {
            // show just the spelling rule
            return FilterOutcome::reject();
        }
        let mut message = ctx.message.clone();
        if used_es_accent {
            message.push_str(" \"És\" (del verb 'ser') s'escriu amb accent.");
        }
        if used_es {
            message.push_str(" \"Es\" (pronom) acompanya un verb en tercera persona.");
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: Some(suggestions(definitive_replacements)),
        }
    }
}

// ---------------------------------------------------------------------------
// SynthesizeWithDAFilter / SynthesizeWithAnyDeterminerFilter
// ---------------------------------------------------------------------------

const GENDER_NUMBER_PATTERNS: [(&str, &str); 4] = [
    ("MS", "^(?:(N|A.).[MC][SN].*|V.P.*SM.)$"),
    ("FS", "^(?:(N|A.).[FC][SN].*|V.P.*SF.)$"),
    ("MP", "^(?:(N|A.).[MC][PN].*|V.P.*PM.)$"),
    ("FP", "^(?:(N|A.).[FC][PN].*|V.P.*PF.)$"),
];

fn matches_gender_number(postag: &str, gender_number: &str) -> bool {
    GENDER_NUMBER_PATTERNS
        .iter()
        .find(|(gn, _)| *gn == gender_number)
        .and_then(|(_, pattern)| regex::Regex::new(pattern).ok())
        .is_some_and(|re| re.is_match(postag))
}

fn insert_second_best(
    list: &mut Vec<AnalyzedToken>,
    at: AnalyzedToken,
    second_gender_number: &str,
) {
    if list.contains(&at) {
        return;
    }
    let swapped = if second_gender_number.len() == 2 {
        format!(
            "{}{}",
            &second_gender_number[1..2],
            &second_gender_number[0..1]
        )
    } else {
        String::new()
    };
    // Java `tag.contains(secondGenderNumber)`: with an empty value this is
    // true (the `||` short-circuits before `substring(1, 2)`).
    let matches = at
        .pos_tag
        .as_deref()
        .is_some_and(|tag| tag.contains(second_gender_number) || tag.contains(&swapped));
    if matches {
        list.insert(1, at);
    } else {
        list.push(at);
    }
}

/// `org.languagetool.rules.ca.SynthesizeWithDAFilter`.
pub struct SynthesizeWithDAFilter {
    pub(crate) env: Env,
}

impl RuleFilter for SynthesizeWithDAFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(lemma_from_str) = ctx.args.get("lemmaFrom") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_select) = ctx.args.get("lemmaSelect") else {
            return FilterOutcome::reject();
        };
        let synth_all_forms = ctx
            .args
            .get("synthAllForms")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let preposition_from_str = ctx.args.get("prepositionFrom").cloned().unwrap_or_default();
        let Some(lemma_from) =
            get_position(lemma_from_str, ctx.pattern_tokens, ctx.match_range.start)
        else {
            return FilterOutcome::reject();
        };
        let mut preposition = String::new();
        if !preposition_from_str.is_empty() {
            if let Ok(pos) = preposition_from_str.parse::<usize>() {
                if let Some(position) =
                    get_position(&pos.to_string(), ctx.pattern_tokens, ctx.match_range.start)
                {
                    preposition = ctx.pattern_tokens[position]
                        .surface()
                        .chars()
                        .next()
                        .map(|c| c.to_lowercase().to_string())
                        .unwrap_or_default();
                }
            } else {
                preposition = preposition_from_str
                    .chars()
                    .next()
                    .map(|c| c.to_string())
                    .unwrap_or_default();
            }
        }
        let original_word = ctx.pattern_tokens[lemma_from].surface().to_string();
        let Some(original_at) = crate::ca::helpers::reading_with_tag_regex(
            ctx.pattern_tokens[lemma_from],
            lemma_select,
        )
        .cloned() else {
            return FilterOutcome::reject();
        };
        let is_sentence_start =
            is_match_at_sentence_start(ctx.sentence_tokens, ctx.match_range.start);
        let mut potential_suggestions: Vec<AnalyzedToken> = vec![original_at.clone()];
        let mut second_gender_number = String::new();
        if lemma_from > 0 {
            if let Some(reading) = crate::ca::helpers::reading_with_tag_regex(
                ctx.pattern_tokens[lemma_from - 1],
                "D.*",
            ) {
                if let Some(tag) = &reading.pos_tag {
                    second_gender_number = tag.get(3..5).unwrap_or("").to_string();
                }
            }
        }
        let Ok(re) = regex::Regex::new(&format!("^(?:{lemma_select})$")) else {
            return FilterOutcome::reject();
        };
        for tag in self.env.synth.inner().possible_tags() {
            if !re.is_match(tag) {
                continue;
            }
            for synth_form in self.env.synth.inner().synthesize(&original_at, tag, false) {
                let at =
                    AnalyzedToken::new(synth_form, original_at.stem.clone(), Some(tag.clone()));
                if !synth_all_forms && !at.token.eq_ignore_ascii_case(&original_word) {
                    continue;
                }
                insert_second_best(&mut potential_suggestions, at, &second_gender_number);
            }
        }
        let mut results: Vec<String> = Vec::new();
        for potential_suggestion in &potential_suggestions {
            let new_form = &potential_suggestion.token;
            let postag = potential_suggestion.pos_tag.as_deref().unwrap_or("");
            for gender_number in ["MS", "FS", "MP", "FP"] {
                if !matches_gender_number(postag, gender_number) {
                    continue;
                }
                let mut suggestion = format!(
                    "{}{}",
                    get_preposition_and_determiner(new_form, gender_number, &preposition),
                    preserve_case(new_form, &original_word)
                );
                if is_sentence_start {
                    suggestion = lt_tagger::uppercase_first_char(&suggestion);
                }
                if !results.contains(&suggestion) {
                    results.push(suggestion);
                }
            }
        }
        // Java `match.addSuggestedReplacements(suggestions)`: append.
        let mut all: Vec<String> = ctx.suggestions.iter().map(|s| s.value.clone()).collect();
        all.extend(results);
        accept_with(all)
    }
}

/// `org.languagetool.rules.ca.SynthesizeWithAnyDeterminerFilter`.
pub struct SynthesizeWithAnyDeterminerFilter {
    pub(crate) env: Env,
}

impl RuleFilter for SynthesizeWithAnyDeterminerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(lemma_select) = ctx.args.get("lemmaSelect") else {
            return FilterOutcome::reject();
        };
        let synth_all_forms = ctx
            .args
            .get("synthAllForms")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let tokens = tokens_without_whitespace(ctx);
        let mut pos_word = pos_word_start(&tokens, ctx.match_range.start);
        if pos_word >= tokens.len() {
            pos_word = tokens.len() - 1;
        }
        let original_word = tokens[pos_word].surface().to_string();
        let Some(original_at) =
            crate::ca::helpers::reading_with_tag_regex(tokens[pos_word], lemma_select).cloned()
        else {
            return FilterOutcome::reject();
        };
        let mut second_gender_number = String::new();
        let mut determiner_type = String::new();
        let mut determiner_reading: Option<AnalyzedToken> = None;
        let mut preposition = String::new();
        let mut done = false;
        let mut k = 1usize;
        let mut first_underlined_token = pos_word;
        let mut between_string = String::new();
        while pos_word > k && !done {
            done = true;
            let idx = pos_word - k;
            if determiner_reading.is_none() {
                if let Some(reading) =
                    crate::ca::helpers::reading_with_tag_regex(tokens[idx], "D.*")
                {
                    determiner_reading = Some(reading.clone());
                    if let Some(tag) = &reading.pos_tag {
                        second_gender_number = tag.get(3..5).unwrap_or("").to_string();
                        determiner_type = tag.get(0..2).unwrap_or("").to_string();
                        done = determiner_type != "DA";
                    }
                    first_underlined_token = idx;
                }
            }
            let token = tokens[idx];
            if token.has_pos_tag("_QM_OPEN") {
                // Java `betweenToken`: the opening quotation mark between the
                // determiner and the noun (`una "arreplec`).
                between_string = token.surface().to_string();
                done = false;
            }
            if ["a", "de", "per", "pe"].contains(&token.surface()) {
                preposition = token
                    .surface()
                    .chars()
                    .next()
                    .map(|c| c.to_lowercase().to_string())
                    .unwrap_or_default();
                first_underlined_token = idx;
            }
            k += 1;
        }
        let mut potential_suggestions: Vec<AnalyzedToken> = vec![original_at.clone()];
        let Ok(re) = regex::Regex::new(&format!("^(?:{lemma_select})$")) else {
            return FilterOutcome::reject();
        };
        for tag in self.env.synth.inner().possible_tags() {
            if !re.is_match(tag) {
                continue;
            }
            for synth_form in self.env.synth.inner().synthesize(&original_at, tag, false) {
                let at =
                    AnalyzedToken::new(synth_form, original_at.stem.clone(), Some(tag.clone()));
                if !synth_all_forms && !at.token.eq_ignore_ascii_case(&original_word) {
                    continue;
                }
                if potential_suggestions
                    .iter()
                    .any(|i| i.stem == at.stem && i.pos_tag == at.pos_tag)
                {
                    continue;
                }
                insert_second_best(&mut potential_suggestions, at, &second_gender_number);
            }
        }
        let mut results: Vec<String> = Vec::new();
        for potential_suggestion in &potential_suggestions {
            let new_form = &potential_suggestion.token;
            let postag = potential_suggestion.pos_tag.as_deref().unwrap_or("");
            for gender_number in ["MS", "FS", "MP", "FP"] {
                if !matches_gender_number(postag, gender_number) {
                    continue;
                }
                let mut suggestion = if determiner_type == "DA" || determiner_type.is_empty() {
                    format!(
                        "{}{}{}",
                        preserve_case(
                            &get_preposition_and_determiner(new_form, gender_number, &preposition),
                            tokens[first_underlined_token].surface()
                        ),
                        between_string,
                        preserve_case(new_form, &original_word)
                    )
                } else if let Some(determiner_reading) = &determiner_reading {
                    // Java iterates every synthesized determiner form
                    // (`migs` + `mitjos` for `DI.[CM]P.`).
                    let pattern = format!(
                        "{}.[C{}]{}{}",
                        determiner_type,
                        &gender_number[0..1],
                        &gender_number[1..2],
                        "."
                    );
                    let forms =
                        self.env
                            .synth
                            .inner()
                            .synthesize(determiner_reading, &pattern, true);
                    let mut added = false;
                    for form in forms {
                        let mut suggestion = format!(
                            "{} {}{}",
                            preserve_case(&form, tokens[first_underlined_token].surface()),
                            between_string,
                            preserve_case(new_form, &original_word)
                        );
                        if first_underlined_token == 1 {
                            suggestion = lt_tagger::uppercase_first_char(&suggestion);
                        }
                        if !results.contains(&suggestion) {
                            results.push(suggestion);
                        }
                        added = true;
                    }
                    if !added {
                        continue;
                    }
                    continue;
                } else {
                    continue;
                };
                if first_underlined_token == 1 {
                    suggestion = lt_tagger::uppercase_first_char(&suggestion);
                }
                if !results.contains(&suggestion) {
                    results.push(suggestion);
                }
            }
        }
        // Java creates the match with `tokens[firstUnderlinedToken].getStartPos()`
        // as the from position, so the reported range covers the determiner.
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[first_underlined_token].start_pos,
                ctx.match_range.end,
            )),
            message: None,
            suggestions: Some(suggestions(results)),
        }
    }
}

// ---------------------------------------------------------------------------
// EnNoInfinitiuSuggestionFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.EnNoInfinitiuSuggestionFilter`.
pub struct EnNoInfinitiuSuggestionFilter {
    pub(crate) env: Env,
}

impl RuleFilter for EnNoInfinitiuSuggestionFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let temps_verbals_present = ["IP", "IF"];
        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let verb_synth_infinitiu =
            VerbSynthesizer::new(&tokens, pos_word, self.env.synth.inner(), false);
        let next_pos = (verb_synth_infinitiu.get_last_index() + 1).max(0) as usize;
        let verb_after = VerbSynthesizer::new(&tokens, next_pos, self.env.synth.inner(), false);
        let before_pos = (verb_synth_infinitiu.get_first_verb_index() - 1).max(0) as usize;
        let verb_before = VerbSynthesizer::new(&tokens, before_pos, self.env.synth.inner(), true);
        if verb_synth_infinitiu.is_undefined() {
            return FilterOutcome::reject();
        }
        if tokens[verb_synth_infinitiu.get_last_verb_index() as usize].end_pos()
            > ctx.match_range.end
        {
            return FilterOutcome::reject();
        }
        let verb_after_postag = verb_after.get_first_verb_is_postag();
        let verb_before_postag = verb_before.get_first_verb_is_postag();
        let (postag_temps_verbal, verb_before_defined, is_passat_perifrastic) =
            if !verb_after.is_undefined() {
                match verb_after_postag {
                    Some(postag) => (
                        postag,
                        !verb_before.is_undefined() && verb_before_postag.is_some(),
                        verb_after.is_passat_perifrastic(),
                    ),
                    None => return FilterOutcome::reject(),
                }
            } else if let Some(postag) = verb_before_postag {
                (postag, true, verb_before.is_passat_perifrastic())
            } else {
                return FilterOutcome::reject();
            };
        let postag_prefix = if temps_verbals_present
            .contains(&postag_temps_verbal.get(2..4).unwrap_or(""))
            && !is_passat_perifrastic
        {
            "VMIP"
        } else {
            "VMII"
        };
        let mut synth_verbs: Vec<String> = Vec::new();
        let mut verb_synth_infinitiu = verb_synth_infinitiu;
        if postag_temps_verbal.get(4..6) != Some("3S") {
            verb_synth_infinitiu.set_postag(&format!(
                "{postag_prefix}3S{}",
                postag_temps_verbal.get(6..).unwrap_or("")
            ));
            synth_verbs.push(verb_synth_infinitiu.synthesize());
        }
        verb_synth_infinitiu.set_postag(&format!(
            "{postag_prefix}{}",
            postag_temps_verbal.get(4..).unwrap_or("")
        ));
        synth_verbs.push(verb_synth_infinitiu.synthesize());
        let mut start_pos = (verb_synth_infinitiu.get_first_verb_index() - 2).max(0) as usize;
        if tokens[start_pos].surface().eq_ignore_ascii_case("l") {
            start_pos = start_pos.saturating_sub(1);
        }
        let end_pos = verb_synth_infinitiu.get_last_index() as usize;
        let original_str =
            &ctx.sentence_text[tokens[start_pos].start_pos..tokens[end_pos].end_pos()];
        let mut synth_suggestions: Vec<String> = Vec::new();
        for synth_verb in synth_verbs {
            let mut suggestion = String::new();
            if verb_before_defined {
                suggestion.push_str("perquè no ");
            } else {
                suggestion.push_str("com que no ");
            }
            let pronouns_after = verb_synth_infinitiu.get_pronouns_str_after();
            if !pronouns_after.is_empty() {
                suggestion.push_str(&transform_davant(&pronouns_after, &synth_verb));
            }
            suggestion.push_str(&synth_verb);
            let suggestion_str = preserve_case(&suggestion, original_str);
            if !synth_suggestions.contains(&suggestion_str) {
                synth_suggestions.push(suggestion_str);
            }
        }
        if synth_suggestions.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[start_pos].start_pos,
                tokens[end_pos].end_pos(),
            )),
            message: None,
            suggestions: Some(suggestions(synth_suggestions)),
        }
    }
}

// ---------------------------------------------------------------------------
// PossessiusRedundantsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.PossessiusRedundantsFilter`.
pub struct PossessiusRedundantsFilter;

impl RuleFilter for PossessiusRedundantsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let mut pos_possessive = ctx.pattern_token_pos;
        while pos_possessive < tokens.len()
            && !crate::ca::helpers::has_partial_pos_tag(tokens[pos_possessive], "PX")
        {
            pos_possessive += 1;
        }
        let Some(possessive_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[pos_possessive], "PX.*")
        else {
            return FilterOutcome::reject();
        };
        let possessive_postag = possessive_reading.pos_tag.clone().unwrap_or_default();
        let number = possessive_postag.get(6..7).unwrap_or("").to_string();
        let persona = possessive_postag.get(2..3).unwrap_or("").to_string();
        let mut pos_verb = ctx.pattern_token_pos.saturating_sub(1);
        while pos_verb > 0 && tokens[pos_verb].chunk_tags.iter().any(|c| c == "GV") {
            pos_verb -= 1;
        }
        pos_verb += 1;
        let mut pronoun_found = false;
        let mut has_some_pronoun = false;
        let matches_pronoun = |token: &lt_core::AnalyzedTokenReadings| {
            crate::ca::helpers::reading_with_tag_regex(token, "P.*")
                .and_then(|r| r.pos_tag.as_deref())
                .is_some_and(|tag| {
                    tag.get(2..3) == Some(persona.as_str())
                        && (number == "C" || tag.get(4..5) == Some(number.as_str()))
                })
        };
        let mut pos_pronoun = pos_verb.saturating_sub(1);
        while !pronoun_found
            && pos_pronoun > 0
            && (tokens[pos_pronoun].has_pos_tag_starting_with("PP")
                || tokens[pos_pronoun].has_pos_tag_starting_with("P0"))
        {
            has_some_pronoun = true;
            pronoun_found = matches_pronoun(tokens[pos_pronoun]);
            pos_pronoun -= 1;
        }
        pos_pronoun = ctx.pattern_token_pos + 1;
        while !pronoun_found
            && pos_pronoun < tokens.len()
            && (tokens[pos_pronoun].has_pos_tag_starting_with("PP")
                || tokens[pos_pronoun].has_pos_tag_starting_with("P0"))
        {
            has_some_pronoun = true;
            pronoun_found = matches_pronoun(tokens[pos_pronoun]);
            pos_pronoun += 1;
        }
        let apostrophe_needed = pos_possessive > 0
            && (tokens[pos_possessive - 1].has_pos_tag("DA0MS0")
                || tokens[pos_possessive - 1].has_pos_tag("DA0FS0"))
            && tokens.get(pos_possessive + 1).is_some_and(|t| {
                crate::ca::helpers::apostrophe_needed().is_match(&t.surface().to_lowercase())
            });
        if pronoun_found {
            if apostrophe_needed {
                return FilterOutcome {
                    accepted: true,
                    range: Some(TextRange::new(
                        tokens[pos_possessive - 1].start_pos,
                        tokens[pos_possessive + 1].end_pos(),
                    )),
                    message: None,
                    suggestions: Some(suggestions(vec![format!(
                        "l'{}",
                        tokens[pos_possessive + 1].surface()
                    )])),
                };
            }
            return FilterOutcome {
                accepted: true,
                range: Some(TextRange::new(
                    tokens[pos_possessive].start_pos,
                    tokens[pos_possessive].end_pos(),
                )),
                message: None,
                suggestions: Some(suggestions(vec![String::new()])),
            };
        }
        if !has_some_pronoun {
            let mut suggestion = String::new();
            // Java `hasAnyPartialPosTag("VMN", "VMG")` — prefix match
            if crate::ca::helpers::has_partial_pos_tag(tokens[pos_verb], "VMN")
                || crate::ca::helpers::has_partial_pos_tag(tokens[pos_verb], "VMG")
            {
                let pronoun_sugg = transform_darrere(
                    &crate::ca::helpers::get_dative_pronoun(&format!("{persona}{number}")),
                    tokens[pos_verb].surface(),
                );
                suggestion.push_str(tokens[pos_verb].surface());
                suggestion.push_str(&pronoun_sugg);
            } else {
                let pronoun_sugg = transform_davant(
                    &crate::ca::helpers::get_dative_pronoun(&format!("{persona}{number}")),
                    tokens[pos_verb].surface(),
                );
                suggestion.push_str(&preserve_case(&pronoun_sugg, tokens[pos_verb].surface()));
                suggestion.push_str(&tokens[pos_verb].surface().to_lowercase());
            }
            if pos_possessive >= 2 {
                for token in &tokens[pos_verb + 1..=pos_possessive - 2] {
                    if token.whitespace_before {
                        suggestion.push(' ');
                    }
                    suggestion.push_str(&token.surface().to_lowercase());
                }
            }
            if apostrophe_needed {
                suggestion.push(' ');
                suggestion.push_str(&format!("l'{}", tokens[pos_possessive + 1].surface()));
            } else if pos_possessive >= 1 {
                for (i, token) in tokens
                    .iter()
                    .enumerate()
                    .take(pos_possessive + 2)
                    .skip(pos_possessive - 1)
                {
                    if i == pos_possessive {
                        continue;
                    }
                    if token.whitespace_before {
                        suggestion.push(' ');
                    }
                    suggestion.push_str(token.surface());
                }
            }
            return FilterOutcome {
                accepted: true,
                range: Some(TextRange::new(
                    tokens[pos_verb].start_pos,
                    tokens[pos_possessive + 1].end_pos(),
                )),
                message: None,
                suggestions: Some(suggestions(vec![suggestion])),
            };
        }
        FilterOutcome::reject()
    }
}

// ---------------------------------------------------------------------------
// OblidarseSugestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.OblidarseSugestionsFilter`.
pub struct OblidarseSugestionsFilter {
    pub(crate) env: Env,
}

impl RuleFilter for OblidarseSugestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let add_reflexive_vowel: [(&str, &str); 6] = [
            ("1S", "m'"),
            ("2S", "t'"),
            ("3S", "s'"),
            ("1P", "ens "),
            ("2P", "us "),
            ("3P", "s'"),
        ];
        let add_reflexive_consonant: [(&str, &str); 6] = [
            ("1S", "em "),
            ("2S", "et "),
            ("3S", "es "),
            ("1P", "ens "),
            ("2P", "us "),
            ("3P", "es "),
        ];
        let add_reflexive_en_vowel: [(&str, &str); 6] = [
            ("1S", "me n'"),
            ("2S", "te n'"),
            ("3S", "se n'"),
            ("1P", "ens n'"),
            ("2P", "us n'"),
            ("3P", "se n'"),
        ];
        let add_reflexive_en_consonant: [(&str, &str); 6] = [
            ("1S", "me'n "),
            ("2S", "te'n "),
            ("3S", "se'n "),
            ("1P", "ens en "),
            ("2P", "us en "),
            ("3P", "se'n "),
        ];
        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let Some(pronom_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[pos_word + 1], "P.*")
        else {
            return FilterOutcome::reject();
        };
        let pronom_postag = pronom_reading.pos_tag.clone().unwrap_or_default();
        let pronom_gender_number = format!(
            "{}{}",
            pronom_postag.get(2..3).unwrap_or(""),
            pronom_postag.get(4..5).unwrap_or("")
        );
        let mut index_main_verb = pos_word + 2;
        while index_main_verb < tokens.len()
            && !(tokens[index_main_verb].has_lemma("oblidar")
                || tokens[index_main_verb].has_lemma("descuidar")
                || tokens[index_main_verb].has_lemma("passar"))
        {
            index_main_verb += 1;
        }
        if index_main_verb >= tokens.len() {
            return FilterOutcome::reject();
        }
        let Some(verb_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[pos_word + 2], "V.*")
        else {
            return FilterOutcome::reject();
        };
        let verb_postag = verb_reading.pos_tag.clone().unwrap_or_default();
        let mut lemma = verb_reading.lemma().to_string();
        if lemma == "passar" {
            lemma = "descuidar".to_string();
        }
        let at = AnalyzedToken::new("", Some(lemma), None);
        let synth_forms = self.env.synth.inner().synthesize(
            &at,
            &format!(
                "{}{}{}",
                verb_postag.get(0..4).unwrap_or(""),
                pronom_gender_number,
                verb_postag.get(6..8).unwrap_or("")
            ),
            false,
        );
        let Some(new_verb_first) = synth_forms.first() else {
            return FilterOutcome::reject();
        };
        let mut new_verb = new_verb_first.clone();
        for token in &tokens[pos_word + 3..=index_main_verb] {
            if token.whitespace_before {
                new_verb.push(' ');
            }
            new_verb.push_str(
                &token
                    .surface()
                    .replace("passar", "descuidar")
                    .replace("passat", "descuidat")
                    .replace("passant", "descuidant"),
            );
        }
        let verb_vowel = crate::ca::helpers::apostrophe_needed().is_match(&new_verb.to_lowercase());
        let mut word_after = String::new();
        if index_main_verb + 1 < tokens.len() {
            if let Some(reading) = crate::ca::helpers::reading_with_tag_regex(
                tokens[index_main_verb + 1],
                "D.*|V.N.*|P[DI].*|NC.*",
            ) {
                word_after = reading.token.clone();
            }
            let next_lower = tokens[index_main_verb + 1].surface().to_lowercase();
            if ["com", "de", "d'", "que"].contains(&next_lower.as_str()) {
                word_after = tokens[index_main_verb + 1].surface().to_string();
            }
        }
        let transform = if word_after.is_empty()
            && !word_after.eq_ignore_ascii_case("de")
            && !word_after.eq_ignore_ascii_case("d'")
            && !word_after.eq_ignore_ascii_case("que")
        {
            if verb_vowel {
                &add_reflexive_en_vowel
            } else {
                &add_reflexive_en_consonant
            }
        } else if verb_vowel {
            &add_reflexive_vowel
        } else {
            &add_reflexive_consonant
        };
        let Some(transform_value) = transform
            .iter()
            .find(|(k, _)| *k == pronom_gender_number.as_str())
            .map(|(_, v)| *v)
        else {
            return FilterOutcome::reject();
        };
        let mut sugg_bld = String::new();
        sugg_bld.push_str(transform_value);
        sugg_bld.push_str(&new_verb);
        let mut characters_after_correction = 0usize;
        if word_after.eq_ignore_ascii_case("el") || word_after.eq_ignore_ascii_case("els") {
            sugg_bld.push_str(" d");
            sugg_bld.push_str(&word_after.to_lowercase());
            characters_after_correction = word_after.chars().count() + 1;
        } else if !word_after.is_empty()
            && !word_after.eq_ignore_ascii_case("de")
            && !word_after.eq_ignore_ascii_case("d'")
            && !word_after.eq_ignore_ascii_case("que")
        {
            let word_after_apostrophe =
                crate::ca::helpers::apostrophe_needed().is_match(&word_after.to_lowercase());
            sugg_bld.push_str(if word_after_apostrophe { " d'" } else { " de" });
            characters_after_correction = usize::from(word_after_apostrophe);
        }
        let replacement = preserve_case(&sugg_bld, tokens[pos_word].surface());
        let mut replacements: Vec<String> = vec![replacement];
        for s in &ctx.suggestions {
            let mut s = s.value.clone();
            if characters_after_correction == 1 {
                s.push(' ');
            }
            replacements.push(adapt_suggestion(&s, tokens[pos_word].surface()));
        }
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[pos_word].start_pos,
                tokens[index_main_verb].end_pos() + characters_after_correction,
            )),
            message: Some(ctx.message.replace("passar", "descuidar")),
            suggestions: Some(suggestions(replacements)),
        }
    }
}

// ---------------------------------------------------------------------------
// DonarTempsSuggestionsFilter / AnarASuggestionsFilter / Portar*Filters
// ---------------------------------------------------------------------------

fn tag_slice(tag: &str, start: usize, end: usize) -> &str {
    tag.get(start..end).unwrap_or("")
}

/// `org.languagetool.rules.ca.DonarTempsSuggestionsFilter`.
pub struct DonarTempsSuggestionsFilter {
    pub(crate) env: Env,
}

impl RuleFilter for DonarTempsSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let Some(pronom_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[pos_word], "P.*")
        else {
            return FilterOutcome::reject();
        };
        let pronom_postag = pronom_reading.pos_tag.clone().unwrap_or_default();
        let pronom_gender_number = format!(
            "{}{}",
            tag_slice(&pronom_postag, 2, 3),
            tag_slice(&pronom_postag, 4, 5)
        );
        let index_first_verb = pos_word + 1;
        let mut index_main_verb = index_first_verb;
        while index_main_verb < tokens.len() && !tokens[index_main_verb].has_lemma("donar") {
            index_main_verb += 1;
        }
        if index_main_verb >= tokens.len() {
            return FilterOutcome::reject();
        }
        let Some(verb_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[index_main_verb], "V.*")
        else {
            return FilterOutcome::reject();
        };
        let verb_postag = verb_reading.pos_tag.clone().unwrap_or_default();
        // haver-hi temps
        let at = AnalyzedToken::new("", Some("haver".to_string()), None);
        let synth_forms = self.env.synth.inner().synthesize(
            &at,
            &format!("VA{}", tag_slice(&verb_postag, 2, 8)),
            false,
        );
        let mut suggestion1 = String::new();
        if let Some(first) = synth_forms.first() {
            let mut index = index_first_verb;
            suggestion1.push_str("hi");
            while index < index_main_verb {
                if tokens[index].whitespace_before || suggestion1.chars().count() == 2 {
                    suggestion1.push(' ');
                }
                suggestion1.push_str(tokens[index].surface());
                index += 1;
            }
            suggestion1.push(' ');
            suggestion1.push_str(first);
            suggestion1.push_str(" temps");
        }
        let mut replacements: Vec<String> = Vec::new();
        let mut sugg1 = suggestion1.replace("de haver", "d'haver");
        sugg1 = preserve_case(&sugg1, tokens[pos_word].surface());
        if !sugg1.is_empty() {
            replacements.push(sugg1);
        }
        // tenir temps
        let mut suggestion2 = String::new();
        let mut index = index_first_verb;
        if index == index_main_verb {
            let synth_forms2 = self.env.synth.inner().synthesize(
                &AnalyzedToken::new("", Some("tenir".to_string()), None),
                &format!(
                    "{}{}{}",
                    tag_slice(&verb_postag, 0, 4),
                    pronom_gender_number,
                    tag_slice(&verb_postag, 6, 8)
                ),
                false,
            );
            if let Some(first) = synth_forms2.first() {
                suggestion2.push_str(first);
                suggestion2.push_str(" temps");
            }
        } else {
            let at2 = tokens[index_first_verb].readings.first().cloned();
            if let Some(at2) = at2 {
                let at2_postag = at2.pos_tag.clone().unwrap_or_default();
                let synth_forms2 = self.env.synth.inner().synthesize(
                    &at2,
                    &format!(
                        "{}{}{}",
                        tag_slice(&at2_postag, 0, 4),
                        pronom_gender_number,
                        tag_slice(&at2_postag, 6, 8)
                    ),
                    false,
                );
                if let Some(first) = synth_forms2.first() {
                    suggestion2.push_str(first);
                    index += 1;
                    while index < index_main_verb {
                        if tokens[index].whitespace_before {
                            suggestion2.push(' ');
                        }
                        suggestion2.push_str(tokens[index].surface());
                        index += 1;
                    }
                    let last_postag = tokens[index_main_verb]
                        .readings
                        .first()
                        .and_then(|r| r.pos_tag.clone())
                        .unwrap_or_default();
                    let synth_forms3 = self.env.synth.inner().synthesize(
                        &AnalyzedToken::new("", Some("tenir".to_string()), None),
                        &last_postag,
                        false,
                    );
                    if let Some(first3) = synth_forms3.first() {
                        suggestion2.push(' ');
                        suggestion2.push_str(first3);
                        suggestion2.push_str(" temps");
                    } else {
                        suggestion2.clear();
                    }
                }
            }
        }
        let sugg2 = preserve_case(&suggestion2, tokens[pos_word].surface());
        if !sugg2.is_empty() {
            replacements.push(sugg2);
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let end_token = tokens
            .get(index_main_verb + 1)
            .copied()
            .unwrap_or(tokens[index_main_verb]);
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[pos_word].start_pos,
                end_token.end_pos(),
            )),
            message: None,
            suggestions: Some(suggestions(replacements)),
        }
    }
}

/// `org.languagetool.rules.ca.AnarASuggestionsFilter`.
pub struct AnarASuggestionsFilter {
    pub(crate) env: Env,
}

impl RuleFilter for AnarASuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let init_pos = pos_word_start(&tokens, ctx.match_range.start);
        let verb_synth = VerbSynthesizer::new(&tokens, init_pos, self.env.synth.inner(), false);
        if verb_synth.is_undefined() {
            return FilterOutcome::reject();
        }
        if tokens[verb_synth.get_last_verb_index() as usize].end_pos() > ctx.match_range.end {
            return FilterOutcome::reject();
        }
        let init_pos = verb_synth.get_first_verb_index() as usize;
        let Some(verb_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[init_pos], "V.IP.*")
        else {
            return FilterOutcome::reject();
        };
        let verb_postag = verb_reading.pos_tag.clone().unwrap_or_default();
        let Some(lemma_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[init_pos + 2], "V.N.*")
        else {
            return FilterOutcome::reject();
        };
        let lemma = lemma_reading.lemma().to_string();
        let at = AnalyzedToken::new("", Some(lemma), None);
        let mut synth_forms_list: Vec<String> = Vec::new();
        let new_postag = format!("V[MS]IF{}", tag_slice(&verb_postag, 4, 8));
        synth_forms_list.extend(self.env.synth.inner().synthesize(&at, &new_postag, true));
        let new_postag = format!("V[MS]IP{}", tag_slice(&verb_postag, 4, 8));
        synth_forms_list.extend(self.env.synth.inner().synthesize(&at, &new_postag, true));
        if synth_forms_list.is_empty() {
            return FilterOutcome::reject();
        }
        let adjust_end_pos = verb_synth.get_num_pronouns_after();
        let pronoms_darrere = verb_synth.get_pronouns_str_after();
        let adjust_start_pos = verb_synth.get_num_pronouns_before();
        let pronoms_davant = verb_synth.get_pronouns_str_before();
        let mut replacements: Vec<String> =
            ctx.suggestions.iter().map(|s| s.value.clone()).collect();
        for verb in &synth_forms_list {
            let mut suggestion = String::new();
            if !pronoms_darrere.is_empty() {
                suggestion = transform_davant(&pronoms_darrere, verb);
            } else if !pronoms_davant.is_empty() {
                suggestion = transform_davant(&pronoms_davant, verb);
            }
            suggestion.push_str(verb);
            suggestion = preserve_case(
                &suggestion,
                tokens[(init_pos as isize - adjust_start_pos) as usize].surface(),
            );
            replacements.push(suggestion);
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let replacements: Vec<Suggestion> = replacements
            .into_iter()
            .map(|value| Suggestion {
                value: adapt_suggestion(&value, ""),
                short_description: None,
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[(init_pos as isize - adjust_start_pos) as usize].start_pos,
                tokens[(init_pos as isize + 2 + adjust_end_pos) as usize].end_pos(),
            )),
            message: None,
            suggestions: Some(replacements),
        }
    }
}

/// `org.languagetool.rules.ca.PortarGerundiSuggestionsFilter`.
pub struct PortarGerundiSuggestionsFilter {
    pub(crate) env: Env,
}

impl RuleFilter for PortarGerundiSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let new_lemma = ctx.args.get("newLemma").cloned().unwrap_or_default();
        let Some(atr1) = crate::ca::helpers::reading_with_tag_regex(tokens[pos_word], "V.[IS].*")
        else {
            return FilterOutcome::reject();
        };
        let atr1_postag = atr1.pos_tag.clone().unwrap_or_default();
        let Some(atr2) = crate::ca::helpers::reading_with_tag_regex(tokens[pos_word + 1], "V.G.*")
        else {
            return FilterOutcome::reject();
        };
        let lemma = if new_lemma.is_empty() {
            atr2.lemma().to_string()
        } else {
            new_lemma
        };
        let synth_forms1 = self.env.synth.inner().synthesize(
            &AnalyzedToken::new("", Some("haver".to_string()), None),
            &format!("VA{}", tag_slice(&atr1_postag, 2, atr1_postag.len())),
            true,
        );
        let synth_forms2 = self.env.synth.inner().synthesize(
            &AnalyzedToken::new("", Some(lemma.clone()), None),
            "V.P..SM.",
            true,
        );
        let mut replacements: Vec<String> = Vec::new();
        for f1 in &synth_forms1 {
            for f2 in &synth_forms2 {
                replacements.push(format!("{f1} {f2}"));
            }
        }
        // faig
        let synth_forms3 = self.env.synth.inner().synthesize(
            &AnalyzedToken::new("", Some(lemma), None),
            &format!("V.{}", tag_slice(&atr1_postag, 2, atr1_postag.len())),
            true,
        );
        if let Some(first) = synth_forms3.first() {
            replacements.push(first.clone());
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let verb_synth = VerbSynthesizer::new(&tokens, pos_word, self.env.synth.inner(), false);
        let mut correct_start_index = 0isize;
        let mut correct_end_index = 0isize;
        for replacement in &mut replacements {
            let mut pronouns_suggestion = String::new();
            if verb_synth.get_num_pronouns_after() > 0 {
                pronouns_suggestion =
                    transform_davant(&verb_synth.get_pronouns_str_after(), replacement);
                correct_end_index = verb_synth.get_num_pronouns_after();
            } else if verb_synth.get_num_pronouns_before() > 0 {
                pronouns_suggestion =
                    transform_davant(&verb_synth.get_pronouns_str_before(), replacement);
                correct_start_index = -verb_synth.get_num_pronouns_before();
            }
            *replacement = preserve_case(
                &format!("{pronouns_suggestion}{replacement}"),
                tokens[(pos_word as isize + correct_start_index) as usize].surface(),
            );
        }
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[(pos_word as isize + correct_start_index) as usize].start_pos,
                tokens[(pos_word as isize + 1 + correct_end_index) as usize].end_pos(),
            )),
            message: None,
            suggestions: Some(suggestions(replacements)),
        }
    }
}

/// `org.languagetool.rules.ca.PortarTempsSuggestionsFilter`.
pub struct PortarTempsSuggestionsFilter {
    pub(crate) env: Env,
}

impl RuleFilter for PortarTempsSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let Some(verb_reading) =
            crate::ca::helpers::reading_with_tag_regex(tokens[pos_word], "V.*")
        else {
            return FilterOutcome::reject();
        };
        let verb_postag = verb_reading.pos_tag.clone().unwrap_or_default();
        let at = AnalyzedToken::new("", Some("fer".to_string()), None);
        let new_postag = format!(
            "{}[30][S0].{}",
            tag_slice(&verb_postag, 0, 4),
            tag_slice(&verb_postag, 7, 8)
        );
        let synth_forms = self.env.synth.inner().synthesize(&at, &new_postag, true);
        let Some(first) = synth_forms.first() else {
            return FilterOutcome::reject();
        };
        let mut suggestion = first.clone();
        let mut i = pos_word + 1;
        while i < tokens.len() && tokens[i].chunk_tags.iter().any(|c| c == "PTime") {
            if tokens[i].whitespace_before {
                suggestion.push(' ');
            }
            suggestion.push_str(tokens[i].surface());
            i += 1;
        }
        let last_token_pos = i;
        if last_token_pos + 1 >= tokens.len() {
            return FilterOutcome::reject();
        }
        let mut adjust_end_pos = 0isize;
        let last_token = tokens[last_token_pos];
        if last_token.surface() == "que" {
            suggestion.push_str(" que");
        } else if last_token.has_pos_tag_starting_with("VMG")
            || last_token.has_pos_tag_starting_with("VSG")
        {
            suggestion.push_str(" que ");
            let verb_synth =
                VerbSynthesizer::new(&tokens, last_token_pos, self.env.synth.inner(), false);
            let pronoms = verb_synth.get_pronouns_str_after();
            adjust_end_pos += verb_synth.get_num_pronouns_after();
            let Some(at2) = crate::ca::helpers::reading_with_tag_regex(last_token, "V.G.*") else {
                return FilterOutcome::reject();
            };
            let synth_forms2 = self.env.synth.inner().synthesize(
                &AnalyzedToken::new("", Some(at2.lemma().to_string()), None),
                &format!("V.I{}", tag_slice(&verb_postag, 3, 8)),
                true,
            );
            let Some(first2) = synth_forms2.first() else {
                return FilterOutcome::reject();
            };
            if !pronoms.is_empty() {
                suggestion.push_str(&transform_davant(&pronoms, first2));
            }
            suggestion.push_str(first2);
        } else if last_token.surface() == "sense"
            && (tokens[last_token_pos + 1].has_pos_tag_starting_with("VSN")
                || tokens[last_token_pos + 1].has_pos_tag_starting_with("VMN"))
        {
            suggestion.push_str(" que no ");
            adjust_end_pos += 1;
            let verb_synth =
                VerbSynthesizer::new(&tokens, last_token_pos + 1, self.env.synth.inner(), false);
            let pronoms = verb_synth.get_pronouns_str_after();
            adjust_end_pos += verb_synth.get_num_pronouns_after();
            let Some(at2) =
                crate::ca::helpers::reading_with_tag_regex(tokens[last_token_pos + 1], "V.N.*")
            else {
                return FilterOutcome::reject();
            };
            let synth_forms2 = self.env.synth.inner().synthesize(
                &AnalyzedToken::new("", Some(at2.lemma().to_string()), None),
                &format!("V.I{}", tag_slice(&verb_postag, 3, 8)),
                false,
            );
            let Some(first2) = synth_forms2.first() else {
                return FilterOutcome::reject();
            };
            if !pronoms.is_empty() {
                suggestion.push_str(&transform_davant(&pronoms, first2));
            }
            suggestion.push_str(first2);
        } else if [
            "així", "a", "en", "ací", "aquí", "ahí", "allí", "allà", "de",
        ]
        .contains(&last_token.surface())
            || last_token.has_pos_tag_starting_with("AQ")
            || last_token.has_pos_tag_starting_with("VMP")
        {
            let at2 = AnalyzedToken::new("", Some("estar".to_string()), None);
            let synth_forms2 = self.env.synth.inner().synthesize(
                &at2,
                &format!("V.I{}", tag_slice(&verb_postag, 3, 8)),
                false,
            );
            let Some(first2) = synth_forms2.first() else {
                return FilterOutcome::reject();
            };
            suggestion.push_str(" que ");
            suggestion.push_str(first2);
            adjust_end_pos -= 1;
        } else {
            return FilterOutcome::reject();
        }
        let replacement = preserve_case(&suggestion, tokens[pos_word].surface());
        if replacement.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[pos_word].start_pos,
                tokens[(last_token_pos as isize + adjust_end_pos) as usize].end_pos(),
            )),
            message: None,
            suggestions: Some(suggestions(vec![replacement])),
        }
    }
}

// ---------------------------------------------------------------------------
// Documented stubs (behavior to be ported; see `ca-rule-port.md`)
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.SuppressIfAnyRuleMatchesFilter`: suppress the
/// match when a replacement creates a new match of one of the `ruleIDs`.
pub struct CaSuppressIfAnyRuleMatchesFilter {
    pub(crate) env: Env,
}

impl RuleFilter for CaSuppressIfAnyRuleMatchesFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(rule_ids_arg) = ctx.args.get("ruleIDs") else {
            return FilterOutcome::reject();
        };
        let rule_ids: Vec<String> = rule_ids_arg.split(',').map(str::to_string).collect();
        let Some(outer) = self.env.outer_pipeline.get().and_then(|w| w.upgrade()) else {
            return FilterOutcome::accept();
        };
        let sentence_start = ctx
            .sentence_tokens
            .first()
            .map(|t| t.start_pos)
            .unwrap_or(0);
        let sentence_text = ctx.sentence_text;
        let from = ctx
            .match_range
            .start
            .saturating_sub(sentence_start)
            .min(sentence_text.len());
        let to = ctx
            .match_range
            .end
            .saturating_sub(sentence_start)
            .min(sentence_text.len());
        for replacement in &ctx.suggestions {
            let mut new_sentence =
                String::with_capacity(sentence_text.len() + replacement.value.len());
            new_sentence.push_str(&sentence_text[..from]);
            new_sentence.push_str(&replacement.value);
            new_sentence.push_str(&sentence_text[to..]);
            let Some(analyzed) = self.env.analyze_text(&new_sentence) else {
                continue;
            };
            let matches = outer.match_rule_ids(&analyzed, &new_sentence, 0, &rule_ids);
            for m in &matches {
                if (m.range.end >= from && m.range.end <= to)
                    || (to >= m.range.start && to <= m.range.end)
                {
                    return FilterOutcome::reject();
                }
            }
        }
        FilterOutcome::accept()
    }
}
