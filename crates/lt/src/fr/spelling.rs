//! French spelling rule (`MorfologikFrenchSpellerRule`): port of the
//! morfologik speller path of `MorfologikSpellerRule` and
//! `SpellingCheckRule` with the French overrides (`orderSuggestions`,
//! `getAdditionalTopSuggestions`, `setIgnoreTaggedWords`,
//! `prepareLineForSpeller`).
use lt_data::PathExt as _;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::{is_email, is_punctuation_mark, is_url};
use lt_spell::morfologik::{
    self, DictSource, MorfologikSpeller, MultiSpeller, SpellerMetadata, WeightedSuggestion,
};
use lt_tagger::{DictionaryInfo, FrenchTagger};
use regex::Regex;

pub const RULE_ID: &str = "FR_SPELLING_RULE";
const DESCRIPTION: &str = "Faute de frappe possible";
const MESSAGE: &str = "Faute de frappe possible trouvée.";
const SHORT_MESSAGE: &str = "Faute de frappe";
const CATEGORY_ID: &str = "TYPOS";
const CATEGORY_NAME: &str = "Faute de frappe possible";
/// `MorfologikSpellerRule.MAX_FREQUENCY_FOR_SPLITTING`
const MAX_FREQUENCY_FOR_SPLITTING: i32 = 21;
/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;

static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d[.,\d]*|\P{L}+)(.*)$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\p{C}\-\$%&]+)(.*)$").unwrap());

/// `MorfologikFrenchSpellerRule.PREFIX_WITH_WHITESPACE`
static PREFIX_WITH_WHITESPACE: &[&str] = &[
    "agro",
    "anti",
    "archi",
    "auto",
    "aéro",
    "cardio",
    "co",
    "cyber",
    "demi",
    "ex",
    "extra",
    "géo",
    "hospitalo",
    "hydro",
    "hyper",
    "hypo",
    "infra",
    "inter",
    "macro",
    "mega",
    "meta",
    "mi",
    "micro",
    "mini",
    "mono",
    "multi",
    "musculo",
    "méga",
    "méta",
    "néo",
    "omni",
    "pan",
    "para",
    "pluri",
    "poly",
    "post",
    "prim",
    "pro",
    "proto",
    "pré",
    "pseudo",
    "psycho",
    "péri",
    "re",
    "retro",
    "ré",
    "semi",
    "simili",
    "socio",
    "super",
    "supra",
    "sus",
    "trans",
    "tri",
    "télé",
    "ultra",
    "uni",
    "vice",
    "éco",
];

/// `MorfologikFrenchSpellerRule.TOKEN_AT_START`
static TOKEN_AT_START: &[&str] = &[
    "non", "en", "a", "le", "la", "les", "pour", "de", "du", "des", "un", "une", "mon", "ma",
    "mes", "ton", "ta", "tes", "son", "sa", "ses", "leur", "leurs", "ce", "cet",
];

/// `MorfologikFrenchSpellerRule.exceptionsEgrave`
static EXCEPTIONS_EGRAVE: &[&str] = &["burkinabè", "koinè", "épistémè"];

fn re(pattern: &str) -> Regex {
    Regex::new(&format!("(?i)^(?:{pattern})$")).unwrap()
}

