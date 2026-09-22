//! Port of `org.languagetool.synthesis.sr.EkavianSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag.
//!
//! `SerbianSynthesizer extends BaseSynthesizer` with no language overrides,
//! so this is the unmodified base lookup: `ekavian/serbian_synth.dict` keyed
//! by `lemma|tag` and the tag list from `serbian_synth_tags.txt`. The base
//! manual synthesizers (`/sr/added.txt`, `/sr/removed.txt`,
//! `/sr/do-not-synthesize.txt`) do not exist in the Serbian module.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::{DictionaryInfo, SynthDictionary};

pub struct EkavianSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
}

impl EkavianSynthesizer {
    /// Load from the vendored data directory (`sr/dictionaries`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("sr/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("ekavian/serbian_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("ekavian/serbian_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("serbian_synth_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read serbian_synth_tags.txt: {e}")))?;
        let possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        Ok(Self {
            dict,
            possible_tags,
        })
    }

    /// `BaseSynthesizer.lookup`.
    fn lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
        let key = format!("{lemma}|{pos_tag}");
        self.dict.lookup(&key)
    }

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String)` /
    /// `synthesize(AnalyzedToken, String, boolean)`.
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
