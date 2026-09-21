//! Danish tagger: port of `org.languagetool.tagging.da.DanishTagger`, a plain
//! `BaseTagger` over the `danish.dict` Morfologik dictionary (no
//! `additionalTags` heuristics, no word-tokenizer overrides).

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::english::{is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// Port of the plain `BaseTagger` over a loaded Morfologik dictionary.
#[derive(Debug)]
pub struct DanishTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl DanishTagger {
    /// Load `da/dictionaries/danish.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("da/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("danish.info"))?;
        let dict = Dictionary::load(&dict_dir.join("danish.dict"), &info)?;
        let words_dir = data_dir.join("da/words");
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
            manual,
            removals,
        })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
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

    /// `BaseTagger.getAnalyzedTokens` for one word (Danish: no
    /// `additionalTags`, `tagLowercaseWithUppercase = true`).
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
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: `DanishTagger.INSTANCE.tag([word]).get(0).isTagged()`.
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
