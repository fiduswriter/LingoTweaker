//! Morfologik speller dictionary lookup and suggestion generation.
//!
//! Dictionary entry format (empirically verified against the vendored
//! `data/en/hunspell/*` dictionaries, which are Morfologik hunspell
//! conversions): every stored sequence is `<word><separator><freq-code>` where
//! the separator comes from `fsa.dict.separator` (`+` for en_US/en_AU/en_CA/
//! en_NZ, `_` for en_GB) and the annotation is exactly one byte in `'A'..='Z'`
//! giving the word's frequency range ('A' = least frequent; see morfologik
//! `Speller.getFrequency`, `fsa.dict.frequency-included=true` in all vendored
//! files). This differs from the tagger dictionaries, which store
//! `surface + [trim-encoded stem] + tag`.
//!
//! Deviations from upstream (morfologik-speller `Speller`, LT
//! `MorfologikSpeller`):
//! - Suggestions use a pragmatic generate-and-validate candidate generator
//!   (deletions, adjacent transpositions, replacements, insertions up to edit
//!   distance 2, validated against the expanded word set) instead of Oflazer's
//!   error-tolerant FSA walk; the exact restricted-Damerau (OSA) distance is
//!   recomputed per candidate for ranking.
//! - Candidates are ordered by distance, then stored frequency (higher
//!   first), then alphabetically for determinism. LT orders by distance, then
//!   stored frequency.
//! - `fsa.dict.speller.replacement-pairs` ("a ei", "ph f", ...) are not
//!   applied during suggestion generation.
//! - Edit-distance-2 generation is skipped for words longer than
//!   [`MAX_DISTANCE_2_LENGTH`] chars; LT caps its search by candidate count
//!   (`UPPER_SEARCH_LIMIT`) and word length (`word.length() < 50`) instead.
//! - Diacritic equivalence (`fsa.dict.speller.ignore-diacritics`) is not used
//!   in candidate generation.

use std::path::Path;

use lt_core::{CoreError, Result};
use lt_tagger::{Cfsa2, DictionaryInfo};

/// Maximum length of a word for which edit-distance-2 candidates are generated.
const MAX_DISTANCE_2_LENGTH: usize = 12;
const FIRST_FREQ_CODE: u8 = b'A';
const MAX_FREQ: u8 = 25;

const DEFAULT_SEPARATOR: char = '+';

/// A Morfologik speller dictionary, looked up on demand from the compressed
/// automaton (expanding it into a `HashMap` cost ~200 MB for `de_DE.dict`,
/// D-033), with LT capitalization semantics (`is_correct`) and edit-distance
/// suggestions.
#[derive(Debug, Clone)]
pub struct SpellChecker {
    automaton: Cfsa2,
    separator: u8,
    frequency_included: bool,
    word_count: usize,
    alphabet: Vec<char>,
    convert_case: bool,
    ignore_punctuation: bool,
    ignore_numbers: bool,
    ignore_camel_case: bool,
    ignore_all_uppercase: bool,
}

