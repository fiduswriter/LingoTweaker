//! Generic `MorfologikSpellerRule` for the languages whose Java speller rule
//! is the plain base class (a binary morfologik dictionary at
//! `<lang>/hunspell/<name>.dict` with no language override beyond the file
//! name/id): Slovak (`MorfologikSlovakSpellerRule`) and Slovenian
//! (`MorfologikSlovenianSpellerRule`).
//!
//! Port of the morfologik path of `MorfologikSpellerRule` +
//! `SpellingCheckRule`, mirroring the Romanian implementation with the
//! per-language strings/paths externalized.

use lt_data::PathExt as _;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::{is_email, is_punctuation_mark, is_url};
use lt_spell::morfologik::{
    self, DictSource, MorfologikSpeller, MultiSpeller, SpellerMetadata, WeightedSuggestion,
};
use lt_tagger::DictionaryInfo;
use regex::Regex;

/// `MorfologikSpellerRule.MAX_FREQUENCY_FOR_SPLITTING`
const MAX_FREQUENCY_FOR_SPLITTING: i32 = 21;
/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;

static HAS_NO_LETTER_LATIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{L}]+$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d[.,\d]*|\P{L}+)(.*)$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\p{C}\-\$%&]+)(.*)$").unwrap());

/// Per-language configuration of the generic rule.
pub struct MorfologikSpellerConfig {
    /// Directory under `data/`, e.g. `sk`.
    pub lang_dir: &'static str,
    /// Binary dictionary stem under `<lang>/hunspell/`, e.g. `sk_SK`.
    pub dict_stem: &'static str,
    pub rule_id: &'static str,
    pub description: &'static str,
    pub message: &'static str,
    pub short_message: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    /// `SpellingCheckRule.isLatinScript()`: false for Greek (any non-Unicode
    /// letter token is ignored instead of only non-Latin ones).
    pub is_latin_script: bool,
    /// `MorfologikSpellerRule.setIgnoreTaggedWords()`: tagged words are not
    /// spell-checked (Breton).
    pub ignore_tagged_words: bool,
    /// `MorfologikSpellerRule.tokenizingPattern()` == `-`: the token is split
    /// at hyphens and every segment is checked separately (Breton).
    pub split_on_hyphen: bool,
}

pub struct MorfologikSpellingRule {
    config: MorfologikSpellerConfig,
    /// the binary dictionary alone (`FindSuggestionsFilter`)
    binary_speller: MorfologikSpeller,
    speller1: MultiSpeller,
    speller2: MultiSpeller,
    speller3: MultiSpeller,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java)
    ignore: HashSet<String>,
    /// `SpellingCheckRule.antiPatterns`: multi-word word-list entries
    /// (`addIgnoreWords` tokenizes the line; >1 token becomes an
    /// `IGNORE_SPELLING` anti-pattern). Keyed by the first phrase token.
    ignore_phrases: std::collections::HashMap<String, Vec<Vec<String>>>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    /// the base class does not call `setIgnoreTaggedWords()`
    ignore_tagged_words: bool,
    suggestion_cache:
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Vec<Suggestion>>>>,
}

