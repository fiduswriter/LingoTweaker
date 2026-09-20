//! Catalan rule filters (internal development notes).
//!
//! All XML-referenced filter classes are wired (stage 2 for the
//! `MultitokenSpellerFilter`; stages 3a–3p for the rest). The target list,
//! by reference count:
//!
//! - `org.languagetool.rules.AdaptSuggestionsFilter` (197)
//! - `org.languagetool.rules.ca.AdjustVerbSuggestionsFilter` (95 incl. the
//!   ca-ES-valencia file)
//! - `org.languagetool.rules.ca.CatalanSuppressMisspelledSuggestionsFilter` (88)
//! - `org.languagetool.rules.ca.ConvertToGenderAndNumberFilter` (82)
//! - `org.languagetool.rules.ca.AdvancedSynthesizerFilter` (67)
//! - `org.languagetool.rules.ca.FindSuggestionsFilter` (59)
//! - `org.languagetool.rules.ca.DiacriticsCheckFilter` (47)
//! - `org.languagetool.rules.ca.AdjustPronounsFilter` (46)
//! - `org.languagetool.rules.AddCommasFilter` (32)
//! - `org.languagetool.rules.ca.SynthesizeWithDAFilter` (18)
//! - `org.languagetool.rules.ca.CatalanRemoteRewriteFilter` (10)
//! - `org.languagetool.rules.ca.OblidarseSugestionsFilter` (7)
//! - `org.languagetool.rules.ca.DateCheckFilter` (4)
//! - `org.languagetool.rules.SuppressIfAnyRuleMatchesFilter` (4)
//! - `org.languagetool.rules.ca.PossessiusRedundantsFilter` (4)
//! - `org.languagetool.rules.ca.EnNoInfinitiuSuggestionFilter` (4)
//! - `org.languagetool.rules.ca.SynthesizeWithAnyDeterminerFilter` (3)
//! - `org.languagetool.rules.ca.TextToNumberFilter` (3)
//! - `org.languagetool.rules.ca.PostponedAdjectiveConcordanceFilter` (2)
//! - `org.languagetool.rules.CheckPostagsInSuggestionFilter` (2)
//! - `org.languagetool.rules.ca.QueIniciFilter` (2)
//! - `org.languagetool.rules.patterns.ApostropheTypeFilter` (2)
//! - `org.languagetool.rules.ca.DonarTempsSuggestionsFilter` (2)
//! - `org.languagetool.rules.ca.DonarseliBeFilter` (2)
//! - `org.languagetool.rules.ca.AnarASuggestionsFilter` (2)
//! - `org.languagetool.rules.ca.PortarGerundiSuggestionsFilter` (2)
//! - `org.languagetool.rules.ca.PortarTempsSuggestionsFilter` (2)
//! - `org.languagetool.rules.ca.CatalanNumberSpellerFilter` (2)
//! - `org.languagetool.rules.ca.NewYearDateFilter` (1)
//! - `org.languagetool.rules.ca.CatalanNumberInWordFilter` (1)
//! - `org.languagetool.rules.ca.FindSuggestionsEsFilter` (1)
//! - `org.languagetool.rules.WhitespaceCheckFilter` (1)
//! - `org.languagetool.rules.ConvertToSentenceCaseFilter` (1)
//! - `org.languagetool.rules.ca.RemoveSuggestionsFilter` (1)

use std::sync::Arc;

use lt_core::{Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::ca::diacritics::DiacriticsCheckFilter;
use crate::dates::Ymd;
use crate::wordutil::is_punctuation_mark;

/// `org.languagetool.rules.IsEnglishWordFilter`: the pinned Catalan module
/// only depends on `languagetool-core`, so Java's
/// `Languages.getLanguageForShortCode("en-US")` throws
/// (`'en-US' is not a language code known to LanguageTool`), the filter
/// constructor leaves `tagger == null` and every match is rejected — the 21
/// `IGNORE_ENGLISH_WORDS` disambiguation rules never add `_english_ignore_`
/// (so the speller still reports English words inside Catalan sentences).
struct IsEnglishWordFilter;

impl RuleFilter for IsEnglishWordFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }
}

