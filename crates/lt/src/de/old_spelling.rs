//! `org.languagetool.rules.de.OldSpellingRule` (`OLD_SPELLING_RULE`):
//! finds spellings that were only correct in the pre-reform orthography.
//!
//! The Java class builds two Aho-Corasick tries over `/de/alt_neu.csv`
//! (normal + sentence-start capitalization) and reverses the hit list to keep
//! the longest match per start position. The Rust port keeps the same maps
//! and selection semantics with a first-character-bucketed scanner (the data
//! set is ~1k entries; no Aho-Corasick dependency is needed).

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedSentence, Match, Result, Suggestion, TextRange};

pub const RULE_ID: &str = "OLD_SPELLING_RULE";
const MESSAGE: &str = "Diese Schreibweise war nur in der alten Rechtschreibung korrekt.";
const SHORT_MESSAGE: &str = "Alte Rechtschreibung";
const SUFFIX_MESSAGE: &str =
    " Das Wort wird mit 'ss' geschrieben, wenn davor eine kurz gesprochene Silbe steht.";

/// `OldSpellingRule.EXCEPTIONS`.
const EXCEPTIONS: &[&str] = &[
    "Schloß Holte",
    "Schloß Neuhaus",
    "Schloß Ricklingen",
    "Schloß-Nauses",
    "Schloß Rötteln",
    "Klinikum Schloß Winnenden",
    "Grazer Schloßberg",
    "Höchster Schloß",
    "Bell Telephone",
    "Telephone Company",
    "American Telephone",
    "England Telephone",
    "Mobile Telephone",
    "Cordless Telephone",
    "Telephone Line",
    "World Telephone",
    "Tip Top",
    "Hans Joachim Blaß",
    "kurz fassen",
];

/// A pattern with its replacement (`alt_neu.csv` value, `|`-separated).
struct Entry {
    pattern: String,
    value: String,
}

pub struct OldSpellingRule {
    trie: Vec<Entry>,
    sentence_start_trie: Vec<Entry>,
    by_first: HashMap<char, Vec<usize>>,
    by_first_start: HashMap<char, Vec<usize>>,
    /// de-AT: `Geschoß` is correct in both spellings (pronunciation).
    austrian: bool,
}

impl OldSpellingRule {
    pub fn load(
        data_dir: &Path,
        variant: &str,
        synth: &lt_tagger::GermanSynthesizer,
    ) -> Result<Self> {
        let path = data_dir.join("de/words/alt_neu.csv");
        let mut synth_cache = std::collections::HashMap::new();
        let trie = get_coherency_map(&path, false, synth, &mut synth_cache)?;
        let sentence_start_trie = get_coherency_map(&path, true, synth, &mut synth_cache)?;
        Ok(Self {
            by_first: index_by_first(&trie),
            by_first_start: index_by_first(&sentence_start_trie),
            trie,
            sentence_start_trie,
            austrian: variant == "de-AT",
        })
    }

    /// All hits of `trie` in the text, ordered by start position ascending
    /// and (for equal starts) longest match first. Java's Aho-Corasick
    /// `parseText` order is reversed before the selection, so
    /// `OldSpellingRule` ends up with the longest match per start position;
    /// this order produces the same accepted set.
    fn hits(
        trie: &[Entry],
        by_first: &HashMap<char, Vec<usize>>,
        text: &str,
    ) -> Vec<(usize, usize, usize)> {
        let mut hits = Vec::new();
        for (start, _) in text.char_indices() {
            let Some(first) = text[start..].chars().next() else {
                break;
            };
            let Some(ids) = by_first.get(&first) else {
                continue;
            };
            for &id in ids {
                let entry = &trie[id];
                if text[start..].starts_with(&entry.pattern) {
                    hits.push((start, start + entry.pattern.len(), id));
                }
            }
        }
        hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
        hits
    }

    /// `OldSpellingRule.isBoundary`: `!CHARS.matches(s)` with
    /// `CHARS = [a-zA-Zöäüß]`.
    fn is_boundary(s: &str) -> bool {
        !s.chars()
            .any(|c| c.is_ascii_alphabetic() || matches!(c, 'ö' | 'ä' | 'ü' | 'ß'))
    }

    /// `OldSpellingRule.ignoreMatch`. Java mixes `hit.begin`/`hit.end`
    /// (UTF-16 indices) with `substring`, so the byte offsets are converted
    /// to char indices first, keeping a `begin - 6` region inside a multibyte
    /// `ß` valid.
    fn ignore_match(&self, begin: usize, end: usize, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        let char_begin = text[..begin].chars().count();
        let char_end = char_begin + text[begin..end].chars().count();
        let region_matches = |start: usize, needle: &str| -> bool {
            let n = needle.chars().count();
            if start + n > chars.len() {
                return false;
            }
            let prefix: String = chars[start..start + n].iter().collect();
            prefix.to_lowercase() == needle.to_lowercase()
        };
        for exception in EXCEPTIONS {
            let n = exception.chars().count();
            if region_matches(char_begin, exception)
                || char_end
                    .checked_sub(n)
                    .is_some_and(|start| region_matches(start, exception))
            {
                return true;
            }
        }
        if char_begin > 0 && !Self::is_boundary(&chars[char_begin - 1].to_string()) {
            return true;
        }
        if char_end < chars.len() && !Self::is_boundary(&chars[char_end].to_string()) {
            return true;
        }
        if char_begin >= 6 {
            let before6: String = chars[char_begin - 6..char_begin].iter().collect();
            if before6.starts_with("Prof.") {
                return true;
            }
        }
        if char_begin >= 5 {
            let before5: String = chars[char_begin - 5..char_begin - 1].iter().collect();
            if before5 == "Herr" || before5 == "Frau" {
                return true;
            }
        }
        if char_begin >= 4 {
            let before4: String = chars[char_begin - 4..char_begin - 1].iter().collect();
            if before4 == "Hr." || before4 == "Fr." || before4 == "Dr." {
                return true;
            }
        }
        false
    }