impl MorfologikSpellingRule {
    pub fn load(data_dir: &Path, config: MorfologikSpellerConfig) -> Result<Self> {
        let hunspell = data_dir.join(config.lang_dir).join("hunspell");
        let dict_file = hunspell.join(format!("{}.dict", config.dict_stem));
        let info_file = hunspell.join(format!("{}.info", config.dict_stem));
        let info = DictionaryInfo::load(&info_file)?;
        let meta = SpellerMetadata::from_info(&info)?;
        let binary_source = Arc::new(DictSource::from_dict_file(&dict_file, &info_file)?);
        let binary = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(Arc::clone(&binary_source), meta.clone(), distance)
        };
        let lines = load_plain_text_dict_lines(data_dir, config.lang_dir);
        let plain_source = Arc::new(DictSource::from_lines(&lines));
        let plain = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(Arc::clone(&plain_source), meta.clone(), distance)
        };
        let binary_speller = binary(1);
        let speller1 = MultiSpeller::new(vec![binary(1), plain(1)], vec![0, 1]);
        let speller2 = MultiSpeller::new(vec![binary(2), plain(2)], vec![0, 1]);
        let speller3 = MultiSpeller::new(vec![binary(3), plain(3)], vec![0, 1]);

        let ignore_tagged_words = config.ignore_tagged_words;
        let mut rule = Self {
            config,
            binary_speller,
            speller1,
            speller2,
            speller3,
            ignore: HashSet::new(),
            ignore_phrases: std::collections::HashMap::new(),
            prohibit: HashSet::new(),
            ignore_tagged_words,
            suggestion_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        };
        // `SpellingCheckRule.init`: ignore file, spelling file, additional
        // spelling files (the global list), then the prohibit files.
        for path in [
            hunspell.join("ignore.txt"),
            hunspell.join("spelling.txt"),
            hunspell.join("spelling_custom.txt"),
            data_dir.join("core/spelling_global.txt"),
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
        self.config.rule_id
    }

    /// The binary-dictionary speller (`FindSuggestionsFilter`).
    pub fn dict_speller(&self) -> &MorfologikSpeller {
        &self.binary_speller
    }

    /// `MorfologikSpellerRule.getSpellingSuggestions` (one synthetic token).
    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let token = lt_core::AnalyzedTokenReadings::new(vec![lt_core::AnalyzedToken::new(
            word, None, None,
        )]);
        self.check_sentence(&[token], 0)
            .into_iter()
            .next()
            .map(|m| m.suggestions.into_iter().map(|s| s.value).collect())
            .unwrap_or_default()
    }

    fn load_ignore(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            // `SpellingCheckRule.addIgnoreWords`: a multi-token line becomes a
            // case-sensitive `IGNORE_SPELLING` anti-pattern instead of a
            // single ignored word.
            let tokens: Vec<String> = lt_tokenize::wordtokenizer::string_tokenize(
                &word,
                &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
            )
            .into_iter()
            .filter(|t| !t.trim().is_empty())
            .collect();
            if tokens.len() > 1 {
                self.ignore_phrases
                    .entry(tokens[0].clone())
                    .or_default()
                    .push(tokens);
            } else {
                self.ignore.insert(word);
            }
        }
    }

    fn load_prohibit(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            self.prohibit.insert(word);
        }
    }

    /// `MorfologikSpellerRule.isMisspelled(speller1, word)` (`checkCompound`
    /// is false).
    pub(crate) fn is_misspelled(&self, word: &str) -> bool {
        self.speller1.is_misspelled(word)
    }

    fn is_prohibited(&self, word: &str) -> bool {
        self.prohibit.contains(word)
    }

    fn is_ignored_no_case(&self, word: &str) -> bool {
        let converts_case = true;
        self.ignore.contains(word)
            || (!morfologik::is_mixed_case(word)
                && converts_case
                && self.ignore.contains(&word.to_lowercase()))
    }

    /// `SpellingCheckRule.ignoreWord(String)`.
    fn ignore_word(&self, word: &str) -> bool {
        if word.chars().count() > MAX_TOKEN_LENGTH {
            return true;
        }
        let has_no_letter = if self.config.is_latin_script {
            &HAS_NO_LETTER_LATIN
        } else {
            &HAS_NO_LETTER
        };
        if has_no_letter.is_match(word) {
            return true;
        }
        if let Some(stripped) = word.strip_suffix('.') {
            if !self.ignore.contains(word) {
                return self.is_ignored_no_case(stripped);
            }
        }
        self.is_ignored_no_case(word)
    }

    /// `SpellingCheckRule.ignoreWord(String)` + `StringTools.isEmoji`.
    fn ignore_word_with_emoji(&self, word: &str) -> bool {
        self.ignore_word(word) || lt_tagger::is_emoji(word)
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
            || (self.ignore_tagged_words && token.is_tagged && !self.is_prohibited(token.surface()))
            || self.ignore_word_with_emoji(tokens[idx].surface())
    }

    /// `SpellingCheckRule.getAntiPatterns()`: every multi-word word-list
    /// entry is a case-sensitive `IGNORE_SPELLING` anti-pattern; the matched
    /// tokens count as `isIgnoredBySpeller`. Returns one flag per token of
    /// `tokens`.
    fn phrase_ignored_flags(&self, tokens: &[&AnalyzedTokenReadings]) -> Vec<bool> {
        let mut flags = vec![false; tokens.len()];
        if self.ignore_phrases.is_empty() {
            return flags;
        }
        for i in 0..tokens.len() {
            let Some(phrases) = self.ignore_phrases.get(tokens[i].surface()) else {
                continue;
            };
            for phrase in phrases {
                if i + phrase.len() <= tokens.len()
                    && phrase
                        .iter()
                        .enumerate()
                        .all(|(k, p)| tokens[i + k].surface() == p)
                {
                    for k in 0..phrase.len() {
                        flags[i + k] = true;
                    }
                }
            }
        }
        flags
    }

    /// `MorfologikSpellerRule.match` over one sentence's token stream
    /// (absolute byte offsets already applied by the caller).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut matches: Vec<Match> = Vec::new();
        let mut is_first_word = true;
        let phrase_ignored = self.phrase_ignored_flags(&non_blank);
        for (idx, token) in non_blank.iter().enumerate() {
            if phrase_ignored[idx] || self.can_be_ignored(&non_blank, idx, token) {
                if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                    is_first_word = false;
                }
                continue;
            }
            let start_pos = token.start_pos;
            let word = token
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_else(|| token.surface().to_string());
            // `MorfologikSpellerRule.match`: a non-null `tokenizingPattern()`
            // splits the word (Breton splits at every hyphen) and checks each
            // segment separately, at `startPos + index`.
            if self.config.split_on_hyphen && word.contains('-') {
                let mut index = 0usize;
                for (pos, _) in word.match_indices('-') {
                    let segment = &word[index..pos];
                    let new_matches = self.get_rule_matches(
                        segment,
                        start_pos + index,
                        &mut matches,
                        idx,
                        &non_blank,
                    );
                    matches.extend(new_matches);
                    index = pos + 1;
                }
                let new_matches = self.get_rule_matches(
                    &word[index..],
                    start_pos + index,
                    &mut matches,
                    idx,
                    &non_blank,
                );
                matches.extend(new_matches);
            } else {
                let new_matches =
                    self.get_rule_matches(&word, start_pos, &mut matches, idx, &non_blank);
                matches.extend(new_matches);
            }

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

    /// `MorfologikSpellerRule.getRuleMatches` (no language override).
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
        // Java `startPos + word.length()`: with `tokenizingPattern()` the
        // covered word is the segment, not the whole token.
        let word_end = start_pos + word.len();

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
                            word_end,
                            &sugg1a,
                            &sugg1b,
                            prev_start_pos,
                        ));
                        before_suggestion_str = format!("{prev_word} ");
                    }
                }
                // "than kyou" -> "thank you"
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
                                    word_end,
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
                            let mut m = self.new_rule_match(prev_start_pos, word_end);
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
                        rule_match = Some(self.create_wrong_split_match(
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
                                rule_match = Some(self.create_wrong_split_match(
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
                            let mut m = self.new_rule_match(start_pos, tokens[idx + 1].end_pos());
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
            rule_match = Some(self.new_rule_match(start_pos, word_end));
        }

        // word starting with numbers or bullets
        if let Some(caps) = STARTS_WITH_NUMBERS_BULLETS.captures(word) {
            if !STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS.is_match(word) {
                let first_part = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let second_part = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let second_part_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
                    lt_tokenize::wordtokenizer::string_tokenize(
                        second_part,
                        &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
                    ),
                );
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

        if let Some(rm) = rule_match {
            let mut rule_match = rm;
            rule_match.suggestions = dedupe(rule_match.suggestions);
            rule_matches.push(rule_match);
        }
        rule_matches
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
        let mut m = self.new_rule_match(prev_pos, end_pos);
        m.suggestions.push(Suggestion {
            value: format!("{suggestion1} {suggestion2}").trim().to_string(),
            short_description: None,
        });
        m
    }

    fn new_rule_match(&self, from: usize, to: usize) -> Match {
        Match::new(
            self.config.rule_id,
            Option::<String>::None,
            self.config.message,
            Some(self.config.short_message.to_string()),
            TextRange::new(from, to),
            Vec::new(),
            self.config.category_id,
            self.config.category_name,
        )
        .with_metadata(self.config.description, "misspelling", 0)
        .with_match_type("UnknownWord")
    }

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

    /// `MorfologikSpellerRule.calcSpellerSuggestions` (no top or additional
    /// suggestions).
    fn calc_speller_suggestions(&self, word: &str) -> Vec<Suggestion> {
        let mut default_suggestions: Vec<Suggestion> = self
            .speller1
            .get_weighted_suggestions_from_default_dicts(word)
            .into_iter()
            .map(weighted_to_suggestion)
            .collect();
        let user_suggestions: Vec<Suggestion> = Vec::new();
        let only_case_differs = default_suggestions
            .first()
            .is_some_and(|s| java_equals_ignore_case(&s.value, word));
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
        // `SpellingCheckRule.getAdditionalTopSuggestions`: the LanguageTool
        // sentinel suggestions (the base does not override this).
        let mut top_suggestions = self.additional_top_suggestions(&default_suggestions, word);
        default_suggestions.splice(0..0, top_suggestions.drain(..));

        if default_suggestions.is_empty() && user_suggestions.is_empty() {
            return Vec::new();
        }
        let default_suggestions = self.filter_suggestions(default_suggestions);
        let user_suggestions = dedupe(user_suggestions);
        if word.chars().count() > 4 {
            user_suggestions
                .into_iter()
                .chain(default_suggestions)
                .collect()
        } else {
            default_suggestions
                .into_iter()
                .chain(user_suggestions)
                .collect()
        }
    }

    /// `SpellingCheckRule.getAdditionalTopSuggestions` (base: the
    /// LanguageTool/LanguageTooler sentinels).
    fn additional_top_suggestions(
        &self,
        suggestions: &[Suggestion],
        word: &str,
    ) -> Vec<Suggestion> {
        let mut more: Vec<&str> = Vec::new();
        if (word == "Languagetool" || word == "languagetool")
            && !suggestions
                .iter()
                .any(|s| s.value == morfologik::LANGUAGETOOL)
        {
            more.push(morfologik::LANGUAGETOOL);
        }
        if (word == "Languagetooler" || word == "languagetooler")
            && !suggestions
                .iter()
                .any(|s| s.value == morfologik::LANGUAGETOOLER)
        {
            more.push(morfologik::LANGUAGETOOLER);
        }
        more.into_iter()
            .map(|value| Suggestion {
                value: value.to_string(),
                short_description: None,
            })
            .collect()
    }

    /// `SpellingCheckRule.filterSuggestions`.
    fn filter_suggestions(&self, suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
        dedupe(
            suggestions
                .into_iter()
                .filter(|s| !self.is_prohibited(&s.value))
                .collect(),
        )
    }
}

fn weighted_to_suggestion(s: WeightedSuggestion) -> Suggestion {
    Suggestion {
        value: s.word,
        short_description: None,
    }
}

/// Java `String.equalsIgnoreCase`: per-character full-Unicode case folding
/// (ASCII `eq_ignore_ascii_case` misses Cyrillic/Greek case pairs, which
/// changes `MorfologikSpellerRule.calcSpellerSuggestions`' `onlyCaseDiffers`
/// and therefore the suggestion tiers).
fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    let mut a_chars = a.chars();
    let mut b_chars = b.chars();
    loop {
        match (a_chars.next(), b_chars.next()) {
            (None, None) => return true,
            (Some(c1), Some(c2)) => {
                if c1 != c2 && java_upper(c1) != java_upper(c2) && java_lower(c1) != java_lower(c2)
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// `Character.toUpperCase(char)`: the char itself when the mapping is not
/// one-to-one (e.g. `ß`).
fn java_upper(c: char) -> char {
    let mut it = c.to_uppercase();
    let first = it.next().unwrap_or(c);
    if it.next().is_none() {
        first
    } else {
        c
    }
}

/// `Character.toLowerCase(char)`: the char itself when the mapping is not
/// one-to-one.
fn java_lower(c: char) -> char {
    let mut it = c.to_lowercase();
    let first = it.next().unwrap_or(c);
    if it.next().is_none() {
        first
    } else {
        c
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

/// Plain-text speller dictionary lines: `spelling.txt`,
/// `spelling_custom.txt`, `spelling_global.txt`. The base
/// `Language.prepareLineForSpeller` is the identity for Slovak/Slovenian.
fn load_plain_text_dict_lines(data_dir: &Path, lang_dir: &str) -> Vec<Vec<u8>> {
    let hunspell = data_dir.join(lang_dir).join("hunspell");
    let mut paths: Vec<PathBuf> = Vec::new();
    for path in [
        hunspell.join("spelling.txt"),
        hunspell.join("spelling_custom.txt"),
        data_dir.join("core/spelling_global.txt"),
    ] {
        if path.lt_exists() {
            paths.push(path);
        }
    }
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for path in &paths {
        lines.extend(read_speller_lines(path));
    }
    lines
}

/// LT `MorfologikMultiSpeller.getLines` (identity `prepareLineForSpeller`).
fn read_speller_lines(path: &Path) -> Vec<Vec<u8>> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for original_line in text.lines() {
        if original_line.starts_with('#') || original_line.is_empty() {
            continue;
        }
        let line = original_line.split('#').next().unwrap_or("").trim();
        if !line.is_empty() {
            out.push(line.as_bytes().to_vec());
        }
    }
    out
}
