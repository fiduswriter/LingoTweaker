//! Port of `org.languagetool.tokenizers.de.GermanCompoundTokenizer`, which
//! wraps Daniel Naber's jWordSplitter (`de.danielnaber:jwordsplitter:4.7`).
//!
//! The splitting algorithm (`AbstractWordSplitter`/`GermanWordSplitter`/
//! `GermanInterfixDisambiguator`/`ExceptionSplits`, Apache-2.0) is ported
//! verbatim; the embedded word list and exception list are vendored as
//! `data/de/compound/wordsGerman.txt` / `exceptionsGerman.txt` (extracted
//! from the jwordsplitter jar, see `data/manifest.json`).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lt_core::{CoreError, Result};

/// jwordsplitter `GermanWordSplitter.INTERFIXES`, order matters
/// (`findInterfixOrNull` returns the first match).
const INTERFIXES: [&str; 3] = ["s-", "s", "-"];

/// jwordsplitter dictionary additions from `GermanCompoundTokenizer.
/// ExtendedGermanWordSplitter.extendedList()`.
const EXTENDED_WORDS: &[&str] = &[
    "influencer",
    "katheterisierung",
    "rücklass",
    "abdichtung",
    "laptop",
    "verschattung",
    "paradeiser",
    "einreichung",
    "bestatter",
    "divergenz",
    "schrumpf",
    "degustation",
    "schaft",
    "abstreifer",
    "aufputz",
    "glühwürmchen",
    "aufwertung",
    "einhausung",
    "lackier",
    "zarge",
    "pluralisierung",
    "schanzen",
    "abscheide",
    "rangier",
    "temporal",
    "kartonage",
    "kartonagen",
    "rebellion",
    "binokular",
    "umverlegung",
    "umhausung",
    "überholung",
    "chloroplasten",
    "nachrangigkeit",
    "spital",
    "turnus",
    "teilnehmenden",
    "pensionisten",
    "graduierten",
    "beladung",
    "controller",
    "resilienz",
    "mitführ",
    "trauma",
    "abtau",
    "normung",
    "mikroskopie",
    "bitumen",
    "erfolglosigkeit",
    "pneumatik",
    "anlasser",
    "allozierung",
    "alphabetisierung",
    "aktuator",
    "akademisierung",
    "allergiker",
    "queer",
    "filament",
    "querung",
    "curling",
    "opioid",
    "booster",
    "schmuse",
    "thrombozyten",
    "dysfunktion",
    "storchen",
    "nasch",
    "esperanto",
    "passivierung",
    "radikalisierung",
    "erleuchtung",
    "verwalter",
    "verbiss",
    "ausleih",
    "rutsch",
    "kufen",
    "entferner",
    "debitoren",
    "terrakotta",
    "graffiti",
    "auffahr",
    "anmutung",
    "kritzel",
    "salami",
    "eukalyptus",
    "kreativ",
    "hochvolt",
    "trading",
    "extraktion",
    "verstetigung",
    "diagonal",
    "margen",
    "synonym",
    "aufbringung",
    "robustheit",
    "nachuntersuchung",
    "erstkommunion",
    "hauptstadt",
    "neustart",
    "polarisierung",
    "vollstreckbarkeit",
    "vollziehung",
    "kasko",
    "blitzableiter",
    "abschattungen",
    "kuscheltier",
    "gastro",
    "hortensien",
];

