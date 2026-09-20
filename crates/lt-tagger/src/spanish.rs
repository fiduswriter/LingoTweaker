//! Spanish tagger: port of `org.languagetool.tagging.es.SpanishTagger` over
//! the `es-ES.dict` Morfologik dictionary, with `BaseTagger`'s manual
//! `added/removed` taggers and the Spanish `additionalTags` heuristics
//! (-mente adverbs, prefixed verbs/adjectives, hyphenated adjectives).

use std::path::Path;
use std::sync::OnceLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::english::{is_all_uppercase, is_capitalized_word, is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// `StringTools.isEmoji`: an astral code point that `^[\p{L}\d\p{P}\p{Zs}]+$`
/// does not accept.
pub fn is_emoji(word: &str) -> bool {
    if word.encode_utf16().count() == word.chars().count() {
        return false;
    }
    static WORD_FOR_SPELLER: OnceLock<Regex> = OnceLock::new();
    let re = WORD_FOR_SPELLER.get_or_init(|| Regex::new(r"^[\p{L}\d\p{P}\p{Zs}]+$").unwrap());
    !re.is_match(word)
}

fn re(pattern: &str) -> Regex {
    // Java patterns are anchored with `matcher.matches()` (full match).
    Regex::new(&format!("^(?:{pattern})$")).unwrap()
}

fn full_match(pattern: &str, text: &str) -> bool {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<String, Regex>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        let compiled = cache
            .entry(pattern.to_string())
            .or_insert_with(|| re(pattern))
            .clone();
        compiled.is_match(text)
    })
}

/// Port of the Spanish tagger over a loaded Morfologik dictionary.
#[derive(Debug)]
pub struct SpanishTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl SpanishTagger {
    /// Load `es/dictionaries/es-ES.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("es/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("es-ES.info"))?;
        let dict = Dictionary::load(&dict_dir.join("es-ES.dict"), &info)?;
        let words_dir = data_dir.join("es/words");
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
    /// is false for Spanish).
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

    /// `SpanishTagger.tag(List<String>)` for a single word (the tokenizer's
    /// dictionary check uses `tag([word]).get(0).isTagged()`).
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(word);
        let is_all_upper = is_all_uppercase(word);

