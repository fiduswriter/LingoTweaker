//! `CompoundAcceptor` (`org.languagetool.rules.nl.CompoundAcceptor`): accept
//! Dutch 2-part compounds that the speller does not know, extending the
//! speller rather than accepting all valid compounds.
//!
//! The Java class holds a static second `MorfologikDutchSpellerRule` for
//! `spellingOk`; here the acceptor gets an `Arc<DutchSpellingRule>` set once
//! after construction (the speller in turn holds the acceptor for its
//! `ignorePotentiallyMisspelledWord` override — the same mutual recursion
//! Java has through its statics).

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use regex::Regex;

use crate::nl::spelling::DutchSpellingRule;

/// `CompoundAcceptor.MAX_WORD_SIZE`.
const MAX_WORD_SIZE: usize = 35;

/// `CompoundAcceptor.acronymPattern` (`[A-Z]{2,4}-`).
fn acronym_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Z]{2,4}-$").unwrap())
}

/// `CompoundAcceptor.specialAcronymPattern` (`[A-Za-z]{2,4}-`).
fn special_acronym_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z]{2,4}-$").unwrap())
}

/// `CompoundAcceptor.normalCasePattern` (`[A-Za-z][a-zé]*`, `matches()`).
fn normal_case_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z][a-zé]*$").unwrap())
}

pub struct CompoundAcceptor {
    no_s: HashSet<String>,
    needs_s: HashSet<String>,
    geographical_directions: HashSet<String>,
    always_needs_s: HashSet<String>,
    always_needs_hyphen: HashSet<String>,
    part1_exceptions: HashSet<String>,
    part2_exceptions: HashSet<String>,
    acronym_exceptions: HashSet<String>,
    tagger: Arc<lt_tagger::DutchTagger>,
    /// `CompoundAcceptor.speller` (the static speller instance in Java); set
    /// by the pipeline after both exist.
    speller: OnceLock<Arc<DutchSpellingRule>>,
}

impl CompoundAcceptor {
    /// Load `nl/compound_acceptor/*.txt` with the `CachingWordListLoader`
    /// semantics (trim, cut at `#`).
    pub fn load(data_dir: &Path, tagger: Arc<lt_tagger::DutchTagger>) -> Self {
        let dir = data_dir.join("nl/compound_acceptor");
        let load =
            |name: &str| -> HashSet<String> { load_words(&dir.join(name)).into_iter().collect() };
        Self {
            no_s: load("no_s.txt"),
            needs_s: load("needs_s.txt"),
            geographical_directions: load("directions.txt"),
            always_needs_s: load("always_needs_s.txt"),
            always_needs_hyphen: load("always_needs_hyphen.txt"),
            part1_exceptions: load("part1_exceptions.txt"),
            part2_exceptions: load("part2_exceptions.txt"),
            acronym_exceptions: load("acronym_exceptions.txt"),
            tagger,
            speller: OnceLock::new(),
        }
    }

    /// The pipeline sets the spelling rule once it is constructed.
    pub fn set_speller(&self, speller: Arc<DutchSpellingRule>) {
        let _ = self.speller.set(speller);
    }

    /// `CompoundAcceptor.acceptCompound(String)`.
    pub fn accept_compound(&self, word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() > MAX_WORD_SIZE {
            return false; // prevent long runtime
        }
        if chars.len() < 7 {
            return false;
        }
        for i in 3..chars.len() - 3 {
            let part1: String = chars[..i].iter().collect();
            let part2: String = chars[i..].iter().collect();
            if part1 != part2 && self.accept_compound_parts(&part1, &part2) {
                return true;
            }
        }
        false
    }

    /// `CompoundAcceptor.getParts(String)`.
    pub fn get_parts(&self, word: &str) -> Vec<String> {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() > MAX_WORD_SIZE {
            return Vec::new();
        }
        if chars.len() < 7 {
            return Vec::new();
        }
        for i in 3..chars.len() - 3 {
            let part1: String = chars[..i].iter().collect();
            let part2: String = chars[i..].iter().collect();
            if part1 != part2 && self.accept_compound_parts(&part1, &part2) {
                return vec![part1, part2];
            }
        }
        Vec::new()
    }