/// Exception splits from the `GermanCompoundTokenizer` constructor.
const EXCEPTION_SPLITS: &[(&str, &[&str])] = &[
    ("Absolventen", &["Absolventen"]),
    ("Acetat", &["Acetat"]),
    ("Alkoholabstinenz", &["Alkohol", "abstinenz"]),
    ("Androgen", &["Androgen"]),
    ("Auberginen", &["Auberginen"]),
    ("Auckland", &["Auckland"]),
    ("Boston", &["Boston"]),
    ("Brandenburg", &["Brandenburg"]),
    ("Broadcast", &["Broadcast"]),
    ("Buchsbaum", &["Buchsbaum"]),
    ("Chiemsee", &["Chiemsee"]),
    ("Coffein", &["Coffein"]),
    ("Drohnen", &["Drohnen"]),
    ("Eiben", &["Eiben"]),
    ("Eingroschen", &["Eingroschen"]),
    ("Einkomponenten", &["Einkomponenten"]),
    ("Elster", &["Elster"]),
    ("Engineering", &["Engineering"]),
    ("Factoring", &["Factoring"]),
    ("Flexodruck", &["Flexo", "druck"]),
    ("Graviton", &["Graviton"]),
    ("Göttinnen", &["Göttinnen"]),
    ("Hallesche", &["Hallesche"]),
    ("Hinspiel", &["Hinspiel"]),
    ("Homogen", &["Homogen"]),
    ("Kolleggen", &["Kolleggen"]),
    ("Karstadt", &["Karstadt"]),
    ("Kartier", &["Kartier"]),
    ("Kaukasus", &["Kaukasus"]),
    ("Knoblauch", &["Knoblauch"]),
    ("Kollagen", &["Kollagen"]),
    ("Kommerz", &["Kommerz"]),
    ("Mentoring", &["Mentoring"]),
    ("Monarchen", &["Monarchen"]),
    ("Oligarchen", &["Oligarchen"]),
    ("Optimal", &["Optimal"]),
    ("Saunieren", &["Saunieren"]),
    ("Schiessen", &["Schiessen"]),
    ("Spielgeleier", &["Spielgeleier"]),
    ("Halleschen", &["Halleschen"]),
    ("Reinigungstab", &["Reinigungs", "tab"]),
    ("Reinigungstabs", &["Reinigungs", "tabs"]),
    ("Tauschwerte", &["Tausch", "werte"]),
    ("Tauschwertes", &["Tausch", "wertes"]),
    ("Kinderspielen", &["Kinder", "spielen"]),
    ("Buchhaltungstrick", &["Buchhaltungs", "trick"]),
    ("Buchhaltungstricks", &["Buchhaltungs", "tricks"]),
    ("Haushaltstrick", &["Haushalts", "trick"]),
    ("Haushaltstricks", &["Haushalts", "tricks"]),
    ("Verkaufstrick", &["Verkaufs", "trick"]),
    ("Verkaufstricks", &["Verkaufs", "tricks"]),
    ("Ablenkungstrick", &["Ablenkungs", "trick"]),
    ("Ablenkungstricks", &["Ablenkungs", "tricks"]),
    ("Manipulationstrick", &["Manipulations", "trick"]),
    ("Manipulationstricks", &["Manipulations", "tricks"]),
    ("Erziehungstrick", &["Erziehungs", "trick"]),
    ("Erziehungstricks", &["Erziehungs", "tricks"]),
    ("Messetage", &["Messe", "tage"]),
    ("Messetagen", &["Messe", "tagen"]),
    ("karamelligen", &["karamelligen"]),
    ("Häkelnadel", &["Häkel", "nadel"]),
    ("Häkelnadeln", &["Häkel", "nadeln"]),
    ("Freiberg", &["Freiberg"]),
    ("Abtestat", &["Abtestat"]),
    ("Abtestaten", &["Abtestaten"]),
    ("Freibergs", &["Freibergs"]),
    ("Kreuzberg", &["Kreuzberg"]),
    ("Kreuzbergs", &["Kreuzbergs"]),
    ("Digitalisierung", &["Digitalisierung"]),
    ("Abtrocknung", &["Abtrocknung"]),
    ("Erlösung", &["Erlösung"]),
    ("Feuerung", &["Feuerung"]),
    ("Aktivierung", &["Aktivierung"]),
    ("Protokollierung", &["Protokollierung"]),
    ("Budgetierung", &["Budgetierung"]),
    ("Faltung", &["Faltung"]),
    ("Anhäufung", &["Anhäufung"]),
    ("Aufkohlung", &["Aufkohlung"]),
    ("Festigung", &["Festigung"]),
    ("Allerheiligen", &["Allerheiligen"]),
    ("Druckerpressen", &["Drucker", "pressen"]),
    ("Habitat", &["Habitat"]),
    ("Augarten", &["Augarten"]),
    ("Auszeit", &["Auszeit"]),
    ("Bewegtbild", &["Bewegt", "bild"]),
    ("Bigband", &["Bigband"]),
    ("Bisexuelle", &["Bisexuelle"]),
    ("Bisexuellen", &["Bisexuellen"]),
    ("Bunsenbrenner", &["Bunsenbrenner"]),
    ("Carbon", &["Carbon"]),
    ("Carsharing", &["Carsharing"]),
    ("Castor", &["Castor"]),
    ("Catering", &["Catering"]),
    ("Cholesterin", &["Cholesterin"]),
    ("Damast", &["Damast"]),
    ("Dispositiv", &["Dispositiv"]),
    ("Emittent", &["Emittent"]),
    ("Emittenten", &["Emittenten"]),
    ("Express", &["Express"]),
    ("Fairness", &["Fairness"]),
    ("Fiberglas", &["Fiberglas"]),
    ("Globus", &["Globus"]),
    ("Göttinnen", &["Göttinnen"]),
    ("Illustration", &["Illustration"]),
    ("Muttertag", &["Muttertag"]),
    ("Muttertags", &["Muttertags"]),
    ("Patriarchen", &["Patriarchen"]),
    ("Phosgen", &["Phosgen"]),
    ("Vatertag", &["Vatertag"]),
    ("Vatertags", &["Vatertags"]),
    ("Vaterland", &["Vaterland"]),
    ("Vaterlands", &["Vaterlands"]),
    ("Wehrmacht", &["Wehrmacht"]),
    ("Wehrmachts", &["Wehrmachts"]),
];

