//! Speller, confusion sets, and suggestion generation.
//!
//! The speller ([`SpellChecker`], in the [`speller`] submodule) reads the
//! Morfologik CFSA2 hunspell-derived speller dictionaries directly via
//! `lt_tagger::Cfsa2`; see the submodule docs for the entry format and the
//! documented deviations from upstream morfologik-speller.

pub mod hunspell;
pub mod morfologik;
pub mod speller;

pub use morfologik::{
    CandidateData, DictSource, MorfologikSpeller, MultiSpeller, Speller, SpellerMetadata,
    WeightedSuggestion,
};
pub use speller::SpellChecker;

use std::collections::HashMap;
use std::path::Path;

use lt_core::Result;

/// One confusion pair/group as used by LT confusion rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfusionSet {
    pub terms: Vec<String>,
    pub line: usize,
}

/// Parse a legacy `confusion_sets.txt` file (one `a; b; c` group per
/// line; `#` comments and blank lines ignored).
pub fn parse_confusion_sets(text: &str) -> Vec<ConfusionSet> {
    let mut sets = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let terms: Vec<String> = line
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if terms.len() >= 2 {
            sets.push(ConfusionSet { terms, line: i + 1 });
        }
    }
    sets
}

pub fn load_confusion_sets(path: &Path) -> Result<Vec<ConfusionSet>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    Ok(parse_confusion_sets(&text))
}

/// Groups confusion sets for fast lookup of all sets containing a word.
#[derive(Debug, Clone, Default)]
pub struct ConfusionSetIndex {
    sets: Vec<ConfusionSet>,
    by_term: HashMap<String, Vec<usize>>,
}

impl ConfusionSetIndex {
    /// Build an index over the given sets; terms are matched lowercased.
    pub fn build(sets: &[ConfusionSet]) -> Self {
        let sets = sets.to_vec();
        let mut by_term: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, set) in sets.iter().enumerate() {
            for term in &set.terms {
                let entry = by_term.entry(term.to_lowercase()).or_default();
                if !entry.contains(&i) {
                    entry.push(i);
                }
            }
        }
        Self { sets, by_term }
    }

    /// All confusion-set groups containing the (lowercased) word, in file order.
    pub fn lookup(&self, word: &str) -> Vec<&ConfusionSet> {
        self.by_term
            .get(&word.to_lowercase())
            .map(|indices| indices.iter().map(|&i| &self.sets[i]).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn parses_confusion_sets() {
        let sets = parse_confusion_sets("# comment\nits; it's\ntheir; there; they're\n\n");
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].terms, vec!["its", "it's"]);
        assert_eq!(sets[1].terms, vec!["their", "there", "they're"]);
    }

    #[test]
    fn confusion_index_finds_groups() {
        let sets = parse_confusion_sets("its; it's\ntheir; there; they're\naffect; effect\n");
        let index = ConfusionSetIndex::build(&sets);
        let groups = index.lookup("ITS");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].terms, vec!["its", "it's"]);
        assert_eq!(index.lookup("it's").len(), 1);
        assert_eq!(index.lookup("there").len(), 1);
        assert_eq!(index.lookup("effect").len(), 1);
        assert!(index.lookup("qqzzww").is_empty());
    }

    #[test]
    fn confusion_index_handles_duplicate_terms() {
        let sets = parse_confusion_sets("its; it's\nits; it is\n");
        let index = ConfusionSetIndex::build(&sets);
        let groups = index.lookup("its");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].line, 1);
        assert_eq!(groups[1].line, 2);
    }

    fn en_us() -> Option<SpellChecker> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/hunspell");
        if !dir.lt_exists() {
            eprintln!("skipping: no vendored data");
            return None;
        }
        Some(SpellChecker::new(&dir.join("en_US.dict"), &dir.join("en_US.info")).unwrap())
    }

    #[test]
    fn loads_en_us_dictionary() {
        let Some(speller) = en_us() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert!(
            speller.word_count() > 10_000,
            "words: {}",
            speller.word_count()
        );
    }

    #[test]
    fn common_word_correctness() {
        let Some(speller) = en_us() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert!(speller.is_correct("the"));
        assert!(!speller.is_correct("teh"));
        assert!(!speller.is_correct("qqzzww"));
    }

    #[test]
    fn capitalization_semantics() {
        let Some(speller) = en_us() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert!(speller.is_correct("The"));
        assert!(speller.is_correct("THE"));
        assert!(!speller.is_correct("TEH"));
    }

    #[test]
    fn suggests_for_teh() {
        let Some(speller) = en_us() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let suggestions = speller.suggest("teh", 5);
        eprintln!("suggest(teh, 5) = {suggestions:?}");
        assert!(!suggestions.is_empty());
        assert!(suggestions.len() <= 5);
        for s in &suggestions {
            assert!(
                speller.is_correct(s),
                "suggestion {s:?} is not a correct dictionary word"
            );
        }
        assert!(
            suggestions.iter().take(5).any(|s| s == "the"),
            "expected 'the' in top 5: {suggestions:?}"
        );
    }

    #[test]
    fn loads_all_vendored_variants() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/hunspell");
        if !dir.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        for variant in ["en_US", "en_GB", "en_AU", "en_CA", "en_NZ"] {
            let speller = SpellChecker::new(
                &dir.join(format!("{variant}.dict")),
                &dir.join(format!("{variant}.info")),
            )
            .unwrap_or_else(|e| panic!("{variant}: {e}"));
            assert!(
                speller.word_count() > 10_000,
                "{variant}: {}",
                speller.word_count()
            );
            assert!(speller.is_correct("the"), "{variant}");
            // en_GB stores `word_` + freq under the '_' separator; a '_' word
            // part must never leak into lookups.
            assert!(!speller.is_correct("the_the"), "{variant}");
        }
    }

    #[test]
    fn suggestions_have_distance_order() {
        let Some(speller) = en_us() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let suggestions = speller.suggest("teh", 50);
        let distances: Vec<usize> = suggestions
            .iter()
            .map(|s| super::speller::osa_distance("teh", s))
            .collect();
        assert!(distances.windows(2).all(|w| w[0] <= w[1]), "{distances:?}");
        assert!(!suggestions.iter().any(|s| s == "teh"));
    }
}
