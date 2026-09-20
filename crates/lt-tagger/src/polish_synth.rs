//! Port of `org.languagetool.synthesis.pl.PolishSynthesizer` (a `BaseSynthesizer`
//! subclass with PoliMorf-specific negation and regexp handling).
//!
//! Differences from the plain base: the 2-arg `synthesize` does not special-case
//! `_spell_number_`; a tag containing `+` is treated as a regexp alternation;
//! negated tags (`:neg`) are rewritten to the dictionary's `:aff` affix forms
//! and prefixed with `nie`; the regexp path dedupes the results; and
//! `getPosTagCorrection` expands `.`-alternatives (`m1.m2` → `(.*m1.*|.*m2.*)`).

use std::collections::HashSet;
use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::{DictionaryInfo, SynthDictionary};

const RESOURCE_FILENAME: &str = "polish_synth";
const TAGS_FILE_NAME: &str = "polish_tags.txt";

const POTENTIAL_NEGATION_TAG: &str = ":aff";
const NEGATION_TAG: &str = ":neg";
const COMP_TAG: &str = "com";
const SUP_TAG: &str = "sup";

pub struct PolishSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
}

impl PolishSynthesizer {
    /// Load from the vendored data directory (`pl/dictionaries`, `pl/words`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("pl/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join(format!("{RESOURCE_FILENAME}.info")))?;
        let dict =
            SynthDictionary::load(&dict_dir.join(format!("{RESOURCE_FILENAME}.dict")), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join(TAGS_FILE_NAME))
            .map_err(|e| CoreError::Data(format!("cannot read {TAGS_FILE_NAME}: {e}")))?;
        let possible_tags: Vec<String> = tag_text
            .lines()
            .map(|l| l.trim_start_matches('\u{feff}').trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("pl/words");
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        let do_not_synthesize = ManualSynthesizer::load(&words_dir.join("do-not-synthesize.txt"));
        Ok(Self {
            dict,
            possible_tags,
            manual,
            removed,
            do_not_synthesize,
        })
    }

    /// `BaseSynthesizer.lookup`.
    fn lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
        let key = format!("{lemma}|{pos_tag}");
        let mut results = self.dict.lookup(&key);
        if let Some(manual) = &self.manual {
            if let Some(forms) = manual.lookup(lemma, pos_tag) {
                results.extend(forms);
            }
        }
        if let Some(removed) = &self.removed {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        if let Some(removed) = &self.do_not_synthesize {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        results
    }

    /// `PolishSynthesizer.getWordForms`.
    fn get_word_forms(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        is_negated: bool,
    ) -> Vec<String> {
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        if is_negated {
            let key_tag = pos_tag.replacen(NEGATION_TAG, POTENTIAL_NEGATION_TAG, 1);
            self.lookup(&lemma, &key_tag)
                .into_iter()
                .map(|form| format!("nie{form}"))
                .collect()
        } else {
            self.lookup(&lemma, pos_tag)
        }
    }

    /// `PolishSynthesizer.synthesize(AnalyzedToken, String, boolean)` when
    /// `posTagRegExp` is true, and the 2-arg form's `+` delegation.
    fn synthesize_regexp(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        is_negated: bool,
    ) -> Vec<String> {
        let mut pos_tag = pos_tag.to_string();
        if is_negated {
            pos_tag = pos_tag.replace(NEGATION_TAG, &format!("{POTENTIAL_NEGATION_TAG}?"));
        }
        let pattern = pos_tag.replace('+', "|");
        let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pattern})$")) else {
            // Java catches PatternSyntaxException, prints it and returns the
            // (empty) result list.
            return Vec::new();
        };
        let mut results = Vec::new();
        for tag in &self.possible_tags {
            if re.is_match(tag).unwrap_or(false) {
                results.extend(self.get_word_forms(token, tag, is_negated));
            }
        }
        // Java's `new HashSet<>(results)` dedupe is order-insensitive and the
        // returned order is the HashSet's bucket order; reproduce it so
        // suggestions match Java byte-for-byte.
        java_hash_set_order(results)
    }

    /// `PolishSynthesizer.synthesize(AnalyzedToken, String, boolean)`.
    pub fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        if pos_tag.is_empty() {
            return Vec::new();
        }
        let is_negated = self.is_negated(token, pos_tag);
        let plus = pos_tag.find('+').is_some_and(|i| i > 0);
        if pos_tag_regexp || plus {
            return self.synthesize_regexp(token, pos_tag, is_negated);
        }
        self.get_word_forms(token, pos_tag, is_negated)
    }

    /// `PolishSynthesizer.synthesize(AnalyzedToken, String)` (2-arg).
    pub fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synthesize(token, pos_tag, false)
    }

    fn is_negated(&self, token: &AnalyzedToken, pos_tag: &str) -> bool {
        let contains_neg = |s: &str| s.find(NEGATION_TAG).is_some_and(|i| i > 0);
        let contains_after = |s: &str, needle: &str| s.find(needle).is_some_and(|i| i > 0);
        contains_neg(pos_tag)
            || (token.pos_tag.as_deref().is_some_and(contains_neg)
                && !contains_after(pos_tag, COMP_TAG)
                && !contains_after(pos_tag, SUP_TAG))
    }

    /// `PolishSynthesizer.getPosTagCorrection`.
    pub fn pos_tag_correction(&self, pos_tag: &str) -> String {
        if !pos_tag.contains('.') {
            return pos_tag.to_string();
        }
        let mut tags: Vec<String> = pos_tag.split(':').map(str::to_string).collect();
        let mut found = false;
        for tag in &mut tags {
            if matches_letter_dot_letter(tag) {
                let replaced = tag.replace('.', ".*|.*");
                *tag = format!("(.*{replaced}.*)");
                found = true;
            }
        }
        if !found {
            return pos_tag.to_string();
        }
        tags.join(":")
    }

    /// `BaseSynthesizer.getTargetPosTag` (not overridden by Polish): the last
    /// matching tag, else the fallback.
    pub fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        pos_tags
            .last()
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }
}

