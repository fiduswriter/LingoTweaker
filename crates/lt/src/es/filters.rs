//! Spanish filter registry (`<filter class="...">` implementations used by
//! `es/rules/grammar.xml`, `style.xml` and the disambiguation rules).
//!
//! Every class referenced by the pinned Spanish rule data is registered here;
//! a class that fails to compile because a filter is missing shows up in
//! `engine.compile_failures()` (never silently dropped).

use std::collections::HashMap;
use std::sync::Arc;

use lt_core::{Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::Ymd;

/// Shared state for Spanish filters.
pub struct EsFilterEnv {
    pub tagger: Arc<lt_tagger::SpanishTagger>,
    pub synth: Arc<crate::es::synthesizer::SpanishSynthesizerAdapter>,
    /// `MorfologikSpanishSpellerRule` (the default spelling rule)
    pub spelling: Arc<crate::es::spelling::SpanishSpellingRule>,
    /// `SpanishMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// Used by the date filters.
    pub today: Ymd,
    /// Data root (rules-dir resources like `es/rules/confusion_pairs.txt`).
    pub data_dir: std::path::PathBuf,
}

pub type Env = Arc<EsFilterEnv>;

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a str> {
    ctx.args.get(key).map(|s| s.as_str())
}

/// `AnalyzedTokenReadings.matchesPosTagRegex`: at least one reading's POS tag
/// fully matches the pattern.
fn matches_pos_tag_regex(tr: &lt_core::AnalyzedTokenReadings, pattern: &str) -> bool {
    let Ok(re) = regex::Regex::new(&format!("^(?:{pattern})$")) else {
        return false;
    };
    tr.readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| re.is_match(t)))
}

/// `RuleFilter.getPosition` (`wordFrom` / `marker[N]`).
fn get_position(
    from_str: &str,
    pattern_tokens: &[&lt_core::AnalyzedTokenReadings],
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

fn pos_tag_regex(pattern: &str) -> Option<fancy_regex::Regex> {
    fancy_regex::Regex::new(&format!("^(?:{pattern})$")).ok()
}

/// `StringTools.isPunctuationOrSymbol` (`[\p{IsPunctuation}\p{S}']`).
fn is_punctuation_or_symbol(s: &str) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{P}\p{S}']$").unwrap());
    RE.is_match(s)
}

fn is_not_word_string(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| !c.is_alphabetic())
}

/// `org.languagetool.rules.IsEnglishWordFilter`: with the pinned Spanish
/// classpath there is no `en-US` language (`Languages.getLanguageForShortCode`
/// returns null in the language-es module and in the Docker oracle), so Java's
/// filter constructor leaves `tagger == null` and every match is rejected.
struct IsEnglishWordFilter;

impl RuleFilter for IsEnglishWordFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }
}

// ---------------------------------------------------------------------------
// ConfusionCheckFilter (+ ConfusionPairsDataLoader)
// ---------------------------------------------------------------------------

/// `ConfusionPairsDataLoader`: `word;suggestion;postag` lines, all readings
/// per word.
struct ConfusionPairs {
    map: HashMap<String, lt_core::AnalyzedTokenReadings>,
}

impl ConfusionPairs {
    fn load(path: &std::path::Path) -> Self {
        let mut map: HashMap<String, lt_core::AnalyzedTokenReadings> = HashMap::new();
        let Ok(text) = lt_data::fs::read_to_string(path) else {
            return Self { map };
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
            let analyzed =
                lt_core::AnalyzedToken::new(parts[1], None::<String>, Some(parts[2].to_string()));
            match map.get_mut(parts[0]) {
                Some(atrs) => atrs.add_reading(analyzed),
                None => {
                    let mut atrs = lt_core::AnalyzedTokenReadings::new(vec![analyzed]);
                    atrs.start_pos = 0;
                    map.insert(parts[0].to_string(), atrs);
                }
            }
        }
        Self { map }
    }
}

/// `org.languagetool.rules.es.ConfusionCheckFilter`.
struct ConfusionCheckFilter {
    pairs: ConfusionPairs,
}

impl ConfusionCheckFilter {
    fn load(path: &std::path::Path) -> Self {
        Self {
            pairs: ConfusionPairs::load(path),
        }
    }
}