impl SpellChecker {
    /// Load `.dict` + `.info` pair and expand the automaton into a word set.
    pub fn new(dict_path: &Path, info_path: &Path) -> Result<Self> {
        let info_text = lt_data::fs::read_to_string(info_path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", info_path.display())))?;
        let info = DictionaryInfo::parse(&info_text)?;

        if let Some(encoding) = info.encoding() {
            if !encoding.eq_ignore_ascii_case("utf-8") {
                return Err(CoreError::Parse(
                    "dict".into(),
                    format!("unsupported speller dictionary encoding: {encoding}"),
                ));
            }
        }

        let separator = info
            .fields
            .get("fsa.dict.separator")
            .and_then(|s| s.chars().next())
            .unwrap_or(DEFAULT_SEPARATOR);
        if !separator.is_ascii() {
            return Err(CoreError::Parse(
                "dict".into(),
                format!("separator must be a single byte, got {separator:?}"),
            ));
        }
        let sep = separator as u8;

        let flag = |key: &str, default: bool| -> bool {
            info.fields
                .get(key)
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(default)
        };
        // Boolean attributes fall back to morfologik DictionaryMetadata
        // defaults when absent from the .info file.
        let frequency_included = flag("fsa.dict.frequency-included", false);
        let convert_case = flag("fsa.dict.speller.convert-case", true);
        let ignore_punctuation = flag("fsa.dict.speller.ignore-punctuation", true);
        let ignore_numbers = flag("fsa.dict.speller.ignore-numbers", true);
        let ignore_camel_case = flag("fsa.dict.speller.ignore-camel-case", true);
        let ignore_all_uppercase = flag("fsa.dict.speller.ignore-all-uppercase", true);

        let dict_bytes = lt_data::fs::read(dict_path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", dict_path.display())))?;
        let automaton = Cfsa2::parse(&dict_bytes)?;

        // one traversal for the suggestion alphabet and the word count (the
        // words themselves are decoded per lookup; morfologik dictionaries
        // list each surface once, so counting sequences counts words)
        let mut alphabet: std::collections::BTreeSet<char> = Default::default();
        let mut word_count = 0usize;
        automaton.visit_sequences(automaton.root_node(), &mut |seq: &[u8]| {
            let surface = match seq.iter().position(|&b| b == sep) {
                Some(pos) if pos > 0 => &seq[..pos],
                Some(_) => return,
                None => seq,
            };
            if surface.is_empty() {
                return;
            }
            word_count += 1;
            for c in String::from_utf8_lossy(surface).chars() {
                alphabet.insert(c);
            }
        });

        Ok(Self {
            automaton,
            separator: sep,
            frequency_included,
            word_count,
            alphabet: alphabet.into_iter().collect(),
            convert_case,
            ignore_punctuation,
            ignore_numbers,
            ignore_camel_case,
            ignore_all_uppercase,
        })
    }

    /// Frequency of `word` (0 when the entry has none), or `None` when the
    /// word is not in the dictionary. Decodes the automaton on demand.
    fn lookup_freq(&self, word: &str) -> Option<u8> {
        let (node, key_is_final) = self
            .automaton
            .walk_final(self.automaton.root_node(), word.as_bytes())?;
        let mut freq = if key_is_final { Some(0) } else { None };
        let sep = self.separator;
        let frequency_included = self.frequency_included;
        // Only the separator arcs carry this word's annotations; walking the
        // letter arcs would traverse every longer word below this node.
        self.automaton
            .visit_sequences_with_first(node, sep, &mut |seq: &[u8]| {
                let ann = &seq[1..];
                freq = Some(if frequency_included && !ann.is_empty() {
                    ((ann[ann.len() - 1] as i32) - (FIRST_FREQ_CODE as i32))
                        .clamp(0, MAX_FREQ as i32) as u8
                } else {
                    0
                });
            });
        freq
    }

    /// Exact-dictionary membership test.
    pub fn contains(&self, word: &str) -> bool {
        self.lookup_freq(word).is_some()
    }

    pub fn set_ignore_all_uppercase(&mut self, value: bool) {
        self.ignore_all_uppercase = value;
    }

    /// See [`Self::set_ignore_all_uppercase`].
    pub fn set_ignore_camel_case(&mut self, value: bool) {
        self.ignore_camel_case = value;
    }

    /// LT capitalization semantics: a word is correct if found as-is; a word
    /// with initial capital if its lowercase form is found; an all-uppercase
    /// word if its lowercase or initial-uppercase form is found. Empty words
    /// are correct (LT `MorfologikSpeller.isMisspelled` short-circuits them).
    pub fn is_correct(&self, word: &str) -> bool {
        if word.is_empty() {
            return true;
        }
        if self.ignore_punctuation
            && word.chars().count() == 1
            && !word.chars().next().unwrap().is_alphabetic()
        {
            return true;
        }
        if self.ignore_numbers && word.chars().any(|c| c.is_numeric()) {
            return true;
        }
        if self.ignore_camel_case && is_camel_case(word) {
            return true;
        }
        if self.ignore_all_uppercase && is_all_uppercase(word) {
            return true;
        }
        if self.contains(word) {
            return true;
        }
        if self.convert_case && !is_mixed_case(word) {
            if self.contains(&word.to_lowercase()) {
                return true;
            }
            if is_all_uppercase(word) {
                let mut chars = word.chars();
                if let Some(first) = chars.next() {
                    let mut initial: String = first.to_uppercase().collect();
                    initial.extend(chars.flat_map(|c| c.to_lowercase()));
                    if self.contains(&initial) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Up to `max` suggestions for `word`, ordered by edit distance, then
    /// stored frequency, then alphabetically. Empty when the word is in the
    /// dictionary as-is.
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        if word.is_empty() || max == 0 || self.contains(word) {
            return Vec::new();
        }
        let len = word.chars().count();
        let mut candidates: std::collections::BTreeSet<String> = Default::default();
        let edits1 = self.edits1(word);
        for cand in &edits1 {
            if cand != word && self.contains(cand) {
                candidates.insert(cand.clone());
            }
        }
        if len <= MAX_DISTANCE_2_LENGTH {
            for first in &edits1 {
                for second in self.edits1(first) {
                    if second != word && self.contains(&second) && !candidates.contains(&second) {
                        candidates.insert(second);
                    }
                }
            }
        }

        let mut ranked: Vec<(u8, u8, String)> = candidates
            .into_iter()
            .map(|cand| {
                let dist = osa_distance(word, &cand).min(u8::MAX as usize) as u8;
                let freq = self.lookup_freq(&cand).unwrap_or(0);
                (dist, freq, cand)
            })
            .filter(|(dist, _, _)| *dist as usize <= 2)
            .collect();
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.cmp(&b.2))
        });

        for (_, _, cand) in &mut ranked {
            apply_case_pattern(word, cand, self.convert_case);
        }
        let mut seen = std::collections::HashSet::new();
        ranked
            .into_iter()
            .filter(|(_, _, cand)| seen.insert(cand.clone()))
            .take(max)
            .map(|(_, _, cand)| cand)
            .collect()
    }

    /// Number of distinct words in the dictionary.
    pub fn word_count(&self) -> usize {
        self.word_count
    }

    /// Stored frequency range (0..=25, 0 = least frequent) of a word.
    pub fn frequency(&self, word: &str) -> Option<u8> {
        self.lookup_freq(word)
    }

    fn edits1(&self, word: &str) -> Vec<String> {
        let chars: Vec<char> = word.chars().collect();
        let n = chars.len();
        let alphabet = &self.alphabet;
        let mut out = Vec::with_capacity(4 * (n + 1) * (alphabet.len() + 1));
        for i in 0..n {
            let mut s = String::with_capacity(n - 1);
            s.extend(&chars[..i]);
            s.extend(&chars[i + 1..]);
            out.push(s);
        }
        for i in 0..n.saturating_sub(1) {
            let mut s = String::with_capacity(n);
            s.extend(&chars[..i]);
            s.push(chars[i + 1]);
            s.push(chars[i]);
            s.extend(&chars[i + 2..]);
            out.push(s);
        }
        for i in 0..n {
            for &c in alphabet {
                if c != chars[i] {
                    let mut s = String::with_capacity(n);
                    s.extend(&chars[..i]);
                    s.push(c);
                    s.extend(&chars[i + 1..]);
                    out.push(s);
                }
            }
        }
        for i in 0..=n {
            for &c in alphabet {
                let mut s = String::with_capacity(n + 1);
                s.extend(&chars[..i]);
                s.push(c);
                s.extend(&chars[i..]);
                out.push(s);
            }
        }
        out
    }
}

/// Mirrors LT `MorfologikSpeller.getSuggestions`: uppercase the suggestion
/// (fully or its first character) when the query word has the matching case
/// pattern, keeping mixed-case candidates untouched.
fn apply_case_pattern(word: &str, candidate: &mut String, convert_case: bool) {
    if !convert_case {
        return;
    }
    if is_all_uppercase(word) {
        let upper = candidate.to_uppercase();
        if upper != *word && !is_mixed_case(candidate) {
            *candidate = upper;
        }
    } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
        let mut upper = String::with_capacity(candidate.len());
        let mut chars = candidate.chars();
        if let Some(c) = chars.next() {
            upper.extend(c.to_uppercase());
            upper.push_str(chars.as_str());
        }
        if upper != *word && !is_mixed_case(candidate) {
            *candidate = upper;
        }
    }
}

fn is_all_uppercase(word: &str) -> bool {
    !word.chars().any(|c| c.is_alphabetic() && c.is_lowercase())
}

fn is_not_all_lowercase(word: &str) -> bool {
    word.chars().any(|c| c.is_alphabetic() && !c.is_lowercase())
}

fn is_capitalized_word(word: &str) -> bool {
    match word.chars().next() {
        Some(c) if c.is_uppercase() => !word
            .chars()
            .skip(1)
            .any(|c| c.is_alphabetic() && !c.is_lowercase()),
        _ => false,
    }
}

fn is_mixed_case(word: &str) -> bool {
    !is_all_uppercase(word) && !is_capitalized_word(word) && is_not_all_lowercase(word)
}

fn is_camel_case(word: &str) -> bool {
    match word.chars().next() {
        Some(c) if c.is_uppercase() => {
            let second_lower = word.chars().nth(1).is_none_or(|c| c.is_lowercase());
            !is_all_uppercase(word)
                && !is_capitalized_word(word)
                && second_lower
                && is_not_all_lowercase(word)
        }
        _ => false,
    }
}

pub(crate) fn osa_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev2: Vec<usize> = vec![0; b.len() + 1];
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur: Vec<usize> = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                cur[j] = cur[j].min(prev2[j - 2] + 1);
            }
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    fn en_us() -> Option<SpellChecker> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/hunspell");
        if !dir.lt_exists() {
            return None;
        }
        Some(SpellChecker::new(&dir.join("en_US.dict"), &dir.join("en_US.info")).unwrap())
    }

    #[test]
    fn entries_are_word_plus_frequency_code() {
        let Some(speller) = en_us() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        // Entry format finding: `<word>+<freq code 'A'..'Z'>`, 'A' least frequent.
        let freq = speller.frequency("the").expect("'the' in dictionary");
        assert!(freq <= MAX_FREQ);
        assert!(
            speller.frequency("0th").is_some(),
            "words with digits are stored"
        );
        assert!(
            speller.frequency("USA").is_some(),
            "capitalized entries are stored"
        );
    }

    #[test]
    fn osa_distance_matches_reference() {
        assert_eq!(osa_distance("teh", "the"), 1);
        assert_eq!(osa_distance("kitten", "sitting"), 3);
        assert_eq!(osa_distance("abc", "ab"), 1);
        assert_eq!(osa_distance("", "ab"), 2);
    }
}
