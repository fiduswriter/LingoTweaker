//! French filter registry (`<filter class="...">` implementations used by
//! `fr/rules/grammar.xml`, `style.xml` and the disambiguation rules).
//!
//! Every class referenced by the pinned French rule data is registered here;
//! a class that fails to compile because a filter is missing shows up in
//! `engine.compile_failures()` (never silently dropped).

use std::sync::Arc;

use lt_core::{Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::Ymd;

/// Shared state for French filters.
pub struct FrFilterEnv {
    pub tagger: Arc<lt_tagger::FrenchTagger>,
    pub synth: Arc<crate::fr::synthesizer::FrenchSynthesizerAdapter>,
    /// `MorfologikFrenchSpellerRule` (the default spelling rule)
    pub spelling: Arc<crate::fr::spelling::FrenchSpellingRule>,
    /// `FrenchMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// Used by the date filters.
    pub today: Ymd,
    /// Installed by `Pipeline::new_french` once the XML rules are compiled:
    /// `WordWithDeterminerFilter.suggestionHasNoErrors` re-checks a candidate
    /// against the `CAT_ELISION` rules with the full pipeline.
    pub suggestion_checker: Arc<std::sync::OnceLock<SuggestionChecker>>,
}

/// `WordWithDeterminerFilter.suggestionHasNoErrors` re-check: `true` = clean.
pub type SuggestionChecker = Arc<dyn Fn(&str) -> bool + Send + Sync>;

pub type Env = Arc<FrFilterEnv>;

#[allow(dead_code)] // used by the WordWithDeterminer/InterrogativeVerb filters
fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a str> {
    ctx.args.get(key).map(|s| s.as_str())
}

/// `AnalyzedTokenReadings.matchesPosTagRegex`: at least one reading's POS tag
/// fully matches the pattern.
#[allow(dead_code)] // used by the WordWithDeterminer filter
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

/// `org.languagetool.rules.IsEnglishWordFilter`: with the pinned French
/// classpath there is no `en-US` language (`Languages.getLanguageForShortCode`
/// returns null in the language-fr module and in the Docker oracle), so Java's
/// filter constructor leaves `tagger == null` and every match is rejected.
struct IsEnglishWordFilter;

impl RuleFilter for IsEnglishWordFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }
}

/// `org.languagetool.rules.SuppressIfAnyRuleMatchesFilter`: the only pinned
/// French usage passes `ruleIDs` of AI_FR_GGEC rules that are not vendored,
/// so no active rule can ever match and the filter accepts.
struct SuppressIfAnyRuleMatchesFilter;

impl RuleFilter for SuppressIfAnyRuleMatchesFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::accept()
    }
}

// ---------------------------------------------------------------------------
// FindSuggestionsFilter (AbstractFindSuggestionsFilter)
// ---------------------------------------------------------------------------

const MAX_SUGGESTIONS: usize = 10;

/// `StringTools.makeWrong` (the generic version; `InterrogativeVerbFilter`
/// has its own mapping).
fn make_wrong(s: &str) -> String {
    for (from, to) in [
        ("a", "ä"),
        ("e", "ë"),
        ("i", "ï"),
        ("o", "ö"),
        ("u", "ù"),
        ("á", "ä"),
        ("é", "ë"),
        ("í", "ï"),
        ("ó", "ö"),
        ("ú", "ù"),
        ("à", "ä"),
        ("è", "ë"),
        ("ì", "i"),
        ("ò", "ö"),
        ("ï", "ì"),
        ("ü", "ù"),
    ] {
        if s.contains(from) {
            return s.replace(from, to);
        }
    }
    format!("{s}-")
}

/// `AbstractFindSuggestionsFilter.StringComparator` (Damerau distance ≤ 4,
/// `-1` counted as `2 * maxDistance`).
struct StringComparator {
    base: String,
    max_distance: i32,
}

impl StringComparator {
    fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let d = |s: &str| {
            let d = damerau_levenshtein(&self.base, s, self.max_distance);
            if d < 0 {
                2 * self.max_distance
            } else {
                d
            }
        };
        d(a).cmp(&d(b))
    }
}

/// `EditDistance.DamerauLevenshteinDistance` (symspell): a banded
/// Damerau-Levenshtein distance with `maxDistance`; `-1` when the distance
/// exceeds it. Java trims the common suffix/prefix first, so e.g.
/// `inform`/`informeraient` reports the exact distance 6 (not `-1`) and the
/// suggestion `StringComparator` can order the synthesized forms.
fn damerau_levenshtein(base: &str, other: &str, max_distance: i32) -> i32 {
    let base_chars: Vec<char> = base.chars().collect();
    let other_chars: Vec<char> = other.chars().collect();
    if base_chars.is_empty() {
        return other_chars.len() as i32;
    }
    if other_chars.is_empty() {
        return base_chars.len() as i32;
    }
    if max_distance == 0 {
        return if base == other { 0 } else { -1 };
    }
    // `if strings of different lengths, ensure shorter string is in string1`
    let (string1, mut string2) = if base_chars.len() > other_chars.len() {
        (other_chars.clone(), base_chars.clone())
    } else {
        (base_chars.clone(), other_chars.clone())
    };
    let mut s_len = string1.len();
    let mut t_len = string2.len();
    // suffix common to both strings can be ignored
    while s_len > 0 && string1[s_len - 1] == string2[t_len - 1] {
        s_len -= 1;
        t_len -= 1;
    }
    let mut start = 0usize;
    if string1[0] == string2[0] || s_len == 0 {
        // prefix common to both strings can be ignored
        while start < s_len && string1[start] == string2[start] {
            start += 1;
        }
        s_len -= start;
        t_len -= start;
        // all of the shorter string matches prefix/suffix of the longer one
        if s_len == 0 {
            return t_len as i32;
        }
        string2 = string2[start..start + t_len].to_vec();
    }
    let len_diff = t_len as i32 - s_len as i32;
    let mut max_distance = max_distance;
    if max_distance < 0 || max_distance > t_len as i32 {
        max_distance = t_len as i32;
    } else if len_diff > max_distance {
        return -1;
    }
    let t_len_i = t_len as i32;
    let mut v0 = vec![max_distance + 1; t_len];
    let mut v2 = vec![0i32; t_len];
    for (j, v) in v0.iter_mut().enumerate().take(max_distance as usize) {
        *v = j as i32 + 1;
    }
    let have_max = max_distance < t_len_i;
    let j_start_offset = max_distance - (t_len_i - s_len as i32);
    let mut j_start = 0i32;
    let mut j_end = max_distance;
    let mut s_char = string1[0];
    let mut current = 0i32;
    for i in 0..s_len {
        let prev_s_char = s_char;
        s_char = string1[start + i];
        let mut t_char = string2[0];
        let mut left = i as i32;
        current = left + 1;
        let mut next_trans_cost = 0i32;
        if i as i32 > j_start_offset {
            j_start += 1;
        }
        if j_end < t_len_i {
            j_end += 1;
        }
        for j in j_start..j_end {
            let j = j as usize;
            let above = current;
            let this_trans_cost = next_trans_cost;
            next_trans_cost = v2[j];
            v2[j] = left; // `v2[j] = current = left`
            current = left;
            left = v0[j];
            let prev_t_char = t_char;
            t_char = string2[j];
            if s_char != t_char {
                if left < current {
                    current = left; // insertion
                }
                if above < current {
                    current = above; // deletion
                }
                current += 1;
                if i != 0 && j != 0 && s_char == prev_t_char && prev_s_char == t_char {
                    // transposition
                    if this_trans_cost + 1 < current {
                        current = this_trans_cost + 1;
                    }
                }
            }
            v0[j] = current;
        }
        if have_max && v0[(i as i32 + len_diff) as usize] > max_distance {
            return -1;
        }
    }
    if current <= max_distance {
        current
    } else {
        -1
    }
}

