//! Polish spelling rule (`MorfologikPolishSpellerRule`,
//! `MORFOLOGIK_RULE_PL_PL`): a `MorfologikSpellerRule` subclass over
//! `pl/hunspell/pl_PL.dict` with three Polish overrides:
//!
//! - `tokenizingPattern()` = `(?:[Qq]uasi|[Nn]iby)-`: the matched prefix is
//!   dropped and the remainder checked (with the offset adjusted);
//! - `getRuleMatches()` replaces the base split-word logic with a simpler
//!   one: suppress compounds via `isNotCompound()` (prefix + compound-adjective
//!   checks, adding accepted words to the ignore list) and prune run-on
//!   suggestions via `pruneSuggestions()` (banned morphological suffixes);
//! - the match type stays the `RuleMatch` default `Other` (the Polish override
//!   never calls `setType(UnknownWord)`).
//!
//! The binary dictionary is `pl/hunspell/pl_PL.dict` (PREFIX encoder, UTF-8);
//! ignore / spelling / prohibit lists come from `pl/hunspell/` plus the global
//! `core/spelling_global.txt`.
use lt_data::PathExt as _;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::{is_email, is_punctuation_mark, is_url};
use lt_spell::morfologik::{self, DictSource, MorfologikSpeller, MultiSpeller, SpellerMetadata};
use lt_tagger::DictionaryInfo;
use regex::Regex;

pub const RULE_ID: &str = "MORFOLOGIK_RULE_PL_PL";
const DESCRIPTION: &str = "Prawdopodobny błąd pisowni";
const MESSAGE: &str = "Wykryto prawdopodobny błąd pisowni";
const SHORT_MESSAGE: &str = "Błędna pisownia";
const CATEGORY_ID: &str = "TYPOS";
const CATEGORY_NAME: &str = "Prawdopodobna literówka";
/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;

static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
/// `MorfologikPolishSpellerRule.POLISH_TOKENIZING_CHARS`.
static POLISH_TOKENIZING_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:[Qq]uasi|[Nn]iby)-").unwrap());

/// `MorfologikPolishSpellerRule.prefixes`: never split off these prefixes.
const PREFIXES: &[&str] = &[
    "arcy", "neo", "pre", "anty", "eks", "bez", "beze", "ekstra", "hiper", "infra", "kontr",
    "maksi", "midi", "między", "mini", "nad", "nade", "około", "ponad", "post", "pro", "przeciw",
    "pseudo", "super", "śród", "ultra", "wice", "wokół", "wokoło",
];

/// `MorfologikPolishSpellerRule.bannedSuffixes`: run-on word endings that
/// should not be suggested.
const BANNED_SUFFIXES: &[&str] = &[
    "ami", "ach", "e", "ego", "em", "emu", "ie", "im", "m", "om", "owie", "owi", "ze",
];

pub struct PolishSpellingRule {
    /// the binary `pl_PL.dict` alone (`MorfologikSpeller` for
    /// `FindSuggestionsFilter`'s `findSimilarWords`)
    binary_speller: MorfologikSpeller,
    /// `speller1` (edit distance 1) — the only speller the Polish override uses
    speller1: MultiSpeller,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java) plus
    /// the runtime `addIgnoreTokens` additions from `isNotCompound`
    ignore: Mutex<HashSet<String>>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    /// `language.getTagger()` for `isNotCompound`'s compound-adjective check
    tagger: Arc<lt_tagger::PolishTagger>,
    suggestion_cache: Mutex<HashMap<String, Arc<Vec<Suggestion>>>>,
}

