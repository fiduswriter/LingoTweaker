//! Port of `org.languagetool.rules.spelling.multitoken.MultitokenSpeller`
//! (with `EnglishMultitokenSpeller`'s configuration: word lists
//! `en/words/multiwords.txt` + `core/spelling_global.txt`, `English.
//! prepareLineForSpeller` line handling) and its filter
//! `MultitokenSpellerFilter`.
//!
//! Deviations from the legacy engine:
//! - the Guava suggestions cache is replaced by the caller's usage pattern
//!   (suggestions are only computed on filter invocation);
//! - the per-first-character candidate map is stored pre-sorted in Java
//!   `HashMap` iteration order (see `JavaBucket`) instead of a Rust `HashMap`,
//!   whose iteration order is randomized; Java's `Collections.sort` on
//!   `WeightedSuggestion` is stable and compares the weight only, so the
//!   candidate insertion order is observable in the suggestion list.

use std::collections::HashMap;
use std::sync::Arc;

use lt_spell::morfologik::WeightedSuggestion;
use unicode_normalization::UnicodeNormalization;

/// The default spelling rule's `isMisspelled` (LT
/// `SpellingCheckRule.isMisspelled` over the en-US Morfologik dictionary).
pub type IsMisspelled = Arc<dyn Fn(&str) -> bool + Send + Sync>;
/// `MultitokenSpeller.isException(original, candidate)`.
pub type IsException = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;
/// `MultitokenSpeller.getAdditionalSuggestions(word)` (`WeightedSuggestion`
/// list; Catalan uses it for `ca-ES_spelling_multitoken.dict`).
pub type AdditionalSuggestions = Arc<dyn Fn(&str) -> Vec<WeightedSuggestion> + Send + Sync>;

/// One first-character bucket of `suggestionsMap`, in Java `HashMap`
/// iteration order: keys with their candidate lists. Java iterates the
/// inner `HashMap<String, List<String>>` by hash-table slot, and
/// `Collections.sort` is stable, so that order is visible in the
/// suggestion list (`Stephen Hawkings` before `Stephen Hawking`).
type JavaBucket = Vec<(String, Vec<String>)>;

/// Builds one `JavaBucket`, preserving key insertion order for equal slots.
#[derive(Default)]
struct BucketBuilder {
    entries: Vec<(String, Vec<String>)>,
    index: HashMap<String, usize>,
}

impl BucketBuilder {
    /// `MultitokenSpeller.addToMap`.
    fn add(&mut self, key: &str, value: &str) {
        if let Some(&i) = self.index.get(key) {
            let list = &mut self.entries[i].1;
            if !list.iter().any(|v| v == value) {
                list.push(value.to_string());
            }
        } else {
            self.index.insert(key.to_string(), self.entries.len());
            self.entries
                .push((key.to_string(), vec![value.to_string()]));
        }
    }

    /// Sort the keys by their slot in the final `HashMap` table. Java
    /// doubles the table (starting at 16) whenever `size` exceeds 75% of the
    /// capacity; iteration walks the slots in index order and each slot's
    /// chain in insertion order.
    fn finish(self) -> JavaBucket {
        let capacity = java_hashmap_capacity(self.entries.len());
        let mut keyed: Vec<(u32, (String, Vec<String>))> = self
            .entries
            .into_iter()
            .map(|entry| (java_hashmap_slot(&entry.0, capacity), entry))
            .collect();
        keyed.sort_by_key(|(slot, _)| *slot);
        keyed.into_iter().map(|(_, entry)| entry).collect()
    }
}

/// `HashMap`'s table capacity for `size` stored keys (`DEFAULT_INITIAL_CAPACITY`
/// 16, doubled while `size > 0.75 * capacity`).
fn java_hashmap_capacity(size: usize) -> u32 {
    let mut capacity = 16u32;
    while size as u64 > (capacity as u64) * 3 / 4 {
        capacity *= 2;
    }
    capacity
}

/// `HashMap.hash(key)` slot index: `(h = key.hashCode()) ^ (h >>> 16)` masked
/// with the table size. `String.hashCode` hashes UTF-16 code units.
fn java_hashmap_slot(key: &str, capacity: u32) -> u32 {
    let mut h: u32 = 0;
    for unit in key.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u32::from(unit));
    }
    (h ^ (h >> 16)) & (capacity - 1)
}

