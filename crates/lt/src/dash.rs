//! Port of `AbstractDashRule` and its language subclasses: compounds from a
//! compounds file written with an en/em dash instead of a hyphen
//! (`EN_DASH_RULE`, `PT_POSAO_DASH_RULE`, `PT_PREAO_DASH_RULE`).

use std::path::Path;

use lt_core::{AnalyzedSentence, Match, Result, Suggestion, TextRange};

fn dash_spaces_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(" ?[–—] ?").unwrap());
    &RE
}

/// Per-language parts of `AbstractDashRule`.
pub struct DashConfig {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub message: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    /// `setTags(singletonList(Tag.picky))` (all current subclasses inherit it)
    pub picky: bool,
    /// `isBoundary(s)`: `true` when the single character can bound a match.
    pub boundary: fn(char) -> bool,
}

pub struct DashRule {
    /// one automaton over all compound variants (all four dash spellings per
    /// compound): Java uses Aho-Corasick; the naive per-compound `str::find`
    /// scan was 60%+ of the Portuguese check time (every compound rescans
    /// the sentence)
    automaton: aho_corasick::AhoCorasick,
    config: DashConfig,
}

impl DashRule {
    /// `AbstractDashRule.loadCompoundFile` over `data_file` (a path relative
    /// to the data directory).
    pub fn from_data(data_dir: &Path, data_file: &str, config: DashConfig) -> Result<Self> {
        let path = data_dir.join(data_file);
        let text = lt_data::fs::read_to_string(&path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display()))
        })?;
        let mut compounds = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.ends_with('+') || line.ends_with('?') {
                continue; // non-hyphenated suggestions
            }
            let word = if line.ends_with('*') || line.ends_with('$') {
                &line[..line.len() - 1]
            } else {
                line
            };
            for variant in [
                word.replace('-', "–"),
                word.replace('-', "—"),
                word.replace('-', " – "),
                word.replace('-', " — "),
            ] {
                if seen.insert(variant.clone()) {
                    compounds.push(variant);
                }
            }
        }
        compounds.sort();
        compounds.dedup();
        // Java `AbstractDashRule` builds an AhoCorasick over the compounds
        // and finds all overlapping occurrences in one pass over the text.
        let automaton = aho_corasick::AhoCorasick::new(&compounds)
            .map_err(|e| lt_core::CoreError::Data(format!("dash automaton: {e}")))?;
        Ok(Self { automaton, config })
    }

    pub fn rule_id(&self) -> &str {
        self.config.rule_id
    }

    /// `AbstractDashRule.match` over one sentence.
    pub fn check_sentence(&self, sentence: &AnalyzedSentence) -> Vec<Match> {
        let text = &sentence.text;
        let mut hits: Vec<(usize, usize)> = Vec::new();
        // overlapping (byte-level) matches over all compounds at once; a
        // UTF-8 continuation byte can never start a match, so byte offsets
        // are char-aligned like the old slide-by-one-char scan
        for m in self.automaton.find_overlapping_iter(text.as_bytes()) {
            hits.push((m.start(), m.end()));
        }
        // Java reverses the Aho-Corasick hits (ordered by end position), so
        // later matches are processed first and win the overlap check.
        hits.sort_by_key(|(begin, end)| (*end, *begin));
        hits.reverse();
        let is_boundary = |c: char| (self.config.boundary)(c);
        let mut start_positions = std::collections::HashSet::new();
        let mut matches = Vec::new();
        for (begin, end) in hits {
            if start_positions.contains(&begin) {
                continue;
            }
            if begin > 0 && !text[..begin].chars().next_back().is_some_and(is_boundary) {
                continue;
            }
            if end < text.len() && !text[end..].chars().next().is_some_and(is_boundary) {
                continue;
            }
            let covered = &text[begin..end];
            let replacement = dash_spaces_re().replace_all(covered, "-").to_string();
            let mut m = Match::new(
                self.config.rule_id,
                Option::<String>::None,
                self.config.message,
                Option::<String>::None,
                TextRange::new(sentence.offset + begin, sentence.offset + end),
                vec![Suggestion {
                    value: replacement,
                    short_description: None,
                }],
                self.config.category_id,
                self.config.category_name,
            )
            .with_metadata(self.config.description, "typographical", 0);
            if self.config.picky {
                m = m.with_picky(true);
            }
            matches.push(m);
            start_positions.insert(begin);
        }
        matches
    }
}

/// `EnglishDashRule`: `en/words/compounds.txt`, ASCII-letter boundaries.
pub fn english(data_dir: &Path) -> Result<DashRule> {
    DashRule::from_data(
        data_dir,
        "en/words/compounds.txt",
        DashConfig {
            rule_id: "EN_DASH_RULE",
            description: "Checks if hyphenated words were spelled with dashes (e.g., 'T — shirt' instead 'T-shirt').",
            message: "A dash was used instead of a hyphen.",
            category_id: "TYPOGRAPHY",
            category_name: "Typography",
            picky: true,
            boundary: |c| !c.is_ascii_alphabetic(),
        },
    )
}

/// `pl.DashRule`: `pl/words/compounds.txt`, base `DASH_RULE` id, Polish
/// messages, TYPOGRAPHY, picky, ASCII-letter boundaries.
pub fn polish(data_dir: &Path) -> Result<DashRule> {
    DashRule::from_data(
        data_dir,
        "pl/words/compounds.txt",
        DashConfig {
            rule_id: "DASH_RULE",
            description: "Sprawdza, czy wyrazy pisane z łącznikiem zapisano z myślnikami (np. „Lądek — Zdrój” zamiast „Lądek-Zdrój”).",
            message: "Błędne użycie myślnika zamiast łącznika.",
            category_id: "TYPOGRAPHY",
            category_name: "Błędy typograficzne",
            picky: true,
            boundary: |c| !c.is_ascii_alphabetic(),
        },
    )
}

/// `PostReformPortugueseDashRule` / `PreReformPortugueseDashRule`.
pub fn portuguese(data_dir: &Path, pre_reform: bool) -> Result<DashRule> {
    // chars from http://unicode.e-workers.de/portugiesisch.php
    fn pt_boundary(c: char) -> bool {
        !matches!(
            c,
            'a'..='z'
                | 'A'..='Z'
                | 'Â' | 'â' | 'Ã' | 'ã' | 'Ç' | 'ç' | 'Ê' | 'ê'
                | 'Ó' | 'ó' | 'Ô' | 'ô' | 'Õ' | 'õ' | 'Ü'
        )
    }
    DashRule::from_data(
        data_dir,
        if pre_reform {
            "pt/words/pre-reform-compounds.txt"
        } else {
            "pt/words/post-reform-compounds.txt"
        },
        DashConfig {
            rule_id: if pre_reform {
                "PT_PREAO_DASH_RULE"
            } else {
                "PT_POSAO_DASH_RULE"
            },
            description: "Travessões no lugar de hífens",
            message: "Um travessão foi utilizado em vez de um hífen.",
            category_id: "TYPOGRAPHY",
            category_name: "Typography",
            picky: true,
            boundary: pt_boundary,
        },
    )
}
