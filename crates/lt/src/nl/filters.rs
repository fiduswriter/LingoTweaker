//! Dutch rule filters (internal development notes).
//!
//! Stage 2 wires the `MultitokenSpellerFilter` over the Dutch speller and
//! multitoken speller. The remaining XML-referenced Dutch filter classes are
//! stage-3 work:
//!
//! - `DutchSuppressMisspelledSuggestionsFilter` (22 refs)
//! - `CompoundFilter` (9 refs, needs `CompoundAcceptor`)
//! - `DateCheckFilter` (3 refs, `AbstractDateCheckFilter` with the Dutch
//!   day/month names)
//! - `DutchNumberInWordFilter` (1 ref)
//!
//! Until they land the `compile_failures()` count is the progress metric.

use std::sync::Arc;

use lt_core::Suggestion;
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::Ymd;
use crate::wordutil::is_punctuation_mark;

/// `AbstractSuppressMisspelledSuggestionsFilter.isMisspelled`: tokenize with
/// the language tokenizer (stage 3 uses the base `WordTokenizer`) and ask the
/// speller's `isMisspelled`.
fn is_misspelled_multiword(env: &Env, word: &str) -> bool {
    let tokens = lt_tokenize::DutchWordTokenizer.tokenize(word);
    for token in tokens {
        if env.spelling.is_misspelled(&token) {
            return true;
        }
    }
    false
}

pub struct NlFilterEnv {
    /// `MultitokenSpellerFilter.isMisspelled` and the stage-3 taggers
    tagger: Arc<lt_tagger::DutchTagger>,
    /// `MorfologikDutchSpellerRule` (the default spelling rule)
    spelling: Arc<crate::nl::spelling::DutchSpellingRule>,
    /// `DutchMultitokenSpeller` (`MultitokenSpellerFilter`)
    multitoken: Arc<crate::multitoken::MultitokenSpeller>,
}

pub type Env = Arc<NlFilterEnv>;

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
/// language tokenizer (stage 2: the base `WordTokenizer`) and ask the
/// speller's `isMisspelled`.
fn is_misspelled(env: &Env, text: &str) -> bool {
    let tokens = lt_tokenize::DutchWordTokenizer.tokenize(text);
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

struct DutchMultitokenSpellerFilter {
    env: Env,
}

impl RuleFilter for DutchMultitokenSpellerFilter {
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
// IsEnglishWordFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.IsEnglishWordFilter`: the pinned Dutch module
/// only depends on `languagetool-core` + `dutch-pos-dict`, so Java's
/// `Languages.getLanguageForShortCode("en-US")` is null and the filter's
/// constructor leaves `tagger == null` — every match is rejected, i.e. the
/// `IGNORE_ENGLISH_WORDS` disambiguation rules never add
/// `_english_ignore_` for Dutch.
struct DutchIsEnglishWordFilter;

impl RuleFilter for DutchIsEnglishWordFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }
}

// ---------------------------------------------------------------------------
// DutchSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

struct DutchSuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl RuleFilter for DutchSuppressMisspelledSuggestionsFilter {
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
            if is_misspelled_multiword(&self.env, &replacement.value) {
                continue;
            }
            let mut add = true;
            if suppress_postag.is_some() || filter_postag.is_some() {
                let readings = self.env.tagger.tag_word(&replacement.value);
                let atr = lt_core::AnalyzedTokenReadings {
                    readings,
                    chunk_tags: Vec::new(),
                    whitespace_before: false,
                    start_pos: 0,
                    raw_byte_len: replacement.value.len(),
                    is_whitespace: false,
                    is_sentence_start: false,
                    is_sentence_end: false,
                    is_paragraph_end: false,
                    is_tagged: true,
                    is_immunized: false,
                    is_ignore_spelling: false,
                    has_typographic_apostrophe: false,
                    is_pos_tag_unknown: false,
                };
                let matches = |pattern: &str| -> bool {
                    let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pattern})$")) else {
                        return false;
                    };
                    atr.readings.iter().any(|r| {
                        r.pos_tag
                            .as_deref()
                            .map(|t| re.is_match(t).unwrap_or(false))
                            .unwrap_or(false)
                    })
                };
                if let Some(p) = suppress_postag {
                    if matches(p) {
                        add = false;
                    }
                }
                if add {
                    if let Some(p) = filter_postag {
                        if !matches(p) {
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
// DutchNumberInWordFilter
// ---------------------------------------------------------------------------

struct DutchNumberInWordFilter {
    env: Env,
}

impl RuleFilter for DutchNumberInWordFilter {
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
                suggestions: Some(suggestions_values(&replacements)),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CompoundFilter
// ---------------------------------------------------------------------------

/// `CompoundFilter`: glue the `word1`…`word5` arguments, replace the
/// `<suggestion>` element of the message and use the glued form as the only
/// suggestion.
struct DutchCompoundFilter;

impl RuleFilter for DutchCompoundFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let mut words: Vec<&str> = Vec::new();
        for i in 1..6 {
            let Some(arg) = ctx.args.get(&format!("word{i}")) else {
                break;
            };
            words.push(arg.as_str());
        }
        let repl = crate::nl::tools::glue_parts(&words);
        let replace_suggestion = |text: &str| -> String {
            static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
                regex::Regex::new("<suggestion>.*?</suggestion>").unwrap()
            });
            RE.replace_all(text, format!("<suggestion>{repl}</suggestion>"))
                .into_owned()
        };
        let message = replace_suggestion(&ctx.message);
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: Some(vec![Suggestion {
                value: repl,
                short_description: None,
            }]),
        }
    }
}

/// Dutch filter registry: the stage-2 `MultitokenSpellerFilter` plus the
pub fn dutch_filter_registry(
    today: Ymd,
    tagger: Arc<lt_tagger::DutchTagger>,
    spelling: Arc<crate::nl::spelling::DutchSpellingRule>,
    multitoken: Arc<crate::multitoken::MultitokenSpeller>,
) -> FilterRegistry {
    let env: Env = Arc::new(NlFilterEnv {
        tagger,
        spelling,
        multitoken,
    });
    FilterRegistry::builder()
        .register(
            "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter",
            Arc::new(DutchMultitokenSpellerFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.IsEnglishWordFilter",
            Arc::new(DutchIsEnglishWordFilter),
        )
        .register(
            "org.languagetool.rules.nl.DutchSuppressMisspelledSuggestionsFilter",
            Arc::new(DutchSuppressMisspelledSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.nl.DutchNumberInWordFilter",
            Arc::new(DutchNumberInWordFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.nl.CompoundFilter",
            Arc::new(DutchCompoundFilter),
        )
        .register(
            "org.languagetool.rules.nl.DateCheckFilter",
            Arc::new(crate::nl::date_filters::DateCheckFilter { today }),
        )
        .build()
}