static APOSTROF_INICI_VERBS: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([lnts])(h?[aeiouàéèíòóú].*[^è])$"));
static APOSTROF_INICI_VERBS_M: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(m)(h?[aeiouàéèíòóú].*[^è])$"));
static APOSTROF_INICI_VERBS_C: LazyLock<Regex> = LazyLock::new(|| re(r"^(c)([eiéèê].*)$"));
static APOSTROF_INICI_NOM_SING: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([ld])(h?[aeiouàéèíòóú]...+)$"));
static APOSTROF_INICI_NOM_PLURAL: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(d)(h?[aeiouàéèíòóú].+)$"));
static APOSTROF_INICI_VERBS_INF: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([lntsmd]|nous|vous)(h?[aeiouàéèíòóú].*[^è])$"));
static HYPHEN_ON: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([\p{L}]+[^aeiou])[’']?(il|elle|ce|on)$"));
static HYPHEN_JE: LazyLock<Regex> = LazyLock::new(|| re(r"^([\p{L}]+[^e])[’']?(je)$"));
static HYPHEN_TU: LazyLock<Regex> = LazyLock::new(|| re(r"^([\p{L}]+)[’']?(tu)$"));
static HYPHEN_NOUS: LazyLock<Regex> = LazyLock::new(|| re(r"^([\p{L}]+)[’']?(nous)$"));
static HYPHEN_VOUS: LazyLock<Regex> = LazyLock::new(|| re(r"^([\p{L}]+)[’']?(vous)$"));
static HYPHEN_ILS: LazyLock<Regex> = LazyLock::new(|| re(r"^([\p{L}]+)[’']?(ils|elles)$"));
static IMPERATIVE_HYPHEN: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([\p{L}]+)[’']?(moi|toi|le|la|lui|nous|vous|les|leur|y|en)$"));

static VERB_INDSUBJ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:V .*(ind|sub).*)$").unwrap());
static VERB_IMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V.* imp .*)$").unwrap());
static VERB_INF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V.* inf)$").unwrap());
static VERB_INDSUBJ_M: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:V .* [123] s|V .* [23] p)$").unwrap());
static VERB_INDSUBJ_C: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .* 3 s)$").unwrap());
static NOM_SING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[NJZ] .* (s|sp)|V .inf|V .*ppa.* s)$").unwrap());
static NOM_PLURAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[NJZ] .* (p|sp)|V .*ppa.* p)$").unwrap());
static VERB_1S: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .*(ind).* 1 s)$").unwrap());
static VERB_2S: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .*(ind).* 2 s)$").unwrap());
static VERB_3S: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .*(ind).* 3 s)$").unwrap());
static VERB_1P: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .*(ind).* 1 p)$").unwrap());
static VERB_2P: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .*(ind).* 2 p)$").unwrap());
static VERB_3P: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V .*(ind).* 3 p)$").unwrap());
static HYPHEN_OR_QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"['-]").unwrap());

/// `MorfologikFrenchSpellerRule.SPLIT_DIGITS_AT_END`
static SPLIT_DIGITS_AT_END: &[&str] = &["et", "ou", "de", "en", "à", "aux", "des"];

pub struct FrenchSpellingRule {
    /// the binary `french.dict` alone
    #[allow(dead_code)]
    binary_speller: MorfologikSpeller,
    speller1: MultiSpeller,
    speller2: MultiSpeller,
    speller3: MultiSpeller,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java)
    ignore: HashSet<String>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    tagger: Arc<FrenchTagger>,
    /// `setIgnoreTaggedWords()`
    ignore_tagged_words: bool,
    suggestion_cache:
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Vec<Suggestion>>>>,
}

impl FrenchSpellingRule {
    pub fn load(data_dir: &Path, tagger: Arc<FrenchTagger>) -> Result<Self> {
        let dict_dir = data_dir.join("fr/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("french.info"))?;
        let meta = SpellerMetadata::from_info(&info)?;
        let binary_source = Arc::new(DictSource::from_dict_file(
            &dict_dir.join("french.dict"),
            &dict_dir.join("french.info"),
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
            binary_speller,
            speller1,
            speller2,
            speller3,
            ignore: HashSet::new(),
            prohibit: HashSet::new(),
            tagger,
            ignore_tagged_words: true,
            suggestion_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        };
        let hunspell = data_dir.join("fr/hunspell");
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
        RULE_ID
    }

    /// `MorfologikMultiSpeller.getSuggestions` (used by the French
    /// `findSuggestion` recursion of `getAdditionalTopSuggestions`).
    pub fn raw_suggestions(&self, word: &str) -> Vec<String> {
        self.speller1.get_suggestions_words(word)
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
            // `tokenizeNewWords()` is false: multi-token lines stay entries
            self.ignore.insert(word);
        }
    }

    fn load_prohibit(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            self.prohibit.insert(word);
        }
    }

    /// `MorfologikSpellerRule.isMisspelled(speller1, word)` (`checkCompound`
    /// is false for French).
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