impl PolishSpellingRule {
    pub fn load(data_dir: &Path, tagger: Arc<lt_tagger::PolishTagger>) -> Result<Self> {
        let hunspell = data_dir.join("pl/hunspell");
        let info = DictionaryInfo::load(&hunspell.join("pl_PL.info"))?;
        let meta = SpellerMetadata::from_info(&info)?;
        let binary_source = Arc::new(DictSource::from_dict_file(
            &hunspell.join("pl_PL.dict"),
            &hunspell.join("pl_PL.info"),
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

        let mut ignore = HashSet::new();
        // `SpellingCheckRule.init`: ignore file, spelling file, additional
        // spelling files (the global list).
        for path in [
            hunspell.join("ignore.txt"),
            hunspell.join("spelling.txt"),
            hunspell.join("spelling_custom.txt"),
            data_dir.join("core/spelling_global.txt"),
        ] {
            for word in cache_word_list(&path) {
                ignore.insert(word);
            }
        }
        let mut prohibit = HashSet::new();
        for path in [
            hunspell.join("prohibit.txt"),
            hunspell.join("prohibit_custom.txt"),
        ] {
            for word in cache_word_list(&path) {
                prohibit.insert(word);
            }
        }
        Ok(Self {
            binary_speller,
            speller1,
            ignore: Mutex::new(ignore),
            prohibit,
            tagger,
            suggestion_cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    /// The binary-dictionary speller (`FindSuggestionsFilter`).
    pub fn dict_speller(&self) -> &MorfologikSpeller {
        &self.binary_speller
    }

    /// `MorfologikSpellerRule.isMisspelled(speller1, word)` (`checkCompound`
    /// is false).
    pub(crate) fn is_misspelled(&self, word: &str) -> bool {
        !word.is_empty() && self.speller1.is_misspelled(word)
    }

    fn is_prohibited(&self, word: &str) -> bool {
        self.prohibit.contains(word)
    }

    fn is_ignored_no_case(&self, word: &str) -> bool {
        let ignore = self.ignore.lock().unwrap_or_else(|e| e.into_inner());
        ignore.contains(word)
            || (!morfologik::is_mixed_case(word) && ignore.contains(&word.to_lowercase()))
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
            if !self
                .ignore
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(word)
            {
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
            || self.ignore_word_with_emoji(tokens[idx].surface())
    }

    /// `MorfologikPolishSpellerRule.getRuleMatches`: the Polish override
    /// (no split-word suggestions; `isNotCompound` + `pruneSuggestions`).
    fn get_rule_matches(&self, word: &str, start_pos: usize) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        if word.is_empty() {
            return rule_matches;
        }
        if !((self.is_misspelled(word) && self.is_not_compound(word)) || self.is_prohibited(word)) {
            return rule_matches;
        }
        let mut rule_match = self.new_rule_match(start_pos, start_pos + word.len());
        let lower = word.to_lowercase();
        // If the lower case word is not misspelled, return it as the only
        // suggestion.
        if !self.is_misspelled(&lower) {
            rule_match.suggestions = vec![Suggestion {
                value: lower,
                short_description: None,
            }];
            rule_matches.push(rule_match);
            return rule_matches;
        }
        let mut suggestions: Vec<Suggestion> = self
            .speller1
            .get_suggestions_words(word)
            .into_iter()
            .map(|value| Suggestion {
                value,
                short_description: None,
            })
            .collect();
        let top = self.additional_top_suggestions(&suggestions, word);
        suggestions.splice(0..0, top);
        // `getAdditionalSuggestions` is the base empty list for Polish;
        // `orderSuggestions` is the base identity.
        let suggestions = self.prune_suggestions(&suggestions);
        if !suggestions.is_empty() {
            rule_match.suggestions = suggestions;
        }
        rule_matches.push(rule_match);
        rule_matches
    }

    /// `MorfologikPolishSpellerRule.isNotCompound`: accept a compound
    /// adjective or a word with a non-splitting prefix, adding accepted words
    /// to the ignore list (`addIgnoreTokens`).
    fn is_not_compound(&self, word: &str) -> bool {
        let mut probably_correct: Vec<String> = Vec::new();
        // char boundaries so `first`/`second` split at the i-th character
        let boundaries: Vec<usize> = word
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(word.len()))
            .collect();
        for &split in boundaries
            .iter()
            .skip(2)
            .take(word.chars().count().saturating_sub(2))
        {
            let first = &word[..split];
            let second = &word[split..];
            if PREFIXES.contains(&first.to_lowercase().as_str())
                && !self.is_misspelled(second)
                && second.chars().count() > first.chars().count()
            {
                probably_correct.push(word.to_string());
            } else {
                let tested = vec![first.to_string(), second.to_string()];
                let tagged = self.tagger.tag(&tested);
                if tagged.len() == 2 {
                    let has = |tr: &AnalyzedTokenReadings, tag: &str| tr.has_pos_tag(tag);
                    let t0 = &tagged[0];
                    let t1 = &tagged[1];
                    let first_ok = has(t0, "adja") || (has(t0, "num:comp") && !has(t0, "adv"));
                    let second_ok = t1
                        .readings
                        .iter()
                        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains("adj:")));
                    if first_ok && second_ok {
                        probably_correct.push(word.to_string());
                    }
                }
            }
        }
        if !probably_correct.is_empty() {
            let mut ignore = self.ignore.lock().unwrap_or_else(|e| e.into_inner());
            for word in probably_correct {
                ignore.insert(word);
            }
            return false;
        }
        true
    }

    /// `MorfologikPolishSpellerRule.pruneSuggestions`: drop run-on
    /// suggestions whose second space-separated part is a banned suffix.
    fn prune_suggestions(&self, suggestions: &[Suggestion]) -> Vec<Suggestion> {
        let mut pruned = Vec::with_capacity(suggestions.len());
        for suggestion in suggestions {
            if !suggestion.value.contains(' ') {
                pruned.push(suggestion.clone());
                continue;
            }
            let mut parts: Vec<&str> = suggestion.value.split(' ').collect();
            while parts.len() > 1 && parts.last() == Some(&"") {
                parts.pop();
            }
            if parts.len() > 1 && !BANNED_SUFFIXES.contains(&parts[1]) {
                pruned.push(suggestion.clone());
            }
        }
        pruned
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

    fn new_rule_match(&self, from: usize, to: usize) -> Match {
        Match::new(
            RULE_ID,
            Option::<String>::None,
            MESSAGE,
            Some(SHORT_MESSAGE.to_string()),
            TextRange::new(from, to),
            Vec::new(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "misspelling", 0)
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

            // `tokenizingPattern()`: split at `Quasi-`/`Niby-`, dropping the
            // matched prefix.
            let mut index = 0usize;
            for m in POLISH_TOKENIZING_CHARS.find_iter(&word) {
                let segment = &word[index..m.start()];
                matches.extend(self.get_rule_matches(segment, start_pos + index));
                index = m.end();
            }
            if index == 0 {
                matches.extend(self.get_rule_matches(&word, start_pos));
            } else {
                matches.extend(self.get_rule_matches(&word[index..], start_pos + index));
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

    /// `MorfologikSpellerRule.getSpellingSuggestions` (one synthetic token),
    /// used by `FindSuggestionsFilter`.
    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let cached = self.calc_speller_suggestions_cached(word);
        cached.iter().map(|s| s.value.clone()).collect()
    }

    fn calc_speller_suggestions_cached(&self, word: &str) -> Arc<Vec<Suggestion>> {
        if let Ok(cache) = self.suggestion_cache.lock() {
            if let Some(hit) = cache.get(word) {
                return Arc::clone(hit);
            }
        }
        let computed = Arc::new(self.calc_speller_suggestions(word));
        if let Ok(mut cache) = self.suggestion_cache.lock() {
            if cache.len() >= 50_000 {
                cache.clear();
            }
            cache.insert(word.to_string(), Arc::clone(&computed));
        }
        computed
    }

    /// `MorfologikPolishSpellerRule`'s suggestion path for a single word
    /// (no split-word handling): speller1 + top suggestions, pruned.
    fn calc_speller_suggestions(&self, word: &str) -> Vec<Suggestion> {
        let mut suggestions: Vec<Suggestion> = self
            .speller1
            .get_suggestions_words(word)
            .into_iter()
            .map(|value| Suggestion {
                value,
                short_description: None,
            })
            .collect();
        let top = self.additional_top_suggestions(&suggestions, word);
        suggestions.splice(0..0, top);
        self.prune_suggestions(&suggestions)
    }
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
/// `Language.prepareLineForSpeller` is the identity for Polish.
fn load_plain_text_dict_lines(data_dir: &Path) -> Vec<Vec<u8>> {
    let hunspell = data_dir.join("pl/hunspell");
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
