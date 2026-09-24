//! Danish synthesizer dictionary (`BaseSynthesizer` lookup over a Morfologik
//! synth dictionary): inflected forms for a lemma and POS tag.
//!
//! Danish has no Java synthesizer class (`Danish.getSynthesizer` would be
//! the base `null`), so this is infrastructure for `<match postag>` synthesis
//! (plan item M2): `danish_synth.dict` built by inverting the tagger dict
//! `danish.dict` (form → `lemma|tag`), keyed by `lemma|tag`, with the tag
//! list from `danish_synth_tags.txt` and the manual `added.txt`/`removed.txt`
//! synthesizers, exactly like the Swedish plain `BaseSynthesizer` port.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::{DictionaryInfo, SynthDictionary};

pub struct DanishSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
}

impl DanishSynthesizer {
    /// Load from the vendored data directory (`da/dictionaries`, `da/words`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("da/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("danish_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("danish_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("danish_synth_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read danish_synth_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("da/words");
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
    /// file name for Danish (no `da.sor` is vendored), so `getSpelledNumber`
    /// returns the numeral unchanged (`_spell_number_:feminine` therefore
    /// prepends `"feminine "`); no Danish rule references the Roman tag.
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