const COMMENT_CHAR: char = '#';
const DELIMITER_CHAR: char = '|';

/// jwordsplitter `ExceptionSplits` (plain-text exception list, `word|part`
/// lines; `/NS` lines additionally register the `n`/`s` suffixed forms).
#[derive(Debug, Default)]
struct ExceptionSplits {
    map: HashMap<String, Vec<String>>,
}

impl ExceptionSplits {
    fn add_split(&mut self, complete_word: &str, parts: &[&str]) {
        self.map.insert(
            complete_word.to_lowercase(),
            parts.iter().map(|s| s.to_string()).collect(),
        );
    }

    fn load(path: &Path) -> Result<Self> {
        let text = lt_data::fs::read_to_string(path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
        let mut result = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(COMMENT_CHAR) {
                continue;
            }
            let parts: Vec<String> = line
                .replace("/NS", "")
                .split(DELIMITER_CHAR)
                .map(|s| s.to_string())
                .collect();
            let complete_word: String = line.replace(DELIMITER_CHAR, "");
            if complete_word.contains('/') {
                if complete_word.ends_with("/NS") {
                    let real_word = complete_word.replace("/NS", "").to_lowercase();
                    result.map.insert(real_word.clone(), parts.clone());
                    result
                        .map
                        .insert(real_word.clone() + "n", add_to_last_part(&parts, "n"));
                    result
                        .map
                        .insert(real_word + "s", add_to_last_part(&parts, "s"));
                } else {
                    return Err(CoreError::Data(format!(
                        "unknown suffix in jwordsplitter exception line: {line}"
                    )));
                }
            } else {
                result.map.insert(complete_word.to_lowercase(), parts);
            }
        }
        Ok(result)
    }

    fn get(&self, word: &str) -> Option<Vec<String>> {
        let lc_word = word.to_lowercase();
        let result = self.map.get(&lc_word)?;
        let check: String = result.join("");
        if lc_word == check.to_lowercase() {
            // re-split the original word by the same lengths to keep its case
            let mut out = Vec::with_capacity(result.len());
            let mut offset = 0usize;
            for part in result {
                let len = part.chars().count();
                let slice: String = word.chars().skip(offset).take(len).collect();
                out.push(slice);
                offset += len;
            }
            Some(out)
        } else {
            Some(result.clone())
        }
    }
}

fn add_to_last_part(parts: &[String], suffix: &str) -> Vec<String> {
    let mut result = parts.to_vec();
    if let Some(last) = result.last_mut() {
        last.push_str(suffix);
    }
    result
}

/// `GermanInterfixDisambiguator`.
#[derive(Debug)]
struct InterfixDisambiguator {
    dictionary: HashSet<String>,
}

impl InterfixDisambiguator {
    fn disambiguate(&self, parts: &[String]) -> Vec<String> {
        let mut new_parts = parts.to_vec();
        let last_part_idx = parts.len().saturating_sub(1);
        if parts.len() > 1 {
            let last_part = &parts[last_part_idx];
            if ["samt", "samts", "samtes"].contains(&last_part.as_str()) {
                // Verkehr+s+amt = Verkehrs+amt
                new_parts[last_part_idx - 1] = format!("{}s", parts[last_part_idx - 1]);
                new_parts[last_part_idx] =
                    last_part.strip_prefix('s').unwrap_or(last_part).to_string();
                return new_parts;
            }
        }
        let mut i = new_parts.len();
        while i >= 3 {
            i -= 1;
            let part = new_parts[i].clone();
            let prev_part = new_parts[i - 1].clone();
            let prev_prev_part = new_parts[i - 2].clone();
            if prev_part == "s" {
                let part_is_word = self.is_word(&format!("s{part}"));
                if !part_is_word || prev_prev_part == "Verhalten" {
                    // Schönheit+s+tempel = Schönheits-tempel
                    new_parts[i - 2] = format!("{prev_prev_part}s");
                    new_parts.remove(i - 1);
                }
            }
        }
        new_parts
    }

