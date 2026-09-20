//! Dutch tagger: the `BaseTagger` lookup (`dutch.dict` + `added*`/`removed*`
//! manual taggers) behind `DutchTagger`.
//!
//! Stage 1 wires the plain dictionary foundation; the `DutchTagger.tag()`
//! overrides (optional-accent handling, hyphen/mixed-case rules,
//! compound-acceptor interaction) land in stage 3
//! (internal development notes).

use std::path::Path;
use std::sync::{Arc, OnceLock};

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::english::{is_all_uppercase, is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// The `CompoundAcceptor.getParts` service the Dutch tagger needs (the
/// acceptor lives in the `lt` crate next to the speller it calls back into,
/// so it is injected through this trait).
pub trait CompoundPartsProvider: Send + Sync {
    fn get_parts(&self, word: &str) -> Vec<String>;
}

/// Plain `BaseTagger` over the loaded Morfologik `dutch.dict`.
pub struct DutchTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
    /// `Dutch.getCompoundAcceptor()` for the unknown-compound tagging;
    /// set by the pipeline after both exist.
    compound_acceptor: OnceLock<Arc<dyn CompoundPartsProvider>>,
}

impl std::fmt::Debug for DutchTagger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DutchTagger").finish_non_exhaustive()
    }
}

impl DutchTagger {
    /// Load `nl/dictionaries/dutch.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("nl/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("dutch.info"))?;
        let dict = Dictionary::load(&dict_dir.join("dutch.dict"), &info)?;
        let words_dir = data_dir.join("nl/words");
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
            compound_acceptor: OnceLock::new(),
        })
    }

    /// The pipeline sets `Dutch.getCompoundAcceptor()` after both exist.
    pub fn set_compound_acceptor(&self, acceptor: Arc<dyn CompoundPartsProvider>) {
        let _ = self.compound_acceptor.set(acceptor);
    }

    fn acceptor(&self) -> Option<&Arc<dyn CompoundPartsProvider>> {
        self.compound_acceptor.get()
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    /// `DutchTagger.getPostags`: the raw `CombiningTagger` lookup for the
    /// word (no `tag()` heuristics, so `CompoundAcceptor` cannot recurse
    /// into the tagger).
    pub fn postags(&self, word: &str) -> Vec<(String, String)> {
        self.word_lookup(word)
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

    /// `BaseTagger.getAnalyzedTokens` for one word (Dutch stage-1 plain
    /// subset: no `additionalTags`, `tagLowercaseWithUppercase = true`).
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

    /// Tokenizer helper: `DutchTagger.INSTANCE.tag([word]).get(0).isTagged()`.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// `DutchTagger.tag(List<String>)`: the `BaseTagger` lookup plus the
    /// Dutch overrides (typewriter-apostrophe rewriting, optional-accent
    /// retries, hyphen part handling, unknown-compound tagging and the
    /// `ignoreSpelling` flag). One entry per input token with the cumulative
    /// Java UTF-16 position (the pipeline overrides `start_pos` with byte
    /// offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let original_word = raw.clone();
            let word = raw.replace(['`', '\u{2019}', '\u{2018}', '\u{00B4}'], "'");
            let lower_word = word.to_lowercase();
            let is_lowercase = word == lower_word;
            let is_mixed = is_mixed_case(&word);
            let is_all_upper = is_all_uppercase(&word);

            let mut l: Vec<AnalyzedToken> = Vec::new();
            let mut ignore_spelling = false;
            self.add_from_word_tagger(&original_word, &word, &mut l);
            if !is_lowercase && !is_mixed {
                self.add_from_word_tagger(&original_word, &lower_word, &mut l);
            }
            if l.is_empty() && is_all_upper {
                let first_upper = uppercase_first_char(&lower_word);
                self.add_from_word_tagger(&original_word, &first_upper, &mut l);
            }
            if l.is_empty() {
                let mut word2 = word.clone();
                word2 = pattern1_a().replace_all(&word2, "${1}a${3}").into_owned();
                word2 = pattern1_e().replace_all(&word2, "${1}e${3}").into_owned();
                word2 = pattern1_i().replace_all(&word2, "${1}i${3}").into_owned();
                word2 = pattern1_o().replace_all(&word2, "${1}o${3}").into_owned();
                word2 = pattern1_u().replace_all(&word2, "${1}u${3}").into_owned();
                for (from, to) in [
                    ("\u{00E1}\u{00E1}", "aa"),
                    ("\u{00E1}\u{00E9}", "ae"),
                    ("\u{00E1}\u{00ED}", "ai"),
                    ("\u{00E1}\u{00FA}", "au"),
                    ("\u{00E9}\u{00E9}", "ee"),
                    ("\u{00E9}\u{00ED}", "ei"),
                    ("\u{00E9}\u{00FA}", "eu"),
                    ("\u{00ED}\u{00E9}", "ie"),
                    ("\u{00F3}\u{00E9}", "oe"),
                    ("\u{00F3}\u{00ED}", "oi"),
                    ("\u{00F3}\u{00F3}", "oo"),
                    ("\u{00F3}\u{00FA}", "ou"),
                    ("\u{00FA}\u{00ED}", "ui"),
                    ("\u{00FA}\u{00FA}", "uu"),
                    ("\u{00ED}j", "ij"),
                ] {
                    word2 = word2.replace(from, to);
                }
                word2 = pattern2_a().replace_all(&word2, "${1}a${2}").into_owned();
                word2 = pattern2_e().replace_all(&word2, "${1}e${2}").into_owned();
                word2 = pattern2_i().replace_all(&word2, "${1}i${2}").into_owned();
                word2 = pattern2_o().replace_all(&word2, "${1}o${2}").into_owned();
                word2 = pattern2_u().replace_all(&word2, "${1}u${2}").into_owned();
                if word2.contains('-') {
                    let part2 = hyphen1_pattern().replace(&word2, "$2").into_owned();
                    if !self.word_lookup(&part2).is_empty() {
                        word2 = hyphen2_pattern().replace_all(&word2, "$1$2").into_owned();
                    }
                }
                if word2 != word {
                    let l2 = self.word_lookup(&word2);
                    if !l2.is_empty() {
                        for (stem, tag) in l2 {
                            l.push(AnalyzedToken::new(&original_word, Some(stem), Some(tag)));
                        }
                        ignore_spelling = true;
                    }
                }
                // Tag unknown compound words
                if l.is_empty() && word.chars().count() > 5 {
                    if let Some(acceptor) = self.acceptor() {
                        let parts = acceptor.get_parts(&word);
                        if parts.len() == 2 {
                            let part1 = &parts[0];
                            let part2 = &parts[1];
                            let part2_readings = self.tag(std::slice::from_ref(part2));
                            let part1lc = part1.to_lowercase();
                            if let Some(part2_atr) = part2_readings.first() {
                                for reading in &part2_atr.readings {
                                    let Some(tag) = &reading.pos_tag else {
                                        continue;
                                    };
                                    if part1.ends_with('-') && tag.starts_with("ENM:LOC") {
                                        l.push(AnalyzedToken::new(
                                            &word,
                                            Some(part2.clone()),
                                            Some(tag.clone()),
                                        ));
                                        break;
                                    }
                                    if tag.starts_with("ZNW") {
                                        let special = if always_needs_het().contains(part2.as_str())
                                        {
                                            Some("ZNW:EKV:HET")
                                        } else if always_needs_de().contains(part2.as_str()) {
                                            Some("ZNW:EKV:DE_")
                                        } else if always_needs_mrv().contains(part2.as_str()) {
                                            Some("ZNW:MRV:DE_")
                                        } else {
                                            None
                                        };
                                        let lemma = format!(
                                            "{part1lc}{}",
                                            reading.stem.clone().unwrap_or_default()
                                        );
                                        l.push(AnalyzedToken::new(
                                            &word,
                                            Some(lemma),
                                            Some(special.unwrap_or(tag).to_string()),
                                        ));
                                        if special.is_some() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if l.is_empty() {
                l.push(AnalyzedToken::new(&original_word, None, None));
            }
            let mut atr = AnalyzedTokenReadings::new(l.clone());
            if ignore_spelling {
                if is_lowercase {
                    let upper = uppercase_first_char(&original_word);
                    if self.word_lookup(&upper).is_empty() {
                        atr.is_ignore_spelling = true;
                    } else {
                        atr.readings = vec![AnalyzedToken::new(&original_word, None, None)];
                    }
                } else {
                    atr.is_ignore_spelling = true;
                }
            }
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            atr.is_tagged = atr.readings.iter().any(|r| r.pos_tag.is_some());
            out.push(atr);
            pos += original_word.encode_utf16().count();
        }
        out
    }
}

/// `DutchTagger.alwaysNeedsHet`.
fn always_needs_het() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        ["patroon", "punt", "gemaal", "weer", "kussen", "deel"]
            .into_iter()
            .collect()
    })
}

/// `DutchTagger.alwaysNeedsDe`.
fn always_needs_de() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| ["keten", "boor", "dans"].into_iter().collect())
}

/// `DutchTagger.alwaysNeedsMrv`.
fn always_needs_mrv() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| ["pies", "koeken", "heden"].into_iter().collect())
}