/// `MultitokenSpellerFilter.isMisspelled`: tokenize with
/// `CatalanWordTokenizer` and ask the speller's `isMisspelled`.
fn is_misspelled(env: &Env, text: &str) -> bool {
    let is_tagged = |w: &str| env.tagger.is_tagged_word(w);
    let tokens = lt_tokenize::CatalanWordTokenizer::new(&is_tagged).tokenize(text);
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

pub struct CaFilterEnv {
    /// Requested variant (`ca-ES`, `ca-ES-balear`, `ca-ES-valencia`); selects
    /// the `eixir`/`sortir` lemma in `DonarseliBeFilter` and similar.
    pub(crate) variant: String,
    /// The Catalan tagger (stage-3 filters and `CatalanWordTokenizer`).
    pub(crate) tagger: Arc<lt_tagger::CatalanTagger>,
    /// `VerbClassifier` over `ca/words/verbs_classification.txt`
    /// (`QueIniciFilter` transitivity checks).
    pub(crate) verb_classifier: Arc<crate::ca::helpers::VerbClassifier>,
    /// The synthesizer through the pattern engine's trait
    /// (`AdvancedSynthesizerFilter` and friends).
    pub(crate) synth: Arc<crate::ca::CatalanSynthesizerAdapter>,
    /// `MorfologikCatalanSpellerRule` (the default spelling rule)
    pub(crate) spelling: Arc<crate::ca::spelling::CatalanSpellingRule>,
    /// `CatalanMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub(crate) multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// Back-reference to the pipeline for the stage-3 filters that need the
    /// full analyze path (`createDefaultJLanguageTool().analyzeText`:
    /// `CatalanSuppressMisspelledSuggestionsFilter`,
    /// `AdjustVerbSuggestionsFilter.numberFromNextWords`). Set by
    /// `Pipeline::new_catalan` after the pipeline is assembled (weak, so the
    /// engine's Arc cycle is broken).
    pub(crate) pipeline: std::sync::OnceLock<std::sync::Weak<crate::ca::CatalanPipeline>>,
    /// Back-reference to the assembled `Pipeline` for
    /// `SuppressIfAnyRuleMatchesFilter` (`JLanguageTool` rule matching). Set
    /// by `EngineBuilder::build` after the pipeline is wrapped in its `Arc`.
    pub(crate) outer_pipeline: std::sync::OnceLock<std::sync::Weak<crate::pipeline::Pipeline>>,
}

impl CaFilterEnv {
    /// `JLanguageTool.analyzeText(...)` of one suggestion string through the
    /// full Catalan pipeline (tokenizer, tagger, chunkers, disambiguation).
    /// Returns `None` when the pipeline back-reference is not wired yet.
    pub(crate) fn analyze_text(&self, text: &str) -> Option<lt_core::AnalyzedSentence> {
        let pipeline = self.pipeline.get()?.upgrade()?;
        let mut sentence = crate::ca::analyze_catalan_sentence(&pipeline, text);
        pipeline.disambiguate(&mut sentence);
        Some(sentence)
    }
}

pub type Env = Arc<CaFilterEnv>;

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

/// `org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter`.
struct CatalanMultitokenSpellerFilter {
    env: Env,
}

impl RuleFilter for CatalanMultitokenSpellerFilter {
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

fn is_punctuation_or_symbol(s: &str) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{P}\p{S}']$").unwrap());
    RE.is_match(s)
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
// CatalanSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.CatalanSuppressMisspelledSuggestionsFilter`
/// (`AbstractSuppressMisspelledSuggestionsFilter` with the full analyze path:
/// a suggestion is dropped when any token of its analyzed sentence carries
/// the `_incorrect_verb_` chunk tag, or when the Catalan speller matches it).
struct CatalanSuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl CatalanSuppressMisspelledSuggestionsFilter {
    /// `CatalanSuppressMisspelledSuggestionsFilter.isMisspelled`.
    fn is_misspelled(&self, s: &str) -> bool {
        let Some(sentence) = self.env.analyze_text(s) else {
            // Java always has the pipeline; without the back-reference keep
            // the suggestion (conservative).
            return false;
        };
        let has_incorrect_verb = sentence
            .tokens
            .iter()
            .any(|t| t.chunk_tags.iter().any(|c| c == "_incorrect_verb_"));
        has_incorrect_verb
            || !self
                .env
                .spelling
                .check_sentence(&sentence.tokens, 0)
                .is_empty()
    }
}

impl RuleFilter for CatalanSuppressMisspelledSuggestionsFilter {
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
            if self.is_misspelled(&replacement.value) {
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

/// Catalan filter registry (stage 2: the `MultitokenSpellerFilter`; the
/// remaining 35 XML-referenced classes are stage 3). Returns the registry
/// plus the shared environment so the caller can set the pipeline
/// back-reference after assembling it.
#[allow(clippy::too_many_arguments)] // one parameter per pipeline component
pub fn catalan_filter_registry(
    today: Ymd,
    variant: &str,
    tagger: Arc<lt_tagger::CatalanTagger>,
    verb_classifier: Arc<crate::ca::helpers::VerbClassifier>,
    synth: Arc<crate::ca::CatalanSynthesizerAdapter>,
    spelling: Arc<crate::ca::spelling::CatalanSpellingRule>,
    multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    diacritics: Arc<DiacriticsCheckFilter>,
) -> (FilterRegistry, Env) {
    let env: Env = Arc::new(CaFilterEnv {
        variant: variant.to_string(),
        tagger,
        verb_classifier,
        synth,
        spelling,
        multitoken,
        pipeline: std::sync::OnceLock::new(),
        outer_pipeline: std::sync::OnceLock::new(),
    });
    let registry = FilterRegistry::builder()
        .register(
            "org.languagetool.rules.IsEnglishWordFilter",
            Arc::new(IsEnglishWordFilter),
        )
        .register(
            "org.languagetool.rules.AdaptSuggestionsFilter",
            Arc::new(crate::ca::adapt::AdaptSuggestionsFilter),
        )
        .register(
            "org.languagetool.rules.AddCommasFilter",
            Arc::new(AddCommasFilter),
        )
        .register(
            "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter",
            Arc::new(CatalanMultitokenSpellerFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.DiacriticsCheckFilter",
            diacritics as Arc<dyn RuleFilter>,
        )
        .register(
            "org.languagetool.rules.ca.AdjustVerbSuggestionsFilter",
            Arc::new(crate::ca::verb_filters::AdjustVerbSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.AdjustPronounsFilter",
            Arc::new(crate::ca::verb_filters::AdjustPronounsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.CatalanSuppressMisspelledSuggestionsFilter",
            Arc::new(CatalanSuppressMisspelledSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.ConvertToGenderAndNumberFilter",
            Arc::new(crate::ca::gender_number::ConvertToGenderAndNumberFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.AdvancedSynthesizerFilter",
            Arc::new(crate::ca::synth_filters::AdvancedSynthesizerFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.FindSuggestionsFilter",
            Arc::new(crate::ca::synth_filters::FindSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.CheckPostagsInSuggestionFilter",
            Arc::new(crate::ca::small_filters::CheckPostagsInSuggestionFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ConvertToSentenceCaseFilter",
            Arc::new(crate::ca::small_filters::ConvertToSentenceCaseFilter),
        )
        .register(
            "org.languagetool.rules.SuppressIfAnyRuleMatchesFilter",
            Arc::new(crate::ca::small_filters::CaSuppressIfAnyRuleMatchesFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.RemoveSuggestionsFilter",
            Arc::new(crate::ca::small_filters::CaRemoveSuggestionsFilter),
        )
        .register(
            "org.languagetool.rules.ca.CatalanNumberSpellerFilter",
            Arc::new(crate::ca::small_filters::CatalanNumberSpellerFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.CatalanNumberInWordFilter",
            Arc::new(crate::ca::small_filters::CatalanNumberInWordFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.TextToNumberFilter",
            Arc::new(crate::ca::small_filters::TextToNumberFilter),
        )
        .register(
            "org.languagetool.rules.ca.FindSuggestionsEsFilter",
            Arc::new(crate::ca::small_filters::FindSuggestionsEsFilter {
                env: Arc::clone(&env),
                base: crate::ca::synth_filters::FindSuggestionsFilter {
                    env: Arc::clone(&env),
                },
            }),
        )
        .register(
            "org.languagetool.rules.ca.SynthesizeWithDAFilter",
            Arc::new(crate::ca::small_filters::SynthesizeWithDAFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.SynthesizeWithAnyDeterminerFilter",
            Arc::new(
                crate::ca::small_filters::SynthesizeWithAnyDeterminerFilter {
                    env: Arc::clone(&env),
                },
            ),
        )
        .register(
            "org.languagetool.rules.ca.EnNoInfinitiuSuggestionFilter",
            Arc::new(crate::ca::small_filters::EnNoInfinitiuSuggestionFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.PossessiusRedundantsFilter",
            Arc::new(crate::ca::small_filters::PossessiusRedundantsFilter),
        )
        .register(
            "org.languagetool.rules.ca.OblidarseSugestionsFilter",
            Arc::new(crate::ca::small_filters::OblidarseSugestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.CatalanRemoteRewriteFilter",
            Arc::new(crate::ca::remote::CatalanRemoteRewriteFilter),
        )
        .register(
            "org.languagetool.rules.ca.DonarTempsSuggestionsFilter",
            Arc::new(crate::ca::small_filters::DonarTempsSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.AnarASuggestionsFilter",
            Arc::new(crate::ca::small_filters::AnarASuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.PortarGerundiSuggestionsFilter",
            Arc::new(crate::ca::small_filters::PortarGerundiSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.PortarTempsSuggestionsFilter",
            Arc::new(crate::ca::small_filters::PortarTempsSuggestionsFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.DonarseliBeFilter",
            Arc::new(crate::ca::donarseli::DonarseliBeFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.PostponedAdjectiveConcordanceFilter",
            Arc::new(crate::ca::postponed::PostponedAdjectiveConcordanceFilter {
                env: Arc::clone(&env),
            }),
        )
        .register(
            "org.languagetool.rules.ca.DateCheckFilter",
            Arc::new(crate::ca::date_filters::DateCheckFilter { today }),
        )
        .register(
            "org.languagetool.rules.ca.NewYearDateFilter",
            Arc::new(crate::ca::date_filters::NewYearDateFilter { today }),
        )
        .register(
            "org.languagetool.rules.ca.QueIniciFilter",
            Arc::new(crate::ca::que_inici::QueIniciFilter {
                env: Arc::clone(&env),
            }),
        );
    let registry = registry.build();
    (registry, env)
}
