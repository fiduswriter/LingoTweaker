//! Generic Hunspell-backed spelling rule for hand-authored languages whose
//! speller ships as a plain `.aff`/`.dic` pair (Norwegian Bokmål, Nordum,
//! Guaraní). This is the `SpellingCheckRule` token loop without the
//! Morfologik-specific machinery; Hunspell stays the spelling authority while
//! suggestions (when enabled) come either from a bounded edit-distance search
//! over a dictionary word list or from a vendored Morfologik `.dict`.
//!
//! Differences from the ported Morfologik speller rules:
//! - no split-word / frequency heuristics,
//! - `suggestions` are opt-in: a plain candidate list is capped by
//!   [`MAX_SUGGESTION_DICT`], and [`HunspellSpellingConfig::morfologik_dict`]
//!   selects the FSA search for dictionaries that are too large to scan.

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use lt_spell::hunspell::HunspellChecker;
use lt_spell::morfologik::MorfologikSpeller;
use regex::Regex;

use crate::wordutil::{is_email, is_punctuation_mark, is_url};

/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;
/// Above this dictionary size the edit-distance suggestion search is skipped
/// (the Norwegian `nb_NO.dic` has ~700k entries).
const MAX_SUGGESTION_DICT: usize = 300_000;
/// Maximum edit distance for suggestion candidates.
const MAX_SUGGESTION_DISTANCE: usize = 1;
/// Edit distance of the Morfologik suggestion search (LT uses 1/2/3 tiers;
/// the hand-authored languages have no frequency data, so one pass at 2
/// returns the same ranked candidates).
const MORFOLOGIK_MAX_EDIT_DISTANCE: i32 = 2;
/// Shortest misspelling for which Morfologik suggestions are computed.
const MIN_SUGGESTION_WORD_LENGTH: usize = 3;
/// Longest misspelling for which suggestions are computed.
const MAX_SUGGESTION_WORD_LENGTH: usize = 30;

static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());

/// Configuration for one [`HunspellSpellingRule`].
pub struct HunspellSpellingConfig {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub message: &'static str,
    pub short_message: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    /// Directory under `data/`, e.g. `no`.
    pub lang_dir: &'static str,
    /// `.aff` file name inside `data/<lang_dir>/hunspell/`.
    pub aff: &'static str,
    /// `.dic` file name inside `data/<lang_dir>/hunspell/`.
    pub dic: &'static str,
    /// Optional additional word file (inside `data/<lang_dir>/hunspell/`)
    /// used as the suggestion candidate list; defaults to the dictionary.
    pub suggestion_file: Option<&'static str>,
    /// Optional Morfologik dictionary `(dict, info)` paths relative to
    /// `data/`, used for suggestions when the plain candidate list is too
    /// large to scan (`suggestion_file` list empty).
    pub morfologik_dict: Option<(&'static str, &'static str)>,
    /// `0` disables suggestions.
    pub max_suggestions: usize,
    /// Use the ported native hunspell `suggest()` (affix/compound/REP/MAP
    /// generators) instead of the bounded edit-distance search.
    pub native_suggestions: bool,
}

/// A Hunspell speller wired as an LT spelling rule.
pub struct HunspellSpellingRule {
    config: HunspellSpellingConfig,
    checker: HunspellChecker,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java)
    ignore: HashSet<String>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    suggestion_words: Vec<String>,
    /// Vendored Morfologik speller used for suggestions when
    /// [`HunspellSpellingConfig::morfologik_dict`] is set.
    morfologik: Option<MorfologikSpeller>,
}

impl HunspellSpellingRule {
    pub fn load(data_dir: &Path, config: HunspellSpellingConfig) -> Result<Self> {
        let dir = data_dir.join(config.lang_dir).join("hunspell");
        let checker = HunspellChecker::load(&dir.join(config.aff), &dir.join(config.dic))?;

        let mut ignore = HashSet::new();
        for path in [
            dir.join("ignore.txt"),
            dir.join("spelling.txt"),
            dir.join("spelling_custom.txt"),
            data_dir.join("core/spelling_global.txt"),
        ] {
            load_list(&path, &mut ignore);
        }
        let mut prohibit = HashSet::new();
        for path in [dir.join("prohibit.txt"), dir.join("prohibit_custom.txt")] {
            load_list(&path, &mut prohibit);
        }

        let suggestion_words = if config.max_suggestions == 0 {
            Vec::new()
        } else {
            let path = match config.suggestion_file {
                Some(file) => dir.join(file),
                None => dir.join(config.dic),
            };
            let mut words = read_dictionary_words(&path);
            words.sort();
            words.dedup();
            if words.len() > MAX_SUGGESTION_DICT {
                Vec::new()
            } else {
                words
            }
        };

        // The Morfologik FSA is only loaded when it can actually be used:
        // suggestions enabled and no plain candidate list available.
        let morfologik = if config.max_suggestions == 0 || !suggestion_words.is_empty() {
            None
        } else if let Some((dict, info)) = config.morfologik_dict {
            Some(MorfologikSpeller::from_dict_file(
                &data_dir.join(dict),
                &data_dir.join(info),
                MORFOLOGIK_MAX_EDIT_DISTANCE,
            )?)
        } else {
            None
        };

        Ok(Self {
            config,
            checker,
            ignore,
            prohibit,
            suggestion_words,
            morfologik,
        })
    }