    /// `CompoundAcceptor.acceptCompound(String, String)`.
    fn accept_compound_parts(&self, part1: &str, part2: &str) -> bool {
        let part1lc = part1.to_lowercase();
        if part1.ends_with('s')
            && !self.part1_exceptions.contains(strip_last_char(part1))
            && !self.always_needs_s.contains(part1)
            && !self.no_s.contains(part1)
            && !part1.contains('-')
        {
            for suffix in &self.always_needs_s {
                if part1lc.ends_with(suffix.as_str()) {
                    return self.is_noun(part2)
                        && self.is_existing_word(strip_last_char(&part1lc))
                        && self.spelling_ok(part2);
                }
            }
            return self.needs_s.contains(&part1lc)
                && self.is_noun(part2)
                && self.spelling_ok(strip_last_char(part1))
                && self.spelling_ok(part2);
        }
        if self.geographical_directions.contains(part1) {
            return self.is_geographical_compound(part2);
        }
        if part1.ends_with('-') {
            // abbreviations
            return (self.acronym_ok(part1) || self.always_needs_hyphen.contains(&part1lc))
                && self.spelling_ok(part2);
        }
        if let Some(stripped) = part2.strip_prefix('-') {
            // vowel collision
            return self.no_s.contains(&part1lc)
                && self.is_noun(stripped)
                && self.spelling_ok(part1)
                && self.spelling_ok(stripped)
                && self.has_colliding_vowels(part1, stripped);
        }
        (self.no_s.contains(&part1lc) || self.part1_exceptions.contains(&part1lc))
            && self.is_noun(part2)
            && self.spelling_ok(part1)
            && !self.has_colliding_vowels(part1, part2)
    }

    /// `CompoundAcceptor.isNoun`.
    fn is_noun(&self, word: &str) -> bool {
        if self.part2_exceptions.contains(word) {
            return false;
        }
        self.tagger
            .postags(word)
            .iter()
            .any(|(_, tag)| tag.starts_with("ZNW"))
    }

    /// `CompoundAcceptor.isExistingWord`.
    fn is_existing_word(&self, word: &str) -> bool {
        !self.tagger.postags(word).is_empty()
    }

    /// `CompoundAcceptor.isGeographicalCompound`.
    fn is_geographical_compound(&self, word: &str) -> bool {
        self.tagger
            .postags(word)
            .iter()
            .any(|(_, tag)| tag.starts_with("ENM:LOC"))
    }

    /// `CompoundAcceptor.hasCollidingVowels`.
    fn has_colliding_vowels(&self, part1: &str, part2: &str) -> bool {
        static COLLIDING: &[&str] = &[
            "aa", "ae", "ai", "au", "ee", "ée", "ei", "éi", "eu", "éu", "ie", "ii", "ij", "oe",
            "oi", "oo", "ou", "ui", "uu",
        ];
        let Some(c1) = part1.chars().next_back() else {
            return false;
        };
        let Some(c2) = part2.chars().next() else {
            return false;
        };
        let pair: String = [c1, c2].iter().collect::<String>().to_lowercase();
        COLLIDING.contains(&pair.as_str())
    }

    /// `CompoundAcceptor.acronymOk`.
    fn acronym_ok(&self, non_compound: &str) -> bool {
        if acronym_pattern().is_match(non_compound) {
            let uppercase = strip_last_char(non_compound).to_uppercase();
            // Java uppercases the exception entry as well (`cao` in the file
            // matches the acronym `CAO`).
            !self
                .acronym_exceptions
                .iter()
                .any(|e| e.to_uppercase() == uppercase)
        } else if special_acronym_pattern().is_match(non_compound) {
            self.acronym_exceptions
                .contains(strip_last_char(non_compound))
        } else {
            false
        }
    }

    /// `CompoundAcceptor.spellingOk`: the Java `speller.match(as)` result for
    /// a synthetic one-token sentence, expressed through the spelling rule
    /// (canBeIgnored → `ignoreWord`, the speller1/prohibit check, then the
    /// recursive `acceptCompound`).
    fn spelling_ok(&self, non_compound: &str) -> bool {
        if !normal_case_pattern().is_match(non_compound) {
            return false; // e.g. kinderenHet -> split as kinder,enHet
        }
        let Some(speller) = self.speller.get() else {
            return false;
        };
        let lower = non_compound.to_lowercase();
        if speller.ignore_word(&lower) {
            return true;
        }
        if !speller.is_misspelled(&lower) && !speller.is_prohibited(&lower) {
            return true;
        }
        // `MorfologikDutchSpellerRule.ignorePotentiallyMisspelledWord`
        self.accept_compound(&lower)
    }
}

/// `CompoundAcceptor.getParts` through the tagger's provider trait (the
/// tagger is in `lt-tagger` and cannot name this type).
impl lt_tagger::CompoundPartsProvider for CompoundAcceptor {
    fn get_parts(&self, word: &str) -> Vec<String> {
        CompoundAcceptor::get_parts(self, word)
    }
}

fn strip_last_char(word: &str) -> &str {
    let mut chars = word.char_indices();
    chars.next_back();
    chars.as_str()
}

/// `CachingWordListLoader.loadWords`: skip empty/`#`-starting lines, cut at
/// `#`, trim.
fn load_words(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cut = line.trim().split('#').next().unwrap_or("").trim();
        if !cut.is_empty() {
            result.push(cut.to_string());
        }
    }
    result
}