    fn is_word(&self, word: &str) -> bool {
        self.dictionary.contains(&word.to_lowercase())
    }
}

/// Port of `GermanCompoundTokenizer` (strict or non-strict mode).
#[derive(Debug)]
pub struct GermanCompoundTokenizer {
    words: HashSet<String>,
    exceptions: ExceptionSplits,
    disambiguator: InterfixDisambiguator,
    strict_mode: bool,
    hide_interfix_characters: bool,
    minimum_word_length: usize,
    maximum_word_length: usize,
}

impl GermanCompoundTokenizer {
    /// Load the vendored jwordsplitter data (`words_file`, `exceptions_file`).
    pub fn load(words_file: &Path, exceptions_file: &Path, strict: bool) -> Result<Self> {
        let text = lt_data::fs::read_to_string(words_file)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", words_file.display())))?;
        let mut words = HashSet::new();
        for line in text.lines() {
            if line.starts_with(COMMENT_CHAR) {
                continue;
            }
            words.insert(line.trim().to_lowercase());
        }
        for word in EXTENDED_WORDS {
            words.insert((*word).to_string());
        }
        let mut exceptions = ExceptionSplits::load(exceptions_file)?;
        for (word, parts) in EXCEPTION_SPLITS {
            exceptions.add_split(word, parts);
        }
        let disambiguator = InterfixDisambiguator {
            dictionary: words.clone(),
        };
        Ok(Self {
            words,
            exceptions,
            disambiguator,
            strict_mode: strict,
            // `new ExtendedGermanWordSplitter(false)` in the Java constructor
            hide_interfix_characters: false,
            minimum_word_length: 3,
            maximum_word_length: 70,
        })
    }

    /// `AbstractWordSplitter.getAllSplits(String)`: all splits where at
    /// least one side is a dictionary word, combined left-to-right and
    /// right-to-left (input order preserved, duplicates removed). Java
    /// throws `InputTooLongException`; the caller treats that as "no
    /// splits", so this returns an empty list for over-long words.
    pub fn get_all_splits(&self, word: &str) -> Vec<Vec<String>> {
        if word.chars().count() > self.maximum_word_length {
            return Vec::new();
        }
        let mut result = self.get_all_splits_direction(word, true);
        for split in self.get_all_splits_direction(word, false) {
            if !result.contains(&split) {
                result.push(split);
            }
        }
        result
    }

    fn get_all_splits_direction(&self, word: &str, from_left: bool) -> Vec<Vec<String>> {
        let chars: Vec<char> = word.chars().collect();
        let min = self.minimum_word_length;
        let mut result: Vec<Vec<String>> = Vec::new();
        let mut i: isize = if from_left {
            min as isize
        } else {
            chars.len() as isize - min as isize
        };
        loop {
            let end = if from_left {
                i < chars.len() as isize - min as isize
            } else {
                i > min as isize
            };
            if !end || i <= 0 || i as usize >= chars.len() {
                break;
            }
            let left: String = chars[..i as usize].iter().collect();
            let right: String = chars[i as usize..].iter().collect();
            let relevant = if from_left { &left } else { &right };
            if self.is_simple_word(relevant) {
                result.push(vec![left.clone(), right.clone()]);
                let other = if from_left { &right } else { &left };
                // Java recurses into the *public* `getAllSplits(String)`
                // (both directions), not the one-direction variant
                let other_splits = self.get_all_splits(other);
                for other_split in other_splits {
                    let mut sub: Vec<String> = Vec::new();
                    if from_left {
                        sub.push(left.clone());
                        sub.extend(other_split);
                    } else {
                        sub.extend(other_split);
                        sub.push(right.clone());
                    }
                    result.push(sub);
                }
            }
            i += if from_left { 1 } else { -1 };
        }
        result
    }

    pub fn tokenize(&self, word: &str) -> Vec<String> {
        if word.chars().count() > self.maximum_word_length {
            return vec![word.to_string()];
        }
        self.split_word(word)
    }

