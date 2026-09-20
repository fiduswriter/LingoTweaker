//! Port of `org.languagetool.synthesis.es.SpanishSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag,
//! including the verb-with-noun lemma split and the `PostagComparator`.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::{DictionaryInfo, SynthDictionary};

pub struct SpanishSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
}

impl SpanishSynthesizer {
    /// Load from the vendored data directory (`es/dictionaries`, `es/words`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("es/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("es-ES_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("es-ES_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("es-ES_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read es-ES_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("es/words");
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        let do_not_synthesize = ManualSynthesizer::load(&words_dir.join("do-not-synthesize.txt"));
        if let Some(manual) = &manual {
            for tag in &manual.possible_tags {
                if !possible_tags.contains(tag) {
                    possible_tags.push(tag.clone());
                }
            }
        }
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

    /// `SpanishSynthesizer.synthesize(AnalyzedToken, String, boolean)`.
    pub fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        if pos_tag.starts_with("_spell_number_") {
            // number spelling (Soros) is not ported
            return Vec::new();
        }
        let mut lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        let mut to_add_after = String::new();
        // verbs with noun: "dar a luz" style lemmas, the noun follows the verb
        if pos_tag.starts_with('V') {
            if let Some(idx) = lemma.find(' ') {
                to_add_after = lemma[idx + 1..].to_string();
                lemma.truncate(idx);
            }
        }
        if pos_tag_regexp {
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pos_tag})$")) else {
                return Vec::new();
            };
            let mut results = Vec::new();
            for tag in &self.possible_tags {
                if re.is_match(tag).unwrap_or(false) {
                    results.extend(self.lookup(&lemma, tag));
                }
            }
            return add_words_after(results, &to_add_after);
        }
        let results = self.lookup(&lemma, pos_tag);
        add_words_after(results, &to_add_after)
    }

    /// `SpanishSynthesizer.getTargetPosTag`: sort with `PostagComparator`
    /// (indicative > imperative for the ambiguous `VMIP3S0`/`VMM02S0`
    /// pair) and take the last one.
    pub fn target_pos_tag(&self, pos_tags: &[String], target_pos_tag: &str) -> String {
        if pos_tags.is_empty() {
            return target_pos_tag.to_string();
        }
        let mut sorted: Vec<String> = pos_tags.to_vec();
        self.sort_pos_tags(&mut sorted);
        sorted.last().cloned().unwrap_or_default()
    }

    /// Java's `getTargetPosTag` sorts the caller's list in place (the
    /// `postag_replace` join in `MatchState.getTargetPosTag` depends on it).
    /// `List.sort` is stable; the comparator only ever swaps the
    /// indicative/imperative pair.
    pub fn sort_pos_tags(&self, pos_tags: &mut [String]) {
        pos_tags.sort_by(|a, b| postag_compare(a, b));
    }
}

fn postag_compare(a: &str, b: &str) -> std::cmp::Ordering {
    if a.len() > 4 && b.len() > 4 {
        if a == "VMIP3S0" && b == "VMM02S0" {
            return std::cmp::Ordering::Greater;
        }
        if a == "VMM02S0" && b == "VMIP3S0" {
            return std::cmp::Ordering::Less;
        }
    }
    std::cmp::Ordering::Equal
}

fn add_words_after(results: Vec<String>, to_add_after: &str) -> Vec<String> {
    if to_add_after.is_empty() {
        return results;
    }
    results
        .into_iter()
        .map(|r| format!("{r} {to_add_after}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postag_comparator_orders_indicative_last() {
        assert_eq!(
            postag_compare("VMIP3S0", "VMM02S0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            postag_compare("VMM02S0", "VMIP3S0"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            postag_compare("VMIP3S0", "VMIP3S0"),
            std::cmp::Ordering::Equal
        );
    }
}