pub struct MultitokenSpeller {
    by_first_char: HashMap<char, JavaBucket>,
    no_spaces_key: HashMap<String, Vec<String>>,
    is_misspelled: IsMisspelled,
    is_exception: IsException,
    additional_suggestions: AdditionalSuggestions,
}

const MAX_LENGTH_DIFF: usize = 3;

impl MultitokenSpeller {
    /// Load from word-list files applying the English
    /// `prepareLineForSpeller` semantics (keep `form` from `form\tTAG` lines
    /// only when the tag starts with NN/JJ; drop lines containing `+`).
    pub fn load_english(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        Self::load_with_prepare(paths, is_misspelled, prepare_line_english)
    }

    /// `SpanishMultitokenSpeller`: files `/es/multiwords.txt`,
    /// `/spelling_global.txt`, `/es/hyphenated_words.txt` with the Spanish
    /// `prepareLineForSpeller` (keep typed `N*`/`_Latin_`/`LOC_ADV` entries).
    pub fn load_spanish(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        Self::load_with_prepare(paths, is_misspelled, prepare_line_spanish)
    }

    /// `FrenchMultitokenSpeller`: files `/fr/multiwords.txt`,
    /// `/spelling_global.txt`, `/fr/hyphenated_words.txt` with the French
    /// `prepareLineForSpeller` (keep typed `Z*`/`N*`/`A` entries, drop
    /// `Ho Chi Minh`).
    pub fn load_french(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        Self::load_with_prepare(paths, is_misspelled, prepare_line_french)
    }

    /// `PortugueseMultitokenSpeller`: files `/pt/multiwords.txt`,
    /// `/spelling_global.txt`, `/pt/hyphenated_words.txt` with the
    /// Portuguese `prepareLineForSpeller` (keep typed `N*`/`_Latin_`
    /// entries).
    pub fn load_portuguese(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        Self::load_with_prepare(paths, is_misspelled, prepare_line_portuguese)
    }

    /// `DutchMultitokenSpeller`: files `/nl/multiwords.txt`,
    /// `/spelling_global.txt` with the base `Language.prepareLineForSpeller`
    /// (identity) and the Dutch `isException` override (a trailing `s`/`-`
    /// or `'s`/`’s` is accepted as a real inflected form).
    pub fn load_dutch(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        let mut speller = Self::load_with_prepare(paths, is_misspelled, prepare_line_dutch);
        speller.is_exception = Arc::new(|original, candidate| {
            let len = original.chars().count();
            if len <= 2 {
                return false;
            }
            let prefix = |n: usize| -> String { original.chars().take(len - n).collect() };
            if prefix(1) == candidate && (original.ends_with('s') || original.ends_with('-')) {
                return true;
            }
            if prefix(2) == candidate
                && (original.ends_with("'s") || original.ends_with("\u{2019}s"))
            {
                return true;
            }
            false
        });
        speller
    }

    /// `CatalanMultitokenSpeller`: files `/ca/multiwords.txt`,
    /// `/spelling_global.txt`, `/ca/hyphenated_words.txt` with the Catalan
    /// `prepareLineForSpeller`, plus `getAdditionalSuggestions` from
    /// `ca-ES_spelling_multitoken.dict` (`CatalanMorfologikMultitokenSpeller`).
    pub fn load_catalan(
        paths: &[std::path::PathBuf],
        is_misspelled: IsMisspelled,
        additional: AdditionalSuggestions,
    ) -> Self {
        let mut speller = Self::load_with_prepare(
            paths,
            is_misspelled,
            crate::ca::spelling::prepare_line_for_speller,
        );
        speller.additional_suggestions = additional;
        speller
    }

    fn load_with_prepare(
        paths: &[std::path::PathBuf],
        is_misspelled: IsMisspelled,
        prepare: fn(&str) -> Vec<String>,
    ) -> Self {
        let mut by_first_char: HashMap<char, BucketBuilder> = HashMap::new();
        let mut no_spaces_key: HashMap<String, Vec<String>> = HashMap::new();
        for path in paths {
            let Ok(content) = lt_data::fs::read_to_string(path) else {
                continue;
            };
            for line_original in content.lines() {
                if line_original.is_empty() || line_original.starts_with('#') {
                    continue;
                }
                let line = prepare(line_original.split('#').next().unwrap_or("").trim());
                for line in line {
                    if line.is_empty() {
                        continue;
                    }
                    let key = normalize_key(&line);
                    if !key.contains(' ') {
                        // one-token suggestions come from the spelling rule
                        continue;
                    }
                    by_first_char
                        .entry(key.chars().next().unwrap_or(' '))
                        .or_default()
                        .add(&key, &line);
                    add_to_map(&mut no_spaces_key, &key.replace(' ', ""), line);
                }
            }
        }
        Self {
            by_first_char: finish_buckets(by_first_char),
            no_spaces_key,
            is_misspelled,
            is_exception: Arc::new(|_, _| false),
            additional_suggestions: Arc::new(|_| Vec::new()),
        }
    }

