//! Galician tagger: port of `org.languagetool.tagging.gl.GalicianTagger`.
//!
//! A `BaseTagger` over the Freeling/Apertium-derived `galician.dict` with the
//! Galician `tag()` override: typewriter-apostrophe normalization, the
//! `-mente` adverb heuristic and the `auto`/`re` prefixed-verb heuristic, plus
//! the `containsTypewriterApostrophe` chunk tag. `overwriteWithManualTagger()`
//! is false, so manual `added*` readings come first and `removed*` last.

use std::path::Path;
use std::sync::OnceLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::english::is_mixed_case;
use crate::{Dictionary, DictionaryInfo, ManualTagger};

fn adj_part_fs() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?:V.P..SF.|A[QO].[FC][SN].)$").unwrap())
}

fn verb_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^V.+$").unwrap())
}

fn prefixes_for_verbs() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(auto|re)(...+)").unwrap())
}

/// Port of `GalicianTagger`.
#[derive(Debug)]
pub struct GalicianTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` manual tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl GalicianTagger {
    /// Load `gl/dictionaries/galician.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("gl/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("galician.info"))?;
        let dict = Dictionary::load(&dict_dir.join("galician.dict"), &info)?;
        let words_dir = data_dir.join("gl/words");
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

    /// `BaseTagger.asAnalyzedTokenListForTaggedWords`.
    fn add_from_word_tagger(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.word_lookup(lookup) {
            out.push(AnalyzedToken::new(surface, Some(stem), Some(tag)));
        }
    }

    /// `GalicianTagger.additionalTags` (starts from the raw dictionary, like
    /// Java's `DictionaryLookup`).
    fn additional_tags(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut additional: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        // Any well-formed adverb with suffix -mente is tagged as an adverb of
        // manner (RM) when the adjective stem is in the dictionary. Java's
        // `replaceAll("^(.+)mente$", "$1")` needs at least one char before
        // "mente".
        if lower_word.chars().count() >= 6 && lower_word.ends_with("mente") {
            let possible_adj = &lower_word[..lower_word.len() - "mente".len()];
            for (_, pos_tag) in self.dict.lookup(possible_adj) {
                if adj_part_fs().is_match(&pos_tag) {
                    return vec![AnalyzedToken::new(
                        word,
                        Some(lower_word.clone()),
                        Some("RM".to_string()),
                    )];
                }
            }
        }
        // Any well-formed verb with the auto/re prefix is tagged as a verb,
        // copying the original readings and prefixing the lemma.
        if let Some(caps) = prefixes_for_verbs().captures(word) {
            let possible_verb = caps.get(2).unwrap().as_str().to_lowercase();
            let prefix = caps.get(1).unwrap().as_str().to_lowercase();
            for (lemma, pos_tag) in self.dict.lookup(&possible_verb) {
                if verb_pattern().is_match(&pos_tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(format!("{prefix}{lemma}")),
                        Some(pos_tag),
                    ));
                }
            }
            return additional;
        }
        Vec::new()
    }

    /// `GalicianTagger.tag()` for one word.
    pub fn tag_word(&self, word: &str) -> (Vec<AnalyzedToken>, bool) {
        // "This hack allows all rules and dictionary entries to work with
        // typewriter apostrophe". Java only rewrites when the token has more
        // than one character (a lone `’` stays a typographic apostrophe).
        let contains_typewriter_apostrophe = word.chars().count() > 1 && word.contains('\'');
        let word = if word.chars().count() > 1 {
            word.replace('\u{2019}', "'")
        } else {
            word.to_string()
        };
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(&word);

        self.add_from_word_tagger(&word, &word, &mut l);
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(&word, &lower_word, &mut l);
        }
        if l.is_empty() && !is_mixed {
            let additional = self.additional_tags(&word);
            l.extend(additional);
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        (l, contains_typewriter_apostrophe)
    }

    /// `BaseTagger.tag(List<String>)`: one entry per input token with the
    /// cumulative Java UTF-16 position (the pipeline overrides `start_pos`
    /// with byte offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let (readings, has_typewriter_apostrophe) = self.tag_word(raw);
            let mut atr = AnalyzedTokenReadings::new(readings);
            if has_typewriter_apostrophe {
                atr.chunk_tags = vec!["containsTypewriterApostrophe".to_string()];
            }
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            out.push(atr);
            pos += raw.encode_utf16().count();
        }
        out
    }

    /// Lowercase-first-char fallback used by `is_known_word` callers.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).0.iter().any(|t| t.pos_tag.is_some())
    }
}
