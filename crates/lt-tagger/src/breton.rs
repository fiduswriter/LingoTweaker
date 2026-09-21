//! Breton tagger: port of `org.languagetool.tagging.br.BretonTagger`, a
//! `BaseTagger` over the FSA5 `breton.dict` Morfologik dictionary (based on
//! the Apertium Breton dictionary) whose `tag()` override retries the lookup
//! without the demonstrative suffixes `-mañ`, `-se`, `-hont`.

use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::english::uppercase_first_char;
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// `BretonTagger.patternSuffix` = `(?iu)(..+)-(mañ|se|hont)$`.
fn pattern_suffix() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("(?i)(..+)-(mañ|se|hont)$").expect("valid suffix regex"));
    &RE
}

/// Port of `BretonTagger` (`BaseTagger` + the `tag()` override).
#[derive(Debug)]
pub struct BretonTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl BretonTagger {
    /// Load `br/dictionaries/breton.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("br/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("breton.info"))?;
        let dict = Dictionary::load(&dict_dir.join("breton.dict"), &info)?;
        let words_dir = data_dir.join("br/words");
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

    /// `asAnalyzedTokenListForTaggedWords(word, ...)`: the readings carry the
    /// original surface `word`, not the probe.
    fn add_from_word_tagger(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.word_lookup(lookup) {
            out.push(AnalyzedToken::new(
                surface,
                Some(stem.clone()),
                Some(tag.clone()),
            ));
        }
    }

    /// `BretonTagger.tag()` for one word: the custom retry loop that strips
    /// `-mañ`/`-se`/`-hont` when the dictionary probe fails.
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        if word.encode_utf16().count() > 50 {
            // avoid excessively long computation times for long (probably
            // artificial) tokens
            return vec![AnalyzedToken::new(word, None, None)];
        }
        let mut probe_word = word.to_string();
        loop {
            let mut l: Vec<AnalyzedToken> = Vec::new();
            let lower_word = probe_word.to_lowercase();
            let mut tagger_tokens: Vec<AnalyzedToken> = Vec::new();
            self.add_from_word_tagger(word, &probe_word, &mut tagger_tokens);
            let mut lower_tagger_tokens: Vec<AnalyzedToken> = Vec::new();
            self.add_from_word_tagger(word, &lower_word, &mut lower_tagger_tokens);
            let is_lowercase = probe_word == lower_word;

            l.extend(tagger_tokens.iter().cloned());
            if !is_lowercase {
                l.extend(lower_tagger_tokens.iter().cloned());
            }

            if lower_tagger_tokens.is_empty() && tagger_tokens.is_empty() {
                if is_lowercase {
                    let upper = uppercase_first_char(&probe_word);
                    let mut upper_tagger_tokens: Vec<AnalyzedToken> = Vec::new();
                    self.add_from_word_tagger(word, &upper, &mut upper_tagger_tokens);
                    if !upper_tagger_tokens.is_empty() {
                        l.extend(upper_tagger_tokens);
                    }
                }
                if l.is_empty() {
                    if let Some(caps) = pattern_suffix().captures(&probe_word) {
                        // Remove the suffix and probe the dictionary again
                        // (e.g. "xxx-mañ" -> probe "xxx").
                        probe_word = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
                        continue;
                    }
                    l.push(AnalyzedToken::new(word, None, None));
                }
            }
            return l;
        }
    }

    /// Tokenizer helper: `BretonTagger.tag([word]).get(0).isTagged()`.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// `BretonTagger.tag(List<String>)`: one entry per input token with the
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
