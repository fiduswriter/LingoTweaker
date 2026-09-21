//! Port of `org.languagetool.synthesis.ru.RussianSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag.
//!
//! `RussianSynthesizer` is a plain `BaseSynthesizer` with a `null` number
//! speller (`super(RESOURCE_FILENAME, TAGS_FILE_NAME, "ru")`), so
//! `_spell_number_` returns the numeral unchanged; the Roman numberer reads the
//! shared `core/Roman.sor`.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::soros::Soros;
use crate::{DictionaryInfo, SynthDictionary};

pub struct RussianSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
    roman_numberer: Option<Soros>,
}

impl RussianSynthesizer {
    /// Load from the vendored data directory (`ru/dictionaries`, `ru/words`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("ru/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("russian_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("russian_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("tags_russian.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read tags_russian.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("ru/words");
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        let do_not_synthesize = ManualSynthesizer::load(&words_dir.join("do-not-synthesize.txt"));
        if let Some(manual) = &manual {
            for tag in &manual.possible_tags {
                if !possible_tags.contains(tag) {
                    possible_tags.push(tag.clone());
                }
            }
        }
        // `BaseSynthesizer.createRomanNumberer` reads the shared
        // `core/Roman.sor` (`/Roman.sor` in Java).
        let roman_numberer = lt_data::fs::read_to_string(data_dir.join("core/Roman.sor"))
            .ok()
            .map(|source| Soros::new(&source, "Roman"));
        Ok(Self {
            dict,
            possible_tags,
            manual,
            removed,
            do_not_synthesize,
            roman_numberer,
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
        if let Some(removed) = &self.do_not_synthesize {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        results
    }

    /// `BaseSynthesizer.getSpelledNumber`: a `null` number speller returns the
    /// numeral unchanged (Russian's `sorosFileName` is null).
    pub fn get_spelled_number(&self, arabic_numeral: &str) -> String {
        arabic_numeral.to_string()
    }

    /// `BaseSynthesizer.getRomanNumber` (`core/Roman.sor`).
    pub fn get_roman_number(&self, arabic_numeral: &str) -> String {
        match &self.roman_numberer {
            Some(numberer) => numberer.run(arabic_numeral),
            None => arabic_numeral.to_string(),
        }
    }

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String, boolean)`.
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
                "_spell_number_:Roman" => Some(self.get_roman_number(&token.token)),
                _ => None,
            };
            if let Some(spelled) = spelled {
                return vec![spelled];
            }
        }
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        if pos_tag_regexp {
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pos_tag})$")) else {
                return Vec::new();
            };
            let mut results = Vec::new();
            for tag in &self.possible_tags {
                if re.is_match(tag).unwrap_or(false) {
                    results.extend(self.lookup(&lemma, tag));
                }
            }
            return results;
        }
        self.lookup(&lemma, pos_tag)
    }

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String)`.
    pub fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synthesize(token, pos_tag, false)
    }

    /// `BaseSynthesizer.getTargetPosTag`: return the last one to keep the
    /// previous results.
    pub fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        pos_tags
            .last()
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }
}