/// `PolishSynthesizer.PATTERN` = `.*[a-z]\.[a-z].*` (a segment containing a
/// lowercase letter, a dot and a lowercase letter).
fn matches_letter_dot_letter(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes
        .windows(3)
        .any(|w| w[0].is_ascii_lowercase() && w[1] == b'.' && w[2].is_ascii_lowercase())
}

/// Emulate `new HashSet<String>(results)` iteration order: dedupe by first
/// occurrence, then lay the strings out in a `java.util.HashMap` table
/// (capacity `tableSizeFor(max(n/0.75+1, 16))`, bucket `(h ^ h>>>16) & cap-1`,
/// insertion order within a bucket). Java's `String.hashCode` is over UTF-16
/// code units.
fn java_hash_set_order(forms: Vec<String>) -> Vec<String> {
    let n = forms.len();
    let initial = ((n as f64 / 0.75) as usize + 1).max(16);
    let capacity = initial.next_power_of_two();
    let mut seen = HashSet::new();
    let mut buckets: Vec<Vec<String>> = vec![Vec::new(); capacity];
    for form in forms {
        if !seen.insert(form.clone()) {
            continue;
        }
        let h = java_string_hash(&form);
        let spread = (h ^ ((h as u32 >> 16) as i32)) as u32 as usize;
        buckets[spread & (capacity - 1)].push(form);
    }
    buckets.into_iter().flatten().collect()
}

/// Java `String.hashCode()` (31-based over UTF-16 code units, wrapping i32).
fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for unit in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(unit as i32);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn data_dir() -> Option<std::path::PathBuf> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        path.is_dir().then_some(path)
    }

    fn synth(data_dir: &std::path::Path) -> PolishSynthesizer {
        PolishSynthesizer::from_data(data_dir).expect("load Polish synthesizer")
    }

    /// Java probe (`PlSynthProbe`, pinned checkout):
    /// `dobrze [adv:pos] -> adv:com => [lepiej]`,
    /// `niemiecki [adj:sg:nom.voc:m1.m2.m3:pos] -> adj:pl:.* (re)
    ///  => [niemieckim, niemieccy, niemieckich, niemieckie, niemieckimi]`.
    #[test]
    fn synthesizer_matches_java_probe() {
        let Some(data_dir) = data_dir() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let synth = synth(&data_dir);
        let tok = |lemma: &str, tag: &str| {
            AnalyzedToken::new(lemma, Some(lemma.to_string()), Some(tag.to_string()))
        };
        assert_eq!(
            synth.synthesize(&tok("dobrze", "adv:pos"), "adv:com", false),
            vec!["lepiej".to_string()]
        );
        assert!(synth
            .synthesize(&tok("kot", "subst:sg:nom:m1"), "subst:pl:gen:f", false)
            .is_empty());
        assert_eq!(
            synth.synthesize(
                &tok("niemiecki", "adj:sg:nom.voc:m1.m2.m3:pos"),
                "adj:pl:.*",
                true
            ),
            vec![
                "niemieckim".to_string(),
                "niemieccy".to_string(),
                "niemieckich".to_string(),
                "niemieckie".to_string(),
                "niemieckimi".to_string(),
            ]
        );
        // `+` in the tag selects the regexp path.
        assert_eq!(
            synth.synthesize(
                &tok("kot", "subst:sg:nom:m1"),
                "subst:sg:nom:m1+subst:pl:gen:f",
                false
            ),
            vec!["kot".to_string()]
        );
    }

    /// `PolishSynthesizer.getPosTagCorrection`.
    #[test]
    fn pos_tag_correction_expands_dots() {
        let Some(data_dir) = data_dir() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let synth = synth(&data_dir);
        assert_eq!(
            synth.pos_tag_correction("adj:sg:nom.voc:m1.m2.m3:pos"),
            "adj:sg:(.*nom.*|.*voc.*):m1.m2.m3:pos"
        );
        assert_eq!(
            synth.pos_tag_correction("subst:sg:nom:m1"),
            "subst:sg:nom:m1"
        );
    }
}
