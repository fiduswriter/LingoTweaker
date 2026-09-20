//! Portuguese spelling rule (`MorfologikPortugueseSpellerRule`): port of the
//! morfologik speller path of `MorfologikSpellerRule` /
//! `SpellingCheckRule` plus the Portuguese overrides.
//!
//! Portuguese specifics (all in `MorfologikPortugueseSpellerRule`):
//! - per-variant binary dictionary: `pt-BR` → `pt-BR`,
//!   `pt-PT` → `pt-PT-90`, `pt-AO`/`pt-MZ` → `pt-PT-45`;
//! - `getAdditionalSpellingFileNames` = global + `pt/spelling.txt` +
//!   `pt/multiwords.txt`, with the Portuguese `prepareLineForSpeller`
//!   (`form\tTAG` keeps the form only for `N*`/`_Latin_`);
//! - top suggestions: abbreviations get `word + "."`;
//! - `filterNoSuggestWords` drops `pt/do_not_suggest.txt`;
//! - `getRuleMatches` post-processing: `_english_ignore_` tokens,
//!   clitic-verb validation, hyphenated titlecase words, compound-element
//!   suggestions, the pt-BR `-ámos` → `-amos` message, the diaeresis
//!   message and dialect-alternation suggestions
//!   (`pt/dialect_alternations.txt` + tagger/synthesizer).
//!
//! Stage-2 note: the base `WordTokenizer` stands in for
//! `PortugueseWordTokenizer` (stage 3) in the dialect-alternation and
//! multitoken paths.
use lt_data::PathExt as _;

use std::collections::{HashMap, HashSet};
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

static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d[.,\d]*|\P{L}+)(.*)$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\p{C}\-\$%&]+)(.*)$").unwrap());
/// `MorfologikPortugueseSpellerRule.isValidCliticVerb`: `V.+:P.+`.
static CLITIC_VERB_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^V.+:P.+$").unwrap());

pub struct PortugueseSpellingRule {
    /// `MORFOLOGIK_RULE_PT_PT` etc.
    rule_id: String,
    message: &'static str,
    short_message: &'static str,
    description: &'static str,
    category_name: &'static str,
    /// `pt-BR` enables the `-ámos` check; dialect alternations only exist
    /// for pt-PT/pt-BR.
    br: bool,
    /// true for pt-PT (unused until the stage-3 filter set needs it)
    #[allow(dead_code)]
    pt: bool,
    /// the variant binary dictionary alone (`MorfologikSpeller` for
    /// `FindSuggestionsFilter`'s `findSimilarWords`)
    binary_speller: MorfologikSpeller,
    speller1: MultiSpeller,
    speller2: MultiSpeller,
    speller3: MultiSpeller,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java)
    ignore: HashSet<String>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    /// `MorfologikPortugueseSpellerRule` does not call `setIgnoreTaggedWords()`
    ignore_tagged_words: bool,
    /// `pt/dialect_alternations.txt` direction for this variant.
    dialect_alternation: HashMap<String, String>,
    /// `MorfologikPortugueseSpellerRule.getAdditionalTopSuggestions`.
    abbreviations: HashSet<String>,
    /// `MorfologikPortugueseSpellerRule.filterNoSuggestWords`.
    do_not_suggest: HashSet<String>,
    tagger: Arc<lt_tagger::PortugueseTagger>,
    synth: Arc<lt_tagger::PortugueseSynthesizer>,
    suggestion_cache: std::sync::Mutex<HashMap<String, std::sync::Arc<Vec<Suggestion>>>>,
}

