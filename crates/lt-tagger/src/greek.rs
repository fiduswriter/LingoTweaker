//! Greek tagger: port of `org.languagetool.tagging.el.GreekTagger`, a
//! `BaseTagger` over the small in-tree `greek.dict` POS dictionary plus the
//! `org.ioperm:morphology-el` `GreekAnalyzer` (`additionalTags`).
//!
//! Java's `BaseTagger.getAnalyzedTokens` only consults `additionalTags` when
//! the dictionary (and its lowercase/uppercase variants) produced no reading
//! at all; the analyzer is then queried on the exact surface form
//! (`getLemma(word, false)`, no case folding) and each `WordData` becomes an
//! `AnalyzedToken(word, tag, lemma)`.

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::english::{is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// `org.ioperm.morphology.el.GreekAnalyzer`: the `analysis.dict` Morfologik
/// dictionary queried on the exact surface form.
#[derive(Debug)]
pub struct GreekAnalyzer {
    dict: Dictionary,
}

impl GreekAnalyzer {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join("el/morphology");
        let info = DictionaryInfo::load(&dir.join("analysis.info"))?;
        let dict = Dictionary::load(&dir.join("analysis.dict"), &info)?;
        Ok(Self { dict })
    }

    /// `GreekAnalyzer.getLemma(word, false)`: `(lemma, tag)` per reading.
    pub fn lookup(&self, word: &str) -> Vec<(String, String)> {
        self.dict.lookup(word)
    }
}

/// Port of `GreekTagger` (the `BaseTagger` plus the analyzer fallback).
#[derive(Debug)]
pub struct GreekTagger {
    dict: Dictionary,
    analyzer: GreekAnalyzer,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl GreekTagger {
    /// Load `el/dictionaries/greek.{dict,info}`, the `el/morphology`
    /// analyzer data and the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("el/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("greek.info"))?;
        let dict = Dictionary::load(&dict_dir.join("greek.dict"), &info)?;
        let analyzer = GreekAnalyzer::load(data_dir)?;
        let words_dir = data_dir.join("el/words");
        let manual = ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        Ok(Self {
            dict,
            analyzer,
            manual,
            removals,
        })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    pub fn analyzer(&self) -> &GreekAnalyzer {
        &self.analyzer
    }

    /// `CombiningTagger.tag`: manual readings first, dictionary readings
    /// appended, removal tagger applied last (`overwriteWithManualTagger`
    /// is false).
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

    /// `GreekTagger.additionalTags`: the analyzer readings for `word`.
    fn additional_tags(&self, word: &str) -> Vec<AnalyzedToken> {
        self.analyzer
            .lookup(word)
            .into_iter()
            .map(|(lemma, tag)| AnalyzedToken::new(word, Some(lemma), Some(tag)))
            .collect()
    }

    /// `BaseTagger.getAnalyzedTokens` for one word (Greek: analyzer fallback
    /// when the dictionary produced nothing, `tagLowercaseWithUppercase`).
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(word);

        let before = l.len();
        self.add_from_word_tagger(word, word, &mut l);
        let tagger_tokens_empty = l.len() == before;
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(word, &lower_word, &mut l);
        }
        // tag a lowercase word with startuppercase word tags
        if is_lowercase && tagger_tokens_empty && l.is_empty() {
            let upper = uppercase_first_char(word);
            self.add_from_word_tagger(word, &upper, &mut l);
        }
        // Additional language-dependent tagging (Greek analyzer)
        if l.is_empty() {
            l.extend(self.additional_tags(word));
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: `GreekTagger.INSTANCE.tag([word]).get(0).isTagged()`.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// `BaseTagger.tag(List<String>)`: one entry per input token with the
    /// cumulative Java UTF-16 position (the pipeline overrides `start_pos`
    /// with byte offsets).
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
