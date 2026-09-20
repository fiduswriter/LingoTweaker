//! English spelling rules (P2.1, D-009): port of `MorfologikAmericanSpellerRule`
//! / `MorfologikBritishSpellerRule` via `AbstractEnglishSpellerRule` and
//! `MorfologikSpellerRule`.
//!
//! Variant behavior follows the LT server (`TextChecker`): plain `en`
//! resolves to `en-US`, so spelling always runs with a concrete variant's
//! dictionary. en-US → `MORFOLOGIK_RULE_EN_US` over `en_US.dict`,
//! en-GB → `MORFOLOGIK_RULE_EN_GB` over `en_GB.dict`.
//!
//! Match pipeline: `MorfologikSpellerRule.match` token loop (incl. the
//! split-word heuristics and sentence-start capitalization) →
//! `AbstractEnglishSpellerRule.getRuleMatches` (variant/irregular-forms
//! messages, `cleanSuggestions`) with the suggestion pipeline from
//! `calcSpellerSuggestions` (only/top/curated suggestions, speller1/2/3,
//! hyphen suggestions, filters).
use lt_data::PathExt as _;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::{is_punctuation_mark, split_compound};
use lt_spell::morfologik::{
    self, DictSource, MorfologikSpeller, MultiSpeller, SpellerMetadata, WeightedSuggestion,
};
use lt_tagger::{DictionaryInfo, EnglishTagger};
use regex::Regex;

use crate::en::spelling_data::{
    CLEAN_LC_PREFIXES, CLEAN_PREFIXES, CLEAN_SUFFIXES, LC_DO_NOT_SUGGEST, ONLY_SUGGESTIONS,
    ONLY_SUGGESTION_PATTERNS, TOP_SUGGESTIONS, TOP_SUGGESTIONS_IGNORE_CASE,
};
use crate::wordutil::{is_email, is_url};

pub const AMERICAN_RULE_ID: &str = "MORFOLOGIK_RULE_EN_US";
pub const BRITISH_RULE_ID: &str = "MORFOLOGIK_RULE_EN_GB";

const MESSAGE: &str = "Possible spelling mistake found.";
const DESCRIPTION: &str = "Possible spelling mistake";
/// `MessagesBundle` `desc_spelling_short`
const SHORT_MESSAGE: &str = "Spelling mistake";
const CATEGORY_ID: &str = "TYPOS";
const CATEGORY_NAME: &str = "Possible Typo";
/// `MorfologikSpellerRule.MAX_FREQUENCY_FOR_SPLITTING`
const MAX_FREQUENCY_FOR_SPLITTING: i32 = 21;
/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;

static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d[.,\d]*|\P{L}+)(.*)$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\p{C}\-\$%&]+)(.*)$").unwrap());
static CONTAINS_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:.* (b|c|d|e|f|g|h|j|k|l|m|n|o|p|q|r|s|t|v|w|y|z|ll|ve))$").unwrap()
});
static ONLY_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    ONLY_SUGGESTION_PATTERNS
        .iter()
        // Java compiles these as patterns and uses `matcher(word).matches()`
        // (full match), so anchor the Rust regexes
        .map(|(name, pattern)| (*name, Regex::new(&format!("^(?:{pattern})$")).unwrap()))
        .collect()
});

pub struct SpellingRule {
    rule_id: String,
    /// `AbstractEnglishSpellerRule.getIrregularFormsOrNull` needs the
    /// synthesizer; set by the pipeline after construction.
    synthesizer: Option<std::sync::Arc<crate::en::synthesizer::EnglishSynthesizer>>,
    speller1: MultiSpeller,
    speller2: MultiSpeller,
    speller3: MultiSpeller,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java)
    ignore: HashSet<String>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    converts_case: bool,
    tagger: Arc<EnglishTagger>,
    /// `en/en-US-GB.txt` mapping of the *other* variant's spelling to this
    /// one (`isValidInOtherVariant`)
    other_variant: HashMap<String, String>,
    variant_name: &'static str,
    /// `MorfologikAmericanSpellerRule.getAdditionalTopSuggestions` extras
    american_extras: bool,
    /// Memoized `calcSpellerSuggestions` results (Java caches the same data
    /// in `MorfologikMultiSpeller`/`MorfologikSpeller`); the error-tolerant
    /// FSA walk is the most expensive part of the spelling rule.
    suggestion_cache:
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Vec<Suggestion>>>>,
}

impl SpellingRule {
    /// American configuration (en-US; also used for plain `en`).
    pub fn american(data_dir: &Path, tagger: Arc<EnglishTagger>) -> Result<Self> {
        Self::build(data_dir, "en_US", tagger, true)
    }

    /// British configuration (en-GB).
    pub fn british(data_dir: &Path, tagger: Arc<EnglishTagger>) -> Result<Self> {
        Self::build(data_dir, "en_GB", tagger, false)
    }