    fn split_word(&self, word: &str) -> Vec<String> {
        let trimmed = word.trim();
        if let Some(exception) = self.exceptions.get(trimmed) {
            return exception;
        }
        match self.split(trimmed, false, false) {
            None => vec![trimmed.to_string()],
            Some(parts) => {
                let mut disambiguated = self.disambiguator.disambiguate(&parts);
                self.clean_leading_and_trailing_hyphens(&mut disambiguated);
                disambiguated
            }
        }
    }

    fn clean_leading_and_trailing_hyphens(&self, parts: &mut [String]) {
        for element in parts.iter_mut() {
            if let Some(rest) = element.strip_prefix('-') {
                *element = rest.to_string();
            }
            if let Some(rest) = element.strip_suffix('-') {
                *element = rest.to_string();
            }
        }
    }

    fn split(
        &self,
        word: &str,
        allow_interfix_removal: bool,
        collect_subwords: bool,
    ) -> Option<Vec<String>> {
        if let Some(parts) = self.exceptions.get(word) {
            return Some(parts);
        }
        let lc_word = word.to_lowercase();
        let removable_interfix = self.find_interfix(&lc_word);
        let word_without_interfix = match &removable_interfix {
            Some(interfix) => word[..word.len() - interfix.len()].to_string(),
            None => word.to_string(),
        };
        let can_interfix_be_removed = removable_interfix.is_some() && allow_interfix_removal;

        if self.is_simple_word(word) && !collect_subwords {
            return Some(vec![word.to_string()]);
        }
        if can_interfix_be_removed && self.is_simple_word(&word_without_interfix) {
            return Some(if self.hide_interfix_characters {
                vec![word_without_interfix]
            } else {
                vec![
                    word_without_interfix,
                    removable_interfix.unwrap_or_default(),
                ]
            });
        }
        let mut parts = self.split_from_right(word, collect_subwords);
        if parts.is_none() && self.is_simple_word(word) {
            parts = Some(vec![word.to_string()]);
        } else if let Some(ref mut current) = parts {
            if self.is_simple_word(word) && !current.contains(&word.to_string()) {
                current.push(word.to_string());
            }
        }
        if parts.is_none() && self.ends_with_interfix(&lc_word) {
            parts = self.split_from_right(&word_without_interfix, collect_subwords);
            if let Some(ref mut current) = parts {
                if !self.hide_interfix_characters {
                    if let Some(interfix) = removable_interfix {
                        current.push(interfix);
                    }
                }
            }
        }
        parts
    }

    fn split_from_right(&self, word: &str, collect_subwords: bool) -> Option<Vec<String>> {
        if let Some(parts) = self.exceptions.get(word) {
            return Some(parts);
        }
        let chars: Vec<char> = word.chars().collect();
        let mut parts: Option<Vec<String>> = None;
        let mut i = chars.len() as isize - self.minimum_word_length as isize;
        while i >= self.minimum_word_length as isize {
            let left_part: String = chars[..i as usize].iter().collect();
            let right_part: String = chars[i as usize..].iter().collect();
            if !self.strict_mode {
                if let Some(exception) = self.exception_split_for(&right_part, &left_part) {
                    return Some(exception);
                }
            }
            if self.is_simple_word(&right_part) {
                let left_part_parts = self.split(&left_part, true, collect_subwords);
                match left_part_parts {
                    Some(left_parts) => {
                        if collect_subwords {
                            let current = parts.get_or_insert_with(Vec::new);
                            for left_part_part in &left_parts {
                                if !current.contains(left_part_part) {
                                    current.push(left_part_part.clone());
                                }
                            }
                            if !current.contains(&right_part) {
                                current.push(right_part.clone());
                            }
                            if let Some(right_exceptions) = self.exceptions.get(&right_part) {
                                for exception in right_exceptions {
                                    if !current.contains(&exception) {
                                        current.push(exception);
                                    }
                                }
                            }
                        } else {
                            let mut new_parts = left_parts;
                            new_parts.push(right_part.clone());
                            parts = Some(new_parts);
                        }
                    }
                    None => {
                        if !self.strict_mode {
                            parts = Some(vec![left_part, right_part]);
                        }
                    }
                }
            } else if !self.strict_mode && self.is_simple_word(&left_part) {
                parts = Some(vec![left_part, right_part]);
            }
            i -= 1;
        }
        parts
    }