impl PortugueseSpellingRule {
    /// Load `pt/spelling/<variant>.{dict,info}` plus the pt word lists.
    pub fn load(
        data_dir: &Path,
        variant: &str,
        tagger: Arc<lt_tagger::PortugueseTagger>,
        synth: Arc<lt_tagger::PortugueseSynthesizer>,
    ) -> Result<Self> {
        let br = variant.starts_with("pt-BR");
        let dict_name = match variant {
            "pt-BR" => "pt-BR",
            "pt-AO" | "pt-MZ" => "pt-PT-45",
            _ => "pt-PT-90",
        };
        let dir = data_dir.join("pt/spelling");
        let info = DictionaryInfo::load(&dir.join(format!("{dict_name}.info")))?;
        let meta = SpellerMetadata::from_info(&info)?;
        let binary_source = Arc::new(DictSource::from_dict_file(
            &dir.join(format!("{dict_name}.dict")),
            &dir.join(format!("{dict_name}.info")),
        )?);
        let binary = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(Arc::clone(&binary_source), meta.clone(), distance)
        };
        let lines = load_plain_text_dict_lines(data_dir);
        let plain_source = Arc::new(DictSource::from_lines(&lines));
        let plain = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(Arc::clone(&plain_source), meta.clone(), distance)
        };
        let binary_speller = binary(1);
        let speller1 = MultiSpeller::new(vec![binary(1), plain(1)], vec![0, 1]);
        let speller2 = MultiSpeller::new(vec![binary(2), plain(2)], vec![0, 1]);
        let speller3 = MultiSpeller::new(vec![binary(3), plain(3)], vec![0, 1]);

        let mut rule = Self {
            rule_id: format!(
                "MORFOLOGIK_RULE_{}",
                variant.replace('-', "_").to_uppercase()
            ),
            message: if br {
                "Encontrado possível erro de ortografia."
            } else {
                "Possível erro ortográfico"
            },
            short_message: "Erro ortográfico",
            description: "Possível erro ortográfico",
            category_name: if br {
                "Erro de Escrita"
            } else {
                "Erros Ortográficos"
            },
            br,
            pt: !br,
            binary_speller,
            speller1,
            speller2,
            speller3,
            ignore: HashSet::new(),
            prohibit: HashSet::new(),
            ignore_tagged_words: false,
            dialect_alternation: HashMap::new(),
            abbreviations: HashSet::new(),
            do_not_suggest: HashSet::new(),
            tagger,
            synth,
            suggestion_cache: std::sync::Mutex::new(HashMap::new()),
        };
        // `SpellingCheckRule.init`: ignore file, spelling file, additional
        // spelling files (global + spelling + multiwords), then prohibit.
        rule.load_ignore(&data_dir.join("pt/words/ignore.txt"));
        rule.load_ignore(&data_dir.join("pt/words/spelling.txt"));
        rule.load_ignore(&data_dir.join("core/spelling_global.txt"));
        rule.load_ignore(&data_dir.join("pt/words/multiwords.txt"));
        rule.load_prohibit(&data_dir.join("pt/words/prohibit.txt"));
        rule.abbreviations = cache_word_list(&data_dir.join("pt/words/abbreviations.txt"))
            .into_iter()
            .collect();
        rule.do_not_suggest = cache_word_list(&data_dir.join("pt/words/do_not_suggest.txt"))
            .into_iter()
            .collect();
        rule.dialect_alternation =
            load_dialect_alternations(&data_dir.join("pt/words/dialect_alternations.txt"), variant);
        Ok(rule)
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
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
            self.ignore.insert(word);
        }
    }

    fn load_prohibit(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            self.prohibit.insert(word);
        }
    }

    /// `MorfologikSpellerRule.isMisspelled(speller1, word)` (`checkCompound`
    /// is false for Portuguese).
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
        for (idx, token) in non_blank.iter().enumerate() {
            if self.can_be_ignored(&non_blank, idx, token) {
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

    /// `MorfologikPortugueseSpellerRule.getRuleMatches`: the base matches
    /// plus the Portuguese post-processing.
    fn get_rule_matches(
        &self,
        word: &str,
        start_pos: usize,
        rule_matches_so_far: &mut Vec<Match>,
        idx: usize,
        tokens: &[&AnalyzedTokenReadings],
    ) -> Vec<Match> {
        let mut rule_matches =
            self.base_rule_matches(word, start_pos, rule_matches_so_far, idx, tokens);
        // `tokens[idx].hasPosTag("_english_ignore_")`
        if tokens
            .get(idx)
            .is_some_and(|t| t.has_pos_tag("_english_ignore_"))
        {
            return Vec::new();
        }
        if rule_matches.is_empty() {
            return rule_matches;
        }
        if self.is_valid_clitic_verb(word) {
            return Vec::new();
        }
        if word.contains('-') {
            let parts: Vec<&str> = word.split('-').collect();
            if is_titlecased_hyphenated_word(&parts) && !self.is_misspelled(&word.to_lowercase()) {
                return Vec::new();
            }
            if rule_matches[0].suggestions.is_empty() {
                match self.check_compound_elements(&parts) {
                    Some(new_suggestion) => {
                        self.replace_forms_of_first_match(
                            &mut rule_matches,
                            &new_suggestion,
                            None,
                            false,
                        );
                    }
                    None => return Vec::new(),
                }
            }
        }
        // pt-BR: European-style 1st person plural past tense (`-ámos` → `-amos`)
        if self.br && word.ends_with("ámos") {
            let word_with_br_style = word.replace('á', "a");
            self.replace_forms_of_first_match(
                &mut rule_matches,
                &word_with_br_style,
                Some("No Brasil, o pretérito perfeito da primeira pessoa do plural escreve-se sem acento."),
                true,
            );
        }
        // the diaeresis is gone from the current spelling agreement
        if word.contains('ü') {
            let word_without_diaeresis = word.replace('ü', "u");
            self.replace_forms_of_first_match(
                &mut rule_matches,
                &word_without_diaeresis,
                Some("No mais recente acordo ortográfico, não se usa mais o trema no português."),
                false,
            );
        }
        if let Some(dialect_alternative) = self.dialect_alternative(word) {
            let other_variant = if self.br { "europeu" } else { "brasileiro" };
            let message = format!(
                "Possível erro de ortografia: esta é a grafia utilizada no português {other_variant}."
            );
            let suggestion = if lt_tagger::is_capitalized_word(word) {
                morfologik::uppercase_first_char(&dialect_alternative)
            } else {
                dialect_alternative
            };
            self.replace_forms_of_first_match(&mut rule_matches, &suggestion, Some(&message), true);
        }
        rule_matches
    }

    /// `MorfologikPortugueseSpellerRule.replaceFormsOfFirstMatch`.
    fn replace_forms_of_first_match(
        &self,
        rule_matches: &mut [Match],
        suggestion: &str,
        message: Option<&str>,
        dialect_issue: bool,
    ) {
        if rule_matches.is_empty() {
            return;
        }
        let old = &rule_matches[0];
        let mut new_match = Match::new(
            self.rule_id.clone(),
            Option::<String>::None,
            message.unwrap_or(&old.message).to_string(),
            old.short_message.clone(),
            old.range,
            vec![Suggestion {
                value: suggestion.to_string(),
                short_description: None,
            }],
            "TYPOS",
            self.category_name,
        )
        .with_metadata(self.description, "misspelling", 0)
        .with_match_type("UnknownWord");
        if dialect_issue {
            new_match.specific_rule_id = Some(format!("{}_DIALECT", self.rule_id));
        }
        rule_matches[0] = new_match;
    }

    /// `MorfologikPortugueseSpellerRule.dialectAlternative`.
    fn dialect_alternative(&self, word: &str) -> Option<String> {
        if self.dialect_alternation.is_empty() {
            return None;
        }
        let lemma_check_result = self.dialect_alternation.get(&word.to_lowercase());
        if let Some(result) = lemma_check_result {
            return Some(result.clone());
        }
        for reading in self
            .tagger
            .tag(std::slice::from_ref(&word.to_string()))
            .iter()
            .flat_map(|atr| atr.readings.iter())
        {
            let lemma = reading.stem.clone().unwrap_or_default();
            let tag = match reading.pos_tag.clone() {
                Some(tag) => tag,
                None => continue,
            };
            if let Some(candidate) = self.dialect_alternation.get(&lemma) {
                let forms: Vec<String> =
                    self.synth.synthesize_for_pos_tags(candidate, &|t| t == tag);
                if let Some(first) = forms.first() {
                    return Some(first.clone());
                }
            }
        }
        None
    }

    /// `MorfologikPortugueseSpellerRule.isValidCliticVerb`.
    fn is_valid_clitic_verb(&self, word: &str) -> bool {
        for reading in self
            .tagger
            .tag(std::slice::from_ref(&word.to_string()))
            .iter()
            .flat_map(|atr| atr.readings.iter())
        {
            let Some(pos_tag) = reading.pos_tag.as_deref() else {
                continue;
            };
            if CLITIC_VERB_TAG.is_match(pos_tag) {
                let lemma = reading.stem.clone().unwrap_or_default();
                return !self.dialect_alternation.contains_key(&lemma);
            }
        }
        false
    }

    /// `MorfologikPortugueseSpellerRule.checkCompoundElements`.
    fn check_compound_elements(&self, word_parts: &[&str]) -> Option<String> {
        let mut suggested_parts: Vec<String> = Vec::new();
        for part in word_parts {
            if self.speller1.is_misspelled(part) {
                let part_suggestions = self.speller1.get_suggestions(part);
                if part_suggestions.is_empty() {
                    suggested_parts.push(part.to_string());
                } else {
                    suggested_parts.push(part_suggestions[0].word.clone());
                }
            } else {
                suggested_parts.push(part.to_string());
            }
        }
        let new_suggestion = suggested_parts.join("-");
        if new_suggestion == word_parts.join("-") {
            None
        } else {
            Some(new_suggestion)
        }
    }

    /// `MorfologikSpellerRule.getRuleMatches` (the base the pt override
    /// calls first).
    fn base_rule_matches(
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
                                self.message,
                                self.short_message,
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
                            let mut m = self.new_rule_match(
                                start_pos,
                                tokens[idx + 1].end_pos(),
                                self.message,
                                self.short_message,
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
            rule_match = Some(self.new_rule_match(
                start_pos,
                tokens[idx].end_pos(),
                self.message,
                self.short_message,
            ));
        }

        // word starting with numbers or bullets
        if let Some(caps) = STARTS_WITH_NUMBERS_BULLETS.captures(word) {
            if !STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS.is_match(word) {
                let first_part = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let second_part = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let second_part_tokens = {
                    let is_tagged = |w: &str| self.tagger.is_tagged_word(w);
                    lt_tokenize::PortugueseWordTokenizer::new(&is_tagged).tokenize(second_part)
                };
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
        let mut m = self.new_rule_match(prev_pos, end_pos, self.message, self.short_message);
        m.suggestions.push(Suggestion {
            value: format!("{suggestion1} {suggestion2}").trim().to_string(),
            short_description: None,
        });
        m
    }

    fn new_rule_match(&self, from: usize, to: usize, message: &str, short_message: &str) -> Match {
        Match::new(
            self.rule_id.clone(),
            Option::<String>::None,
            message,
            Some(short_message.to_string()),
            TextRange::new(from, to),
            Vec::new(),
            "TYPOS",
            self.category_name,
        )
        .with_metadata(self.description, "misspelling", 0)
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

    /// `MorfologikSpellerRule.calcSpellerSuggestions` with the Portuguese
    /// `getAdditionalTopSuggestions` (abbreviation + ".").
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
        // `MorfologikPortugueseSpellerRule.getAdditionalTopSuggestions`
        // replaces the base sentinels: abbreviations get `word + "."`.
        let top_suggestions = self.additional_top_suggestions(word);
        default_suggestions.splice(0..0, top_suggestions);

        if default_suggestions.is_empty() && user_suggestions.is_empty() {
            return Vec::new();
        }
        let default_suggestions = self.filter_suggestions(default_suggestions);
        let user_suggestions = dedupe(user_suggestions);
        let default_suggestions = self.order_suggestions(default_suggestions, word);
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

    /// `MorfologikPortugueseSpellerRule.getAdditionalTopSuggestions`.
    fn additional_top_suggestions(&self, word: &str) -> Vec<Suggestion> {
        if self.is_abbreviation(word) {
            return vec![Suggestion {
                value: format!("{word}."),
                short_description: None,
            }];
        }
        Vec::new()
    }

    fn is_abbreviation(&self, word: &str) -> bool {
        self.abbreviations.contains(&format!("{word}."))
            || self
                .abbreviations
                .contains(&format!("{}.", word.to_lowercase()))
    }

    /// `SpellingCheckRule.filterSuggestions`: prohibited words first, then
    /// the Portuguese `filterNoSuggestWords` (`do_not_suggest.txt`,
    /// lowercased comparison).
    fn filter_suggestions(&self, suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
        let filtered = dedupe(
            suggestions
                .into_iter()
                .filter(|s| !self.is_prohibited(&s.value))
                .collect(),
        );
        filtered
            .into_iter()
            .filter(|s| !self.do_not_suggest.contains(&s.value.to_lowercase()))
            .collect()
    }

    /// `MorfologikPortugueseSpellerRule` does not override `orderSuggestions`,
    /// so the base returns them unchanged.
    fn order_suggestions(&self, suggestions: Vec<Suggestion>, _word: &str) -> Vec<Suggestion> {
        suggestions
    }
}

fn weighted_to_suggestion(s: WeightedSuggestion) -> Suggestion {
    Suggestion {
        value: s.word,
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

fn split_last_char(word: &str) -> Option<(&str, &str)> {
    let mut chars = word.char_indices();
    chars
        .next_back()
        .map(|(split, _)| (&word[..split], &word[split..]))
}

/// `MorfologikPortugueseSpellerRule.isTitlecasedHyphenatedWord`.
fn is_titlecased_hyphenated_word(word_parts: &[&str]) -> bool {
    word_parts
        .iter()
        .all(|part| !lt_tagger::is_mixed_case(part))
}

/// `MorfologikPortugueseSpellerRule.getDialectAlternationMapping`: column 0
/// is the pt-BR spelling, column 1 the pt-PT one; unsupported variants get
/// an empty map.
fn load_dialect_alternations(path: &Path, variant: &str) -> HashMap<String, String> {
    let column = match variant {
        "pt-BR" => 1,
        "pt-PT" => 0,
        _ => return HashMap::new(),
    };
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parsed: Vec<&str> = line.split('=').collect();
        if parsed.len() != 2 {
            continue;
        }
        map.insert(
            parsed[column].to_lowercase(),
            parsed[1 - column].to_string(),
        );
    }
    map
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

/// Plain-text speller dictionary lines: `pt/spelling.txt`,
/// `core/spelling_global.txt`, `pt/spelling.txt` (again, like Java's
/// additional-list duplicate) and `pt/multiwords.txt`, all with the
/// Portuguese `prepareLineForSpeller`.
fn load_plain_text_dict_lines(data_dir: &Path) -> Vec<Vec<u8>> {
    let paths: Vec<PathBuf> = [
        data_dir.join("pt/words/spelling.txt"),
        data_dir.join("core/spelling_global.txt"),
        data_dir.join("pt/words/spelling.txt"),
        data_dir.join("pt/words/multiwords.txt"),
    ]
    .into_iter()
    .filter(|p| p.lt_exists())
    .collect();
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for path in &paths {
        lines.extend(read_speller_lines_pt(path));
    }
    lines
}

/// LT `MorfologikMultiSpeller.getLines` with the Portuguese
/// `prepareLineForSpeller` (`form\tTAG` / `form;TAG` keeps the form only for
/// `N*`/`_Latin_` tags; untagged lines keep the whole line).
fn read_speller_lines_pt(path: &Path) -> Vec<Vec<u8>> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for original_line in text.lines() {
        if original_line.starts_with('#') || original_line.is_empty() {
            continue;
        }
        let line = original_line.split('#').next().unwrap_or("").trim();
        let form_tag: Vec<&str> = line.split(['\t', ';']).collect();
        let prepared: Vec<String> = if form_tag.len() > 1 {
            let form = form_tag[0].trim();
            let tag = form_tag[1].trim();
            if tag.starts_with('N') || tag == "_Latin_" {
                vec![form.to_string()]
            } else {
                vec![String::new()]
            }
        } else {
            vec![line.to_string()]
        };
        for prepared in prepared {
            let prepared = prepared.split('#').next().unwrap_or("").trim();
            if !prepared.is_empty() {
                out.push(prepared.as_bytes().to_vec());
            }
        }
    }
    out
}