    fn build(
        data_dir: &Path,
        variant: &str,
        tagger: Arc<EnglishTagger>,
        american_extras: bool,
    ) -> Result<Self> {
        let hunspell = data_dir.join("en/hunspell");
        let dict_path = hunspell.join(format!("{variant}.dict"));
        let info_path = hunspell.join(format!("{variant}.info"));
        let info_text = lt_data::fs::read_to_string(&info_path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", info_path.display()))
        })?;
        let info = DictionaryInfo::parse(&info_text)?;
        let meta = SpellerMetadata::from_info(&info)?;

        // one shared source per dictionary: the three edit distances only
        // parameterize the walk, so they do not need private FSA/trie copies
        let binary_source =
            std::sync::Arc::new(DictSource::from_dict_file(&dict_path, &info_path)?);
        let binary = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(
                std::sync::Arc::clone(&binary_source),
                meta.clone(),
                distance,
            )
        };
        let lines = load_plain_text_dict_lines(data_dir, variant);
        let plain_source = std::sync::Arc::new(DictSource::from_lines(&lines));
        let plain = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(
                std::sync::Arc::clone(&plain_source),
                meta.clone(),
                distance,
            )
        };
        let speller1 = MultiSpeller::new(vec![binary(1), plain(1)], vec![0, 1]);
        let speller2 = MultiSpeller::new(vec![binary(2), plain(2)], vec![0, 1]);
        let speller3 = MultiSpeller::new(vec![binary(3), plain(3)], vec![0, 1]);

        let mut rule = Self {
            rule_id: if variant == "en_GB" {
                BRITISH_RULE_ID.to_string()
            } else {
                AMERICAN_RULE_ID.to_string()
            },
            synthesizer: None,
            speller1,
            speller2,
            speller3,
            ignore: HashSet::new(),
            prohibit: HashSet::new(),
            converts_case: true,
            tagger,
            other_variant: if variant == "en_GB" {
                // Java `MorfologikBritishSpellerRule`: key = American form
                load_variant_list(&data_dir.join("en/words/en-US-GB.txt"), 0)
            } else {
                // Java `MorfologikAmericanSpellerRule`: key = British form
                load_variant_list(&data_dir.join("en/words/en-US-GB.txt"), 1)
            },
            variant_name: if variant == "en_GB" {
                "American English"
            } else {
                "British English"
            },
            american_extras,
            suggestion_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        };

        // `SpellingCheckRule.init`: ignore file, spelling file, additional
        // spelling files, language-specific ignore file.
        for path in [
            hunspell.join("ignore.txt"),
            hunspell.join("spelling.txt"),
            hunspell.join("spelling_custom.txt"),
            data_dir.join("core/spelling_global.txt"),
            data_dir.join("en/words/multiwords.txt"),
            hunspell.join(format!(
                "spelling_{}.txt",
                if variant == "en_GB" { "en-GB" } else { "en-US" }
            )),
        ] {
            rule.load_ignore(&path);
        }
        for path in [
            hunspell.join("prohibit.txt"),
            hunspell.join("prohibit_custom.txt"),
        ] {
            rule.load_prohibit(&path);
        }
        Ok(rule)
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Late-bound synthesizer (pipeline builds it after the speller).
    pub(crate) fn set_synthesizer(
        &mut self,
        synthesizer: std::sync::Arc<crate::en::synthesizer::EnglishSynthesizer>,
    ) {
        self.synthesizer = Some(synthesizer);
    }

    fn load_ignore(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            // English `tokenizeNewWords()` is false: keep multi-token lines
            // as one entry (they can never match a single token).
            self.ignore.insert(word);
        }
    }

    fn load_prohibit(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            self.prohibit.insert(word);
        }
    }

    /// `MorfologikSpellerRule.isMisspelled(speller1, word)` incl. the
    /// hyphen-compound check (`setCheckCompound(true)`, `compoundRegex "-"`).
    pub(crate) fn is_misspelled(&self, word: &str) -> bool {
        if !self.speller1.is_misspelled(word) {
            return false;
        }
        if word.contains('-') {
            let parts = split_compound(word);
            for part in parts {
                if self.speller1.is_misspelled(part) {
                    return true;
                }
            }
            return false;
        }
        true
    }

    fn is_prohibited(&self, word: &str) -> bool {
        self.prohibit.contains(word)
    }

    fn is_ignored_no_case(&self, word: &str) -> bool {
        self.ignore.contains(word)
            || (!morfologik::is_mixed_case(word)
                && self.converts_case
                && self.ignore.contains(&word.to_lowercase()))
            || word.chars().count() <= 1
    }

    /// `SpellingCheckRule.ignoreWord(String)`.
    fn ignore_word(&self, word: &str) -> bool {
        if word.chars().count() > MAX_TOKEN_LENGTH {
            return true;
        }
        if HAS_NO_LETTER.is_match(word) {
            return true;
        }
        if let Some(stripped) = word.strip_suffix('.') {
            if !self.ignore.contains(word) {
                return self.is_ignored_no_case(stripped);
            }
        }
        self.is_ignored_no_case(word)
    }

    /// `MorfologikSpellerRule.canBeIgnored`.
    fn can_be_ignored(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        idx: usize,
        token: &AnalyzedTokenReadings,
    ) -> bool {
        token.is_sentence_start
            || token.is_immunized
            || token.is_ignore_spelling
            || is_url(token.surface())
            || is_email(token.surface())
            || self.ignore_word(tokens[idx].surface())
    }

    /// `MorfologikSpellerRule.match` over one sentence's token stream
    /// (absolute byte offsets already applied by the caller).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        // non-whitespace view, like LT's getTokensWithoutWhitespace
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut matches: Vec<Match> = Vec::new();
        let mut is_first_word = true;
        for (idx, token) in non_blank.iter().enumerate() {
            if self.can_be_ignored(&non_blank, idx, token) {
                if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                    is_first_word = false;
                }
                continue;
            }
            let start_pos = token.start_pos;
            let word = token.surface().to_string();
            let new_matches =
                self.get_rule_matches(&word, start_pos, &mut matches, idx, &non_blank);
            matches.extend(new_matches);

            // Capitalize the (first) match's suggestions when the word is the
            // sentence's first word and not its last token.
            if is_first_word && !matches.is_empty() && idx < non_blank.len() - 1 {
                let values: Vec<String> = matches[0]
                    .suggestions
                    .iter()
                    .map(|s| s.value.clone())
                    .collect();
                let mut new_values: Vec<String> = Vec::new();
                for replacement in values {
                    if replacement == replacement.to_lowercase() {
                        let capitalized = morfologik::uppercase_first_char(&replacement);
                        if !new_values.contains(&capitalized) {
                            new_values.push(capitalized);
                        }
                    } else if !new_values.contains(&replacement) {
                        new_values.push(replacement);
                    }
                }
                matches[0].suggestions = new_values
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect();
            }
            if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                is_first_word = false;
            }
        }
        let mut out = Vec::with_capacity(matches.len());
        for mut m in matches {
            m.range = TextRange::new(
                sentence_offset + m.range.start,
                sentence_offset + m.range.end,
            );
            out.push(m);
        }
        out
    }

    /// `MorfologikSpellerRule.getRuleMatches` with the English overrides of
    /// `AbstractEnglishSpellerRule.getRuleMatches`.
    fn get_rule_matches(
        &self,
        word: &str,
        start_pos: usize,
        rule_matches_so_far: &mut Vec<Match>,
        idx: usize,
        tokens: &[&AnalyzedTokenReadings],
    ) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        let mut rule_match: Option<Match> = None;

        if !self.is_misspelled(word) && !self.is_prohibited(word) {
            return rule_matches;
        }
        if rule_matches_so_far
            .last()
            .is_some_and(|m| m.range.end > start_pos)
        {
            return rule_matches;
        }

        let mut before_suggestion_str = String::new();
        let mut after_suggestion_str = String::new();

        // Check for split word with previous word
        if idx > 0 && tokens[idx].whitespace_before {
            let prev_word = tokens[idx - 1].surface().to_string();
            if !prev_word.is_empty()
                && !prev_word.chars().any(|c| c.is_ascii_digit())
                && self.speller1.get_frequency(&prev_word) < MAX_FREQUENCY_FOR_SPLITTING
            {
                let prev_start_pos = tokens[idx - 1].start_pos;
                // "thanky ou" -> "thank you"
                if let Some((prev_head, prev_tail)) = split_last_char(&prev_word) {
                    let sugg1a = prev_head.to_string();
                    let sugg1b = format!("{prev_tail}{word}");
                    if sugg1a.chars().count() > 1
                        && sugg1b.chars().count() > 2
                        && !self.is_misspelled(&sugg1a)
                        && !self.is_misspelled(&sugg1b)
                        && self.speller1.get_frequency(&sugg1a)
                            + self.speller1.get_frequency(&sugg1b)
                            > self.speller1.get_frequency(&prev_word)
                    {
                        rule_match = Some(self.create_wrong_split_match(
                            rule_matches_so_far,
                            tokens[idx].end_pos(),
                            &sugg1a,
                            &sugg1b,
                            prev_start_pos,
                        ));
                        before_suggestion_str = format!("{prev_word} ");
                    }
                }
                // "than kyou" -> "thank you" ; but not "She awaked" -> "Shea waked"
                {
                    let first_char: String = word.chars().take(1).collect();
                    let sugg2a = format!("{prev_word}{first_char}");
                    let sugg2b: String = word.chars().skip(1).collect();
                    if sugg2b.chars().count() > 2
                        && !self.is_misspelled(&sugg2a)
                        && !self.is_misspelled(&sugg2b)
                    {
                        if rule_match.is_none() {
                            if self.speller1.get_frequency(&sugg2a)
                                + self.speller1.get_frequency(&sugg2b)
                                > self.speller1.get_frequency(&prev_word)
                            {
                                rule_match = Some(self.create_wrong_split_match(
                                    rule_matches_so_far,
                                    tokens[idx].end_pos(),
                                    &sugg2a,
                                    &sugg2b,
                                    prev_start_pos,
                                ));
                                before_suggestion_str = format!("{prev_word} ");
                            }
                        } else if let Some(rm) = &mut rule_match {
                            rm.suggestions.push(Suggestion {
                                value: format!("{sugg2a} {sugg2b}").trim().to_string(),
                                short_description: None,
                            });
                        }
                    }
                }
                // "g oing" -> "going"
                let sugg = format!("{prev_word}{word}");
                if word == word.to_lowercase() && !self.is_misspelled(&sugg) {
                    if rule_match.is_none() {
                        if self.speller1.get_frequency(&sugg)
                            >= self.speller1.get_frequency(&prev_word)
                        {
                            let mut m = self.new_rule_match(
                                prev_start_pos,
                                tokens[idx].end_pos(),
                                MESSAGE,
                                SHORT_MESSAGE,
                            );
                            before_suggestion_str = format!("{prev_word} ");
                            m.suggestions.push(Suggestion {
                                value: sugg.clone(),
                                short_description: None,
                            });
                            rule_match = Some(m);
                        }
                    } else if let Some(rm) = &mut rule_match {
                        rm.suggestions.push(Suggestion {
                            value: sugg.clone(),
                            short_description: None,
                        });
                    }
                }
                if rule_match.is_some() && self.is_misspelled(&prev_word) {
                    rule_matches.push(rule_match.take().unwrap());
                    return rule_matches;
                }
            }
        }

        // Check for split word with next word
        if rule_match.is_none() && idx < tokens.len() - 1 && tokens[idx + 1].whitespace_before {
            let next_word = tokens[idx + 1].surface().to_string();
            if !next_word.is_empty()
                && !next_word.chars().any(|c| c.is_ascii_digit())
                && self.speller1.get_frequency(&next_word) < MAX_FREQUENCY_FOR_SPLITTING
            {
                if let Some((word_head, word_tail)) = split_last_char(word) {
                    let sugg1a = word_head.to_string();
                    let sugg1b = format!("{word_tail}{next_word}");
                    if sugg1a.chars().count() > 1
                        && sugg1b.chars().count() > 2
                        && !self.is_misspelled(&sugg1a)
                        && !self.is_misspelled(&sugg1b)
                        && self.speller1.get_frequency(&sugg1a)
                            + self.speller1.get_frequency(&sugg1b)
                            > self.speller1.get_frequency(&next_word)
                    {
                        rule_match = Some(self.create_wrong_split_match_next(
                            rule_matches_so_far,
                            tokens[idx + 1].end_pos(),
                            &sugg1a,
                            &sugg1b,
                            start_pos,
                        ));
                        after_suggestion_str = format!(" {next_word}");
                    }
                }
                {
                    let first_char: String = next_word.chars().take(1).collect();
                    let sugg2a = format!("{word}{first_char}");
                    let sugg2b: String = next_word.chars().skip(1).collect();
                    if sugg2b.chars().count() > 2
                        && !self.is_misspelled(&sugg2a)
                        && !self.is_misspelled(&sugg2b)
                    {
                        if rule_match.is_none() {
                            if self.speller1.get_frequency(&sugg2a)
                                + self.speller1.get_frequency(&sugg2b)
                                > self.speller1.get_frequency(&next_word)
                            {
                                rule_match = Some(self.create_wrong_split_match_next(
                                    rule_matches_so_far,
                                    tokens[idx + 1].end_pos(),
                                    &sugg2a,
                                    &sugg2b,
                                    start_pos,
                                ));
                                after_suggestion_str = format!(" {next_word}");
                            }
                        } else if let Some(rm) = &mut rule_match {
                            rm.suggestions.push(Suggestion {
                                value: format!("{sugg2a} {sugg2b}").trim().to_string(),
                                short_description: None,
                            });
                        }
                    }
                }
                let sugg = format!("{word}{next_word}");
                if next_word == next_word.to_lowercase() && !self.is_misspelled(&sugg) {
                    if rule_match.is_none() {
                        if self.speller1.get_frequency(&sugg)
                            >= self.speller1.get_frequency(&next_word)
                        {
                            let mut m = self.new_rule_match(
                                start_pos,
                                tokens[idx + 1].end_pos(),
                                MESSAGE,
                                SHORT_MESSAGE,
                            );
                            after_suggestion_str = format!(" {next_word}");
                            m.suggestions.push(Suggestion {
                                value: sugg.clone(),
                                short_description: None,
                            });
                            rule_match = Some(m);
                        }
                    } else if let Some(rm) = &mut rule_match {
                        rm.suggestions.push(Suggestion {
                            value: sugg.clone(),
                            short_description: None,
                        });
                    }
                }
                if rule_match.is_some() && self.is_misspelled(&next_word) {
                    rule_matches.push(rule_match.take().unwrap());
                    return rule_matches;
                }
            }
        }

        let mut prevent_further_suggestions = false;
        let mut clean_word = word.to_string();

        if rule_match.is_none() {
            rule_match =
                Some(self.new_rule_match(start_pos, tokens[idx].end_pos(), MESSAGE, SHORT_MESSAGE));
        }

        // word starting with numbers or bullets
        if let Some(caps) = STARTS_WITH_NUMBERS_BULLETS.captures(word) {
            if !STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS.is_match(word) {
                let first_part = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let second_part = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let callback = |w: &str| self.tagger.is_tagged(w);
                let tokenizer = lt_tokenize::EnglishWordTokenizer::new(&callback);
                let second_part_tokens = tokenizer.tokenize(second_part);
                let multitoken_is_misspelled =
                    second_part_tokens.iter().any(|s| self.is_misspelled(s));
                if (!multitoken_is_misspelled || self.is_ignored_no_case(second_part))
                    && !self.is_prohibited(second_part)
                {
                    if let Some(rm) = &mut rule_match {
                        rm.suggestions.push(Suggestion {
                            value: format!("{first_part} {second_part}"),
                            short_description: None,
                        });
                    }
                    prevent_further_suggestions = true;
                } else {
                    before_suggestion_str = format!("{first_part} ");
                    clean_word = second_part.to_string();
                }
            }
        }

        let prev_suggestions = rule_match
            .as_ref()
            .map(|m| m.suggestions.clone())
            .unwrap_or_default();
        if !prevent_further_suggestions {
            let mut joined = Vec::new();
            let cached = self.calc_speller_suggestions_cached(&clean_word);
            for suggestion in cached.iter() {
                joined.push(Suggestion {
                    value: format!(
                        "{before_suggestion_str}{}{after_suggestion_str}",
                        suggestion.value
                    ),
                    short_description: suggestion.short_description.clone(),
                });
            }
            let mut all = prev_suggestions;
            all.extend(joined);
            if let Some(rm) = &mut rule_match {
                rm.suggestions = all;
            }
        }

        // AbstractEnglishSpellerRule.getRuleMatches: irregular forms first,
        // then the other-variant check, then cleanSuggestions.
        if let Some(rm) = rule_match {
            let mut rule_match = rm;
            if let Some((base, pos_name, form_name, forms)) = self.irregular_forms_or_null(word) {
                let message = format!(
                    "Possible spelling mistake. Did you mean <suggestion>{}</suggestion>, \
                     the {form_name} form of the {pos_name} '{base}'?",
                    forms[0]
                );
                rule_match.message = message;
                rule_match.short_message = None;
                let mut all: Vec<Suggestion> = forms
                    .iter()
                    .map(|f| Suggestion {
                        value: f.clone(),
                        short_description: None,
                    })
                    .collect();
                for suggestion in rule_match.suggestions.drain(..) {
                    if !all.iter().any(|s| s.value == suggestion.value) {
                        all.push(suggestion);
                    }
                }
                rule_match.suggestions = all;
            } else if let Some(variant) = self.other_variant.get(&word.to_lowercase()) {
                let message = format!(
                    "Possible spelling mistake. '{word}' is {}.",
                    self.variant_name
                );
                let suggestion = if word.chars().next().is_some_and(char::is_uppercase) {
                    morfologik::uppercase_first_char(variant)
                } else {
                    variant.clone()
                };
                rule_match.message = message;
                rule_match.suggestions = vec![Suggestion {
                    value: suggestion,
                    short_description: Some("English".to_string()),
                }];
            }
            rule_match.suggestions = clean_suggestions(&rule_match.suggestions);
            rule_matches.push(rule_match);
        }
        rule_matches
    }

    /// `AbstractEnglishSpellerRule.getIrregularFormsOrNull`: an inflected
    /// form of a known base whose regular suffix was wrongly appended.
    #[allow(clippy::type_complexity)]
    fn irregular_forms_or_null(
        &self,
        word: &str,
    ) -> Option<(String, &'static str, &'static str, Vec<String>)> {
        use lt_pattern::Synthesizer as _;
        let synth = self.synthesizer.as_ref()?;
        let configs: [(&str, &str, &str, &str, &str); 6] = [
            ("ed", "ed", "VBD", "verb", "past tense"),
            ("ed", "d", "VBD", "verb", "past tense"),
            ("s", "s", "NNS", "noun", "plural"),
            ("es", "es", "NNS", "noun", "plural"),
            ("er", "er", "JJR", "adjective", "comparative"),
            ("est", "est", "JJS", "adjective", "superlative"),
        ];
        for (word_suffix, suffix, pos_tag, pos_name, form_name) in configs {
            if word.ends_with(word_suffix) {
                let base = word[..word.len() - suffix.len()].to_string();
                let token = lt_core::AnalyzedToken::new(word.to_string(), Some(base.clone()), None);
                let forms = synth.synthesize(&token, pos_tag, false);
                let mut result: Vec<String> = forms
                    .into_iter()
                    .filter(|form| !self.is_misspelled(form))
                    .collect();
                result.retain(|form| {
                    form != word && form != "badder" && form != "baddest" && form != "spake"
                });
                if !result.is_empty() {
                    return Some((base, pos_name, form_name, result));
                }
            }
        }
        None
    }

    /// `SpellingCheckRule.createWrongSplitMatch`.
    fn create_wrong_split_match(
        &self,
        rule_matches_so_far: &mut Vec<Match>,
        end_pos: usize,
        suggestion1: &str,
        suggestion2: &str,
        prev_pos: usize,
    ) -> Match {
        if let Some(prev) = rule_matches_so_far.last() {
            if prev.range.start == prev_pos {
                rule_matches_so_far.pop();
            }
        }
        let mut m = self.new_rule_match(prev_pos, end_pos, MESSAGE, SHORT_MESSAGE);
        m.suggestions.push(Suggestion {
            value: format!("{suggestion1} {suggestion2}").trim().to_string(),
            short_description: None,
        });
        m
    }

    /// `createWrongSplitMatch` for the split-with-next-word case (the first
    /// suggestion lives at the end of the previous span).
    fn create_wrong_split_match_next(
        &self,
        rule_matches_so_far: &mut Vec<Match>,
        end_pos: usize,
        suggestion1: &str,
        suggestion2: &str,
        prev_pos: usize,
    ) -> Match {
        self.create_wrong_split_match(
            rule_matches_so_far,
            end_pos,
            suggestion1,
            suggestion2,
            prev_pos,
        )
    }

    fn new_rule_match(&self, from: usize, to: usize, message: &str, short_message: &str) -> Match {
        Match::new(
            &self.rule_id,
            Option::<String>::None,
            message,
            Some(short_message.to_string()),
            TextRange::new(from, to),
            Vec::new(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "misspelling", 0)
        .with_match_type("UnknownWord")
    }

    // -----------------------------------------------------------------
    // Suggestion pipeline (`MorfologikSpellerRule.calcSpellerSuggestions`)
    // -----------------------------------------------------------------

    /// Memoized `calc_speller_suggestions`: the error-tolerant FSA walk is
    /// expensive and the same word recurs across lines/documents. The cache
    /// is bounded and never influences results (suggestions depend only on
    /// the word and the immutable configuration).
    fn calc_speller_suggestions_cached(&self, word: &str) -> std::sync::Arc<Vec<Suggestion>> {
        if let Ok(cache) = self.suggestion_cache.lock() {
            if let Some(hit) = cache.get(word) {
                return std::sync::Arc::clone(hit);
            }
        }
        let computed = std::sync::Arc::new(self.calc_speller_suggestions(word));
        if let Ok(mut cache) = self.suggestion_cache.lock() {
            if cache.len() >= 50_000 {
                cache.clear();
            }
            cache.insert(word.to_string(), std::sync::Arc::clone(&computed));
        }
        computed
    }

    fn calc_speller_suggestions(&self, word: &str) -> Vec<Suggestion> {
        if let Some(only) = self.only_suggestions(word) {
            return only;
        }
        let mut default_suggestions: Vec<Suggestion> = self
            .speller1
            .get_weighted_suggestions_from_default_dicts(word)
            .into_iter()
            .map(weighted_to_suggestion)
            .collect();
        let user_suggestions: Vec<Suggestion> = Vec::new();
        let only_case_differs = default_suggestions
            .first()
            .is_some_and(|s| s.value.eq_ignore_ascii_case(word));
        let full_results = false;
        if word.chars().count() >= 3
            && (only_case_differs || full_results || default_suggestions.is_empty())
        {
            default_suggestions.extend(
                self.speller2
                    .get_weighted_suggestions_from_default_dicts(word)
                    .into_iter()
                    .map(weighted_to_suggestion),
            );
            if word.chars().count() >= 5 && (full_results || default_suggestions.is_empty()) {
                default_suggestions.extend(
                    self.speller3
                        .get_weighted_suggestions_from_default_dicts(word)
                        .into_iter()
                        .map(weighted_to_suggestion),
                );
            }
        }
        let mut top_suggestions: Vec<Suggestion> = Vec::new();
        if default_suggestions.is_empty() && user_suggestions.is_empty() && word.contains('-') {
            self.add_hyphen_suggestions(&word.split('-').collect::<Vec<_>>(), &mut top_suggestions);
        }
        top_suggestions.extend(self.additional_top_suggestions(&default_suggestions, word));
        default_suggestions.splice(0..0, top_suggestions);
        default_suggestions.extend(self.additional_suggestions(&default_suggestions, word));

        if default_suggestions.is_empty() && user_suggestions.is_empty() {
            return Vec::new();
        }
        let mut default_suggestions = self.filter_suggestions(default_suggestions);
        let user_suggestions = dedupe(user_suggestions);
        // orderSuggestions is the identity for English
        if word.chars().count() > 4 {
            user_suggestions
                .into_iter()
                .chain(default_suggestions.drain(..))
                .collect()
        } else {
            default_suggestions
                .drain(..)
                .chain(user_suggestions)
                .collect()
        }
    }

    /// `AbstractEnglishSpellerRule.getOnlySuggestions`.
    fn only_suggestions(&self, word: &str) -> Option<Vec<Suggestion>> {
        debug_assert_eq!(ONLY_PATTERNS.len(), ONLY_SUGGESTIONS.len());
        for ((name, regex), (spec_name, spec)) in ONLY_PATTERNS.iter().zip(ONLY_SUGGESTIONS.iter())
        {
            debug_assert_eq!(name, spec_name);
            if !regex.is_match(word) {
                continue;
            }
            return Some(match *spec {
                "list:QuillBot" => vec![suggestion("QuillBot's"), suggestion("QuillBot")],
                "list:TV" => vec![suggestion("TV"), suggestion("to")],
                "list:JIST" => vec![suggestion("just"), suggestion("gist")],
                spec if spec.starts_with("replace_once_desc:") => {
                    let mut parts = spec.splitn(4, ':');
                    parts.next();
                    let from = parts.next().unwrap_or("");
                    let to = parts.next().unwrap_or("");
                    let desc = parts.next().unwrap_or("");
                    vec![Suggestion {
                        value: word.replacen(from, to, 1),
                        short_description: Some(desc.to_string()),
                    }]
                }
                spec if spec.starts_with("replace_once:") => {
                    let mut parts = spec.splitn(4, ':');
                    parts.next();
                    let from = parts.next().unwrap_or("");
                    let to = parts.next().unwrap_or("");
                    vec![suggestion(&word.replacen(from, to, 1))]
                }
                spec if spec.starts_with("literal:") => {
                    vec![suggestion(&spec["literal:".len()..])]
                }
                _ => Vec::new(),
            });
        }
        None
    }

    /// `AbstractEnglishSpellerRule.getAdditionalTopSuggestions` (shared
    /// part; the en-US extras are applied for the American rule).
    fn additional_top_suggestions(
        &self,
        suggestions: &[Suggestion],
        word: &str,
    ) -> Vec<Suggestion> {
        if self.american_extras {
            let extra = match word {
                "automize" => Some("automate"),
                "automized" => Some("automated"),
                "automizing" => Some("automating"),
                "automizes" => Some("automates"),
                _ => None,
            };
            if let Some(extra) = extra {
                return vec![suggestion(extra)];
            }
        }
        let mut curated: Vec<String> = Vec::new();
        if let Some((_, values)) = TOP_SUGGESTIONS.iter().find(|(key, _)| *key == word) {
            curated.extend(values.iter().map(|s| s.to_string()));
        }
        let lowercase = word.to_lowercase();
        if let Some((_, values)) = TOP_SUGGESTIONS_IGNORE_CASE
            .iter()
            .find(|(key, _)| *key == lowercase)
        {
            curated.extend(values.iter().map(|s| s.to_string()));
        }
        if !curated.is_empty() {
            return curated.into_iter().map(|s| suggestion(&s)).collect();
        }
        if let Some(stripped) = word.strip_suffix("ys") {
            let candidate = format!("{stripped}ies");
            if !self.is_misspelled(&candidate) {
                return vec![suggestion(&candidate)];
            }
        }
        // `SpellingCheckRule.getAdditionalTopSuggestions`
        let mut more = Vec::new();
        if (word == "Languagetool" || word == "languagetool")
            && !suggestions
                .iter()
                .any(|s| s.value == morfologik::LANGUAGETOOL)
        {
            more.push(morfologik::LANGUAGETOOL.to_string());
        }
        if (word == "Languagetooler" || word == "languagetooler")
            && !suggestions
                .iter()
                .any(|s| s.value == morfologik::LANGUAGETOOLER)
        {
            more.push(morfologik::LANGUAGETOOLER.to_string());
        }
        more.into_iter().map(|s| suggestion(&s)).collect()
    }

    fn additional_suggestions(&self, _suggestions: &[Suggestion], _word: &str) -> Vec<Suggestion> {
        Vec::new()
    }

    /// `AbstractEnglishSpellerRule.addHyphenSuggestions`.
    fn add_hyphen_suggestions(&self, parts: &[&str], top_suggestions: &mut Vec<Suggestion>) {
        for (i, part) in parts.iter().enumerate() {
            if self.is_misspelled(part) {
                let mut part_suggestions: Vec<String> = self
                    .speller1
                    .get_weighted_suggestions_from_default_dicts(part)
                    .into_iter()
                    .map(|s| s.word)
                    .collect();
                if part_suggestions.is_empty() {
                    part_suggestions = self
                        .speller2
                        .get_weighted_suggestions_from_default_dicts(part)
                        .into_iter()
                        .map(|s| s.word)
                        .collect();
                }
                if let Some(first) = part_suggestions.first() {
                    let mut new_parts: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
                    new_parts[i] = first.clone();
                    top_suggestions.push(suggestion(&new_parts.join("-")));
                }
            }
        }
    }

    /// `SpellingCheckRule.filterSuggestions` + the English override.
    fn filter_suggestions(&self, suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
        let suggestions: Vec<Suggestion> = suggestions
            .into_iter()
            .filter(|s| !self.is_prohibited(&s.value))
            .collect();
        let mut new_suggestions: Vec<Suggestion> = Vec::new();
        for suggestion in suggestions {
            let replacement = &suggestion.value;
            let replacement_chars = replacement.chars().count();
            let without_s = if replacement_chars > 3 {
                replacement.chars().take(replacement_chars - 2).collect()
            } else {
                String::new()
            };
            if replacement.ends_with(" s") && self.is_proper_noun(&without_s) {
                // "Michael s" -> "Michael's"
                new_suggestions.insert(
                    0,
                    Suggestion {
                        value: without_s.clone(),
                        short_description: None,
                    },
                );
                new_suggestions.insert(
                    0,
                    Suggestion {
                        value: format!("{without_s}'s"),
                        short_description: None,
                    },
                );
            } else {
                new_suggestions.push(suggestion);
            }
        }
        let new_suggestions = dedupe(new_suggestions);
        let new_suggestions = self.filter_no_suggest_words(new_suggestions);
        // `AbstractEnglishSpellerRule.filterSuggestions` override
        new_suggestions
            .into_iter()
            .filter(|s| !CONTAINS_TOKEN.is_match(&s.value))
            .collect()
    }

    fn filter_no_suggest_words(&self, suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
        suggestions
            .into_iter()
            .filter(|s| !LC_DO_NOT_SUGGEST.contains(&s.value.to_lowercase().as_str()))
            .collect()
    }

    fn is_proper_noun(&self, word: &str) -> bool {
        self.tagger
            .tag_word(word)
            .iter()
            .any(|t| t.pos_tag.as_deref() == Some("NNP"))
    }
}

