//! Polish tagger: port of `org.languagetool.tagging.pl.PolishTagger`.
//!
//! `PolishTagger` extends `BaseTagger` (PoliMorf 2.0 `polish.dict`, PREFIX
//! encoder, tags split on `+` into separate readings) and overrides
//! `tag(List<String>)`; unlike `BaseTagger.getAnalyzedTokens` it does not
//! exclude mixed-case words from the lowercase lookup, and it adds a
//! language-specific `additionalTags` fallback for the 2026 joined
//! `nie` + comparative/superlative spelling (`nielepiej`, `nienajlepszy`).

use std::collections::HashSet;
use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, CoreError, Result};

use crate::english::{is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// `PolishTagger.NEGATION_PREFIX`.
const NEGATION_PREFIX: &str = "nie";
/// `PolishTagger.MIN_NEGATED_LENGTH` (`"nie"` + a four-letter comparative).
const MIN_NEGATED_LENGTH: usize = NEGATION_PREFIX.len() + 4;
/// `PolishTagger.COMPAR_SUPERL` = `(adv|adj:.+):(com|sup)`, matched against
/// the whole tag (a `+`-combined tag can still match Java's `.`-based
/// pattern, so this is a real regex rather than a suffix check).
const COMPAR_SUPERL: &str = r"^(?:(adv|adj:.+):(com|sup))$";

/// Port of `PolishTagger` over the loaded Morfologik dictionary.
#[derive(Debug)]
pub struct PolishTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
    /// `pl/nie_compar_exceptions.txt` (`CachingWordListLoader.loadWords`).
    negation_exceptions: HashSet<String>,
    compar_superl: fancy_regex::Regex,
}

impl PolishTagger {
    /// Load `pl/dictionaries/polish.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("pl/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("polish.info"))?;
        let dict = Dictionary::load(&dict_dir.join("polish.dict"), &info)?;
        let words_dir = data_dir.join("pl/words");
        let manual = ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        let negation_exceptions =
            load_negation_exceptions(&words_dir.join("nie_compar_exceptions.txt"))?;
        let compar_superl = fancy_regex::Regex::new(COMPAR_SUPERL)
            .map_err(|e| CoreError::Parse("PolishTagger COMPAR_SUPERL".into(), e.to_string()))?;
        Ok(Self {
            dict,
            manual,
            removals,
            negation_exceptions,
            compar_superl,
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

    /// `PolishTagger.addTokens`: split each combined `+`-tag into separate
    /// readings.
    fn add_tokens(&self, word: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.word_lookup(lookup) {
            for part in tag.split('+') {
                out.push(AnalyzedToken::new(
                    word,
                    Some(stem.clone()),
                    Some(part.to_string()),
                ));
            }
        }
    }

    /// `PolishTagger.additionalTags`: joined `nie` + comparative/superlative
    /// forms are looked up with the prefix stripped; the lemma is the negated
    /// positive form (`nie` + base lemma). Returns `(lemma, tag)` pairs with
    /// the combined tag, as Java's `addTokens` splits them afterwards.
    fn additional_tags(&self, word: &str) -> Vec<(String, String)> {
        let lower_word = word.to_lowercase();
        if lower_word.chars().count() < MIN_NEGATED_LENGTH
            || !lower_word.starts_with(NEGATION_PREFIX)
            || is_mixed_case(word)
            || self.negation_exceptions.contains(&lower_word)
        {
            return Vec::new();
        }
        let base = &lower_word[NEGATION_PREFIX.len()..];
        let mut result = Vec::new();
        for (lemma, tag) in self.word_lookup(base) {
            if self.compar_superl.is_match(&tag).unwrap_or(false) {
                result.push((format!("{NEGATION_PREFIX}{lemma}"), tag));
            }
        }
        result
    }

    /// `PolishTagger.tag` for one word.
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;

        // normal case
        self.add_tokens(word, word, &mut l);
        if !is_lowercase {
            // lowercase lookup (PolishTagger does not skip mixed case here)
            self.add_tokens(word, &lower_word, &mut l);
        }
        // uppercase
        if l.is_empty() && is_lowercase {
            let upper = uppercase_first_char(word);
            self.add_tokens(word, &upper, &mut l);
        }
        // additional language-dependent tagging, e.g. joined "nie"
        if l.is_empty() {
            for (lemma, tag) in self.additional_tags(word) {
                for part in tag.split('+') {
                    l.push(AnalyzedToken::new(
                        word,
                        Some(lemma.clone()),
                        Some(part.to_string()),
                    ));
                }
            }
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: `PolishTagger.tag([word]).get(0).isTagged()`.
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

/// `CachingWordListLoader.loadWords`: skip empty lines and `#` comments,
/// otherwise take the text before an inline `#`.
fn load_negation_exceptions(path: &Path) -> Result<HashSet<String>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut out = HashSet::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let word = line.split('#').next().unwrap_or("").trim();
        if !word.is_empty() {
            out.insert(word.to_string());
        }
    }
    Ok(out)
}
