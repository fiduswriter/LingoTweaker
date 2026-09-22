//! Arabic tagger: port of `org.languagetool.tagging.ar.ArabicTagger`, a
//! `BaseTagger` over the Arramooz-derived `arabic.dict` Morfologik dictionary.
//!
//! Stage 1 ports the faithful dictionary subset: `removeTashkeel` before the
//! lookup (`ArabicStringTools.removeTashkeel`); the custom `additionalTags`
//! affix taxonomy (prefixes `و/ف/ب/ل/ك/س/ال…`, attached-pronoun suffixes and
//! the `ArabicTagManager` tag rewriting) is stage 3 and not wired yet.
//!
//! The dictionary uses `fsa.dict.separator=+`, `fsa.dict.encoding=utf-8`,
//! `fsa.dict.encoder=SUFFIX` (CFSA2, no frequency data).

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::{Dictionary, DictionaryInfo};

/// `ArabicStringTools.removeTashkeel`: the 12 tashkeel characters plus
/// tatweel (`ArabicStringTools.TASHKEEL_CHARS`).
const TASHKEEL_CHARS: [char; 13] = [
    '\u{064B}', '\u{064C}', '\u{064D}', '\u{064E}', '\u{064F}', '\u{0650}', '\u{0651}', '\u{0652}',
    '\u{0653}', '\u{0654}', '\u{0655}', '\u{0656}', '\u{0640}',
];

/// `ArabicStringTools.removeTashkeel`.
pub fn remove_tashkeel(text: &str) -> String {
    text.chars()
        .filter(|c| !TASHKEEL_CHARS.contains(c))
        .collect()
}

/// Stage-1 subset of `ArabicTagger` (plain `BaseTagger` over `arabic.dict`).
#[derive(Debug)]
pub struct ArabicTagger {
    dict: Dictionary,
}

impl ArabicTagger {
    /// Load `ar/dictionaries/arabic.{dict,info}`.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("ar/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("arabic.info"))?;
        let dict = Dictionary::load(&dict_dir.join("arabic.dict"), &info)?;
        Ok(Self { dict })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    /// `ArabicTagger.tag(List<String>)` reduced to the stage-1 subset: the
    /// dictionary lookup is keyed on the tashkeel-stripped word but the
    /// reading keeps the original surface form; an unknown word yields the
    /// `AnalyzedToken(word, null, null)` fallback.
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let stripped = remove_tashkeel(word);
        let mut l: Vec<AnalyzedToken> = Vec::new();
        for (stem, tag) in self.dict.lookup(&stripped) {
            l.push(AnalyzedToken::new(word, Some(stem), Some(tag)));
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: an Arabic word is "tagged" when any reading has a POS
    /// tag.
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