    /// `OldSpellingRule.addMatch`; `false` when the de-AT `Geschoß` special
    /// case drops the match.
    fn add_match(
        &self,
        base: usize,
        text: &str,
        begin: usize,
        end: usize,
        value: &str,
        matches: &mut Vec<Match>,
    ) -> bool {
        let suggestions: Vec<String> = value.split('|').map(|s| s.to_string()).collect();
        let covered = &text[begin..end];
        let mut message = MESSAGE.to_string();
        if let Some(first) = suggestions.first() {
            if first.replacen("ss", "ß", 1) == covered {
                if self.austrian && covered.to_lowercase().contains("geschoß") {
                    return false;
                }
                message.push_str(SUFFIX_MESSAGE);
            }
        }
        matches.push(
            Match::new(
                RULE_ID,
                Option::<String>::None,
                message,
                Some(SHORT_MESSAGE.to_string()),
                TextRange::new(base + begin, base + end),
                suggestions
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
                "TYPOS",
                "Mögliche Tippfehler",
            )
            .with_metadata(
                "Findet Schreibweisen, die nur in der alten Rechtschreibung gültig waren",
                "misspelling",
                0,
            ),
        );
        true
    }

    /// `OldSpellingRule.match(AnalyzedSentence)`; returns offsets relative to
    /// the sentence text plus `base`.
    pub fn check_sentence(&self, sentence: &AnalyzedSentence, base: usize) -> Vec<Match> {
        let text = sentence.text.as_str();
        let mut matches = Vec::new();
        let mut start_positions: Vec<usize> = Vec::new();
        for (begin, end, id) in Self::hits(&self.trie, &self.by_first, text) {
            if start_positions.contains(&begin) {
                continue; // avoid overlapping matches
            }
            if !self.ignore_match(begin, end, text) {
                let entry = &self.trie[id];
                if self.add_match(base, text, begin, end, &entry.value, &mut matches) {
                    start_positions.push(begin);
                }
            }
        }
        for (begin, end, id) in Self::hits(&self.sentence_start_trie, &self.by_first_start, text) {
            if start_positions.contains(&begin) {
                continue;
            }
            if begin == 0 && !self.ignore_match(begin, end, text) {
                let entry = &self.sentence_start_trie[id];
                if self.add_match(base, text, begin, end, &entry.value, &mut matches) {
                    break; // there can only be one match at the start of a sentence
                }
            }
        }
        matches
    }
}

fn index_by_first(entries: &[Entry]) -> HashMap<char, Vec<usize>> {
    let mut map: HashMap<char, Vec<usize>> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        if let Some(first) = entry.pattern.chars().next() {
            map.entry(first).or_default().push(i);
        }
    }
    map
}

/// `SpellingData.getCoherencyMap`.
fn get_coherency_map(
    path: &Path,
    sent_start_mode: bool,
    synth: &lt_tagger::GermanSynthesizer,
    synth_cache: &mut std::collections::HashMap<String, Vec<String>>,
) -> Result<Vec<Entry>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() < 2 {
            return Err(lt_core::CoreError::Parse(
                path.display().to_string(),
                format!("Unexpected format in file: {line}"),
            ));
        }
        let old_spelling = parts[0];
        let new_spelling = parts[1];
        // `sanityChecks`
        if old_spelling == new_spelling {
            return Err(lt_core::CoreError::Parse(
                path.display().to_string(),
                format!("Old and new spelling are the same: {line}"),
            ));
        }
        if let Some(lookup) = seen.get(new_spelling) {
            if lookup == old_spelling {
                return Err(lt_core::CoreError::Parse(
                    path.display().to_string(),
                    format!("Contradictory entry: {line}"),
                ));
            }
        }
        if let Some(existing) = seen.get(old_spelling) {
            if existing != new_spelling {
                return Err(lt_core::CoreError::Parse(
                    path.display().to_string(),
                    format!("Duplicate key: {old_spelling}, {existing} vs. {new_spelling}"),
                ));
            }
        }
        let (key, value) = if sent_start_mode
            && starts_with_lowercase(old_spelling)
            && starts_with_lowercase(new_spelling)
        {
            (
                lt_tagger::uppercase_first_char(old_spelling),
                lt_tagger::uppercase_first_char(new_spelling),
            )
        } else {
            (old_spelling.to_string(), new_spelling.to_string())
        };
        seen.insert(key.clone(), value.clone());
        entries.push(Entry {
            pattern: key,
            value,
        });
        if old_spelling.contains('ß') && old_spelling.replace('ß', "ss") == new_spelling {
            // PROTOTYPE: the sentence-start call repeats every synthesis
            if !synth_cache.contains_key(old_spelling) {
                synth_cache.insert(
                    old_spelling.to_string(),
                    synth.synthesize_for_pos_tags(old_spelling, &|_| true),
                );
            }
            for form in synth_cache.get(old_spelling).unwrap().clone() {
                if !form.contains("ss") {
                    let new_form = form.replace('ß', "ss");
                    seen.insert(form.clone(), new_form.clone());
                    entries.push(Entry {
                        pattern: form,
                        value: new_form,
                    });
                }
            }
        }
    }
    Ok(entries)
}

fn starts_with_lowercase(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_lowercase)
}
