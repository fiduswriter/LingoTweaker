//! Port of `org.languagetool.synthesis.GermanSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag,
//! including compound handling via the strict German compound tokenizer.

use std::collections::HashSet;
use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};
use lt_tokenize::GermanCompoundTokenizer;

use crate::manual_synth::ManualSynthesizer;
use crate::{lowercase_first_char, uppercase_first_char, DictionaryInfo, SynthDictionary};

/// `GermanSynthesizer.REMOVE` (old spellings and rare pronoun forms).
const REMOVE: [&str; 29] = [
    "unsren",
    "unsrem",
    "unsres",
    "unsre",
    "unsern",
    "unserm",
    "unsrer",
    "angepaßt",
    "beschloß",
    "biß",
    "entschloß",
    "ergoß",
    "faßt",
    "genoß",
    "paßt",
    "paßte",
    "preßt",
    "preßte",
    "riß",
    "schloß",
    "streßtest",
    "vergißt",
    "verlaß",
    "verläßt",
    "vermiß",
    "vermißt",
    "wißt",
    "wußtest",
    "wüßtest",
];

fn starts_with_lowercase(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_lowercase)
}

pub struct GermanSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
    compound: GermanCompoundTokenizer,
}

impl GermanSynthesizer {
    /// Load from the vendored data directory (`de/dictionaries`,
    /// `de/words`, `de/compound`).
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("de/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("german_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("german_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("german_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read german_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("de/words");
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
        let compound_dir = data_dir.join("de/compound");
        let compound = GermanCompoundTokenizer::load(
            &compound_dir.join("wordsGerman.txt"),
            &compound_dir.join("exceptionsGerman.txt"),
            true,
        )?;
        Ok(Self {
            dict,
            possible_tags,
            manual,
            removed,
            do_not_synthesize,
            compound,
        })
    }

    /// `BaseSynthesizer.lookup`.
    fn base_lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
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

    /// `GermanSynthesizer.lookup`: keep the case class of the lemma (plus the
    /// mein/ich pronoun exceptions).
    pub fn lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
        let lookup = self.base_lookup(lemma, pos_tag);
        let lc_lemma = starts_with_lowercase(lemma);
        let mut results = Vec::new();
        for s in lookup {
            let lc_lookup = starts_with_lowercase(&s);
            if lc_lemma == lc_lookup
                || lemma == "mein"
                || (lemma == "ich" && !REMOVE.contains(&s.as_str()))
            {
                results.push(s);
            }
        }
        results
    }

    fn is_remove(form: &str) -> bool {
        REMOVE.contains(&form)
    }

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String)` (number spelling
    /// via `de.sor` is not ported; no German rule uses `_spell_number_`).
    fn synthesize_exact(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        self.lookup(&lemma, pos_tag)
    }

    /// `BaseSynthesizer.synthesize(AnalyzedToken, String, boolean)` plus the
    /// `GermanSynthesizer` compound fallback and `REMOVE` filter.
    pub fn synthesize(&self, token: &AnalyzedToken, pos_tag: &str, regexp: bool) -> Vec<String> {
        let result = if regexp {
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pos_tag})$")) else {
                return Vec::new();
            };
            let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
            self.synthesize_for_pos_tags(&lemma, &|tag: &str| re.is_match(tag).unwrap_or(false))
        } else {
            self.synthesize_exact(token, pos_tag)
        };
        if result.is_empty() {
            return self.compound_forms(token, pos_tag, regexp);
        }
        result
            .into_iter()
            .filter(|form| !Self::is_remove(form))
            .collect()
    }

    /// `BaseSynthesizer.synthesizeForPosTags`.
    pub fn synthesize_for_pos_tags(
        &self,
        lemma: &str,
        accept_tag: &dyn Fn(&str) -> bool,
    ) -> Vec<String> {
        let mut results = Vec::new();
        for tag in &self.possible_tags {
            if accept_tag(tag) {
                results.extend(self.lookup(lemma, tag));
            }
        }
        results
    }

    /// `GermanSynthesizer.getCompoundForms`.
    fn compound_forms(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        let lemma = token.stem.clone();
        let mut parts = match &lemma {
            Some(lemma) => self.compound.tokenize(lemma),
            None => Vec::new(),
        };
        if parts.is_empty() {
            return Vec::new();
        }
        let mut maybe_hyphen = "";
        if parts.len() == 1 {
            if let Some(lemma) = &lemma {
                parts = lemma.split('-').map(|s| s.to_string()).collect();
                if parts.len() > 1 {
                    maybe_hyphen = "-";
                }
            }
        }
        let first_part = parts[..parts.len() - 1].join(maybe_hyphen);
        let last_part_raw = parts[parts.len() - 1].clone();
        let last_part = uppercase_first_char(&last_part_raw);
        let uppercase_last_part = !maybe_hyphen.is_empty()
            && last_part_raw.chars().next().is_some_and(char::is_uppercase);
        let last_part_forms = if pos_tag_regexp {
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{pos_tag})$")) else {
                return Vec::new();
            };
            self.synthesize_for_pos_tags(&last_part, &|tag: &str| re.is_match(tag).unwrap_or(false))
        } else {
            self.lookup(&last_part, pos_tag)
        };
        let mut results: Vec<String> = Vec::new();
        for part in last_part_forms {
            let form = if uppercase_last_part {
                part
            } else {
                lowercase_first_char(&part)
            };
            let candidate = format!("{first_part}{maybe_hyphen}{form}");
            if !results.contains(&candidate) {
                results.push(candidate);
            }
        }
        results
    }
}

/// The set of `REMOVE` entries, exposed for tests.
pub fn removed_forms() -> HashSet<&'static str> {
    REMOVE.iter().copied().collect()
}