    fn exception_split_for(&self, right_part: &str, left_part: &str) -> Option<Vec<String>> {
        if let Some(exception) = self.exceptions.get(right_part) {
            let mut parts = vec![left_part.to_string()];
            parts.extend(exception);
            return Some(parts);
        }
        if let Some(exception) = self.exceptions.get(left_part) {
            let mut parts = exception;
            parts.push(right_part.to_string());
            return Some(parts);
        }
        None
    }

    fn find_interfix(&self, word: &str) -> Option<String> {
        for interfix in INTERFIXES {
            if word.ends_with(interfix) {
                return Some(interfix.to_string());
            }
        }
        None
    }

    fn ends_with_interfix(&self, word: &str) -> bool {
        INTERFIXES.iter().any(|interfix| word.ends_with(interfix))
    }

    fn is_simple_word(&self, part: &str) -> bool {
        part.chars().count() >= self.minimum_word_length
            && self.words.contains(&part.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use lt_data::PathExt as _;
    use std::path::PathBuf;

    use super::*;

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/de/compound")
    }

    fn tokenizer(strict: bool) -> Option<GermanCompoundTokenizer> {
        let dir = data_dir();
        if !dir.lt_exists() {
            return None;
        }
        Some(
            GermanCompoundTokenizer::load(
                &dir.join("wordsGerman.txt"),
                &dir.join("exceptionsGerman.txt"),
                strict,
            )
            .unwrap(),
        )
    }

    #[test]
    fn applies_exception_splits_case_insensitively() {
        let Some(t) = tokenizer(true) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert_eq!(t.tokenize("Tauschwerte"), vec!["Tausch", "werte"]);
    }

    #[test]
    fn splits_known_compounds() {
        // Expected values probed against jwordsplitter 4.7 in Docker (the
        // interfix disambiguator merges "s" into the previous part)
        let Some(t) = tokenizer(true) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert_eq!(t.tokenize("Erhebungsfehler"), vec!["Erhebungs", "fehler"]);
        assert_eq!(t.tokenize("Urlaubsorte"), vec!["Urlaubs", "orte"]);
        assert_eq!(t.tokenize("Schönheitstempel"), vec!["Schönheits", "tempel"]);
        assert_eq!(
            t.tokenize("Donaudampfschifffahrt"),
            vec!["Donau", "dampf", "schiff", "fahrt"]
        );
        assert_eq!(t.tokenize("Arbeitsplatz"), vec!["Arbeitsplatz"]);
    }

    #[test]
    fn get_all_splits_matches_jwordsplitter() {
        // Expected values probed against jwordsplitter 4.7 (`AllSplits.java`
        // with the legacy jar)
        let Some(t) = tokenizer(true) else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let cases: &[(&str, &str)] = &[
            ("Arbeitszimmer", "Arbeit,szimmer|Arbeits,zimmer"),
            (
                "Dampfschifffahrtskapitän",
                "Dampf,schifffahrtskapitän|Dampf,schi,fffahrtskapitän|Dampf,schi,fffahrts,kapitän|Dampf,schiff,fahrtskapitän|Dampf,schiff,fahr,tskapitän|Dampf,schiff,fahrt,skapitän|Dampf,schiff,fahrts,kapitän|Dampf,schifffahrts,kapitän|Dampfs,chifffahrtskapitän|Dampfs,chifffahrts,kapitän|Dampfschifffahrts,kapitän",
            ),
            (
                "Kinderzimmer",
                "Kind,erzimmer|Kind,erz,immer|Kinde,rzimmer|Kinder,zimmer",
            ),
            (
                "Bundesausbildungsförderungsgesetz",
                "Bund,esausbildungsförderungsgesetz|Bund,esausbildungsförderungs,gesetz|Bunde,sausbildungsförderungsgesetz|Bunde,sau,sbildungsförderungsgesetz|Bunde,sau,sbildungsförderungs,gesetz|Bunde,sausbildungsförderungs,gesetz|Bundes,ausbildungsförderungsgesetz|Bundes,ausbildung,sförderungsgesetz|Bundes,ausbildung,sförderungs,gesetz|Bundes,ausbildungsförderungs,gesetz|Bundesausbildungsförderungs,gesetz",
            ),
            ("Autobahn", "Auto,bahn"),
            ("xyzabc", ""),
        ];
        for (word, expected) in cases {
            let splits = t.get_all_splits(word);
            let value = splits
                .iter()
                .map(|parts| parts.join(","))
                .collect::<Vec<_>>()
                .join("|");
            assert_eq!(&value, expected, "{word}");
        }
    }
}
