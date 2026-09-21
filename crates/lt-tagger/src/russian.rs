//! Russian tagger: port of `org.languagetool.tagging.ru.RussianTagger`, a
//! `BaseTagger` over `ru/dictionaries/russian.dict` with the language's
//! `tag()` override: stress marks are stripped before lookup, and a word that
//! contains `е` but neither `ё` nor a stress mark is marked with the
//! `MayMissingYO` chunk tag when its `ё` variant is not a known word.

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::english::{is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// Port of `RussianTagger` (`tagLowercaseWithUppercase = true`, no
/// `additionalTags`).
#[derive(Debug)]
pub struct RussianTagger {
    dict: Dictionary,
    /// `/ru/added.txt` + `/ru/added_custom.txt`
    manual: ManualTagger,
    /// `/ru/removed.txt` + `/ru/removed_custom.txt`
    removals: ManualTagger,
}

impl RussianTagger {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("ru/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("russian.info"))?;
        let dict = Dictionary::load(&dict_dir.join("russian.dict"), &info)?;
        let words_dir = data_dir.join("ru/words");
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
    /// appended, removal tagger applied last (`overwriteWithManualTagger` is
    /// false).
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

    /// `BaseTagger.getAnalyzedTokens` for one (stress-stripped) word.
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

    /// `RussianTagger.tag` for one original token: the stress marks are
    /// stripped, the `MayMissingYO` chunk tag is computed on the stripped
    /// word, and the readings are produced for the stripped surface.
    fn tag_token(&self, raw: &str) -> (Vec<AnalyzedToken>, bool) {
        let mut word = raw.to_string();
        let mut may_missing_yo = false;
        if word.chars().count() > 1 {
            if !word.contains('ё')
                && !word.contains('Ё')
                && (word.contains('е') || word.contains('Е'))
                && !contains_any(
                    &word,
                    &[
                        "е\u{301}",
                        "о\u{301}",
                        "а\u{301}",
                        "у\u{301}",
                        "и\u{301}",
                        "ю\u{301}",
                        "ы\u{301}",
                        "э\u{301}",
                        "я\u{301}",
                    ],
                )
            {
                may_missing_yo = true;
            }
            for v in ['о', 'а', 'е', 'у', 'и', 'ы', 'э', 'ю', 'я'] {
                word = word.replace(&format!("{v}\u{301}"), &v.to_string());
                word = word.replace(&format!("{v}\u{300}"), &v.to_string());
            }
            word = word.replace('ѝ', "и");
            word = word.replace('ʼ', "ъ");
        }
        let readings = self.tag_word(&word);
        if may_missing_yo {
            let word_lc = word.to_lowercase().replace('е', "ё");
            if self.word_lookup(&word_lc).is_empty() {
                may_missing_yo = false;
            }
        }
        (readings, may_missing_yo)
    }

    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// `RussianTagger.tag(List<String>)`: one entry per input token; the
    /// `MayMissingYO` chunk tag is set when applicable. `start_pos` is a
    /// placeholder the pipeline overrides with byte offsets.
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let (readings, may_missing_yo) = self.tag_token(raw);
            let mut atr = AnalyzedTokenReadings::new(readings);
            if may_missing_yo {
                atr.chunk_tags = vec!["MayMissingYO".to_string()];
            }
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            out.push(atr);
            pos += raw.chars().count();
        }
        out
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}
