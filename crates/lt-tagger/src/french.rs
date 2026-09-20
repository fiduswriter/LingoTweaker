//! French tagger: port of `org.languagetool.tagging.fr.FrenchTagger` over the
//! `french.dict` Morfologik dictionary, with `BaseTagger`'s manual
//! `added/removed` taggers, the `oe`→`œ` retry, emoji readings, apostrophe
//! chunk tags and the French prefix `additionalTags` heuristics.

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::english::{is_all_uppercase, is_capitalized_word, is_mixed_case};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

pub use crate::spanish::is_emoji;

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

/// `StringTools.convertToTitleCaseIteratingChars`: titlecase the first
/// character after a space or hyphen, lowercase everything else.
fn convert_to_title_case_iterating_chars(text: &str) -> String {
    let mut converted = String::with_capacity(text.len());
    let mut convert_next = true;
    for ch in text.chars() {
        // `Character.isSpaceChar` (categories Zs/Zl/Zp) + '-': Rust's
        // `is_whitespace` (White_Space) minus the control characters is
        // exactly the Z categories (NBSP included, \t/\n excluded).
        let ch = if (ch.is_whitespace() && !ch.is_control()) || ch == '-' {
            convert_next = true;
            ch
        } else if convert_next {
            convert_next = false;
            ch.to_uppercase().next().unwrap_or(ch)
        } else {
            ch.to_lowercase().next().unwrap_or(ch)
        };
        converted.push(ch);
    }
    converted
}

/// `FrenchTagger.ambigousTokens`: hyphenated title-case forms that must not
/// be re-tagged through the lowercase dictionary entry.
const AMBIGUOUS_TOKENS: [&str; 11] = [
    "-Le", "-Les", "-La", "-Elle", "-Elles", "-On", "-Tu", "-Vous", "-Il", "-Ils", "-Ce",
];

/// Port of the French tagger over a loaded Morfologik dictionary.
#[derive(Debug)]
pub struct FrenchTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl FrenchTagger {
    /// Load `fr/dictionaries/french.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("fr/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("french.info"))?;
        let dict = Dictionary::load(&dict_dir.join("french.dict"), &info)?;
        let words_dir = data_dir.join("fr/words");
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
    /// is false for French).
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

    /// `FrenchTagger.tagWord(word, originalWord)`.
    fn tag_word_with_original(
        &self,
        word: &str,
        original_word: &str,
        out: &mut Vec<AnalyzedToken>,
    ) {
        let lower_word = word.to_lowercase();
        let is_start_upper = is_capitalized_word(word);
        let is_all_upper = is_all_uppercase(word);
        let is_hyphenated_title_case = !AMBIGUOUS_TOKENS.contains(&original_word)
            && original_word.contains('-')
            && original_word == convert_to_title_case_iterating_chars(&lower_word);

        self.add_from_word_tagger(original_word, word, out);
        // tag non-lowercase (all-uppercase, start-uppercase, hyphenated title
        // case), but not mixed-case words, with lowercase word tags
        if is_all_upper || is_start_upper || is_hyphenated_title_case {
            let mut lower_tokens = Vec::new();
            self.add_from_word_tagger(original_word, &lower_word, &mut lower_tokens);
            out.extend(lower_tokens);
        }
        // tag all-uppercase proper nouns (ex. FRANCE)
        if out.is_empty() && is_all_upper {
            let first_upper = uppercase_first_char_or_keep(&lower_word);
            self.add_from_word_tagger(original_word, &first_upper, out);
        }
        // additional tagging with prefixes removed
        if out.is_empty() {
            if let Some(additional) = self.additional_tags(word) {
                out.extend(additional);
            }
        }
    }

