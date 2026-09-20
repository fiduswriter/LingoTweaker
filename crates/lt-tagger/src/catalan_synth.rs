//! Port of `org.languagetool.synthesis.ca.CatalanSynthesizer` (+ the parts of
//! `BaseSynthesizer` it uses): inflected forms for a lemma and POS tag, with
//! the Catalan `ca.sor` number speller (`SOR_FILE_NAME = "/ca/ca.sor"`).
//!
//! Stage 1 wires the plain base lookup (`ca-ES_synth.dict` keyed by
//! `lemma|tag`, the manual `added.txt`/`removed.txt` synthesizers and the tag
//! list from `ca-ES_tags.txt`); stage 3 adds the `CatalanSynthesizer`
//! overrides: the `LemmasToIgnore` list, the variant `verbTags` retry and the
//! `toAddAfter` noun suffix.

use std::path::Path;

use lt_core::{AnalyzedToken, CoreError, Result};

use crate::manual_synth::ManualSynthesizer;
use crate::soros::Soros;
use crate::{DictionaryInfo, SynthDictionary};

pub struct CatalanSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    number_speller: Option<Soros>,
    roman_numberer: Option<Soros>,
    /// `CatalanSynthesizer.verbTags` char class for the requested variant.
    variant_verb_tags: &'static str,
}

/// `CatalanSynthesizer.LemmasToIgnore`.
const LEMMAS_TO_IGNORE: [&str; 4] = ["enterar", "sentar", "conseguir", "alcançar"];

/// `CatalanSynthesizer.verbTags` (`ca-ES` default).
fn variant_verb_tags(variant: &str) -> &'static str {
    if variant.ends_with("valencia") {
        "0VXZ13567"
    } else if variant.ends_with("balear") {
        "0BYZ1247"
    } else {
        "0CXY12"
    }
}

impl CatalanSynthesizer {
    /// Load from the vendored data directory (`ca/dictionaries`, `ca/words`).
    /// `variant` selects the `verbTags` fallback class (`ca-ES`,
    /// `ca-ES-valencia`, `ca-ES-balear`).
    pub fn from_data(data_dir: &Path, variant: &str) -> Result<Self> {
        let dict_dir = data_dir.join("ca/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("ca-ES_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("ca-ES_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("ca-ES_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read ca-ES_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("ca/words");
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        if let Some(manual) = &manual {
            for tag in &manual.possible_tags {
                if !possible_tags.contains(tag) {
                    possible_tags.push(tag.clone());
                }
            }
        }
        // `BaseSynthesizer` resolves `/ca/ca.sor`; when the resource is
        // missing Java's `Soros` stays null and the numeral is returned
        // unchanged.
        let number_speller = lt_data::fs::read_to_string(data_dir.join("ca/ca.sor"))
            .ok()
            .map(|source| Soros::new(&source, "ca"));
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
            number_speller,
            roman_numberer,
            variant_verb_tags: variant_verb_tags(variant),
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

    /// `CatalanSynthesizer.pVerb` (`V.*[CVBXYZ0123456]`, full match).
    fn variant_retry_applies(pos_tag: &str) -> bool {
        pos_tag.starts_with('V')
            && pos_tag
                .chars()
                .last()
                .is_some_and(|c| "CVBXYZ0123456".contains(c))
    }

    /// `SynthDictionary` tag list (`BaseSynthesizer.getPossibleTags`).
    pub fn possible_tags(&self) -> &[String] {
        &self.possible_tags
    }

    /// `BaseSynthesizer.getSpelledNumber`.
    pub fn get_spelled_number(&self, arabic_numeral: &str) -> String {
        match &self.number_speller {
            Some(speller) => speller.run(arabic_numeral),
            None => arabic_numeral.to_string(),
        }
    }

    /// `BaseSynthesizer.getRomanNumber` (`core/Roman.sor`).
    pub fn get_roman_number(&self, arabic_numeral: &str) -> String {
        match &self.roman_numberer {
            Some(numberer) => numberer.run(arabic_numeral),
            None => arabic_numeral.to_string(),
        }
    }

    /// `CatalanSynthesizer.synthesize(AnalyzedToken, String, boolean)`.
    pub fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synthesize_impl(token, pos_tag, pos_tag_regexp, true)
    }

    /// `CatalanSynthesizer.synthesize(AnalyzedToken, String)` (the 2-arg
    /// override): the tag is always treated as a regexp and the
    /// `LemmasToIgnore` filter of the 3-arg regexp path does not apply.
    pub fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synthesize_impl(token, pos_tag, true, false)
    }

    fn synthesize_impl(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
        ignore_lemmas: bool,
    ) -> Vec<String> {
        // Java delegates `_spell_number_*` to `BaseSynthesizer` in both the
        // 2-arg and the 3-arg overload.
        if pos_tag.starts_with("_spell_number_") {
            return match pos_tag {
                "_spell_number_" => vec![self.get_spelled_number(&token.token)],
                "_spell_number_:feminine" => {
                    vec![self.get_spelled_number(&format!("feminine {}", token.token))]
                }
                "_spell_number_:Roman" => vec![self.get_roman_number(&token.token)],
                _ => Vec::new(),
            };
        }
        // `LemmasToIgnore` applies only to the regexp path in Java.
        if pos_tag_regexp && ignore_lemmas && LEMMAS_TO_IGNORE.contains(&token.lemma()) {
            return Vec::new();
        }
        let mut lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        let mut to_add_after = String::new();
        // verbs with noun
        if pos_tag.starts_with('V') {
            if let Some((base, after)) = lemma.clone().split_once(' ') {
                lemma = base.to_string();
                to_add_after = after.to_string();
            }
        }
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
        // if not found, try verbs from the active regional variant
        let variant_retry = if pos_tag_regexp {
            Self::variant_retry_applies(pos_tag)
        } else {
            pos_tag.starts_with('V')
        };
        if results.is_empty() && variant_retry {
            let retry = format!(
                "{}[{}]",
                &pos_tag[..pos_tag.len() - 1],
                self.variant_verb_tags
            );
            if let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{retry})$")) {
                for tag in &self.possible_tags {
                    if re.is_match(tag).unwrap_or(false) {
                        results.extend(self.lookup(&lemma, tag));
                    }
                }
            }
        }
        if !to_add_after.is_empty() {
            for result in &mut results {
                *result = format!("{result} {to_add_after}");
            }
        }
        results
    }

    /// `BaseSynthesizer.synthesizeForPosTags`: all lookup results for the
    /// possible tags accepted by the predicate.
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

    /// `CatalanSynthesizer.getTargetPosTag`: sort with the Catalan
    /// `PostagComparator` (3rd person over 1st, indicative over subjunctive)
    /// and return the last one to keep the previous results.
    pub fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        if pos_tags.is_empty() {
            return fallback.to_string();
        }
        let mut sorted: Vec<String> = pos_tags.to_vec();
        self.sort_pos_tags(&mut sorted);
        sorted.last().cloned().unwrap_or_default()
    }

    /// Java's `getTargetPosTag` sorts the caller's list in place (the
    /// `postag_replace` join in `MatchState.getTargetPosTag` depends on it).
    pub fn sort_pos_tags(&self, pos_tags: &mut [String]) {
        pos_tags.sort_by(|a, b| postag_compare(a, b));
    }
}

