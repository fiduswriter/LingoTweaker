//! Port of `org.languagetool.synthesis.ro.RomanianSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag.
//!
//! The Romanian synthesizer is a plain `BaseSynthesizer` — no language
//! overrides — so this is the unmodified base lookup: `romanian_synth.dict`
//! keyed by `lemma|tag`, the manual `added.txt`/`removed.txt` synthesizers
//! and the tag list from `romanian_tags.txt`.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::{DictionaryInfo, SynthDictionary};

pub struct RomanianSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
}

impl RomanianSynthesizer {
    /// Load from the vendored data directory (`ro/dictionaries`, `ro/words`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("ro/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("romanian_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("romanian_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("romanian_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read romanian_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("ro/words");
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

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String)` /
    /// `synthesize(AnalyzedToken, String, boolean)`.
    ///
    /// `BaseSynthesizer` creates its `Soros` number speller with a `null`
    /// file name for Romanian, so `getSpelledNumber` returns the numeral
    /// unchanged (`_spell_number_:feminine` therefore prepends `"feminine "`).
    /// `Roman.sor` is not vendored; no Romanian rule references the Roman tag.
    pub fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        if pos_tag == "_spell_number_" {
            return vec![token.token.clone()];
        }
        if pos_tag == "_spell_number_:feminine" {
            return vec![format!("feminine {}", token.token)];
        }
        if pos_tag == "_spell_number_:Roman" {
            return vec![token.token.clone()];
        }
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
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
            return results;
        }
        self.lookup(&lemma, pos_tag)
    }

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String)` (plain tag).
    pub fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synthesize(token, pos_tag, false)
    }

    /// `BaseSynthesizer.getTargetPosTag`: return the last one to keep the
    /// previous results.
    pub fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        pos_tags
            .last()
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }
}