fn weighted_to_suggestion(s: WeightedSuggestion) -> Suggestion {
    Suggestion {
        value: s.word,
        short_description: None,
    }
}

fn suggestion(value: &str) -> Suggestion {
    Suggestion {
        value: value.to_string(),
        short_description: None,
    }
}

/// `List.distinct()` over suggestions (`equals` = value + short description).
fn dedupe(suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
    let mut seen = Vec::new();
    let mut out = Vec::with_capacity(suggestions.len());
    for s in suggestions {
        if !seen.contains(&s) {
            seen.push(s.clone());
            out.push(s);
        }
    }
    out
}

/// `AbstractEnglishSpellerRule.cleanSuggestions`.
fn clean_suggestions(suggestions: &[Suggestion]) -> Vec<Suggestion> {
    suggestions
        .iter()
        .filter(|s| {
            let rep = &s.value;
            if !rep.contains(' ') {
                return true;
            }
            let rep_lc = rep.to_lowercase();
            !CLEAN_LC_PREFIXES.iter().any(|p| rep_lc.starts_with(p))
                && !CLEAN_PREFIXES.iter().any(|p| rep.starts_with(p))
                && !CLEAN_SUFFIXES.iter().any(|p| rep.ends_with(p))
        })
        .cloned()
        .collect()
}