    /// Load from word-list files applying German's `prepareLineForSpeller`
    /// (`form[/TAG]` lines expand to `form` plus the `E`/`S`/`N` suffixed
    /// forms), with the German `isException` override
    /// (`GermanMultitokenSpeller`).
    pub fn load_german(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        let mut speller = Self::load_german_lines(paths, is_misspelled);
        speller.is_exception = Arc::new(|original, candidate| {
            !original.is_empty()
                && original
                    .get(..original.len() - 1)
                    .is_some_and(|rest| rest == candidate)
                && (original.ends_with('s') || original.ends_with('-'))
        });
        speller
    }

    fn load_german_lines(paths: &[std::path::PathBuf], is_misspelled: IsMisspelled) -> Self {
        let mut by_first_char: HashMap<char, BucketBuilder> = HashMap::new();
        let mut no_spaces_key: HashMap<String, Vec<String>> = HashMap::new();
        for path in paths {
            let Ok(content) = lt_data::fs::read_to_string(path) else {
                continue;
            };
            for line_original in content.lines() {
                if line_original.is_empty() || line_original.starts_with('#') {
                    continue;
                }
                let tag_line = line_original
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                for line in prepare_line_german(&tag_line) {
                    if line.is_empty() {
                        continue;
                    }
                    let key = normalize_key(&line);
                    if !key.contains(' ') {
                        // one-token suggestions come from the spelling rule
                        continue;
                    }
                    by_first_char
                        .entry(key.chars().next().unwrap_or(' '))
                        .or_default()
                        .add(&key, &line);
                    add_to_map(&mut no_spaces_key, &key.replace(' ', ""), line);
                }
            }
        }
        Self {
            by_first_char: finish_buckets(by_first_char),
            no_spaces_key,
            is_misspelled,
            is_exception: Arc::new(|_, _| false),
            additional_suggestions: Arc::new(|_| Vec::new()),
        }
    }

    pub fn suggestions(
        &self,
        original_word: &str,
        are_tokens_accepted_by_speller: bool,
    ) -> Vec<String> {
        let original_word = split_whitespace_normalize(original_word);
        self.compute_suggestions(&original_word, are_tokens_accepted_by_speller)
    }

    /// `MultitokenSpeller.isException` (the language override).
    pub fn is_exception(&self, original: &str, candidate: &str) -> bool {
        (self.is_exception)(original, candidate)
    }