fn pattern1_a() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])(\u{00E1})([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])").unwrap())
}
fn pattern1_e() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])(\u{00E9})([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])").unwrap())
}
fn pattern1_i() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])(\u{00ED})([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])").unwrap())
}
fn pattern1_o() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])(\u{00F3})([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])").unwrap())
}
fn pattern1_u() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])(\u{00FA})([^aeiou\u{00E1}\u{00E9}\u{00ED}\u{00F3}\u{00FA}])").unwrap())
}
fn pattern2_a() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(^|[^aeiou])\u{00E1}([^aeiou]|$)").unwrap())
}
fn pattern2_e() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(^|[^aeiou])\u{00E9}([^aeiou]|$)").unwrap())
}
fn pattern2_i() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(^|[^aeiou])\u{00ED}([^aeiou]|$)").unwrap())
}
fn pattern2_o() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(^|[^aeiou])\u{00F3}([^aeiou]|$)").unwrap())
}
fn pattern2_u() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(^|[^aeiou])\u{00FA}([^aeiou]|$)").unwrap())
}
fn hyphen1_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(^.*)-(.*$)").unwrap())
}
fn hyphen2_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("([a-z])-([a-z])").unwrap())
}

#[cfg(test)]
mod accent_tests {
    use super::*;

    #[test]
    fn accent_patterns_strip_acute() {
        let mut word2 = "áchter".to_string();
        word2 = pattern1_a().replace_all(&word2, "${1}a${3}").into_owned();
        word2 = pattern2_a().replace_all(&word2, "${1}a${2}").into_owned();
        assert_eq!(word2, "achter");
        let w = "énige";
        let p2 = pattern2_e().replace_all(w, "${1}e${2}").into_owned();
        assert_eq!(p2, "enige");
    }
}