    pub fn rule_id(&self) -> &str {
        self.config.rule_id
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

    fn is_ignored_no_case(&self, word: &str) -> bool {
        self.ignore.contains(word)
            || (!lt_spell::morfologik::is_mixed_case(word)
                && self.ignore.contains(&word.to_lowercase()))
    }

    fn is_prohibited(&self, word: &str) -> bool {
        self.prohibit.contains(word)
    }

    /// Can this token be skipped by the spelling rule?
    fn can_be_ignored(&self, token: &AnalyzedTokenReadings) -> bool {
        token.is_sentence_start
            || token.is_immunized
            || token.is_ignore_spelling
            || is_url(token.surface())
            || is_email(token.surface())
            || self.ignore_word(token.surface())
    }

    /// Dictionary membership (speller or ignore list); used by the
    /// compound and gender rules that reason about word forms.
    pub fn is_known(&self, word: &str) -> bool {
        self.checker.spell(word) || self.is_ignored_no_case(word)
    }

    /// The dictionary lookup, with a retry on ASCII-normalized puso
    /// (`’`/`ʼ` → `'`) for Guaraní input variants.
    fn is_misspelled(&self, word: &str) -> bool {
        if self.is_prohibited(word) {
            return true;
        }
        // Productive hyphen compounds (`50-årsdag`, `NRK-medarbeider`) are
        // accepted when every part is a number, an abbreviation or a known
        // word; hunspell's compound rules are not supported by `lt-spell`.
        if word.contains('-') {
            let parts: Vec<&str> = word.split('-').filter(|part| !part.is_empty()).collect();
            if parts.len() > 1
                && parts.iter().all(|part| {
                    part.chars().all(|c| c.is_ascii_digit())
                        || (part.chars().count() >= 2
                            && (part.chars().all(|c| c.is_uppercase())
                                || self.is_ignored_no_case(part)
                                || self.checker.spell(part)))
                })
            {
                return false;
            }
        }
        if self.checker.spell(word) {
            return false;
        }
        let normalized = word.replace(['\u{2019}', '\u{02BC}'], "'");
        if normalized != word && self.checker.spell(&normalized) {
            return false;
        }
        true
    }

    /// `SpellingCheckRule.match` over one sentence's token stream (absolute
    /// byte offsets applied by the caller).
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
            if self.can_be_ignored(token) {
                if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                    is_first_word = false;
                }
                continue;
            }
            let word = token
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_else(|| token.surface().to_string());
            if self.is_misspelled(&word) {
                // Java `HunspellRule.match`: `cleanWord` strips one trailing
                // dot for the reported range and for `calcSuggestions`, while
                // `isMisspelled` still sees the dot (`nonWordPattern` keeps it
                // when the aff `WORDCHARS` lists `.`, as Danish does).
                let clean_word = word.strip_suffix('.').filter(|w| !w.is_empty());
                let end = match clean_word {
                    Some(clean) => token.start_pos + clean.len(),
                    None => token.end_pos(),
                };
                let mut m = self.new_rule_match(
                    token.start_pos,
                    end,
                    is_first_word && idx < non_blank.len() - 1,
                );
                m.suggestions = self.suggestions(clean_word.unwrap_or(&word));
                matches.push(m);
            }
            if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                is_first_word = false;
            }
        }
        for m in &mut matches {
            m.range = TextRange::new(
                sentence_offset + m.range.start,
                sentence_offset + m.range.end,
            );
        }
        matches
    }

    fn new_rule_match(&self, from: usize, to: usize, _first_word: bool) -> Match {
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

    /// Suggestions for a misspelled word: the Morfologik FSA when a vendored
    /// `.dict` is configured, otherwise the bounded edit-distance search over
    /// the candidate word list.
    fn suggestions(&self, word: &str) -> Vec<Suggestion> {
        if self.config.native_suggestions {
            return self.native_suggestions(word);
        }
        if word.chars().count() > MAX_SUGGESTION_WORD_LENGTH {
            return Vec::new();
        }
        if let Some(speller) = &self.morfologik {
            return self.morfologik_suggestions(speller, word);
        }
        self.edit_distance_suggestions(word)
    }

    /// Native hunspell `suggest()`, plus `HunspellRule`'s post-processing
    /// (`filterSuggestions`: drop prohibited replacements, dedup). The list is
    /// not capped here: hunspell itself caps at `MAXSUGGESTION` (15), matching
    /// the legacy engine.
    fn native_suggestions(&self, word: &str) -> Vec<Suggestion> {
        let mut out: Vec<Suggestion> = Vec::new();
        for value in self.checker.suggest(word) {
            if self.is_prohibited(&value) {
                continue;
            }
            if out.iter().any(|s| s.value == value) {
                continue;
            }
            out.push(Suggestion {
                value,
                short_description: None,
            });
        }
        out
    }

    /// Morfologik suggestions (`MorfologikSpeller.getSuggestions`, which
    /// includes LT's case adjustment): ranked by weight, deduped, capped and
    /// only computed for words of a reasonable length.
    fn morfologik_suggestions(&self, speller: &MorfologikSpeller, word: &str) -> Vec<Suggestion> {
        if word.chars().count() < MIN_SUGGESTION_WORD_LENGTH {
            return Vec::new();
        }
        let mut candidates = speller.get_suggestions(word);
        candidates.sort_by_key(|candidate| candidate.weight);
        let mut out: Vec<Suggestion> = Vec::new();
        for candidate in candidates {
            if candidate.word == word || out.iter().any(|s| s.value == candidate.word) {
                continue;
            }
            out.push(Suggestion {
                value: candidate.word,
                short_description: None,
            });
            if out.len() >= self.config.max_suggestions {
                break;
            }
        }
        out
    }

    /// Bounded edit-distance suggestions over the candidate word list.
    fn edit_distance_suggestions(&self, word: &str) -> Vec<Suggestion> {
        if self.suggestion_words.is_empty() {
            return Vec::new();
        }
        let lower = word.to_lowercase();
        let mut candidates: Vec<(usize, String)> = Vec::new();
        for candidate in &self.suggestion_words {
            let candidate_lower = candidate.to_lowercase();
            if candidate_lower.len().abs_diff(lower.len()) > MAX_SUGGESTION_DISTANCE {
                continue;
            }
            if !candidate_lower
                .chars()
                .next()
                .zip(lower.chars().next())
                .is_some_and(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
            {
                continue;
            }
            let distance = edit_distance(&lower, &candidate_lower, MAX_SUGGESTION_DISTANCE);
            if distance <= MAX_SUGGESTION_DISTANCE {
                candidates.push((distance, candidate.clone()));
            }
            if candidates.len() >= 50 {
                break;
            }
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let mut out: Vec<Suggestion> = Vec::new();
        for (_, candidate) in candidates {
            let value = if word.chars().next().is_some_and(char::is_uppercase) {
                uppercase_first(&candidate)
            } else {
                candidate
            };
            if out.iter().any(|s| s.value == value) {
                continue;
            }
            out.push(Suggestion {
                value,
                short_description: None,
            });
            if out.len() >= self.config.max_suggestions {
                break;
            }
        }
        out
    }
}

/// Levenshtein distance with an early cutoff (returns `limit + 1` when above).
fn edit_distance(a: &str, b: &str, limit: usize) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > limit {
        return limit + 1;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        let mut row_min = current[0];
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            current[j + 1] = (previous[j] + cost)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
            row_min = row_min.min(current[j + 1]);
        }
        if row_min > limit {
            return limit + 1;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

fn uppercase_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `CachingWordListLoader`: skip empty/`#` lines, cut at `#`, trim.
fn load_list(path: &Path, out: &mut HashSet<String>) {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if !trimmed.is_empty() {
            out.insert(trimmed.to_string());
        }
    }
}

/// Read the words of a Hunspell `.dic` (skip the count line, strip affix
/// flags after `/` and tab-separated comments).
fn read_dictionary_words(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = text.lines();
    lines.next();
    let mut out = Vec::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let word = line.split('\t').next().unwrap_or("");
        let word = word.split('/').next().unwrap_or("");
        let word = word.trim();
        if !word.is_empty() {
            out.push(word.to_string());
        }
    }
    out
}