impl RuleFilter for ConfusionCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(postag) = required(ctx, "postag") else {
            return FilterOutcome::reject();
        };
        let Some(original_form) = required(ctx, "form") else {
            return FilterOutcome::reject();
        };
        let is_all_uppercase = lt_tagger::is_all_uppercase(original_form);
        let is_capitalized = lt_tagger::is_capitalized_word(original_form);
        let form = original_form.to_lowercase();
        let gendernumber_from = ctx.args.get("gendernumberFrom");

        let mut desired_gender_number_pattern: Option<&str> = None;
        if let Some(g) = gendernumber_from {
            let Ok(i) = g.parse::<usize>() else {
                return FilterOutcome::reject();
            };
            if i < 1 || i > ctx.pattern_tokens.len() {
                return FilterOutcome::reject();
            }
            let atr = ctx.pattern_tokens[i - 1];
            const MS: &str = r"NC[MC][SN]000|A..[MC][SN].|V.P..SM";
            const FS: &str = r"NC[FC][SN]000|A..[FC][SN].|V.P..SF";
            const MP: &str = r"NC[MC][PN]000|A..[MC][PN].|V.P..PM";
            const FP: &str = r"NC[FC][PN]000|A..[FC][PN].|V.P..PF";
            const CP: &str = r"NC[MFC][PN]000|A..[MFC][PN].|V.P..P.";
            const CS: &str = r"NC[MFC][SN]000|A..[MFC][SN].|V.P..S.";
            desired_gender_number_pattern = if matches_pos_tag_regex(atr, r"[NAPD].+MS.*|V.P..SM") {
                Some(MS)
            } else if matches_pos_tag_regex(atr, r"[NAPD].+MP.*|V.P..PM") {
                Some(MP)
            } else if matches_pos_tag_regex(atr, r"[NAPD].+FS.*|V.P..SF") {
                Some(FS)
            } else if matches_pos_tag_regex(atr, r"[NAPD].+FP.*|V.P..PF") {
                Some(FP)
            } else if matches_pos_tag_regex(atr, r"[NAPD].+CP.*|V.P..P.") {
                Some(CP)
            } else if matches_pos_tag_regex(atr, r"[NAPD].+CS.*|V.P..S.") {
                Some(CS)
            } else {
                None
            };
        }

        let mut replacement: Option<String> = None;
        if let Some(entry) = self.pairs.map.get(&form) {
            if matches_pos_tag_regex(entry, postag) {
                if let Some(pattern) = desired_gender_number_pattern {
                    let first_tag = entry
                        .readings
                        .first()
                        .and_then(|r| r.pos_tag.clone())
                        .unwrap_or_default();
                    let ok = regex::Regex::new(&format!("^(?:{pattern})$"))
                        .map(|re| re.is_match(&first_tag))
                        .unwrap_or(false);
                    if !ok {
                        return FilterOutcome::reject();
                    }
                    replacement = Some(entry.readings[0].token.clone());
                } else if gendernumber_from.is_none() {
                    // there is no desired gender number defined
                    replacement = Some(entry.readings[0].token.clone());
                }
            }
        }

        let Some(mut replacement) = replacement else {
            return FilterOutcome::reject();
        };
        let mut message = ctx.message.clone();
        // Change the message if the replacement has no diacritic
        let has_diacritics = |s: &str| crate::multitoken::remove_diacritics(s) != s;
        if !has_diacritics(&replacement) || has_diacritics(&form) {
            message = message.replace("se escribe con tilde", "se escribe de otra manera");
        }
        if is_all_uppercase {
            replacement = replacement.to_uppercase();
        }
        if is_capitalized {
            replacement = lt_tagger::uppercase_first_char(&replacement);
        }
        let suggestion = ctx
            .suggestions
            .first()
            .map(|s| {
                s.value
                    .replace("{suggestion}", &replacement)
                    .replace(
                        "{Suggestion}",
                        &lt_tagger::uppercase_first_char(&replacement),
                    )
                    .replace("{SUGGESTION}", &replacement.to_uppercase())
            })
            .unwrap_or_else(|| replacement.clone());
        FilterOutcome {
            accepted: true,
            range: Some(ctx.match_range),
            message: Some(message),
            suggestions: Some(vec![Suggestion {
                value: suggestion,
                short_description: None,
            }]),
        }
    }
}

// ---------------------------------------------------------------------------
// FindSuggestionsFilter (AbstractFindSuggestionsFilter)
// ---------------------------------------------------------------------------

const MAX_SUGGESTIONS: usize = 10;

/// Java `AnalyzedSentence.getText()` concatenates the *token* texts (which are
/// normalized, e.g. `’` → `'`); `RuleMatch.getOriginalErrorStr()` is a
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

/// `org.languagetool.rules.es.FindSuggestionsFilter` (the Spanish
/// `getSpellingSuggestions` uses `MorfologikSpeller.findSimilarWords`; the
/// Java class does not override `getSynthesizer`, so the synthesis branch is
/// inactive — like the English port).
struct FindSuggestionsFilter {
    env: Env,
}

