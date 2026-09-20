//! Port of `org.languagetool.synthesis.FrenchSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag over
//! `french_synth.dict`, with the French `isException` filter.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::soros::Soros;
use crate::{DictionaryInfo, SynthDictionary};

/// `FrenchSynthesizer.exceptionsEgrave`.
const EXCEPTIONS_EGRAVE: [&str; 3] = ["burkinabè", "koinè", "épistémè"];

pub struct FrenchSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    number_speller: Option<Soros>,
}

impl FrenchSynthesizer {
    /// Load from the vendored data directory (`fr/dictionaries`, `fr/words`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("fr/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("french_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("french_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("french_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read french_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("fr/words");
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        if let Some(manual) = &manual {
            for tag in &manual.possible_tags {
                if !possible_tags.contains(tag) {
                    possible_tags.push(tag.clone());
                }
            }
        }
        let number_speller = lt_data::fs::read_to_string(data_dir.join("fr/fr.sor"))
            .ok()
            .map(|source| Soros::new(&source, "fr"));
        Ok(Self {
            dict,
            possible_tags,
            manual,
            removed,
            number_speller,
        })
    }

    /// `BaseSynthesizer.lookup`.
    fn lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
        let key = format!("{lemma}|{pos_tag}");
        let mut results = self.dict.lookup(&key);
        if let Some(manual) = &self.manual {
            if let Some(forms) = manual.lookup(lemma, pos_tag) {
                results.extend(forms);
            }
        }
        if let Some(removed) = &self.removed {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        results
    }

    /// `FrenchSynthesizer.isException` (`removeExceptions` filter).
    fn is_exception(word: &str) -> bool {
        if word.starts_with("qq") {
            return true;
        }
        if word.ends_with('è') && !EXCEPTIONS_EGRAVE.contains(&word.to_lowercase().as_str()) {
            return true;
        }
        false
    }

    /// `BaseSynthesizer.getSpelledNumber`.
    pub fn get_spelled_number(&self, arabic_numeral: &str) -> String {
        match &self.number_speller {
            Some(speller) => speller.run(arabic_numeral),
            None => arabic_numeral.to_string(),
        }
    }

    /// `FrenchSynthesizer.synthesize(AnalyzedToken, String, boolean)`.
    pub fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        if !pos_tag_regexp {
            let spelled = match pos_tag {
                "_spell_number_" => Some(self.get_spelled_number(&token.token)),
                "_spell_number_:feminine" => {
                    Some(self.get_spelled_number(&format!("feminine {}", token.token)))
                }
                // `Roman.sor` is not vendored; like Java when the resource
                // is missing, the numeral is returned unchanged.
                "_spell_number_:Roman" => Some(token.token.clone()),
                _ => None,
            };
            if let Some(spelled) = spelled {
                return if Self::is_exception(&spelled) {
                    Vec::new()
                } else {
                    vec![spelled]
                };
            }
        }
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        let mut results = if pos_tag_regexp {
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pos_tag})$")) else {
                return Vec::new();
            };
            let mut results = Vec::new();
            for tag in &self.possible_tags {
                if re.is_match(tag).unwrap_or(false) {
                    results.extend(self.lookup(&lemma, tag));
                }
            }
            results
        } else {
            self.lookup(&lemma, pos_tag)
        };
        results.retain(|word| !Self::is_exception(word));
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    fn synth() -> Option<FrenchSynthesizer> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.join("fr/dictionaries/french_synth.dict").lt_exists() {
            return None;
        }
        Some(FrenchSynthesizer::from_data(&dir).unwrap())
    }

    #[test]
    fn synthesizes_regular_forms() {
        let Some(synth) = synth() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let token = AnalyzedToken::new(
            "manger",
            Some("manger".to_string()),
            Some("V inf".to_string()),
        );
        let forms = synth.synthesize(&token, "V ind pres 1 s", false);
        assert!(forms.contains(&"mange".to_string()), "forms: {forms:?}");
    }

    #[test]
    fn filters_egrave_exceptions() {
        assert!(FrenchSynthesizer::is_exception("qqchose"));
        assert!(FrenchSynthesizer::is_exception("informè"));
        assert!(!FrenchSynthesizer::is_exception("burkinabè"));
    }
}
