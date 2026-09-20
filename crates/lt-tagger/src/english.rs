//! English tagger: port of `org.languagetool.tagging.en.EnglishTagger` with
//! the case-handling semantics of `BaseTagger` and `StringTools`.

use lt_core::AnalyzedToken;

use crate::{Dictionary, ManualTagger};

/// Case predicates ported from `StringTools` (Java `Character` semantics on
/// Unicode letters).
pub fn is_all_uppercase(s: &str) -> bool {
    !s.chars().any(|c| c.is_alphabetic() && c.is_lowercase())
}

pub fn is_capitalized_word(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) if first.is_uppercase() => {
            chars.all(|c| !c.is_alphabetic() || c.is_lowercase())
        }
        _ => false,
    }
}

pub fn is_not_all_lowercase(s: &str) -> bool {
    s.chars().any(|c| c.is_alphabetic() && !c.is_lowercase())
}

pub fn is_mixed_case(s: &str) -> bool {
    !is_all_uppercase(s) && !is_capitalized_word(s) && is_not_all_lowercase(s)
}

pub fn uppercase_first_char(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return chars[0].to_uppercase().collect();
    }
    // `StringTools.changeFirstCharCase`: skip leading non-letter-or-digit
    let last = chars.len() - 1;
    let mut pos = 0usize;
    while !chars[pos].is_alphanumeric() && last > pos {
        pos += 1;
    }
    let mut out: String = chars[..pos].iter().collect();
    out.extend(chars[pos].to_uppercase());
    out.extend(&chars[pos + 1..]);
    out
}

/// `StringTools.lowercaseFirstChar` (`changeFirstCharCase(str, false)`).
pub fn lowercase_first_char(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return chars[0].to_lowercase().collect();
    }
    let last = chars.len() - 1;
    let mut pos = 0usize;
    while !chars[pos].is_alphanumeric() && last > pos {
        pos += 1;
    }
    let mut out: String = chars[..pos].iter().collect();
    out.extend(chars[pos].to_lowercase());
    out.extend(&chars[pos + 1..]);
    out
}

/// Port of the English tagger over a loaded Morfologik dictionary.
#[derive(Debug)]
pub struct EnglishTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl EnglishTagger {
    pub fn new(dict: Dictionary) -> Self {
        Self {
            dict,
            manual: ManualTagger::default(),
            removals: ManualTagger::default(),
        }
    }

    /// Like `new`, with the plain-text manual taggers of `BaseTagger`
    /// (`CombiningTagger(morfologik, manual, removals, overwrite=false)`).
    pub fn with_manual(dict: Dictionary, manual: ManualTagger, removals: ManualTagger) -> Self {
        Self {
            dict,
            manual,
            removals,
        }
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    /// `CombiningTagger.tag`: manual readings first, dictionary readings
    /// appended, removal tagger applied last (`overwriteWithManualTagger`
    /// is false for English).
    fn lookup(&self, word: &str) -> Vec<(String, String)> {
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

    /// All readings for one word, mirroring `EnglishTagger.tag`.
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        // typewriter apostrophe hack: lookups use straight apostrophes
        let lookup_word = if word.chars().count() > 1 && word.contains('’') {
            word.replace('’', "'")
        } else {
            word.to_string()
        };
        let lower_word = lookup_word.to_lowercase();
        let is_lowercase = lookup_word == lower_word;
        let is_mixed = is_mixed_case(&lookup_word);
        let is_all_upper = is_all_uppercase(&lookup_word);

        // Java replaces the typographic apostrophe before tagging, so the
        // reading (and fallback) surface is the straight-apostrophe form
        self.add_from_dict(&lookup_word, &lookup_word, &mut l);
        if !is_lowercase && !is_mixed {
            self.add_from_dict(&lookup_word, &lower_word, &mut l);
        }
        if l.is_empty() && is_all_upper {
            let first_upper = uppercase_first_char(&lower_word);
            self.add_from_dict(&lookup_word, &first_upper, &mut l);
        }
        if l.is_empty() && lower_word.ends_with("in'") {
            let mut corrected = lookup_word.clone();
            if is_all_upper {
                corrected.replace_range(corrected.len() - 1.., "G");
            } else {
                corrected.replace_range(corrected.len() - 1.., "g");
            }
            self.add_from_dict(&lookup_word, &corrected, &mut l);
            if !is_lowercase && !is_mixed {
                self.add_from_dict(&lookup_word, &corrected.to_lowercase(), &mut l);
            }
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(lookup_word.clone(), None, None));
        }
        l
    }

    /// Whether the word has a real (tagged) reading — used by the tokenizer's
    /// hyphenated-word check (`AnalyzedTokenReadings.isTagged`).
    pub fn is_tagged(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    fn add_from_dict(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.lookup(lookup) {
            out.push(AnalyzedToken::new(
                surface,
                Some(stem.clone()),
                Some(tag.clone()),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;
    use std::path::Path;

    fn en_tagger() -> Option<EnglishTagger> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/dictionaries");
        if !dir.lt_exists() {
            return None;
        }
        let info = crate::DictionaryInfo::load(&dir.join("english.info")).unwrap();
        let dict = Dictionary::load(&dir.join("english.dict"), &info).unwrap();
        Some(EnglishTagger::new(dict))
    }

    #[test]
    fn case_handling_matches_lt() {
        assert!(is_capitalized_word("Hello"));
        assert!(!is_mixed_case("Hello"));
        assert!(is_mixed_case("iPhone"));
        assert!(is_all_uppercase("NATO"));
        assert!(!is_all_uppercase("Nato"));
    }

    #[test]
    fn tags_common_words() {
        let Some(tagger) = en_tagger() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let walk = tagger.tag_word("walk");
        assert!(walk.iter().any(|t| t.pos_tag.as_deref() == Some("NN")));
        assert!(walk.iter().all(|t| t.token == "walk"));

        // sentence-initial capital gets lowercase-word tags
        let hello = tagger.tag_word("Walk");
        assert!(hello.iter().any(|t| t.pos_tag.as_deref() == Some("VB")));

        // unknown word → single null-tag reading
        let unknown = tagger.tag_word("qqzzwwxx");
        assert_eq!(unknown.len(), 1);
        assert!(unknown[0].pos_tag.is_none());
        assert!(!tagger.is_tagged("qqzzwwxx"));
        assert!(tagger.is_tagged("walk"));
    }
}
