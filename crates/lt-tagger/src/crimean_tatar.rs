//! Crimean Tatar tagger: port of `org.languagetool.tagging.crh.CrimeanTatarTagger`,
//! a `BaseTagger` over the `crimean_tatar.dict` Morfologik dictionary with
//! `tagLowercaseWithUppercase = false` and only `/crh/added.txt` as manual
//! additions.

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::english::is_mixed_case;
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// Port of `BaseTagger` with `tagLowercaseWithUppercase = false`.
#[derive(Debug)]
pub struct CrimeanTatarTagger {
    dict: Dictionary,
    /// `/crh/added.txt` (`getManualAdditionsFileNames` override)
    manual: ManualTagger,
    /// `/crh/removed.txt` (+ the missing `removed_custom.txt`)
    removals: ManualTagger,
}

impl CrimeanTatarTagger {
    /// Load `crh/dictionaries/crimean_tatar.{dict,info}` plus the word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("crh/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("crimean_tatar.info"))?;
        let dict = Dictionary::load(&dict_dir.join("crimean_tatar.dict"), &info)?;
        let words_dir = data_dir.join("crh/words");
        let manual = ManualTagger::load(&[&words_dir.join("added.txt")])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        Ok(Self {
            dict,
            manual,
            removals,
        })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    fn word_lookup(&self, word: &str) -> Vec<(String, String)> {
        let mut result: Vec<(String, String)> = self.manual.lookup(word).to_vec();
        result.extend(self.dict.lookup(word));
        if !self.removals.is_empty() {
            let removals = self.removals.lookup(word);
            if !removals.is_empty() {
                result.retain(|tw| !removals.contains(tw));
            }
        }
        result
    }

    fn add_from_word_tagger(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.word_lookup(lookup) {
            out.push(AnalyzedToken::new(
                surface,
                Some(stem.clone()),
                Some(tag.clone()),
            ));
        }
    }

    /// `BaseTagger.getAnalyzedTokens` for one word
    /// (`tagLowercaseWithUppercase = false`).
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(word);

        self.add_from_word_tagger(word, word, &mut l);
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(word, &lower_word, &mut l);
        }
        // `tagLowercaseWithUppercase` is false: no uppercase fallback.
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let readings = self.tag_word(raw);
            let mut atr = AnalyzedTokenReadings::new(readings);
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            out.push(atr);
            pos += raw.encode_utf16().count();
        }
        out
    }
}