    /// `MorfologikSpellerRule.getRuleMatches` (no French override).
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
                let is_tagged = |w: &str| self.tagger.is_tagged_word(w);
                let tokenizer = lt_tokenize::FrenchWordTokenizer::new(&is_tagged);
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
        let mut m = self.new_rule_match(prev_pos, end_pos, MESSAGE, SHORT_MESSAGE);
        m.suggestions.push(Suggestion {
            value: format!("{suggestion1} {suggestion2}").trim().to_string(),
            short_description: None,
        });
        m
    }

    fn new_rule_match(&self, from: usize, to: usize, message: &str, short_message: &str) -> Match {
        Match::new(
            RULE_ID,
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
        let mut top_suggestions: Vec<Suggestion> = self.additional_top_suggestions_extra(word);
        default_suggestions.splice(0..0, top_suggestions.drain(..));

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

    /// `MorfologikFrenchSpellerRule.getAdditionalTopSuggestionsString`: the
    /// voulai/size-unit special cases, camel-case/digits-at-end splits and
    /// the apostrophe/hyphen pronoun suggestions.
    fn additional_top_suggestions_extra(&self, word: &str) -> Vec<Suggestion> {
        if word == "voulai" {
            return vec![
                Suggestion {
                    value: "voulais".to_string(),
                    short_description: None,
                },
                Suggestion {
                    value: "voulait".to_string(),
                    short_description: None,
                },
            ];
        }
        let lower = word.to_lowercase();
        for (suffix, unit) in [
            ("mm2", "mm²"),
            ("cm2", "cm²"),
            ("dm2", "dm²"),
            ("m2", "m²"),
            ("km2", "km²"),
            ("mm3", "mm³"),
            ("cm3", "cm³"),
            ("dm3", "dm³"),
            ("m3", "m³"),
            ("km3", "km³"),
        ] {
            if lower == suffix {
                return vec![Suggestion {
                    value: unit.to_string(),
                    short_description: None,
                }];
            }
        }
        let parts = split_camel_case(word);
        if parts.len() > 1 && parts[0].chars().count() > 1 {
            let mut is_not_misspelled = true;
            for part in &parts {
                is_not_misspelled &= !self.is_misspelled(part);
            }
            if is_not_misspelled {
                return vec![Suggestion {
                    value: parts.join(" "),
                    short_description: None,
                }];
            }
        }
        // `StringTools.splitDigitsAtEnd`
        let chars: Vec<char> = word.chars().collect();
        let mut last_index = chars.len();
        while last_index > 0 && chars[last_index - 1].is_ascii_digit() {
            last_index -= 1;
        }
        if last_index > 0 && last_index < chars.len() {
            let non_digit: String = chars[..last_index].iter().collect();
            let digit: String = chars[last_index..].iter().collect();
            if self.tagger.is_tagged_word(&non_digit)
                && (non_digit.chars().count() > 2
                    || SPLIT_DIGITS_AT_END.contains(&non_digit.to_lowercase().as_str()))
            {
                return vec![Suggestion {
                    value: format!("{non_digit} {digit}"),
                    short_description: None,
                }];
            }
        }
        let mut new_suggestions: Vec<String> = Vec::new();
        new_suggestions.extend(self.find_suggestion(
            word,
            &APOSTROF_INICI_VERBS,
            &VERB_INDSUBJ,
            2,
            "'",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(
            word,
            &APOSTROF_INICI_VERBS_M,
            &VERB_INDSUBJ_M,
            2,
            "'",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(
            word,
            &APOSTROF_INICI_VERBS_C,
            &VERB_INDSUBJ_C,
            2,
            "'",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(
            word,
            &APOSTROF_INICI_VERBS_INF,
            &VERB_INF,
            2,
            "'",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(
            word,
            &APOSTROF_INICI_NOM_SING,
            &NOM_SING,
            2,
            "'",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(
            word,
            &APOSTROF_INICI_NOM_PLURAL,
            &NOM_PLURAL,
            2,
            "'",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(
            word,
            &IMPERATIVE_HYPHEN,
            &VERB_IMP,
            1,
            "-",
            true,
        ));
        new_suggestions.extend(self.find_suggestion(word, &HYPHEN_JE, &VERB_1S, 1, "-", true));
        new_suggestions.extend(self.find_suggestion(word, &HYPHEN_TU, &VERB_2S, 1, "-", true));
        new_suggestions.extend(self.find_suggestion(word, &HYPHEN_ON, &VERB_3S, 1, "-", true));
        new_suggestions.extend(self.find_suggestion(word, &HYPHEN_NOUS, &VERB_1P, 1, "-", true));
        new_suggestions.extend(self.find_suggestion(word, &HYPHEN_VOUS, &VERB_2P, 1, "-", true));
        new_suggestions.extend(self.find_suggestion(word, &HYPHEN_ILS, &VERB_3P, 1, "-", true));
        new_suggestions
            .into_iter()
            .map(|value| Suggestion {
                value,
                short_description: None,
            })
            .collect()
    }

    /// `MorfologikFrenchSpellerRule.findSuggestion`.
    fn find_suggestion(
        &self,
        word: &str,
        word_pattern: &Regex,
        postag_pattern: &Regex,
        suggestion_position: usize,
        separator: &str,
        recursive: bool,
    ) -> Vec<String> {
        let mut new_suggestions: Vec<String> = Vec::new();
        if let Some(caps) = word_pattern.captures(word) {
            let new_suggestion = caps
                .get(suggestion_position)
                .map(|g| g.as_str().to_string())
                .unwrap_or_default();
            let atr = self.tagger.tag_word(&new_suggestion);
            if match_postag_regexp(&atr, postag_pattern) {
                let group1 = caps.get(1).map(|g| g.as_str()).unwrap_or("");
                let group2 = caps.get(2).map(|g| g.as_str()).unwrap_or("");
                new_suggestions.push(format!("{group1}{separator}{group2}"));
                return new_suggestions;
            }
            if recursive {
                let more_sugg = self.raw_suggestions(&new_suggestion);
                for (i, sugg) in more_sugg.iter().enumerate() {
                    let new_word = if suggestion_position == 1 {
                        format!("{sugg}{}", caps.get(2).map(|g| g.as_str()).unwrap_or(""))
                    } else {
                        format!("{}{sugg}", caps.get(1).map(|g| g.as_str()).unwrap_or(""))
                    };
                    new_suggestions.extend(self.find_suggestion(
                        &new_word,
                        word_pattern,
                        postag_pattern,
                        suggestion_position,
                        separator,
                        false,
                    ));
                    if i > 5 {
                        break;
                    }
                }
            }
        }
        new_suggestions
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

    /// `MorfologikFrenchSpellerRule.orderSuggestions`.
    fn order_suggestions(&self, suggestions: Vec<Suggestion>, word: &str) -> Vec<Suggestion> {
        let word_without_diacritics = crate::multitoken::remove_diacritics(word);
        let total = suggestions.len();
        let mut new_suggestions: Vec<Suggestion> = Vec::new();
        for (i, suggestion) in suggestions.into_iter().enumerate() {
            let replacement_lower = suggestion.value.to_lowercase();
            let parts = java_split_space(&replacement_lower);

            // remove wrong split prefixes
            if parts.len() == 2 && PREFIX_WITH_WHITESPACE.contains(&parts[0]) {
                continue;
            }
            if let Some(first) = parts.first() {
                if first.chars().count() == 1 && *first != "a" && *first != "à" && *first != "y" {
                    continue;
                }
            }
            // remove: informè V ind pres 1 s
            if replacement_lower.ends_with('è')
                && !EXCEPTIONS_EGRAVE.contains(&replacement_lower.as_str())
            {
                continue;
            }

            // Don't change first suggestions if they match word without
            // diacritics
            let mut pos_new_sugg = 0usize;
            while new_suggestions.len() > pos_new_sugg
                && java_equals_ignore_case(
                    &crate::multitoken::remove_diacritics(&new_suggestions[pos_new_sugg].value),
                    &word_without_diacritics,
                )
            {
                pos_new_sugg += 1;
            }
            // move some split words to first place
            if parts.len() == 2
                && TOKEN_AT_START.contains(&parts[0])
                && parts[1].chars().count() > 1
            {
                new_suggestions.insert(pos_new_sugg, suggestion);
                continue;
            }

            let sugg_without_diacritics = crate::multitoken::remove_diacritics(&suggestion.value);
            if java_equals_ignore_case(&word_without_diacritics, &sugg_without_diacritics) {
                new_suggestions.insert(pos_new_sugg, suggestion);
                continue;
            }

            // move words with apostrophe or hyphen to second position
            let clean_suggestion = HYPHEN_OR_QUOTE
                .replace_all(&suggestion.value, "")
                .into_owned();
            if i > 1 && total > 2 && java_equals_ignore_case(&clean_suggestion, word) {
                if pos_new_sugg == 0 {
                    pos_new_sugg = 1;
                }
                new_suggestions.insert(pos_new_sugg, suggestion);
                continue;
            }
            new_suggestions.push(suggestion);
        }
        new_suggestions
    }
}

/// `AnalyzedTokenReadings.matchesPosTagRegex` with Java's null → `UNKNOWN`.
fn match_postag_regexp(readings: &[lt_core::AnalyzedToken], pattern: &Regex) -> bool {
    readings.iter().any(|r| {
        let tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(tag)
    })
}

/// `String.split(" ")` (trailing empty strings removed).
fn java_split_space(s: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split(' ').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Java `String.equalsIgnoreCase`: per-character comparison with the
/// uppercase mapping tried first, so `ı` ≡ `i` (both uppercase to `I`) —
/// unlike `eq_ignore_ascii_case`/lowercase comparison (used e.g. for the
/// speller input `entraı`, where Java treats the suggestion `entrai` as the
/// case-only variant).
fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    fn upper_single(c: char) -> char {
        c.to_uppercase().next().unwrap_or(c)
    }
    fn lower_single(c: char) -> char {
        c.to_lowercase().next().unwrap_or(c)
    }
    let mut b_iter = b.chars();
    for ca in a.chars() {
        let Some(cb) = b_iter.next() else {
            return false;
        };
        if ca == cb {
            continue;
        }
        let (ua, ub) = (upper_single(ca), upper_single(cb));
        if ua == ub {
            continue;
        }
        if lower_single(ua) != lower_single(ub) {
            return false;
        }
    }
    b_iter.next().is_none()
}

fn weighted_to_suggestion(s: WeightedSuggestion) -> Suggestion {
    Suggestion {
        value: s.word,
        short_description: None,
    }
}

/// `StringTools.splitCamelCase`.
fn split_camel_case(input: &str) -> Vec<String> {
    if lt_tagger::is_all_uppercase(input) {
        return vec![input.to_string()];
    }
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_is_uppercase = false;
    for c in input.chars() {
        if c.is_uppercase() {
            if !previous_is_uppercase && !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            previous_is_uppercase = true;
        } else {
            previous_is_uppercase = false;
        }
        current.push(c);
    }
    parts.push(current);
    parts
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
/// `spelling_custom.txt`, `spelling_global.txt`, then `LanguageTool` (LT
/// appends the sentinel so it is also suggested).
fn load_plain_text_dict_lines(data_dir: &Path) -> Vec<Vec<u8>> {
    let hunspell = data_dir.join("fr/hunspell");
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
    lines.push(morfologik::LANGUAGETOOL.as_bytes().to_vec());
    lines
}

/// LT `MorfologikMultiSpeller.getLines` + French `prepareLineForSpeller`.
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

/// `French.prepareLineForSpeller`.
fn prepare_line_for_speller(line: &str) -> Vec<String> {
    let parts: Vec<&str> = line.split('#').collect();
    if parts.is_empty() {
        return vec![line.to_string()];
    }
    let form_tag: Vec<&str> = parts[0].split(['\t', ';']).collect();
    let form = form_tag[0].trim();
    if form == "Ho Chi Minh" {
        return vec![String::new()];
    }
    if form_tag.len() > 1 {
        let tag = form_tag[1].trim();
        if tag.starts_with('Z') || tag.starts_with('N') || tag == "A" {
            return vec![form.to_string()];
        }
        return vec![String::new()];
    }
    vec![line.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_lines_like_java() {
        // `form;TAG`: only Z*/N*/A tags keep the form; the second `;` field is
        // the tag slot, so added.txt-style `form;lemma;tag;` lines are dropped
        // (those files go through the ManualTagger, not the speller).
        assert_eq!(
            prepare_line_for_speller("Spontex;N m sp"),
            vec!["Spontex".to_string()]
        );
        assert_eq!(
            prepare_line_for_speller("Spontex;Spontex;Z e sp;"),
            vec![String::new()]
        );
        assert_eq!(
            prepare_line_for_speller("parler;V inf"),
            vec![String::new()]
        );
        assert_eq!(
            prepare_line_for_speller("Ho Chi Minh;N m sp"),
            vec![String::new()]
        );
        assert_eq!(
            prepare_line_for_speller("mot simple"),
            vec!["mot simple".to_string()]
        );
    }

    #[test]
    fn splits_camel_case_like_java() {
        assert_eq!(split_camel_case("leChat"), vec!["le", "Chat"]);
        assert_eq!(split_camel_case("UNESCO"), vec!["UNESCO"]);
    }
}