fn split_last_char(word: &str) -> Option<(&str, &str)> {
    let mut chars = word.char_indices();
    chars
        .next_back()
        .map(|(split, _)| (&word[..split], &word[split..]))
}

/// CachingWordListLoader: skip empty/`#` lines, cut at `#`, trim.
fn cache_word_list(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let trimmed = line.trim();
        let cut = trimmed.split('#').next().unwrap_or("").trim();
        if !cut.is_empty() {
            result.push(cut.to_string());
        }
    }
    result
}

/// Java `AbstractEnglishSpellerRule.loadWordlist`: `us;gb` pairs, keyed by
/// `column` (lowercased) with the other column as value.
fn load_variant_list(path: &Path, column: usize) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() != 2 {
            continue;
        }
        map.insert(parts[column].to_lowercase(), parts[1 - column].to_string());
    }
    map
}

/// Plain-text speller dictionary lines: `spelling.txt`,
/// `spelling_custom.txt`, `spelling_global.txt`, `multiwords.txt`, the
/// variant file, then `LanguageTool` (LT appends the sentinel so it is also
/// suggested).
fn load_plain_text_dict_lines(data_dir: &Path, variant: &str) -> Vec<Vec<u8>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let hunspell = data_dir.join("en/hunspell");
    for path in [
        hunspell.join("spelling.txt"),
        hunspell.join("spelling_custom.txt"),
        data_dir.join("core/spelling_global.txt"),
        data_dir.join("en/words/multiwords.txt"),
    ] {
        if path.lt_exists() {
            paths.push(path);
        }
    }
    let variant_path = hunspell.join(format!(
        "spelling_{}.txt",
        if variant == "en_GB" { "en-GB" } else { "en-US" }
    ));
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for path in &paths {
        lines.extend(read_speller_lines(path));
    }
    if variant_path.lt_exists() {
        lines.extend(read_speller_lines(&variant_path));
    }
    lines.push(morfologik::LANGUAGETOOL.as_bytes().to_vec());
    lines
}

/// LT `MorfologikMultiSpeller.getLines` + English `prepareLineForSpeller`.
fn read_speller_lines(path: &Path) -> Vec<Vec<u8>> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for original_line in text.lines() {
        for line in prepare_line_for_speller(original_line) {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let line = line.split('#').next().unwrap_or("").trim();
            if !line.is_empty() {
                out.push(line.as_bytes().to_vec());
            }
        }
    }
    out
}

/// English `prepareLineForSpeller`.
fn prepare_line_for_speller(line: &str) -> Vec<String> {
    if line.contains('+') {
        // while the morfologik separator is "+", multiwords with '+' can
        // cause undesired results
        return vec![String::new()];
    }
    let parts: Vec<&str> = line.split('#').collect();
    let form_tag: Vec<&str> = parts[0].split('\t').collect();
    let form = form_tag[0].trim();
    if form_tag.len() > 1 {
        let tag = form_tag[1].trim();
        if tag.starts_with("NN") || tag.starts_with("JJ") {
            return vec![form.to_string()];
        }
        return vec![String::new()];
    }
    vec![line.to_string()]
}