        self.add_from_word_tagger(word, word, &mut l);
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(word, &lower_word, &mut l);
        }
        if is_all_upper {
            let first_upper = uppercase_first_char(&lower_word);
            self.add_from_word_tagger(word, &first_upper, &mut l);
        }
        if l.is_empty() && !is_mixed {
            if let Some(additional) = self.additional_tags(word) {
                l.extend(additional);
            }
        }
        if l.is_empty() && is_emoji(word) {
            l.push(AnalyzedToken::new(
                word,
                Some("_emoji_".to_string()),
                Some("_emoji_".to_string()),
            ));
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: `SpanishTagger.INSTANCE.tag([word]).get(0).isTagged()`.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// `SpanishTagger.tag(List<String>)`: one entry per input token with the
    /// cumulative Java UTF-16 position (the pipeline overrides `start_pos`
    /// with byte offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for (i, raw) in sentence_tokens.iter().enumerate() {
            let mut word = raw.clone();
            let previous_word = i
                .checked_sub(1)
                .map(|p| sentence_tokens[p].as_str())
                .unwrap_or("");
            let next_word = sentence_tokens.get(i + 1).map(String::as_str).unwrap_or("");
            let mut contains_typographic_apostrophe = false;
            if (word.encode_utf16().count() > 1
                || previous_word.eq_ignore_ascii_case("l")
                || previous_word.eq_ignore_ascii_case("d")
                || next_word.eq_ignore_ascii_case("s"))
                && word.contains('’')
            {
                contains_typographic_apostrophe = true;
                word = word.replace('’', "'");
            }
            let readings = self.tag_word(&word);
            let mut atr = AnalyzedTokenReadings::new(readings);
            atr.start_pos = pos;
            atr.raw_byte_len = word.len();
            if contains_typographic_apostrophe {
                atr.has_typographic_apostrophe = true;
            }
            out.push(atr);
            pos += word.encode_utf16().count();
        }
        out
    }

    /// `SpanishTagger.additionalTags` over the binary dictionary.
    fn additional_tags(&self, word: &str) -> Option<Vec<AnalyzedToken>> {
        let mut additional: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        // Any well-formed adverb with suffix -mente is tagged as an adverb
        // (RG): adjective/participle feminine singular + -mente.
        if let Some(stem) = lower_word.strip_suffix("mente") {
            // Java `replaceAll("^(.+)mente$", "$1")` — the greedy `.+` keeps
            // the longest prefix before the literal suffix.
            let possible_adj = stem.to_string();
            for (_lemma, tag) in self.dict.lookup(&possible_adj) {
                if full_match(r"VMP00SF|A[QO].[FC]S.", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(lower_word.clone()),
                        Some("RG".to_string()),
                    ));
                    return Some(additional);
                }
            }
        }
        // Any well-formed verb with prefixes is tagged as a verb copying the
        // original tags.
        if let Some((prefix, possible_verb)) = prefixed_verb(word, r"(auto)([^r]...+)") {
            for (lemma, tag) in self.dict.lookup(&possible_verb) {
                if full_match("V.+", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(prefix.clone() + &lemma),
                        Some(tag),
                    ));
                }
            }
            return Some(additional);
        }
        if let Some((prefix, possible_verb)) = prefixed_verb(word, r"(autor)(r...+)") {
            for (lemma, tag) in self.dict.lookup(&possible_verb) {
                if full_match("V.+", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(prefix.clone() + &lemma),
                        Some(tag),
                    ));
                }
            }
            return Some(additional);
        }
        if let Some((prefix, possible_adj)) =
            prefixed_verb(word, r"(super)(.*[aeiouàéèíòóïü].+[aeiouàéèíòóïü].*)")
        {
            for (lemma, tag) in self.dict.lookup(&possible_adj) {
                if full_match("AQ.*|V.P.*", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(prefix.clone() + &lemma),
                        Some(tag),
                    ));
                }
            }
            return Some(additional);
        }
        // Any well-formed adjective with prefixes is tagged as an adjective
        // copying the original tags.
        if let Some(possible_adj_prefix) = hyphen_prefix(word) {
            if !full_match(
                r"(anti|pre|ex|pro|afro|ultra|super|súper)",
                &possible_adj_prefix,
            ) {
                if let Some((_, possible_adj)) = hyphen_prefix_and_rest(word) {
                    let mut prefix_matches = false;
                    for (_, tag) in self.dict.lookup(&possible_adj_prefix) {
                        if full_match(r"AQ.MS.|AQ.CS.|AQ.MN.", &tag) {
                            prefix_matches = true;
                            break;
                        }
                    }
                    let mut adj_match: Option<(String, String)> = None;
                    for (lemma, tag) in self.dict.lookup(&possible_adj) {
                        if full_match("AQ.+", &tag) {
                            adj_match = Some((tag, possible_adj_prefix.clone() + "-" + &lemma));
                            break;
                        }
                    }
                    if let Some((new_postag, new_lemma)) = adj_match {
                        if prefix_matches {
                            additional.push(AnalyzedToken::new(
                                word,
                                Some(new_lemma),
                                Some(new_postag),
                            ));
                            return Some(additional);
                        }
                    }
                }
            }
        }
        None
    }
}

/// `PREFIXES_FOR_VERBS`/`_2`/`ADJECTIVES` helper: the pattern is
/// case-insensitive and `matcher.matches()` must match. Returns
/// `(group1 lowercased, group2 lowercased)`.
fn prefixed_verb(word: &str, pattern: &str) -> Option<(String, String)> {
    let re = regex::RegexBuilder::new(&format!("^(?:{pattern})$"))
        .case_insensitive(true)
        .build()
        .ok()?;
    let caps = re.captures(word)?;
    Some((
        caps.get(1)?.as_str().to_lowercase(),
        caps.get(2)?.as_str().to_lowercase(),
    ))
}

/// `PREFIXES_FOR_ADJ` group 1 (the part before the first `-`).
fn hyphen_prefix(word: &str) -> Option<String> {
    let re = regex::RegexBuilder::new(r"^(.+)-(.+)$")
        .case_insensitive(true)
        .build()
        .ok()?;
    let caps = re.captures(word)?;
    Some(caps.get(1)?.as_str().to_lowercase())
}

fn hyphen_prefix_and_rest(word: &str) -> Option<(String, String)> {
    let re = regex::RegexBuilder::new(r"^(.+)-(.+)$")
        .case_insensitive(true)
        .build()
        .ok()?;
    let caps = re.captures(word)?;
    Some((
        caps.get(1)?.as_str().to_lowercase(),
        caps.get(2)?.as_str().to_lowercase(),
    ))
}

/// The tagger's `isCapitalizedWord` re-export (rules use it on surfaces).
pub fn is_capitalized(word: &str) -> bool {
    is_capitalized_word(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;
    use std::path::Path;

    fn tagger() -> Option<SpanishTagger> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.join("es/dictionaries/es-ES.dict").lt_exists() {
            return None;
        }
        Some(SpanishTagger::load(&dir).unwrap())
    }

    #[test]
    fn tags_common_words() {
        let Some(tagger) = tagger() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let readings = tagger.tag_word("hablar");
        assert!(readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("VM"))));
        let unknown = tagger.tag_word("qqzzwwxx");
        assert_eq!(unknown.len(), 1);
        assert!(unknown[0].pos_tag.is_none());
    }
}