impl FindSuggestionsFilter {
    fn spelling_suggestions(&self, word: &str) -> Vec<String> {
        self.env
            .spelling
            .dict_speller()
            .speller()
            .find_similar_word_candidates(word)
            .into_iter()
            .map(|c| c.word)
            .collect()
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
            matched_text(ctx).replace(' ', "")
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
            let analyzed = self.env.tagger.tag_word(&suggestion);
            for analyzed_suggestion in &analyzed {
                if replacements.len() >= 2 * MAX_SUGGESTIONS {
                    break;
                }
                let Some(tag) = &analyzed_suggestion.pos_tag else {
                    continue;
                };
                if suggestion != atr_word_token
                    && desired_re.is_match(tag).unwrap_or(false)
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
                                .map(|re| re.is_match(tag).unwrap_or(false))
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
        // Java `definitiveReplacements.remove(match.getOriginalErrorStr())`:
        // the field is empty unless `wordFrom` is `inmarker`, where the
        // filter sets it to the matched sentence text. It is *not* the
        // word-from token.
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

// ---------------------------------------------------------------------------
// AbstractSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.es.SpanishSuppressMisspelledSuggestionsFilter`.
struct SpanishSuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl SpanishSuppressMisspelledSuggestionsFilter {
    fn is_misspelled_multiword(&self, word: &str) -> bool {
        let is_tagged = |w: &str| self.env.tagger.is_tagged_word(w);
        for token in lt_tokenize::SpanishWordTokenizer::new(&is_tagged).tokenize(word) {
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

impl RuleFilter for SpanishSuppressMisspelledSuggestionsFilter {
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
// MultitokenSpellerFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter` with
/// the Spanish `MultitokenSpeller`.
struct SpanishMultitokenSpellerFilter {
    env: Env,
}

impl SpanishMultitokenSpellerFilter {
    fn is_misspelled(&self, text: &str) -> bool {
        let is_tagged = |w: &str| self.env.tagger.is_tagged_word(w);
        for token in lt_tokenize::SpanishWordTokenizer::new(&is_tagged).tokenize(text) {
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

impl RuleFilter for SpanishMultitokenSpellerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        if ctx.pattern_tokens.iter().all(|t| t.is_ignore_spelling) {
            return FilterOutcome::reject();
        }
        let underlined_error = &ctx.sentence_text[ctx.match_range.start..ctx.match_range.end];
        let are_tokens_accepted_by_speller = !self.is_misspelled(underlined_error);
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
                && (crate::wordutil::is_punctuation_mark(non_blank[words_start_pos].surface())
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

// ---------------------------------------------------------------------------
// AddCommasFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.AddCommasFilter`.
struct AddCommasFilter;

impl RuleFilter for AddCommasFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        static OPENING_QUOTES: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r#"[«“"‘'„¿¡]"#).unwrap());
        let suggest_semicolon = ctx
            .args
            .get("suggestSemicolon")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let tokens: Vec<&lt_core::AnalyzedTokenReadings> = ctx
            .sentence_tokens
            .iter()
            .copied()
            .filter(|t| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .collect();
        let mut postag_from = 1usize;
        while postag_from < tokens.len() && tokens[postag_from].start_pos < ctx.match_range.start {
            postag_from += 1;
        }
        let mut postag_to = postag_from;
        while postag_to < tokens.len() && tokens[postag_to].end_pos() < ctx.match_range.end {
            postag_to += 1;
        }
        if postag_from >= tokens.len() || postag_to >= tokens.len() {
            return FilterOutcome::accept();
        }
        let before_ok = postag_from == 1
            || is_punctuation_or_symbol(tokens[postag_from - 1].surface())
            || lt_tagger::is_capitalized_word(tokens[postag_from].surface());
        let after_ok = postag_to < tokens.len() - 1
            && is_punctuation_or_symbol(tokens[postag_to + 1].surface())
            && !(tokens[postag_to + 1].whitespace_before
                && OPENING_QUOTES.is_match(tokens[postag_to + 1].surface()));
        if before_ok && after_ok {
            return FilterOutcome::reject();
        }
        let matched_text = &ctx.sentence_text[ctx.match_range.start..ctx.match_range.end];
        if suggest_semicolon && tokens[postag_from - 1].surface() == "," && !after_ok {
            return FilterOutcome {
                accepted: true,
                range: Some(TextRange::new(
                    tokens[postag_from - 1].start_pos,
                    tokens[postag_to].end_pos(),
                )),
                message: None,
                suggestions: Some(vec![
                    Suggestion {
                        value: format!("; {matched_text},"),
                        short_description: None,
                    },
                    Suggestion {
                        value: format!(", {matched_text},"),
                        short_description: None,
                    },
                ]),
            };
        }
        if before_ok && !after_ok {
            return FilterOutcome {
                accepted: true,
                range: Some(TextRange::new(
                    tokens[postag_to].start_pos,
                    ctx.match_range.end,
                )),
                message: None,
                suggestions: Some(vec![Suggestion {
                    value: format!("{},", tokens[postag_to].surface()),
                    short_description: None,
                }]),
            };
        }
        if !before_ok && after_ok {
            let mut start_pos = tokens[postag_from].start_pos;
            if tokens[postag_from].whitespace_before {
                start_pos -= 1;
            }
            return FilterOutcome {
                accepted: true,
                range: Some(TextRange::new(start_pos, tokens[postag_from].end_pos())),
                message: None,
                suggestions: Some(vec![Suggestion {
                    value: format!(", {}", tokens[postag_from].surface()),
                    short_description: None,
                }]),
            };
        }
        let mut start_pos = tokens[postag_from].start_pos;
        if tokens[postag_from].whitespace_before {
            start_pos -= 1;
        }
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(start_pos, tokens[postag_to].end_pos())),
            message: None,
            suggestions: Some(vec![Suggestion {
                value: format!(", {matched_text},"),
                short_description: None,
            }]),
        }
    }
}

// ---------------------------------------------------------------------------
// AdaptSuggestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.AdaptSuggestionsFilter` with
/// `Spanish.adaptSuggestion` (`\b([Aa]|[Dd]e) e(l)\b` → `$1$2`).
struct AdaptSuggestionsFilter;

impl RuleFilter for AdaptSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        static ES_CONTRACTIONS: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r"\b([Aa]|[Dd]e) e(l)\b").unwrap());
        let suggestions = ctx
            .suggestions
            .iter()
            .map(|s| Suggestion {
                value: ES_CONTRACTIONS.replace_all(&s.value, "$1$2").into_owned(),
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

// ---------------------------------------------------------------------------
// AdvancedSynthesizerFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.es.AdvancedSynthesizerFilter`
/// (`AbstractAdvancedSynthesizerFilter`).
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

// ---------------------------------------------------------------------------
// SpanishNumberInWordFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.es.SpanishNumberInWordFilter`
/// (`AbstractNumberInWordFilter` with the Spanish speller).
struct SpanishNumberInWordFilter {
    env: Env,
}

impl RuleFilter for SpanishNumberInWordFilter {
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
            FilterOutcome::reject()
        } else {
            FilterOutcome {
                accepted: true,
                range: None,
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
}

pub fn spanish_filter_registry(env: Env) -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.IsEnglishWordFilter",
        Arc::new(IsEnglishWordFilter),
    );
    builder = builder.register(
        "org.languagetool.rules.es.ConfusionCheckFilter",
        Arc::new(ConfusionCheckFilter::load(
            &env.data_dir.join("es/rules/confusion_pairs.txt"),
        )),
    );
    builder = builder.register(
        "org.languagetool.rules.es.FindSuggestionsFilter",
        Arc::new(FindSuggestionsFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.es.SpanishSuppressMisspelledSuggestionsFilter",
        Arc::new(SpanishSuppressMisspelledSuggestionsFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter",
        Arc::new(SpanishMultitokenSpellerFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.AddCommasFilter",
        Arc::new(AddCommasFilter),
    );
    builder = builder.register(
        "org.languagetool.rules.AdaptSuggestionsFilter",
        Arc::new(AdaptSuggestionsFilter),
    );
    builder = builder.register(
        "org.languagetool.rules.es.AdvancedSynthesizerFilter",
        Arc::new(AdvancedSynthesizerFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.patterns.ApostropheTypeFilter",
        Arc::new(crate::en::filters::ApostropheTypeFilter),
    );
    builder = builder.register(
        "org.languagetool.rules.es.SpanishNumberInWordFilter",
        Arc::new(SpanishNumberInWordFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.es.PostponedAdjectiveConcordanceFilter",
        Arc::new(PostponedAdjectiveConcordanceFilter {
            env: Arc::clone(&env),
        }),
    );
    let today = env.today;
    builder = builder.register(
        "org.languagetool.rules.es.DateCheckFilter",
        Arc::new(crate::es::date_filters::DateCheckFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.es.NewYearDateFilter",
        Arc::new(crate::es::date_filters::NewYearDateFilter { today }),
    );
    builder.build()
}

// ---------------------------------------------------------------------------
// PostponedAdjectiveConcordanceFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.es.PostponedAdjectiveConcordanceFilter`.
struct PostponedAdjectiveConcordanceFilter {
    env: Env,
}

const MAX_LEVELS: usize = 4;

/// `matchPostagRegexp`: null POS tags are tested as `UNKNOWN`.
fn postag_matches(tr: &lt_core::AnalyzedTokenReadings, pattern: &regex::Regex) -> bool {
    tr.readings.iter().any(|r| {
        let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(pos_tag)
    })
}

fn get_analyzed_token<'a>(
    tr: &'a lt_core::AnalyzedTokenReadings,
    pattern: &regex::Regex,
) -> Option<&'a lt_core::AnalyzedToken> {
    tr.readings.iter().find(|r| {
        let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(pos_tag)
    })
}

fn re(pattern: &str) -> regex::Regex {
    regex::Regex::new(&format!("^(?:{pattern})$")).unwrap()
}

struct PaPatterns {
    nom: regex::Regex,
    nom_ms: regex::Regex,
    nom_fs: regex::Regex,
    nom_mp: regex::Regex,
    nom_mn: regex::Regex,
    nom_fp: regex::Regex,
    nom_cs: regex::Regex,
    nom_cp: regex::Regex,
    nom_det: regex::Regex,
    gn_any: regex::Regex,
    gn_ms: regex::Regex,
    gn_fs: regex::Regex,
    gn_mp: regex::Regex,
    gn_fp: regex::Regex,
    det_cs: regex::Regex,
    det_ms: regex::Regex,
    det_fs: regex::Regex,
    det_mp: regex::Regex,
    det_fp: regex::Regex,
    subst: [(regex::Regex, regex::Regex, regex::Regex); 6],
    adjectiu: regex::Regex,
    adjectiu_ms: regex::Regex,
    adjectiu_fs: regex::Regex,
    adjectiu_mp: regex::Regex,
    adjectiu_fp: regex::Regex,
    adjectiu_cp: regex::Regex,
    adjectiu_cs: regex::Regex,
    adjectiu_s: regex::Regex,
    adjectiu_p: regex::Regex,
    adverbi: regex::Regex,
    conjuncio: regex::Regex,
    punctuacio: regex::Regex,
    loc_adv: regex::Regex,
    adverbis_acceptats: regex::Regex,
    coordinacio_ioni: regex::Regex,
    keep_count: regex::Regex,
    keep_count2: regex::Regex,
    stop_count: regex::Regex,
    preposicions: regex::Regex,
    preposicio_canvi_nivell: regex::Regex,
    verb: regex::Regex,
    gv: regex::Regex,
}

impl PaPatterns {
    fn new() -> Self {
        Self {
            nom: re("N.*"),
            nom_ms: re("N.MS.*|PI0MS000"),
            nom_fs: re("N.FS.*|PI0FS000"),
            nom_mp: re("N.MP.*"),
            nom_mn: re("N.MN.*"),
            nom_fp: re("N.FP.*"),
            nom_cs: re("N.CS.*"),
            nom_cp: re("N.CP.*"),
            nom_det: re("N.*|D[NDA0I].*|PI0[MF]S000"),
            gn_any: re("_GN_.*"),
            gn_ms: re("_GN_MS"),
            gn_fs: re("_GN_FS"),
            gn_mp: re("_GN_MP"),
            gn_fp: re("_GN_FP"),
            det_cs: re("D[NDA0IP]0CS0"),
            det_ms: re("D[NDA0IP]0MS0"),
            det_fs: re("D[NDA0IP]0FS0"),
            det_mp: re("D[NDA0IP]0MP0"),
            det_fp: re("D[NDA0IP]0FP0"),
            subst: [
                (
                    re("N.[FMC][SN].*|A..[FMC][SN].*|D[NDA0I]0[FM]S0||PI0[MFC]S000"),
                    re("_GN_[MF]S"),
                    re("A...[SN].*|V.P..S..?|PX..S.*"),
                ),
                (
                    re("N.[FMC][PN].*|A..[FMC][PN].*|D[NDA0I]0[FM]P0"),
                    re("_GN_[MF]P"),
                    re("A...[PN].*|V.P..P..?|PX..P.*"),
                ),
                (
                    re("N.[MC][SN].*|A..[MC][SN].*|V.P..SM.?|PX.MS.*|D[NDA0I]0MS0|PI0MS000"),
                    re("_GN_MS"),
                    re("A..[MC][SN].*|V.P..SM.?|PX.MS.*"),
                ),
                (
                    re("N.[FC][SN].*|A..[FC][SN].*|V.P..SF.?|PX.FS.*|D[NDA0I]0FS0|PI0FS000"),
                    re("_GN_FS"),
                    re("A..[FC][SN].*|V.P..SF.?|PX.FS.*"),
                ),
                (
                    re("N.[MC][PN].*|A..[MC][PN].*|V.P..PM.?|PX.MP.*|D[NDA0I]0MP0"),
                    re("_GN_MP"),
                    re("A..[MC][PN].*|V.P..PM.?|PX.MP.*"),
                ),
                (
                    re("N.[FC][PN].*|A..[FC][PN].*|V.P..PF.?|PX.FP.*|D[NDA0I]0FP0"),
                    re("_GN_FP"),
                    re("A..[FC][PN].*|V.P..PF.?|PX.FP.*"),
                ),
            ],
            adjectiu: re("AQ.*|V.P.*|PX.*|.*LOC_ADJ.*"),
            adjectiu_ms: re("A..[MC][SN].*|V.P..SM.?|PX.MS.*"),
            adjectiu_fs: re("A..[FC][SN].*|V.P..SF.?|PX.FS.*"),
            adjectiu_mp: re("A..[MC][PN].*|V.P..PM.?|PX.MP.*"),
            adjectiu_fp: re("A..[FC][PN].*|V.P..PF.?|PX.FP.*"),
            adjectiu_cp: re("A..C[PN].*"),
            adjectiu_cs: re("A..C[SN].*"),
            adjectiu_s: re("A...[SN].*|V.P..S..?|PX..S.*"),
            adjectiu_p: re("A...[PN].*|V.P..P..?|PX..P.*"),
            adverbi: re("R.|.*LOC_ADV.*"),
            conjuncio: re("C.|.*LOC_CONJ.*"),
            punctuacio: re("_PUNCT"),
            loc_adv: re(".*LOC_ADV.*"),
            adverbis_acceptats: re("RG_before"),
            coordinacio_ioni: re("y|e|o|u|ni"),
            keep_count: re(
                "A.*|N.*|D[NAIDP].*|SPS.*|SP:DA|.*LOC_ADV.*|V.P.*|_PUNCT.*|.*LOC_ADJ.*|PX.*|PI0.S000|UNKNOWN|V.N.{4}",
            ),
            keep_count2: re(",|y|e|o|ni|u"),
            stop_count: re(";|lo"),
            preposicions: re("SP.*"),
            preposicio_canvi_nivell: re("de|del|en|sobre|a|entre|por|con|sin|contra|para"),
            verb: re("V.[^P].*|_GV_"),
            gv: re("_GV_"),
        }
    }
}

struct PaApparitions {
    adverb: bool,
    conjunction: bool,
    punctuation: bool,
}

impl PostponedAdjectiveConcordanceFilter {
    fn keep_counting(
        p: &PaPatterns,
        a: &PaApparitions,
        tr: &lt_core::AnalyzedTokenReadings,
    ) -> bool {
        if (a.adverb && (a.conjunction || a.punctuation))
            || (a.conjunction && a.punctuation)
            || (a.punctuation && postag_matches(tr, &p.punctuacio))
        {
            return false;
        }
        (postag_matches(tr, &p.keep_count)
            || p.keep_count2.is_match(tr.surface())
            || postag_matches(tr, &p.adverbis_acceptats))
            && !p.stop_count.is_match(tr.surface())
            && (!postag_matches(tr, &p.gv) || postag_matches(tr, &p.gn_any))
    }

    fn update_apparitions(
        p: &PaPatterns,
        a: &mut PaApparitions,
        tr: &lt_core::AnalyzedTokenReadings,
    ) {
        if postag_matches(tr, &p.nom) || postag_matches(tr, &p.adjectiu) {
            a.adverb = false;
            a.conjunction = false;
            a.punctuation = false;
            return;
        }
        a.adverb |= postag_matches(tr, &p.adverbi);
        a.conjunction |= postag_matches(tr, &p.conjuncio);
        a.punctuation |= postag_matches(tr, &p.punctuacio) || tr.surface() == ",";
    }
}

impl RuleFilter for PostponedAdjectiveConcordanceFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let p = PaPatterns::new();
        let add_comma = ctx
            .args
            .get("addComma")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let tokens: Vec<&lt_core::AnalyzedTokenReadings> = ctx
            .sentence_tokens
            .iter()
            .copied()
            .filter(|t| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .collect();
        let i = ctx.pattern_token_pos;
        if i >= tokens.len() {
            return FilterOutcome::reject();
        }
        let mut is_plural = true;
        let mut is_prev_noun = false;
        let mut can_be_ms = false;
        let mut can_be_fs = false;
        let mut can_be_mp = false;
        let mut can_be_fp = false;
        let mut can_be_p = false;
        let mut c_nt = [0i32; MAX_LEVELS];
        let mut c_nms = [0i32; MAX_LEVELS];
        let mut c_nfs = [0i32; MAX_LEVELS];
        let mut c_nmp = [0i32; MAX_LEVELS];
        let mut c_nmn = [0i32; MAX_LEVELS];
        let mut c_nfp = [0i32; MAX_LEVELS];
        let mut c_ncs = [0i32; MAX_LEVELS];
        let mut c_ncp = [0i32; MAX_LEVELS];
        let mut c_dms = [0i32; MAX_LEVELS];
        let mut c_dfs = [0i32; MAX_LEVELS];
        let mut c_dmp = [0i32; MAX_LEVELS];
        let mut c_dfp = [0i32; MAX_LEVELS];
        let mut level = 0usize;
        let mut j = 1usize;
        let mut app = PaApparitions {
            adverb: false,
            conjunction: false,
            punctuation: false,
        };
        while i > j && Self::keep_counting(&p, &app, tokens[i - j]) && level < MAX_LEVELS {
            if !is_prev_noun {
                if postag_matches(tokens[i - j], &p.nom)
                    || (i > j + 1
                        && !postag_matches(tokens[i - j], &p.nom)
                        && postag_matches(tokens[i - j], &p.adjectiu)
                        && postag_matches(tokens[i - j - 1], &re("D[NDA0IP].*")))
                {
                    if postag_matches(tokens[i - j], &p.gn_ms) {
                        c_nms[level] += 1;
                        can_be_ms = true;
                    }
                    if postag_matches(tokens[i - j], &p.gn_fs) {
                        c_nfs[level] += 1;
                        can_be_fs = true;
                    }
                    if postag_matches(tokens[i - j], &p.gn_mp) {
                        c_nmp[level] += 1;
                        can_be_mp = true;
                    }
                    if postag_matches(tokens[i - j], &p.gn_fp) {
                        c_nfp[level] += 1;
                        can_be_fp = true;
                    }
                }
                if !postag_matches(tokens[i - j], &p.gn_any) {
                    if postag_matches(tokens[i - j], &p.nom_ms) {
                        c_nms[level] += 1;
                        can_be_ms = true;
                    } else if postag_matches(tokens[i - j], &p.nom_fs) {
                        c_nfs[level] += 1;
                        can_be_fs = true;
                    } else if postag_matches(tokens[i - j], &p.nom_mp) {
                        c_nmp[level] += 1;
                        can_be_mp = true;
                    } else if postag_matches(tokens[i - j], &p.nom_mn) {
                        c_nmn[level] += 1;
                        can_be_ms = true;
                        can_be_mp = true;
                    } else if postag_matches(tokens[i - j], &p.nom_fp) {
                        c_nfp[level] += 1;
                        can_be_fp = true;
                    } else if postag_matches(tokens[i - j], &p.nom_cs) {
                        c_ncs[level] += 1;
                        can_be_ms = true;
                        can_be_fs = true;
                    } else if postag_matches(tokens[i - j], &p.nom_cp) {
                        c_ncp[level] += 1;
                        can_be_fp = true;
                        can_be_mp = true;
                    }
                }
            }
            // avoid two consecutive nouns
            if postag_matches(tokens[i - j], &p.nom) {
                c_nt[level] += 1;
                is_prev_noun = true;
            } else {
                is_prev_noun = false;
            }

            if postag_matches(tokens[i - j], &p.det_cs) {
                if postag_matches(tokens[i - j + 1], &p.nom_ms) {
                    c_dms[level] += 1;
                    can_be_ms = true;
                }
                if postag_matches(tokens[i - j + 1], &p.nom_fs) {
                    c_dfs[level] += 1;
                    can_be_fs = true;
                }
            }
            if !postag_matches(tokens[i - j], &p.adverbi) {
                if postag_matches(tokens[i - j], &p.det_ms) {
                    c_dms[level] += 1;
                    can_be_ms = true;
                }
                if postag_matches(tokens[i - j], &p.det_fs) {
                    c_dfs[level] += 1;
                    can_be_fs = true;
                }
                if postag_matches(tokens[i - j], &p.det_mp) {
                    c_dmp[level] += 1;
                    can_be_mp = true;
                }
                if postag_matches(tokens[i - j], &p.det_fp) {
                    c_dfp[level] += 1;
                    can_be_fp = true;
                }
            }
            if i > j
                && p.preposicio_canvi_nivell.is_match(tokens[i - j].surface())
                && !p.coordinacio_ioni.is_match(tokens[i - j - 1].surface())
                && !postag_matches(tokens[i - j + 1], &p.adverbi)
            {
                level += 1;
            }
            if level > 0 && p.coordinacio_ioni.is_match(tokens[i - j].surface()) {
                let mut k = 1usize;
                while k < 4
                    && i > j + k
                    && (postag_matches(tokens[i - j - k], &p.keep_count)
                        || p.keep_count2.is_match(tokens[i - j - k].surface())
                        || postag_matches(tokens[i - j - k], &p.adverbis_acceptats))
                    && !p.stop_count.is_match(tokens[i - j - k].surface())
                {
                    if postag_matches(tokens[i - j - k], &p.preposicions) {
                        j += k;
                        break;
                    }
                    k += 1;
                }
            }
            Self::update_apparitions(&p, &mut app, tokens[i - j]);
            j += 1;
        }
        level += 1;
        if level > MAX_LEVELS {
            level = MAX_LEVELS;
        }
        let mut c_n = [0i32; MAX_LEVELS];
        let mut c_d = [0i32; MAX_LEVELS];
        let mut c_ntotal = 0i32;
        let mut c_dtotal = 0i32;
        let mut jj = 0usize;
        while jj < level {
            c_n[jj] =
                c_nms[jj] + c_nfs[jj] + c_nmp[jj] + c_nfp[jj] + c_ncs[jj] + c_ncp[jj] + c_nmn[jj];
            c_d[jj] = c_dms[jj] + c_dfs[jj] + c_dmp[jj] + c_dfp[jj];
            c_ntotal += c_n[jj];
            c_dtotal += c_d[jj];
            // exceptions: adjective is plural and there are several nouns before
            if postag_matches(tokens[i], &p.adjectiu_mp)
                && (c_n[jj] > 1 || c_d[jj] > 1)
                && (c_nms[jj]
                    + c_nmn[jj]
                    + c_nmp[jj]
                    + c_ncs[jj]
                    + c_ncp[jj]
                    + c_dms[jj]
                    + c_dmp[jj])
                    > 0
                && (c_nfs[jj] + c_nfp[jj] <= c_nt[jj])
            {
                return FilterOutcome::reject();
            }
            if postag_matches(tokens[i], &p.adjectiu_fp)
                && (c_n[jj] > 1 || c_d[jj] > 1)
                && ((c_nms[jj] + c_nmp[jj] + c_nmn[jj] + c_dms[jj] + c_dmp[jj]) == 0
                    || (c_nt[jj] > 0 && c_nfs[jj] + c_nfp[jj] >= c_nt[jj]))
            {
                return FilterOutcome::reject();
            }
            // Adjective can't be singular
            if c_n[jj] + c_d[jj] > 0 {
                is_plural = is_plural && c_d[jj] > 1;
                can_be_p = can_be_p || c_n[jj] > 1;
            }
            jj += 1;
        }
        // comma + plural noun
        is_plural = is_plural
            || (i >= 2 && c_nmp[0] + c_nfp[0] + c_ncp[0] > 0 && tokens[i - 2].surface() == ",");

        // there is no noun, (no determinant --> && cDtotal==0)
        if c_ntotal == 0 && c_dtotal == 0 {
            return FilterOutcome::reject();
        }

        // patterns according to the analyzed adjective
        let mut subst_pattern: Option<&(regex::Regex, regex::Regex, regex::Regex)> = None;
        if postag_matches(tokens[i], &p.adjectiu_cs) {
            subst_pattern = Some(&p.subst[0]);
        } else if postag_matches(tokens[i], &p.adjectiu_cp) {
            subst_pattern = Some(&p.subst[1]);
        } else if postag_matches(tokens[i], &p.adjectiu_ms) {
            subst_pattern = Some(&p.subst[2]);
        } else if postag_matches(tokens[i], &p.adjectiu_fs) {
            subst_pattern = Some(&p.subst[3]);
        } else if postag_matches(tokens[i], &p.adjectiu_mp) {
            subst_pattern = Some(&p.subst[4]);
        } else if postag_matches(tokens[i], &p.adjectiu_fp) {
            subst_pattern = Some(&p.subst[5]);
        }
        let Some((subst, gn_pattern, adj_pattern)) = subst_pattern else {
            return FilterOutcome::reject();
        };
        let subst_pattern = subst.clone();
        let gn_pattern = gn_pattern.clone();
        let adj_pattern = adj_pattern.clone();

        // combinations Det/Nom + adv (1,2..) + adj.
        // If there is agreement, the rule doesn't match
        let mut j = 1usize;
        let mut keep_count = true;
        while i > j && keep_count {
            if (postag_matches(tokens[i - j], &p.nom_det)
                && postag_matches(tokens[i - j], &gn_pattern))
                || (!postag_matches(tokens[i - j], &p.gn_any)
                    && postag_matches(tokens[i - j], &subst_pattern))
            {
                return FilterOutcome::reject();
            }
            keep_count = !postag_matches(tokens[i - j], &p.nom_det);
            j += 1;
        }

        // Necessary condition: previous token is a non-agreeing noun
        // or it is adjective or adverb (not preceded by verb)
        let necessary = (postag_matches(tokens[i - 1], &p.nom)
            && !postag_matches(tokens[i - 1], &subst_pattern))
            || (postag_matches(tokens[i - 1], &p.adjectiu)
                && !postag_matches(tokens[i - 1], &gn_pattern))
            || (postag_matches(tokens[i - 1], &p.adjectiu)
                && !postag_matches(tokens[i - 1], &adj_pattern))
            || (i > 2
                && postag_matches(tokens[i - 1], &p.adverbis_acceptats)
                && !postag_matches(tokens[i - 2], &p.verb)
                && !postag_matches(tokens[i - 2], &p.preposicions))
            || (i > 3
                && postag_matches(tokens[i - 1], &p.loc_adv)
                && postag_matches(tokens[i - 2], &p.loc_adv)
                && !postag_matches(tokens[i - 3], &p.verb)
                && !postag_matches(tokens[i - 3], &p.preposicions));
        if !necessary {
            return FilterOutcome::reject();
        }

        // Adjective can't be singular. The rule matches
        if !(is_plural && postag_matches(tokens[i], &p.adjectiu_s)) {
            // look into previous words
            let mut j = 1usize;
            let mut app = PaApparitions {
                adverb: false,
                conjunction: false,
                punctuation: false,
            };
            while i > j && Self::keep_counting(&p, &app, tokens[i - j]) && (level > 1 || j < 4) {
                if (!postag_matches(tokens[i - j], &p.gn_any)
                    && postag_matches(tokens[i - j], &p.nom_det)
                    && postag_matches(tokens[i - j], &subst_pattern))
                    || postag_matches(tokens[i - j], &gn_pattern)
                {
                    return FilterOutcome::reject();
                }
                Self::update_apparitions(&p, &mut app, tokens[i - j]);
                j += 1;
            }
        }

        // The rule matches. Synthesize suggestions.
        let synth = self.env.synth.inner();
        let mut suggestions: Vec<String> = Vec::new();
        if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_cs) {
            suggestions.extend(synth.synthesize(at, "A..CP.", true));
        }
        if suggestions.is_empty() {
            if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_cp) {
                suggestions.extend(synth.synthesize(at, "A..CS.", true));
            }
        }
        if suggestions.is_empty() && is_plural {
            if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_p) {
                suggestions.extend(synth.synthesize(at, "A...P.|V.P..P.|PX..P.*", true));
            }
        }
        if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu) {
            if suggestions.is_empty() {
                if can_be_ms && !is_plural {
                    suggestions.extend(synth.synthesize(at, "A..MS.|V.P..SM|PX.MS.*", true));
                }
                if can_be_fs && !is_plural {
                    suggestions.extend(synth.synthesize(at, "A..FS.|V.P..SF|PX.FS.*", true));
                }
                if can_be_mp {
                    suggestions.extend(synth.synthesize(at, "A..MP.|V.P..PM|PX.MP.*", true));
                }
                if can_be_fp {
                    suggestions.extend(synth.synthesize(at, "A..FP.|V.P..PF|PX.FP.*", true));
                }
                if can_be_ms && (is_plural || can_be_p) {
                    suggestions.extend(synth.synthesize(at, "A..MP.|V.P..PM|PX.MP.*", true));
                }
                if can_be_fs && !can_be_ms && (is_plural || can_be_p) {
                    suggestions.extend(synth.synthesize(at, "A..FP.|V.P..PF|PX.FP.*", true));
                }
            }
        }
        // avoid the original token as suggestion
        let lower = tokens[i].surface().to_lowercase();
        suggestions.retain(|s| *s != lower);
        if suggestions.is_empty() {
            return FilterOutcome::reject();
        }
        let mut definitive: Vec<String> = Vec::new();
        let mut range = None;
        if add_comma {
            definitive.push(format!(", {}", tokens[i].surface()));
            for s in &suggestions {
                definitive.push(format!(" {s}"));
            }
            let start = tokens[i].start_pos.saturating_sub(1);
            range = Some(TextRange::new(start, tokens[i].end_pos()));
        } else {
            definitive.extend(suggestions);
        }
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
        FilterOutcome {
            accepted: true,
            range,
            message: None,
            suggestions: Some(out),
        }
    }
}