/// `org.languagetool.rules.fr.FindSuggestionsFilter` (the French
/// `getSpellingSuggestions` runs the spelling rule on the made-wrong word,
/// singular and plural; the French class overrides `getSynthesizer`, so the
/// synthesis branch is active).
struct FindSuggestionsFilter {
    env: Env,
}

impl FindSuggestionsFilter {
    /// `FindSuggestionsFilter.getSpellingSuggestions`.
    fn spelling_suggestions(&self, atr_word: &lt_core::AnalyzedTokenReadings) -> Vec<String> {
        // Java: `ENDS_IN_VOWEL.matcher(w).matches()` — a **full** match, so
        // only a single vowel character passes (the `$` is redundant there).
        static ENDS_IN_VOWEL: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r"^[aeioué]$").unwrap());
        let w = if atr_word.is_tagged {
            make_wrong(atr_word.surface())
        } else {
            atr_word.surface().to_string()
        };
        let mut suggestions: Vec<String> = Vec::new();
        let mut words_to_check = vec![w.clone()];
        if let Some(stripped) = w.strip_suffix('s') {
            words_to_check.push(stripped.to_string());
        }
        if ENDS_IN_VOWEL.is_match(&w) {
            words_to_check.push(format!("{w}s"));
        }
        for word in words_to_check {
            suggestions.extend(self.env.spelling.suggestions(&word));
        }
        suggestions
    }

    /// `FindSuggestionsFilter.cleanSuggestion`.
    fn clean_suggestion(&self, s: &str) -> String {
        static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
            regex::Regex::new(r"^[smntl]'|^(nous|vous|le|la|les|me|te|se|leur|en|y) ").unwrap()
        });
        let output = PATTERN.replace(s, "");
        output.split(' ').next().unwrap_or("").to_string()
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

        let (atr_word, atr_is_tagged): (lt_core::AnalyzedTokenReadings, bool) =
            if word_from == "inmarker" {
                let text = matched_text(ctx).replace(' ', "");
                (
                    lt_core::AnalyzedTokenReadings::new(vec![lt_core::AnalyzedToken::new(
                        &text,
                        Option::<String>::None,
                        Some(String::new()),
                    )]),
                    false,
                )
            } else {
                let Some(pos) = get_position(word_from, ctx.pattern_tokens, ctx.match_range.start)
                else {
                    return FilterOutcome::reject();
                };
                (
                    ctx.pattern_tokens[pos].clone(),
                    ctx.pattern_tokens[pos].is_tagged,
                )
            };
        let atr_word_token = atr_word.surface().to_string();
        let string_comparator = StringComparator {
            base: atr_word_token.clone(),
            max_distance: 4,
        };
        let is_word_capitalized = lt_tagger::is_capitalized_word(&atr_word_token);
        let is_word_all_upper = lt_tagger::is_all_uppercase(&atr_word_token);

        // Check if the original token (before disambiguation) meets the
        // requirements
        for atr in self.env.tagger.tag_word(&atr_word_token) {
            if let Some(tag) = &atr.pos_tag {
                if desired_re.is_match(tag).unwrap_or(false) && diacritics_mode {
                    return FilterOutcome::reject();
                }
            }
        }

        let mut replacements: Vec<String> = Vec::new();
        let mut replacements2: Vec<String> = Vec::new();
        let mut used_lemmas: Vec<Option<String>> = Vec::new();
        let mut used_priority_pos = 0usize;
        if atr_is_tagged || !atr_word_token.is_empty() {
            let mut analyzed_atr = atr_word.clone();
            analyzed_atr.is_tagged = atr_is_tagged;
            for suggestion in self.spelling_suggestions(&analyzed_atr) {
                let cleaned = self.clean_suggestion(&suggestion);
                let analyzed_suggestions = self.env.tagger.tag_word(&cleaned);
                for analyzed_suggestion in analyzed_suggestions {
                    if replacements.len() >= 2 * MAX_SUGGESTIONS {
                        break;
                    }
                    let Some(tag) = analyzed_suggestion.pos_tag.clone() else {
                        continue;
                    };
                    let mut used = false;
                    if suggestion != atr_word_token && desired_re.is_match(&tag).unwrap_or(false) {
                        let diacritics_ok = !diacritics_mode
                            || crate::multitoken::remove_diacritics(&suggestion).to_lowercase()
                                == crate::multitoken::remove_diacritics(&atr_word_token)
                                    .to_lowercase();
                        if diacritics_ok
                            && !replacements.contains(&suggestion)
                            && !replacements.contains(&suggestion.to_lowercase())
                        {
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
                                    .map(|re| re.is_match(&tag).unwrap_or(false))
                                    .unwrap_or(false);
                                if is_priority {
                                    replacements.insert(
                                        used_priority_pos.min(replacements.len()),
                                        replacement,
                                    );
                                    used_priority_pos += 1;
                                } else {
                                    replacements.push(replacement);
                                }
                                used = true;
                            }
                        }
                    }
                    if !used {
                        // try with the synthesizer
                        let mut synthesized_suggestions: Vec<String> = Vec::new();
                        for at in std::iter::once(&analyzed_suggestion) {
                            let lemma = at.stem.clone();
                            if used_lemmas.contains(&lemma) {
                                continue;
                            }
                            let synthesized =
                                self.env.synth.inner().synthesize(at, desired_postag, true);
                            used_lemmas.push(lemma);
                            for synthesized_suggestion in synthesized {
                                if !synthesized_suggestions.contains(&synthesized_suggestion) {
                                    synthesized_suggestions.push(synthesized_suggestion);
                                }
                            }
                            for mut replacement in synthesized_suggestions.iter().cloned() {
                                if is_word_all_upper {
                                    replacement = replacement.to_uppercase();
                                }
                                if is_word_capitalized {
                                    replacement = lt_tagger::uppercase_first_char(&replacement);
                                }
                                replacements2.push(replacement);
                            }
                        }
                    }
                }
            }
        }

        let match_contains_finished_suggestion = ctx
            .suggestions
            .iter()
            .any(|s| !s.value.to_lowercase().contains("{suggestion}"));
        if diacritics_mode && replacements.is_empty() && !match_contains_finished_suggestion {
            return FilterOutcome::reject();
        }
        if replacements.len() + replacements2.len() == 0
            && suppress_match
            && !match_contains_finished_suggestion
        {
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
                        if !definitive.contains(s2) {
                            definitive.push(s.value.replace("{suggestion}", s2));
                        }
                    } else if s.value.contains("{Suggestion}") {
                        if !definitive.contains(&lt_tagger::uppercase_first_char(s2)) {
                            definitive.push(
                                s.value
                                    .replace("{Suggestion}", &lt_tagger::uppercase_first_char(s2)),
                            );
                        }
                    } else if !definitive.contains(&s2.to_uppercase()) {
                        definitive.push(s.value.replace("{SUGGESTION}", &s2.to_uppercase()));
                    }
                }
            } else if !definitive.contains(&s.value) {
                definitive.push(s.value.clone());
            }
        }
        if !replacements_used {
            if replacements.is_empty() {
                replacements2.sort_by(|a, b| string_comparator.compare(a, b));
                for replacement in replacements2 {
                    if !replacements.contains(&replacement) && !definitive.contains(&replacement) {
                        replacements.push(replacement);
                    }
                }
            }
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
        // the RuleMatch carries the raw matched text (`setOriginalErrorStr`),
        // so a suggestion identical to the input is dropped on every path.
        let original_error = ctx
            .sentence_text
            .get(ctx.match_range.start..ctx.match_range.end)
            .unwrap_or("");
        if let Some(pos) = definitive.iter().position(|s| s == original_error) {
            definitive.remove(pos);
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
        // Java builds a *new* RuleMatch for the filter result, so an empty
        // `definitiveReplacements` clears the rule's own suggestions
        // (`VERB_PRONOUN[14]` on `Aix les Milles-la Pioline`).
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(out),
        }
    }
}