    /// `FrenchTagger.tagWord(String)` for a single word with the per-token
    /// fallbacks of `tag(List)` (typographic apostrophe rewrite, `oe`→`œ`,
    /// emoji, untagged fallback).
    pub fn tag_word(&self, raw_word: &str) -> Vec<AnalyzedToken> {
        let mut word = raw_word.to_string();
        if word.chars().count() > 1 && word.contains('’') {
            word = word.replace('’', "'");
        }
        let mut l: Vec<AnalyzedToken> = Vec::new();
        self.tag_word_with_original(&word, &word, &mut l);
        if l.is_empty() && word.to_lowercase().contains("oe") {
            let replaced = word.replace("oe", "œ").replace("OE", "Œ");
            self.tag_word_with_original(&replaced, &word, &mut l);
        }
        if l.is_empty() && is_emoji(&word) {
            l.push(AnalyzedToken::new(
                word.clone(),
                Some("_emoji_".to_string()),
                Some("_emoji_".to_string()),
            ));
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word.clone(), None, None));
        }
        l
    }

    /// Tokenizer helper: `FrenchTagger.INSTANCE.tag([word]).get(0).isTagged()`.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// One token's readings exactly like `FrenchTagger.tag(List)` does for a
    /// single-element list, including the apostrophe chunk tags.
    fn tag_reading(&self, raw_word: &str) -> AnalyzedTokenReadings {
        let contains_typewriter_apostrophe =
            raw_word.chars().count() > 1 && raw_word.contains('\'');
        let contains_typographic_apostrophe =
            raw_word.chars().count() > 1 && raw_word.contains('’');
        let mut atr = AnalyzedTokenReadings::new(self.tag_word(raw_word));
        // Java `setChunkTags` replaces the list, so the typographic flag wins
        // when a word carries both apostrophe kinds.
        if contains_typewriter_apostrophe {
            atr.chunk_tags = vec!["containsTypewriterApostrophe".to_string()];
        }
        if contains_typographic_apostrophe {
            atr.chunk_tags = vec!["containsTypographicApostrophe".to_string()];
        }
        atr
    }

    /// `FrenchTagger.tag(List<String>)`: one entry per input token with the
    /// cumulative Java UTF-16 position (the pipeline overrides `start_pos`
    /// with byte offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let mut atr = self.tag_reading(raw);
            atr.start_pos = pos;
            pos += raw.encode_utf16().count();
            out.push(atr);
        }
        out
    }

    /// `FrenchTagger.additionalTags` over the binary dictionary.
    fn additional_tags(&self, word: &str) -> Option<Vec<AnalyzedToken>> {
        let mut additional: Vec<AnalyzedToken> = Vec::new();
        // Any well-formed verb with prefixes is tagged as a verb copying the
        // original tags.
        if let Some((prefix, possible_verb)) = prefixed(
            word,
            r"(auto|auto-|re-|sur-)([^-].*[aeiouêàéèíòóïü].+[aeiouêàéèíòóïü].*)",
        ) {
            for (lemma, tag) in self.dict.lookup(&possible_verb) {
                if full_match("V .+", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(prefix.clone() + &lemma),
                        Some(tag),
                    ));
                }
            }
            if !additional.is_empty() {
                return Some(additional);
            }
        }
        // short noun/adj prefixes (mini, méga)
        if let Some((prefix, possible_noun)) = prefixed(
            word,
            r"(mini|méga)([^-].*[aeiouêàéèíòóïü].+[aeiouêàéèíòóïü].*)",
        ) {
            for (lemma, tag) in self.dict.lookup(&possible_noun) {
                if full_match("[NJ] .+|V ppa.*", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(prefix.clone() + &lemma),
                        Some(tag),
                    ));
                }
            }
            if !additional.is_empty() {
                return Some(additional);
            }
        }
        if let Some((prefix, possible_noun)) = prefixed(
            word,
            r"(post-|sur-|mini-|méga-|demi-|péri-|anti-|géo-|nord-|sud-|néo-|méga-|ultra-|pro-|inter-|micro-|macro-|sous-|haut-|auto-|ré-|pré-|super-|vice-|hyper-|proto-|grand-|pseudo-)(.+)",
        ) {
            for (lemma, tag) in self.dict.lookup(&possible_noun) {
                if full_match("[NJ] .+|V ppa.*", &tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(prefix.clone() + &lemma),
                        Some(tag),
                    ));
                }
            }
            // Java looks the lowercased group up a second time when the first
            // pass found nothing; the group is already lowercased, so the
            // result is identical.
            return Some(additional);
        }
        None
    }
}

/// Java `StringTools.uppercaseFirstChar` (only the first char, which is a
/// letter for the tagger's all-uppercase branch).
fn uppercase_first_char_or_keep(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `PREFIXES_*` helper: the pattern is case-insensitive and
/// `matcher.matches()` must match. Returns `(group1 lowercased, group2
/// lowercased)`.
fn prefixed(word: &str, pattern: &str) -> Option<(String, String)> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<String, Regex>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        let compiled = cache
            .entry(pattern.to_string())
            .or_insert_with(|| {
                regex::RegexBuilder::new(&format!("^(?:{pattern})$"))
                    .case_insensitive(true)
                    .build()
                    .unwrap()
            })
            .clone();
        let caps = compiled.captures(word)?;
        Some((
            caps.get(1)?.as_str().to_lowercase(),
            caps.get(2)?.as_str().to_lowercase(),
        ))
    })
}

/// The tagger's `isCapitalizedWord` re-export (rules use it on surfaces).
pub fn is_capitalized(word: &str) -> bool {
    is_capitalized_word(word)
}

/// `StringTools.isMixedCase` re-export for the French rules.
pub fn is_mixed(word: &str) -> bool {
    is_mixed_case(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    fn tagger() -> Option<FrenchTagger> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.join("fr/dictionaries/french.dict").lt_exists() {
            return None;
        }
        Some(FrenchTagger::load(&dir).unwrap())
    }

    #[test]
    fn titlecase_iterating_chars() {
        assert_eq!(
            convert_to_title_case_iterating_chars("sars-cov"),
            "Sars-Cov"
        );
        assert_eq!(
            convert_to_title_case_iterating_chars("donne-t-il"),
            "Donne-T-Il"
        );
    }

    #[test]
    fn tags_common_words() {
        let Some(tagger) = tagger() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let readings = tagger.tag_word("parler");
        assert!(readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with('V'))));
        let unknown = tagger.tag_word("qqzzwwxx");
        assert_eq!(unknown.len(), 1);
        assert!(unknown[0].pos_tag.is_none());
    }
}