/// `CatalanSynthesizer.PostagComparator` (`java.util.List.sort` is stable, so
/// `sort_by` matches).
fn postag_compare(arg0: &str, arg1: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if arg0.len() > 4 && arg1.len() > 4 {
        if arg0.contains("3S") && arg1 == "1S" {
            return Ordering::Greater;
        }
        if arg0.contains("1S") && arg1.contains("3S") {
            return Ordering::Less;
        }
        if arg0 == "VMIP2P00" && arg1 == "VMIS3S00" {
            return Ordering::Greater;
        }
        if arg1 == "VMIP2P00" && arg0 == "VMIS3S00" {
            return Ordering::Less;
        }
        if arg0.as_bytes()[2] == b'I' && arg1.as_bytes()[2] != b'I' {
            return Ordering::Greater;
        }
        if arg1.as_bytes()[2] == b'I' && arg0.as_bytes()[2] != b'I' {
            return Ordering::Less;
        }
        if arg0.as_bytes()[4] == b'3' && arg1.as_bytes()[4] == b'1' {
            return Ordering::Greater;
        }
        if arg1.as_bytes()[4] == b'1' && arg0.as_bytes()[4] == b'3' {
            return Ordering::Less;
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod dbg_tests2 {
    use super::*;

    #[test]
    fn dbg_synth_mig() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.is_dir() {
            return;
        }
        let s = CatalanSynthesizer::from_data(&dir, "ca-ES").unwrap();
        eprintln!(
            "AQ0MP0={:?} AQAMP0={:?}",
            s.lookup("mig", "AQ0MP0"),
            s.lookup("mig", "AQAMP0")
        );
        let tok = AnalyzedToken::new("mig", Some("mig".to_string()), Some("AQ0MS0".to_string()));
        eprintln!("synth={:?}", s.synthesize(&tok, "AQ.MP.", true));
    }
}