// ---------------------------------------------------------------------------
// FrenchSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.fr.FrenchSuppressMisspelledSuggestionsFilter`
/// (`AbstractSuppressMisspelledSuggestionsFilter`, no overrides).
struct FrenchSuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl FrenchSuppressMisspelledSuggestionsFilter {
    fn is_misspelled_multiword(&self, word: &str) -> bool {
        let is_tagged = |w: &str| self.env.tagger.is_tagged_word(w);
        for token in lt_tokenize::FrenchWordTokenizer::new(&is_tagged).tokenize(word) {
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

impl RuleFilter for FrenchSuppressMisspelledSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let suppress_postag = ctx.args.get("SuppressPostag");
        let filter_postag = ctx.args.get("FilterPostag");
        let mut new_replacements: Vec<Suggestion> = Vec::new();
        let tag_all = suppress_postag.is_some() || filter_postag.is_some();
        let tagged = if tag_all {
            self.env.tagger.tag(
                &ctx.suggestions
                    .iter()
                    .map(|s| s.value.clone())
                    .collect::<Vec<_>>(),
            )
        } else {
            Vec::new()
        };
        for (i, replacement) in ctx.suggestions.iter().enumerate() {
            if self.is_misspelled_multiword(&replacement.value) {
                continue;
            }
            let mut add = true;
            if tag_all {
                let Some(atr) = tagged.get(i) else {
                    continue;
                };
                if let Some(re) =
                    suppress_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                {
                    if atr
                        .readings
                        .iter()
                        .any(|r| r.pos_tag.as_deref().is_some_and(|t| re.is_match(t)))
                    {
                        add = false;
                    }
                }
                if add {
                    if let Some(re) =
                        filter_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                    {
                        if !atr
                            .readings
                            .iter()
                            .any(|r| r.pos_tag.as_deref().is_some_and(|t| re.is_match(t)))
                        {
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
/// the French `MultitokenSpeller`.
struct FrenchMultitokenSpellerFilter {
    env: Env,
}

impl FrenchMultitokenSpellerFilter {
    fn is_misspelled(&self, text: &str) -> bool {
        let is_tagged = |w: &str| self.env.tagger.is_tagged_word(w);
        for token in lt_tokenize::FrenchWordTokenizer::new(&is_tagged).tokenize(text) {
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

impl RuleFilter for FrenchMultitokenSpellerFilter {
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
// AdvancedSynthesizerFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.fr.AdvancedSynthesizerFilter`
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
// FrenchNumberInWordFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.fr.FrenchNumberInWordFilter`
/// (`AbstractNumberInWordFilter` with the French speller).
struct FrenchNumberInWordFilter {
    env: Env,
}

impl RuleFilter for FrenchNumberInWordFilter {
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

// ---------------------------------------------------------------------------
// SuggestionsFilter / MakeContractionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.fr.SuggestionsFilter`: remove suggestions that
/// match a regular expression.
struct SuggestionsFilter;

impl RuleFilter for SuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(remove_suggestions_regexp) = ctx.args.get("RemoveSuggestionsRegexp") else {
            return FilterOutcome::reject();
        };
        let Ok(pattern) = regex::RegexBuilder::new(&format!("^(?:{remove_suggestions_regexp})$"))
            .case_insensitive(true)
            .build()
        else {
            return FilterOutcome::reject();
        };
        let new_replacements: Vec<Suggestion> = ctx
            .suggestions
            .iter()
            .filter(|r| !pattern.is_match(&r.value))
            .cloned()
            .collect();
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(new_replacements),
        }
    }
}

/// `org.languagetool.rules.fr.MakeContractionsFilter`
/// (`AbstractMakeContractionsFilter` with the French rules).
struct MakeContractionsFilter;

impl MakeContractionsFilter {
    fn fix_contractions(suggestion: &str) -> String {
        static DE_LE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
            regex::RegexBuilder::new(r"\bde le\b")
                .case_insensitive(true)
                .build()
                .unwrap()
        });
        static A_LE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
            regex::RegexBuilder::new(r"\bà le\b")
                .case_insensitive(true)
                .build()
                .unwrap()
        });
        static DE_LES: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
            regex::RegexBuilder::new(r"\bde les\b")
                .case_insensitive(true)
                .build()
                .unwrap()
        });
        static A_LES: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
            regex::RegexBuilder::new(r"\bà les\b")
                .case_insensitive(true)
                .build()
                .unwrap()
        });
        let suggestion = DE_LE.replace_all(suggestion, "du");
        let suggestion = A_LE.replace_all(&suggestion, "au");
        let suggestion = DE_LES.replace_all(&suggestion, "des");
        A_LES.replace_all(&suggestion, "aux").into_owned()
    }
}

impl RuleFilter for MakeContractionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let new_suggestions: Vec<Suggestion> = ctx
            .suggestions
            .iter()
            .map(|s| Suggestion {
                value: Self::fix_contractions(&s.value),
                short_description: s.short_description.clone(),
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(new_suggestions),
        }
    }
}

// ---------------------------------------------------------------------------
// InterrogativeVerbFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.fr.InterrogativeVerbFilter`.
struct InterrogativeVerbFilter {
    env: Env,
}

impl InterrogativeVerbFilter {
    /// `InterrogativeVerbFilter.makeWrong` (the local mapping, different from
    /// `StringTools.makeWrong`).
    fn make_wrong(s: &str) -> String {
        for (from, to) in [
            ("a", "ä"),
            ("e", "ë"),
            ("i", "í"),
            ("o", "ö"),
            ("u", "ü"),
            ("é", "ë"),
            ("à", "ä"),
            ("è", "ë"),
            ("ù", "ü"),
            ("â", "ä"),
            ("ê", "ë"),
            ("î", "ï"),
            ("ô", "ö"),
            ("û", "ü"),
        ] {
            if s.contains(from) {
                return s.replace(from, to);
            }
        }
        format!("{s}-")
    }
}

impl RuleFilter for InterrogativeVerbFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(pronoun_from) = ctx.args.get("PronounFrom") else {
            return FilterOutcome::reject();
        };
        let Some(verb_from) = ctx.args.get("VerbFrom") else {
            return FilterOutcome::reject();
        };
        let mut replacements: Vec<String> = Vec::new();
        let mut desired_postag: Option<String> = None;
        let mut extra_suggestions: Vec<String> = Vec::new();
        let pos_pronoun = match pronoun_from.parse::<usize>() {
            Ok(p) if p >= 1 && p <= ctx.pattern_tokens.len() => p - 1,
            _ => return FilterOutcome::reject(),
        };
        let pos_verb = match verb_from.parse::<usize>() {
            Ok(p) if p >= 1 && p <= ctx.pattern_tokens.len() => p - 1,
            _ => return FilterOutcome::reject(),
        };
        let atr_pronoun = ctx.pattern_tokens[pos_pronoun];
        let pronoun_token = atr_pronoun.surface().to_string();
        if atr_pronoun
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("R pers obj 2 p"))
        {
            desired_postag = Some("V.* (imp) [23] [sp]|V .*(ind|cond).* 2 p".to_string());
        } else if atr_pronoun
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("R pers obj 1 p"))
        {
            desired_postag = Some("V.* (imp) .*|V .*(ind|cond).* 1 p".to_string());
        } else if matches_pos_tag_regex(atr_pronoun, "R pers obj.*") {
            desired_postag = Some("V.* (imp) .*".to_string());
        } else if matches_pos_tag_regex(atr_pronoun, ".* 1 s") {
            desired_postag = Some("V .*(ind|cond).* 1 s".to_string());
            let atr_verb = ctx.pattern_tokens[pos_verb];
            let reading = atr_verb.readings.iter().find(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| t == "V .*" || fancy_full_match("V .*", t))
            });
            if let Some(reading) = reading {
                let participles =
                    self.env
                        .synth
                        .inner()
                        .synthesize(reading, "V ppa [me] sp?", true);
                if let Some(first) = participles.first() {
                    if first.ends_with('é') {
                        extra_suggestions.push(first.clone());
                        // Java `substring(0, length-1)`: drop the final
                        // (possibly multibyte) character.
                        let without_last: String =
                            first.chars().take(first.chars().count() - 1).collect();
                        extra_suggestions.push(format!("{without_last}è"));
                    }
                }
            }
        } else if matches_pos_tag_regex(atr_pronoun, ".* 2 s") {
            desired_postag = Some("V .*(ind|cond).* 2 s".to_string());
        } else if matches_pos_tag_regex(atr_pronoun, ".* 3( [mfe])? s") {
            desired_postag = Some("V .*(ind|cond).* 3 s".to_string());
        } else if matches_pos_tag_regex(atr_pronoun, ".* 1 p") {
            desired_postag = Some("V .*(ind|cond).* 1 p".to_string());
        } else if matches_pos_tag_regex(atr_pronoun, ".* 2 p") {
            desired_postag = Some("V .*(ind|cond).* 2 p".to_string());
        } else if matches_pos_tag_regex(atr_pronoun, ".* 3( [mf])? p") {
            desired_postag = Some("V .*(ind|cond).* 3 p".to_string());
        }
        if !extra_suggestions.is_empty() {
            // add: trompè-je and trompé-je for original sentence "trompe-je"
            for extra in &extra_suggestions {
                let separator = if pronoun_token.starts_with('-') {
                    ""
                } else {
                    "-"
                };
                let complete = format!("{extra}{separator}{pronoun_token}");
                if !replacements.contains(&complete) && !complete.ends_with("e-je") {
                    replacements.push(complete);
                }
            }
        } else if let Some(desired_postag) = desired_postag {
            let verb_token = ctx.pattern_tokens[pos_verb];
            let word = if verb_token.is_tagged {
                Self::make_wrong(verb_token.surface())
            } else {
                verb_token.surface().to_string()
            };
            let suggestions = self.env.spelling.suggestions(&word);
            let analyzed_suggestions = self.env.tagger.tag(&suggestions);
            for analyzed_suggestion in analyzed_suggestions {
                if matches_pos_tag_regex(&analyzed_suggestion, &desired_postag) {
                    let separator = if pronoun_token.starts_with('-') {
                        ""
                    } else {
                        "-"
                    };
                    let mut complete = format!(
                        "{}{separator}{pronoun_token}",
                        analyzed_suggestion.surface()
                    );
                    if complete.eq_ignore_ascii_case("peux-je") {
                        complete = preserve_case("puis-je", &complete);
                    }
                    if complete.ends_with("e-je") {
                        // Java `substring(0, length-4)`: drop the final four
                        // characters ("e-je"), not four bytes.
                        let truncated: String = complete
                            .chars()
                            .take(complete.chars().count() - 4)
                            .collect();
                        let first = format!("{truncated}é-je");
                        if !replacements.contains(&first) {
                            replacements.push(first);
                        }
                        let second = format!("{truncated}è-je");
                        if !replacements.contains(&second) {
                            replacements.push(second);
                        }
                    } else if !replacements.contains(&complete) {
                        replacements.push(complete);
                    }
                }
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::accept();
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

/// `Pattern.matcher(tag).matches()` for the Java pattern `V .*`.
fn fancy_full_match(pattern: &str, text: &str) -> bool {
    fancy_regex::Regex::new(&format!("^(?:{pattern})$"))
        .ok()
        .is_some_and(|re| re.is_match(text).unwrap_or(false))
}

/// `StringTools.preserveCase`.
fn preserve_case(input: &str, model: &str) -> String {
    if lt_tagger::is_capitalized_word(model) {
        return lt_tagger::uppercase_first_char(&input.to_lowercase());
    }
    if lt_tagger::is_all_uppercase(model) {
        return input.to_uppercase();
    }
    input.to_string()
}

// ---------------------------------------------------------------------------
// WordWithDeterminerFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.fr.WordWithDeterminerFilter`: suggestions for
/// determiner + noun/adjective agreement, filtered through the
/// `CAT_ELISION` re-check hook.
struct WordWithDeterminerFilter {
    env: Env,
}

impl WordWithDeterminerFilter {
    const GENDER_NUMBER: [&'static str; 4] = [
        "([me]) (s|sp)",
        "([fe]) (s|sp)",
        "([me]) (p|sp)",
        "([fe]) (p|sp)",
    ];
    const DETERMINER: &'static str = "((P.)?D |J |V.* ppa )";
    const EXCEPTIONS_DETERMINER: [&'static str; 4] = ["bels", "fols", "mols", "nouvels"];
}

impl WordWithDeterminerFilter {
    fn get_analyzed_token<'a>(
        token: &'a lt_core::AnalyzedTokenReadings,
        pattern: &str,
    ) -> Option<&'a lt_core::AnalyzedToken> {
        let re = regex::Regex::new(&format!("^(?:{pattern})$")).ok()?;
        token.readings.iter().find(|r| {
            let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
            re.is_match(pos_tag)
        })
    }

    fn tag_matches(pos_tag: &str, pattern: &str) -> bool {
        regex::Regex::new(&format!("^(?:{pattern})$"))
            .ok()
            .is_some_and(|re| re.is_match(pos_tag))
    }
}

impl RuleFilter for WordWithDeterminerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(word_from), Some(determiner_from)) =
            (ctx.args.get("wordFrom"), ctx.args.get("determinerFrom"))
        else {
            return FilterOutcome::reject();
        };
        let pos_word = match word_from.parse::<usize>() {
            Ok(p) if p >= 1 && p <= ctx.pattern_tokens.len() => p - 1,
            _ => return FilterOutcome::reject(),
        };
        let pos_determiner = match determiner_from.parse::<usize>() {
            Ok(p) if p >= 1 && p <= ctx.pattern_tokens.len() => p - 1,
            _ => return FilterOutcome::reject(),
        };
        let atr_determiner = ctx.pattern_tokens[pos_determiner];
        let atr_word = ctx.pattern_tokens[pos_word];
        let is_determiner_capitalized = lt_tagger::is_capitalized_word(atr_determiner.surface());
        let is_word_capitalized = lt_tagger::is_capitalized_word(atr_word.surface());
        let is_determiner_all_upper = lt_tagger::is_all_uppercase(atr_determiner.surface())
            && !atr_determiner.surface().eq_ignore_ascii_case("L'");
        let is_word_all_upper = lt_tagger::is_all_uppercase(atr_word.surface());
        let Some(at_determiner) =
            Self::get_analyzed_token(atr_determiner, "(P.)?D .*|J .*|V.* ppa .*")
        else {
            return FilterOutcome::reject();
        };
        let Some(at_word) = Self::get_analyzed_token(atr_word, "[ZNJ] .*|V.* ppa .*") else {
            return FilterOutcome::reject();
        };
        let word_tag = at_word.pos_tag.clone().unwrap_or_default();
        let is_noun = word_tag.starts_with('N') || word_tag.starts_with('Z');
        let is_adjective = word_tag.starts_with('J');
        let prefix = if is_noun && !is_adjective {
            "[NZ] "
        } else if !is_noun && is_adjective {
            "J "
        } else {
            "[ZNJ] "
        };

        // synthesize all forms
        let mut determiner_forms: [Vec<String>; 4] = Default::default();
        let mut word_forms: [Vec<String>; 4] = Default::default();
        for i in 0..4 {
            let gn = Self::GENDER_NUMBER[i];
            let det_pattern = format!("{}{gn}", Self::DETERMINER);
            determiner_forms[i] =
                self.env
                    .synth
                    .inner()
                    .synthesize(at_determiner, &det_pattern, true);
            let word_pattern = format!("{prefix}{gn}");
            word_forms[i] = self
                .env
                .synth
                .inner()
                .synthesize(at_word, &word_pattern, true);
            // if it cannot be synthesized, keep the original determiner/word
            let det_tag = at_determiner.pos_tag.clone().unwrap_or_default();
            if determiner_forms[i].is_empty() && Self::tag_matches(&det_tag, &format!(".+{gn}")) {
                determiner_forms[i] = vec![at_determiner.token.clone()];
            }
            if word_forms[i].is_empty() && Self::tag_matches(&word_tag, &format!(".+{gn}")) {
                word_forms[i] = vec![at_word.token.clone()];
            }
        }

        // generate suggestions
        let mut replacements: Vec<String> = Vec::new();
        for i in 0..4 {
            for word_orig in &word_forms[i] {
                for determiner_orig in &determiner_forms[i] {
                    let mut determiner = determiner_orig.clone();
                    let mut word = word_orig.clone();
                    if Self::EXCEPTIONS_DETERMINER.contains(&determiner.as_str()) {
                        continue;
                    }
                    if is_determiner_capitalized {
                        determiner = lt_tagger::uppercase_first_char(&determiner);
                    }
                    if is_word_capitalized {
                        word = lt_tagger::uppercase_first_char(&word);
                    }
                    if is_determiner_all_upper {
                        determiner = determiner.to_uppercase();
                    }
                    if is_word_all_upper {
                        word = word.to_uppercase();
                    }
                    let r = format!("{determiner} {word}").replace("' ", "'");
                    if !self.suggestion_has_no_errors(&r) {
                        continue;
                    }
                    if replacements.contains(&r) {
                        continue;
                    }
                    if r.ends_with(&at_word.token) {
                        replacements.insert(0, r);
                    } else {
                        replacements.push(r);
                    }
                }
            }
        }
        let mut all = ctx
            .suggestions
            .iter()
            .map(|s| s.value.clone())
            .collect::<Vec<_>>();
        all.extend(replacements);
        if all.is_empty() {
            return FilterOutcome::accept();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(
                all.into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}

impl WordWithDeterminerFilter {
    /// `suggestionHasNoErrors` through the pipeline-installed hook. Before
    /// the hook is installed (rule compilation probes) every candidate is
    /// accepted, like Java without a checkable sentence.
    fn suggestion_has_no_errors(&self, suggestion: &str) -> bool {
        match self.env.suggestion_checker.get() {
            Some(checker) => checker(suggestion),
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// PostponedAdjectiveConcordanceFilter
// ---------------------------------------------------------------------------

const PA_MAX_LEVELS: usize = 4;

fn pa_re(pattern: &str) -> regex::Regex {
    regex::Regex::new(&format!("^(?:{pattern})$")).unwrap()
}

/// Full match of one reading's POS tag (Java `matchPostagRegexp`).
fn postag_matches(tr: &lt_core::AnalyzedTokenReadings, pattern: &regex::Regex) -> bool {
    tr.readings.iter().any(|r| {
        let tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(tag)
    })
}

fn get_analyzed_token<'a>(
    tr: &'a lt_core::AnalyzedTokenReadings,
    pattern: &regex::Regex,
) -> Option<&'a lt_core::AnalyzedToken> {
    tr.readings.iter().find(|r| {
        let tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(tag)
    })
}

/// `PostponedAdjectiveConcordanceFilter` patterns.
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
    det: regex::Regex,
    det_cs: regex::Regex,
    det_ms: regex::Regex,
    det_fs: regex::Regex,
    det_mp: regex::Regex,
    det_fp: regex::Regex,
    det_cp: regex::Regex,
    subst: [(regex::Regex, regex::Regex, regex::Regex); 8],
    adjectiu: regex::Regex,
    adjectiu_ms: regex::Regex,
    adjectiu_fs: regex::Regex,
    adjectiu_mp: regex::Regex,
    adjectiu_fp: regex::Regex,
    adjectiu_cp: regex::Regex,
    adjectiu_cs: regex::Regex,
    adjectiu_mn: regex::Regex,
    adjectiu_fn: regex::Regex,
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
    infinitive: regex::Regex,
    gv: regex::Regex,
}

impl PaPatterns {
    fn new() -> Self {
        Self {
            nom: pa_re("[NZ] .*"),
            nom_ms: pa_re("[NZ] m s"),
            nom_fs: pa_re("[NZ] f s"),
            nom_mp: pa_re("[NZ] m p"),
            nom_mn: pa_re("[NZ] m sp"),
            nom_fp: pa_re("[NZ] f p"),
            nom_cs: pa_re("[NZ] e s"),
            nom_cp: pa_re("[NZ] e sp"),
            nom_det: pa_re("[NZ] .*|(P\\+)?D .*"),
            gn_any: pa_re("_GN_.*"),
            gn_ms: pa_re("_GN_MS"),
            gn_fs: pa_re("_GN_FS"),
            gn_mp: pa_re("_GN_MP"),
            gn_fp: pa_re("_GN_FP"),
            det: pa_re("(P\\+)?D .*"),
            det_cs: pa_re("(P\\+)?D e s"),
            det_ms: pa_re("(P\\+)?D m s"),
            det_fs: pa_re("(P\\+)?D f s"),
            det_mp: pa_re("(P\\+)?D m p"),
            det_fp: pa_re("(P\\+)?D f p"),
            det_cp: pa_re("(P\\+)?D e p"),
            subst: [
                // 0: CS, 1: CP, 2: MN, 3: FN, 4: MS, 5: FS, 6: MP, 7: FP
                (
                    pa_re("[NZ] [fme] (s|sp)|J [fme] (s|sp)|(P\\+)?D [fme] (s|sp)"),
                    pa_re("_GN_[MF]S"),
                    pa_re("J .* (s|sp)|V ppa . s"),
                ),
                (
                    pa_re("[NZ] [fme] (p|sp)|J [fme] (p|sp)|(P\\+)?D [fme] (p|sp)"),
                    pa_re("_GN_[MF]P"),
                    pa_re("J .* (p|sp)|V ppa . p"),
                ),
                (
                    pa_re("[NZ] [me] (s|p|sp)|J [me] (s|p|sp)|(P\\+)?D [me] (s|p|sp)"),
                    pa_re("_GN_M[SP]"),
                    pa_re("J [me] .*|V ppa [me] .*"),
                ),
                (
                    pa_re("[NZ] [fe] (s|p|sp)|J [fe] (s|p|sp)|(P\\+)?D [fe] (s|p|sp)"),
                    pa_re("_GN_F[SP]"),
                    pa_re("J f sp"),
                ),
                (
                    pa_re("[NZ] [me] (s|sp)|J [me] (s|sp)|V ppa m s|(P\\+)?D m (s|sp)"),
                    pa_re("_GN_MS"),
                    pa_re("J [me] (s|sp)|V ppa m s"),
                ),
                (
                    pa_re("[NZ] [fe] (s|sp)|J [fe] (s|sp)|V ppa f s|(P\\+)?D f (s|sp)"),
                    pa_re("_GN_FS"),
                    pa_re("J [fe] (s|sp)|V ppa f s"),
                ),
                (
                    pa_re("[NZ] [me] (p|sp)|J [me] (p|sp)|V ppa m p|(P\\+)?D m (p|sp)"),
                    pa_re("_GN_MP"),
                    pa_re("J [me] (p|sp)|V ppa m p"),
                ),
                (
                    pa_re("[NZ] [fe] (p|sp)|J [fe] (p|sp)|V ppa f p|(P\\+)?D f (p|sp)"),
                    pa_re("_GN_FP"),
                    pa_re("J [fe] (p|sp)|V ppa f p"),
                ),
            ],
            adjectiu: pa_re("J .*|V ppa .*|PX.*"),
            adjectiu_ms: pa_re("J [me] (s|sp)|V ppa m s"),
            adjectiu_fs: pa_re("J [fe] (s|sp)|V ppa f s"),
            adjectiu_mp: pa_re("J [me] (p|sp)|V ppa m p"),
            adjectiu_fp: pa_re("J [fe] (p|sp)|V ppa f p"),
            adjectiu_cp: pa_re("J e (p|sp)"),
            adjectiu_cs: pa_re("J e (s|sp)"),
            adjectiu_mn: pa_re("J m sp"),
            adjectiu_fn: pa_re("J f sp"),
            adjectiu_s: pa_re("J .* (s|sp)|V ppa . s"),
            adjectiu_p: pa_re("J .* (p|sp)|V ppa . p"),
            adverbi: pa_re("A"),
            conjuncio: pa_re("C .*"),
            punctuacio: pa_re("_PUNCT"),
            loc_adv: pa_re("A"),
            adverbis_acceptats: pa_re("A"),
            coordinacio_ioni: pa_re("et|ou|ni"),
            keep_count: pa_re("Y|J .*|N .*|D .*|P.*|V ppa .*|M nonfin|UNKNOWN|Z.*|V.* inf|V ppr"),
            keep_count2: pa_re(",|et|ou|ni"),
            stop_count: pa_re("[;:\\(\\)\\[\\]–—―‒]"),
            preposicions: pa_re("P.*"),
            preposicio_canvi_nivell: pa_re(
                "d'|de|des|du|à|au|aux|en|dans|sur|entre|par|pour|avec|sans|contre|comme",
            ),
            verb: pa_re("V.* (inf|ind|sub|con|ppr|imp).*"),
            infinitive: pa_re("V.* inf"),
            gv: pa_re("_GV_"),
        }
    }
}

struct PaApparitions {
    adverb: bool,
    conjunction: bool,
    punctuation: bool,
    infinitive: bool,
}

impl PaApparitions {
    fn new() -> Self {
        Self {
            adverb: false,
            conjunction: false,
            punctuation: false,
            infinitive: false,
        }
    }
}

/// `org.languagetool.rules.fr.PostponedAdjectiveConcordanceFilter`.
struct PostponedAdjectiveConcordanceFilter {
    env: Env,
}

impl PostponedAdjectiveConcordanceFilter {
    fn keep_counting(
        p: &PaPatterns,
        a: &PaApparitions,
        tr: &lt_core::AnalyzedTokenReadings,
    ) -> bool {
        let token = tr.surface();
        if p.preposicio_canvi_nivell.is_match(token) {
            return true;
        }
        if token == "." {
            // it is not sentence end, but abbreviation
            return true;
        }
        if (a.adverb && (a.conjunction || a.punctuation))
            || (a.conjunction && a.punctuation)
            || (a.punctuation && postag_matches(tr, &p.punctuacio))
            || (a.infinitive && p.coordinacio_ioni.is_match(token))
            || (a.infinitive && a.adverb)
        {
            return false;
        }
        (postag_matches(tr, &p.keep_count)
            || p.keep_count2.is_match(token)
            || postag_matches(tr, &p.adverbis_acceptats))
            && !p.stop_count.is_match(token)
            && (!postag_matches(tr, &p.gv) || postag_matches(tr, &p.gn_any))
    }

    fn update_apparitions(
        p: &PaPatterns,
        a: &mut PaApparitions,
        tr: &lt_core::AnalyzedTokenReadings,
    ) {
        a.conjunction |= postag_matches(tr, &p.conjuncio);
        if tr.surface() == "com" {
            return;
        }
        if postag_matches(tr, &p.nom) || postag_matches(tr, &p.adjectiu) {
            *a = PaApparitions::new();
            return;
        }
        a.adverb |= postag_matches(tr, &p.adverbi);
        a.punctuation |= postag_matches(tr, &p.punctuacio) || tr.surface() == ",";
        a.infinitive |= postag_matches(tr, &p.infinitive);
    }

    /// `PostponedAdjectiveConcordanceFilter.updateJValue`.
    fn update_j_value(
        p: &PaPatterns,
        tokens: &[&lt_core::AnalyzedTokenReadings],
        i: usize,
        j: usize,
    ) -> usize {
        let mut j = j;
        if p.coordinacio_ioni.is_match(tokens[i - j].surface())
            && i > j + 1
            && i - j + 1 < tokens.len()
            && postag_matches(tokens[i - j - 1], &p.det)
            && tokens[i - j + 1].surface() == "plus"
        {
            j += 1;
        }
        j
    }
}

impl RuleFilter for PostponedAdjectiveConcordanceFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let p = PaPatterns::new();
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
        let mut c_nt = [0i32; PA_MAX_LEVELS];
        let mut c_nms = [0i32; PA_MAX_LEVELS];
        let mut c_nfs = [0i32; PA_MAX_LEVELS];
        let mut c_nmp = [0i32; PA_MAX_LEVELS];
        let mut c_nmn = [0i32; PA_MAX_LEVELS];
        let mut c_nfp = [0i32; PA_MAX_LEVELS];
        let mut c_ncs = [0i32; PA_MAX_LEVELS];
        let mut c_ncp = [0i32; PA_MAX_LEVELS];
        let mut c_dms = [0i32; PA_MAX_LEVELS];
        let mut c_dfs = [0i32; PA_MAX_LEVELS];
        let mut c_dmp = [0i32; PA_MAX_LEVELS];
        let mut c_dfp = [0i32; PA_MAX_LEVELS];
        let mut level = 0usize;
        let mut j = 1usize;
        let mut app = PaApparitions::new();
        while i > j && Self::keep_counting(&p, &app, tokens[i - j]) && level < PA_MAX_LEVELS {
            if !is_prev_noun {
                if postag_matches(tokens[i - j], &p.nom)
                    || (i > j + 1
                        && !postag_matches(tokens[i - j], &p.nom)
                        && postag_matches(tokens[i - j], &p.adjectiu)
                        && postag_matches(tokens[i - j - 1], &p.det))
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
            if postag_matches(tokens[i - j], &p.det_cp) {
                if postag_matches(tokens[i - j + 1], &p.nom_mp) {
                    c_dms[level] += 1;
                    can_be_mp = true;
                }
                if postag_matches(tokens[i - j + 1], &p.nom_fp) {
                    c_dfs[level] += 1;
                    can_be_fp = true;
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
            if i > j + 1
                && ((p.preposicio_canvi_nivell.is_match(tokens[i - j].surface())
                    && postag_matches(tokens[i - j], &p.preposicions)
                    && !postag_matches(tokens[i - j], &p.conjuncio)
                    && !p.coordinacio_ioni.is_match(tokens[i - j - 1].surface())
                    && !postag_matches(tokens[i - j + 1], &p.adverbi))
                    // exception: d'environ
                    || (tokens[i - j].surface().eq_ignore_ascii_case("d'")
                        && tokens[i - j + 1].surface().eq_ignore_ascii_case("environ")))
            {
                level += 1;
            }
            j = Self::update_j_value(&p, &tokens, i, j);
            Self::update_apparitions(&p, &mut app, tokens[i - j]);
            j += 1;
        }
        level += 1;
        if level > PA_MAX_LEVELS {
            level = PA_MAX_LEVELS;
        }
        let mut c_n = [0i32; PA_MAX_LEVELS];
        let mut c_d = [0i32; PA_MAX_LEVELS];
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
                is_plural = is_plural && c_d[jj] > 1 && level > 1;
                can_be_p = can_be_p || c_n[jj] > 1;
            }
            jj += 1;
        }
        // comma + plural noun
        is_plural = is_plural
            || (i > 2 && c_nmp[0] + c_nfp[0] + c_ncp[0] > 0 && tokens[i - 2].surface() == ",");

        // there is no noun, (no determinant --> && cDtotal==0)
        if c_ntotal == 0 && c_dtotal == 0 {
            return FilterOutcome::reject();
        }

        // patterns according to the analyzed adjective
        let subst_idx = if postag_matches(tokens[i], &p.adjectiu_cs) {
            Some(0usize)
        } else if postag_matches(tokens[i], &p.adjectiu_cp) {
            Some(1)
        } else if postag_matches(tokens[i], &p.adjectiu_mn) {
            Some(2)
        } else if postag_matches(tokens[i], &p.adjectiu_fn) {
            Some(3)
        } else if postag_matches(tokens[i], &p.adjectiu_ms) {
            Some(4)
        } else if postag_matches(tokens[i], &p.adjectiu_fs) {
            Some(5)
        } else if postag_matches(tokens[i], &p.adjectiu_mp) {
            Some(6)
        } else if postag_matches(tokens[i], &p.adjectiu_fp) {
            Some(7)
        } else {
            None
        };
        let Some(subst_idx) = subst_idx else {
            return FilterOutcome::reject();
        };
        let (subst_pattern, gn_pattern, adj_pattern) = &p.subst[subst_idx];

        // combinations Det/Nom + adv (1,2..) + adj. If there is agreement,
        // the rule doesn't match
        let mut j = 1usize;
        let mut keep_count = true;
        while i > j && keep_count {
            if (postag_matches(tokens[i - j], &p.nom_det)
                && postag_matches(tokens[i - j], gn_pattern))
                || (!postag_matches(tokens[i - j], &p.gn_any)
                    && postag_matches(tokens[i - j], subst_pattern))
            {
                return FilterOutcome::reject(); // there is a previous agreeing noun
            }
            keep_count = !postag_matches(tokens[i - j], &p.nom_det);
            j += 1;
        }

        // Necessary condition: previous token is a non-agreeing noun
        // or it is adjective or adverb (not preceded by verb)
        let necessary = (postag_matches(tokens[i - 1], &p.nom)
            && !postag_matches(tokens[i - 1], subst_pattern))
            || (postag_matches(tokens[i - 1], &p.gn_any)
                && !postag_matches(tokens[i - 1], gn_pattern))
            || (postag_matches(tokens[i - 1], &p.adjectiu)
                && !postag_matches(tokens[i - 1], adj_pattern))
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
            let mut app = PaApparitions::new();
            while i > j && Self::keep_counting(&p, &app, tokens[i - j]) && (level > 1 || j < 4) {
                // there is a previous agreeing noun
                if (!postag_matches(tokens[i - j], &p.gn_any)
                    && postag_matches(tokens[i - j], &p.nom_det)
                    && postag_matches(tokens[i - j], subst_pattern))
                    || postag_matches(tokens[i - j], gn_pattern)
                {
                    return FilterOutcome::reject();
                }
                j = Self::update_j_value(&p, &tokens, i, j);
                Self::update_apparitions(&p, &mut app, tokens[i - j]);
                j += 1;
            }
        }

        // The rule matches. Synthesize suggestions.
        let synth = self.env.synth.inner();
        let mut suggestions: Vec<String> = Vec::new();
        if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_cs) {
            suggestions.extend(synth.synthesize(at, "J e p", true));
        }
        if suggestions.is_empty() {
            if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_cp) {
                suggestions.extend(synth.synthesize(at, "J e s", true));
            }
        }
        if suggestions.is_empty() && is_plural {
            if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_p) {
                suggestions.extend(synth.synthesize(at, "J . p|V ppa . p", true));
            }
        }
        if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu) {
            if suggestions.is_empty() {
                if can_be_ms && !is_plural {
                    suggestions.extend(synth.synthesize(at, "J [me] sp?|V ppa m s", true));
                }
                if can_be_fs && !is_plural {
                    suggestions.extend(synth.synthesize(at, "J [fe] sp?|V ppa f s", true));
                }
                if can_be_mp {
                    suggestions.extend(synth.synthesize(at, "J [me] s?p|V ppa m p", true));
                }
                if can_be_fp {
                    suggestions.extend(synth.synthesize(at, "J [fe] s?p|V ppa f p", true));
                }
                if can_be_ms && (is_plural || can_be_p) {
                    suggestions.extend(synth.synthesize(at, "J [me] s?p|V ppa m p", true));
                }
                if can_be_fs && !can_be_ms && (is_plural || can_be_p) {
                    suggestions.extend(synth.synthesize(at, "J [fe] s?p|V ppa f p", true));
                }
            }
        }
        // set suggestion removing duplicates; avoid the original token
        let mut seen: Vec<String> = Vec::new();
        let lower = tokens[i].surface().to_lowercase();
        suggestions.retain(|s| {
            *s != lower && {
                if seen.contains(s) {
                    false
                } else {
                    seen.push(s.clone());
                    true
                }
            }
        });
        if suggestions.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(
                suggestions
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

pub fn french_filter_registry(env: Env) -> FilterRegistry {
    let mut builder = FilterRegistry::builder()
        .register(
            "org.languagetool.rules.IsEnglishWordFilter",
            Arc::new(IsEnglishWordFilter),
        )
        .register(
            "org.languagetool.rules.SuppressIfAnyRuleMatchesFilter",
            Arc::new(SuppressIfAnyRuleMatchesFilter),
        )
        .register(
            "org.languagetool.rules.fr.FindSuggestionsFilter",
            Arc::new(FindSuggestionsFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.fr.FrenchSuppressMisspelledSuggestionsFilter",
            Arc::new(FrenchSuppressMisspelledSuggestionsFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter",
            Arc::new(FrenchMultitokenSpellerFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.AddCommasFilter",
            Arc::new(AddCommasFilter),
        )
        .register(
            "org.languagetool.rules.fr.AdvancedSynthesizerFilter",
            Arc::new(AdvancedSynthesizerFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.fr.FrenchNumberInWordFilter",
            Arc::new(FrenchNumberInWordFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.fr.SuggestionsFilter",
            Arc::new(SuggestionsFilter),
        )
        .register(
            "org.languagetool.rules.fr.MakeContractionsFilter",
            Arc::new(MakeContractionsFilter),
        )
        .register(
            "org.languagetool.rules.fr.InterrogativeVerbFilter",
            Arc::new(InterrogativeVerbFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.fr.WordWithDeterminerFilter",
            Arc::new(WordWithDeterminerFilter { env: env.clone() }),
        )
        .register(
            "org.languagetool.rules.fr.PostponedAdjectiveConcordanceFilter",
            Arc::new(PostponedAdjectiveConcordanceFilter { env: env.clone() }),
        );
    let date_env = env.clone();
    builder = crate::fr::date_filters::register(builder, date_env);
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::{damerau_levenshtein, StringComparator};

    #[test]
    fn java_damerau_distance_trims_common_prefix() {
        // `EditDistance.DamerauLevenshteinDistance`: the common prefix is
        // trimmed first, so the exact distance is reported even when the raw
        // length difference exceeds `maxDistance`.
        assert_eq!(damerau_levenshtein("inform", "informent", 4), 3);
        assert_eq!(damerau_levenshtein("inform", "informeraient", 4), 7);
        assert_eq!(damerau_levenshtein("inform", "informeraient", 3), 7);
        // no shared prefix: the length-difference guard still applies
        assert_eq!(damerau_levenshtein("xxxxxx", "yyyyyyyyyyyyy", 4), -1);
        assert_eq!(damerau_levenshtein("maizon", "maison", 4), 1);
    }

    #[test]
    fn suggestion_order_matches_java_comparator() {
        let cmp = StringComparator {
            base: "inform".into(),
            max_distance: 4,
        };
        let mut forms = vec![
            "informent",
            "informeraient",
            "informeront",
            "informaient",
            "informèrent",
            "informassent",
        ];
        forms.sort_by(|a, b| cmp.compare(a, b));
        // Java probe (`probe-rule.sh "Ils inform Marine de cela." ILS_VERBE`):
        // informent|informeront|informaient|informèrent|informassent|informeraient
        assert_eq!(
            forms,
            vec![
                "informent",
                "informeront",
                "informaient",
                "informèrent",
                "informassent",
                "informeraient"
            ]
        );
    }
}