    fn compute_suggestions(&self, original_word: &str, accepted: bool) -> Vec<String> {
        let word = &original_word.replace("- ", "-").replace(" -", "-");
        if self.discard_run_on_words(word) {
            return Vec::new();
        }
        let normalized_word = normalize_key(word);
        let mut weighted: Vec<(usize, String)> = Vec::new();
        let normalized_no_spaces = normalized_word.replace(' ', "");
        if let Some(candidates) = self.no_spaces_key.get(&normalized_no_spaces) {
            if self.stop_searching(candidates, original_word) {
                return Vec::new();
            }
            for candidate in candidates {
                weighted.push((0, candidate.clone()));
            }
        }
        if weighted.is_empty() {
            let Some(first_char) = normalized_word.chars().next() else {
                return Vec::new();
            };
            if let Some(bucket) = self.by_first_char.get(&first_char) {
                for (normalized_candidate, candidates) in bucket {
                    if self.stop_searching(candidates, original_word) {
                        return Vec::new();
                    }
                    if normalized_candidate
                        .chars()
                        .count()
                        .abs_diff(word.chars().count())
                        > MAX_LENGTH_DIFF
                    {
                        continue;
                    }
                    let candidate_parts = split_by_space(normalized_candidate);
                    let word_parts = split_by_space(&normalized_word);
                    let distances = distances_per_word(&candidate_parts, &word_parts);
                    let total_distance: usize = distances.iter().sum();
                    if total_distance < 1 {
                        for candidate in candidates {
                            weighted.push((0, candidate.clone()));
                        }
                        // several candidates with different casing are allowed
                        if weighted.len() == 2 {
                            break;
                        }
                        continue;
                    }
                    if normalized_candidate.chars().count() < 7 {
                        continue;
                    }
                    let mut exceeds = false;
                    for (i, &dist) in distances.iter().enumerate() {
                        let max_distance = if word_parts[i].chars().count() > 5
                            && candidate_parts[i].chars().count() > 4
                        {
                            2
                        } else {
                            1
                        };
                        if dist > max_distance {
                            exceeds = true;
                            break;
                        }
                    }
                    if exceeds {
                        continue;
                    }
                    if total_distance <= max_edit_distance(normalized_candidate, &normalized_word) {
                        for candidate in candidates {
                            weighted.push((total_distance, candidate.clone()));
                        }
                    }
                }
            }
        }
        // `MultitokenSpeller.getAdditionalSuggestions` (Catalan: the
        // `ca-ES_spelling_multitoken.dict` weighted suggestions). A
        // suggestion equal to the original word cancels all suggestions.
        for additional in (self.additional_suggestions)(word) {
            if additional.word == original_word {
                return Vec::new();
            }
            weighted.push((additional.weight.max(0) as usize, additional.word));
        }
        if weighted.is_empty() {
            return Vec::new();
        }
        // `Collections.sort` is stable and `WeightedSuggestion.compareTo`
        // compares the weight only, so equal weights keep insertion order.
        weighted.sort_by_key(|(weight, _)| *weight);
        let weight_first = weighted[0].0;
        // Java: `getWord().toUpperCase().equals(originalWord)`, i.e. an exact
        // match after upper-casing (only an all-upper input can match).
        if accepted && weighted[0].1.to_uppercase() == original_word {
            // don't correct all-upper case words accepted by the speller
            return Vec::new();
        }
        if accepted && weight_first > 1 {
            return Vec::new();
        }
        let mut results: Vec<String> = Vec::new();
        for (weight, cand) in weighted {
            if weight.saturating_sub(weight_first) < 1 && !results.contains(&cand) {
                results.push(cand);
            }
        }
        results
    }

    fn stop_searching(&self, candidates: &[String], original_word: &str) -> bool {
        for candidate in candidates {
            if (self.is_exception)(original_word, candidate) {
                return true;
            }
            if candidate == original_word {
                return true;
            }
        }
        for candidate in candidates {
            if *candidate == candidate.to_lowercase() && titlecase(candidate) == original_word {
                return true;
            }
        }
        false
    }

    fn discard_run_on_words(&self, underlined_error: &str) -> bool {
        let parts = split_by_space(underlined_error);
        if parts.len() != 2 {
            return false;
        }
        if lt_tagger::is_capitalized_word(parts[1]) {
            return false;
        }
        if parts[0].is_empty() || parts[1].is_empty() {
            return true;
        }
        let first = parts[0];
        let second = parts[1];
        let sugg1a = &first[..first.len() - first.chars().last().map_or(1, |c| c.len_utf8())];
        let sugg1b: String = first
            .chars()
            .last()
            .into_iter()
            .chain(second.chars())
            .collect();
        if !(self.is_misspelled)(sugg1a) && !(self.is_misspelled)(&sugg1b) {
            return true;
        }
        let sugg2a: String = first.chars().chain(second.chars().take(1)).collect();
        let sugg2b: String = second.chars().skip(1).collect();
        !(self.is_misspelled)(&sugg2a) && !(self.is_misspelled)(&sugg2b)
    }
}

fn split_whitespace_normalize(word: &str) -> String {
    word.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `English.prepareLineForSpeller`: the line already has its `#` comment
/// stripped; keep `form` from `form\tTAG` only for NN/JJ tags; drop lines
/// containing the Morfologik separator `+`.
fn prepare_line_english(line: &str) -> Vec<String> {
    if line.contains('+') {
        return Vec::new();
    }
    let mut parts = line.split('\t');
    let form = parts.next().unwrap_or("").trim();
    match parts.next() {
        Some(tag) => {
            let tag = tag.trim();
            if tag.starts_with("NN") || tag.starts_with("JJ") {
                vec![form.to_string()]
            } else {
                Vec::new()
            }
        }
        None => vec![line.to_string()],
    }
}

fn add_to_map(map: &mut HashMap<String, Vec<String>>, key: &str, value: String) {
    let list = map.entry(key.to_string()).or_default();
    if !list.contains(&value) {
        list.push(value);
    }
}

/// `Spanish.prepareLineForSpeller`: `form\tTAG` / `form;TAG` lines keep the
/// form only for `N*`, `_Latin_` and `LOC_ADV` tags; everything else passes
/// through (the `#` comment was already stripped by the caller).
fn prepare_line_spanish(line: &str) -> Vec<String> {
    let form_tag: Vec<&str> = line.split(['\t', ';']).collect();
    if form_tag.len() > 1 {
        let tag = form_tag[1].trim();
        if tag.starts_with('N') || tag == "_Latin_" || tag == "LOC_ADV" {
            return vec![form_tag[0].trim().to_string()];
        }
        return vec![String::new()];
    }
    vec![line.to_string()]
}

/// `Portuguese.prepareLineForSpeller`: `form\tTAG` / `form;TAG` lines keep
/// the form only for `N*`/`_Latin_` tags; untagged lines keep the line.
fn prepare_line_portuguese(line: &str) -> Vec<String> {
    let form_tag: Vec<&str> = line.split(['\t', ';']).collect();
    if form_tag.len() > 1 {
        let tag = form_tag[1].trim();
        if tag.starts_with('N') || tag == "_Latin_" {
            return vec![form_tag[0].trim().to_string()];
        }
        return vec![String::new()];
    }
    vec![line.to_string()]
}

/// `French.prepareLineForSpeller`: `form\tTAG` / `form;TAG` lines keep the
/// form only for `Z*`/`N*`/`A` tags; `Ho Chi Minh` is dropped (the `#`
/// comment was already stripped by the caller).
fn prepare_line_french(line: &str) -> Vec<String> {
    let form_tag: Vec<&str> = line.split(['\t', ';']).collect();
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

/// `Dutch` keeps the base `Language.prepareLineForSpeller` semantics
/// (`Collections.singletonList(s)`): the whole line is the entry.
fn prepare_line_dutch(line: &str) -> Vec<String> {
    vec![line.to_string()]
}

fn finish_buckets(builders: HashMap<char, BucketBuilder>) -> HashMap<char, JavaBucket> {
    builders
        .into_iter()
        .map(|(first_char, builder)| (first_char, builder.finish()))
        .collect()
}

/// German `Language.prepareLineForSpeller`: `form[/TAG]` → `form` plus the
/// `form`+`e`/`s`/`n` forms for the E/S/N tag letters.
fn prepare_line_german(line: &str) -> Vec<String> {
    let form_tag: Vec<&str> = line.split('/').collect();
    let form = form_tag[0];
    let mut results = vec![form.to_string()];
    let tag = if form_tag.len() == 2 { form_tag[1] } else { "" };
    if tag.contains('E') {
        results.push(format!("{form}e"));
    }
    if tag.contains('S') {
        results.push(format!("{form}s"));
    }
    if tag.contains('N') {
        results.push(format!("{form}n"));
    }
    results
}

fn normalize_key(word: &str) -> String {
    remove_diacritics(&word.to_lowercase()).replace('-', " ")
}

pub fn remove_diacritics(s: &str) -> String {
    s.nfd()
        .filter(|c| !c.is_combining_mark_nonspacing())
        .nfc()
        .collect()
}

trait CombiningMark {
    fn is_combining_mark_nonspacing(&self) -> bool;
}

impl CombiningMark for char {
    fn is_combining_mark_nonspacing(&self) -> bool {
        matches!(*self as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
    }
}

fn split_by_space(s: &str) -> Vec<&str> {
    s.split(' ').filter(|p| !p.is_empty()).collect()
}

fn distances_per_word(parts1: &[&str], parts2: &[&str]) -> Vec<usize> {
    if parts1.len() == parts2.len() && parts1.len() > 1 {
        parts1
            .iter()
            .zip(parts2)
            .map(|(a, b)| levenshtein_distance(a, b))
            .collect()
    } else {
        vec![levenshtein_distance(&parts1.join(" "), &parts2.join(" "))]
    }
}

fn max_edit_distance(normalized_candidate: &str, normalized_word: &str) -> usize {
    let total_length = normalized_word.chars().count();
    let correct_length = total_length.saturating_sub(number_of_correct_chars(
        normalized_candidate,
        normalized_word,
    ));
    let first_char_wrong: f32 = first_character_distances(normalized_candidate, normalized_word)
        .iter()
        .sum();
    if correct_length <= 7 {
        (2.0 - first_char_wrong) as usize
    } else {
        (2.0 + 0.25 * (correct_length - 7) as f32 - 0.6 * first_char_wrong) as usize
    }
}

fn first_character_distances(s1: &str, s2: &str) -> Vec<f32> {
    let parts1 = split_by_space(s1);
    let parts2 = split_by_space(s2);
    if parts1.len() == parts2.len() && parts1.len() == 2 {
        // Java sums the distance of *every* word's first character
        parts1
            .iter()
            .zip(&parts2)
            .map(|(a, b)| {
                char_distance(
                    a.chars().next().unwrap_or(' '),
                    b.chars().next().unwrap_or(' '),
                )
            })
            .collect()
    } else {
        vec![0.0]
    }
}

fn char_distance(a: char, b: char) -> f32 {
    if a == b {
        return 0.0;
    }
    if (a == 's' && b == 'z') || (a == 'z' && b == 's') {
        return 0.2;
    }
    if (a == 'b' && b == 'v') || (a == 'v' && b == 'b') {
        return 0.2;
    }
    if (a == 'i' && b == 'y') || (a == 'y' && b == 'i') {
        return 0.0;
    }
    1.0
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    if s1.replace(' ', "") == s2.replace(' ', "") {
        return 0;
    }
    let mut distance = plain_levenshtein(s1, s2);
    let ns1 = normalize_similar_chars(s1);
    let ns2 = normalize_similar_chars(s2);
    if s1 != ns1 || s2 != ns2 {
        distance = distance.min(plain_levenshtein(&ns1, &ns2));
    }
    let anagram = is_anagram(s1, s2);
    if distance > 1 && anagram {
        distance -= 1;
    }
    if distance > 0 && s1.chars().count() == s2.chars().count() && anagram {
        distance = 1;
    }
    distance
}

fn normalize_similar_chars(s: &str) -> String {
    s.replace("y", "i").replace("ko", "co").replace("ka", "ca")
}

fn is_anagram(s1: &str, s2: &str) -> bool {
    let mut a: Vec<char> = s1.chars().collect();
    let mut b: Vec<char> = s2.chars().collect();
    if a.len() != b.len() {
        return false;
    }
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

fn number_of_correct_chars(s1: &str, s2: &str) -> usize {
    let parts1: Vec<&str> = s1.split(' ').collect();
    let parts2: Vec<&str> = s2.split(' ').collect();
    let mut correct = 0;
    if parts1.len() == parts2.len() && parts1.len() > 1 {
        for (a, b) in parts1.iter().zip(parts2.iter()) {
            if a == b {
                correct += a.chars().count();
            }
        }
    }
    correct
}

fn plain_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur: Vec<usize> = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn titlecase(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut convert_next = true;
    for ch in s.chars() {
        if ch.is_whitespace() || ch == '-' {
            convert_next = true;
            out.push(ch);
        } else if convert_next {
            out.extend(ch.to_uppercase());
            convert_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;
    use std::path::Path;

    fn never_misspelled() -> IsMisspelled {
        Arc::new(|_| false)
    }

    #[test]
    fn dutch_multitoken_is_exception() {
        let speller = MultitokenSpeller::load_dutch(&[], never_misspelled());
        assert!(speller.is_exception("fiets", "fiet"));
        assert!(speller.is_exception("fiets-", "fiets"));
        assert!(speller.is_exception("fiets's", "fiets"));
        assert!(speller.is_exception("fiets\u{2019}s", "fiets"));
        assert!(!speller.is_exception("fiets", "fiets"));
        assert!(!speller.is_exception("ab", "a"));
        assert!(!speller.is_exception("fiets", "fietse"));
    }

    #[test]
    fn loads_english_word_lists_and_suggests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !root.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let speller = MultitokenSpeller::load_english(
            &[
                root.join("en/words/multiwords.txt"),
                root.join("core/spelling_global.txt"),
            ],
            never_misspelled(),
        );
        assert!(!speller.by_first_char.is_empty());
        // normalized (accent-stripped, lower-cased) keys are present
        assert!(speller
            .by_first_char
            .get(&'m')
            .is_some_and(|b| b.iter().any(|(key, _)| key == "menage a trois")));
        // exact normalized key hit → suggestions with weight 0
        let sugg = speller.suggestions("menage a trois", false);
        assert!(!sugg.is_empty());
        assert!(sugg.iter().any(|s| s == "ménage à trois"));
        // single-token entries are not part of the multitoken speller
        let sugg = speller.suggestions("Microsoft", false);
        assert!(sugg.is_empty());
    }
}
