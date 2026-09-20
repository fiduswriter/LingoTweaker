//! Dutch Java-coded rules (stage 3): `SimpleReplaceRule`
//! (`NL_SIMPLE_REPLACE`), `CheckCaseRule` (`NL_CHECKCASE`),
//! `PreferredWordRule` (`NL_PREFERRED_WORD_RULE*`) and
//! `SpaceInCompoundRule` (`NL_SPACE_IN_COMPOUND*`).

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::nl::tools::glue_parts;
use crate::simple_replace::{
    to_id, CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};

/// `SimpleReplaceRule`: `nl/rules/replace.txt`, category VERGISSINGEN,
/// case-sensitive, sub-rule specific ids.
pub fn simple_replace_instance(data_dir: &Path) -> Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("nl/rules/replace.txt")],
        SimpleReplaceConfig {
            rule_id: "NL_SIMPLE_REPLACE",
            description: "Snelle correctie van veel voorkomende vergissingen ($match)",
            short: "Vergissing?",
            message: "'$match' zou fout kunnen zijn. Misschien bedoelt u: $suggestions",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Cs,
            category_id: "VERGISSINGEN",
            category_name: "Vergissingen",
            issue_type: "misspelling",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

/// `CheckCaseRule`: `nl/rules/check_case.txt`, `AbstractCheckCaseRule`
/// (CASING, typographical, CI matching, `isCheckingCase` semantics).
pub fn check_case_instance(data_dir: &Path) -> Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("nl/rules/check_case.txt")],
        SimpleReplaceConfig {
            rule_id: "NL_CHECKCASE",
            description: "Controle op hoofd- en kleine letters: $match",
            short: "Schrijfwijze",
            message: "Juiste schrijfwijze",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "CASING",
            category_name: "Hoofdlettergebruik",
            issue_type: "typographical",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: true,
            ignore_short_uppercase_words: false,
            is_token_exception: TokenException::None,
        },
    )
}

// ---------------------------------------------------------------------------
// PreferredWordRule
// ---------------------------------------------------------------------------

/// One `old;new` CSV entry of `nl/words/preferredwords.csv`.
struct PreferredEntry {
    old_word: String,
    new_word: String,
    parts: Vec<String>,
}

pub struct PreferredWordRule {
    entries: Vec<PreferredEntry>,
}

impl PreferredWordRule {
    pub fn load(data_dir: &Path) -> Self {
        let text = lt_data::fs::read_to_string(data_dir.join("nl/words/preferredwords.csv"))
            .unwrap_or_default();
        let entries = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| {
                let (old_word, new_word) = line.split_once(';')?;
                if old_word.is_empty() || new_word.is_empty() {
                    return None;
                }
                Some(PreferredEntry {
                    old_word: old_word.to_string(),
                    new_word: new_word.to_string(),
                    parts: old_word.split(' ').map(str::to_string).collect(),
                })
            })
            .collect();
        Self { entries }
    }

    /// `PreferredWordRule.match`: the inner `PatternRule` matches (id
    /// `NL_PREFERRED_WORD_RULE_INTERNAL`, case-sensitive literals) with the
    /// first match's suggestion rewritten to the more common word.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut out = Vec::new();
        for entry in &self.entries {
            let mut matches: Vec<Match> = Vec::new();
            'start: for start in 0..non_blank.len() {
                if start + entry.parts.len() > non_blank.len() {
                    break;
                }
                for (i, part) in entry.parts.iter().enumerate() {
                    if non_blank[start + i].surface() != part {
                        continue 'start;
                    }
                }
                let last = start + entry.parts.len() - 1;
                let from = sentence_offset + non_blank[start].start_pos;
                let to = sentence_offset + non_blank[last].end_pos();
                let matched_text: String = non_blank[start..=last]
                    .iter()
                    .map(|t| t.surface())
                    .collect();
                matches.push(
                    Match::new(
                        "NL_PREFERRED_WORD_RULE_INTERNAL",
                        Option::<String>::None,
                        "Voor dit woord is een gebruikelijker alternatief.",
                        Some("Gebruikelijker woord".to_string()),
                        TextRange::new(from, to),
                        Vec::new(),
                        "STYLE",
                        "Stijl",
                    )
                    .with_metadata(
                        "Suggereert een gebruikelijker woord.",
                        "style",
                        1,
                    ),
                );
                if let Some(first) = matches.first_mut() {
                    let suggestion = matched_text.replace(&entry.old_word, &entry.new_word);
                    if suggestion != matched_text {
                        first.suggestions = vec![Suggestion {
                            value: suggestion,
                            short_description: None,
                        }];
                    }
                }
            }
            if matches
                .first()
                .is_some_and(|first| !first.suggestions.is_empty())
            {
                out.extend(matches);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// SpaceInCompoundRule
// ---------------------------------------------------------------------------

/// `SpaceInCompoundRule`: Aho-Corasick-style search over the variants of
/// `nl/words/multipartcompounds.txt`; every hit becomes a specific-id match.
pub struct SpaceInCompoundRule {
    /// variant text -> message key (the glued form)
    variants: HashMap<String, String>,
    /// glued form -> message
    normalized_compound2message: HashMap<String, String>,
}

impl SpaceInCompoundRule {
    pub fn load(data_dir: &Path) -> Self {
        let text = lt_data::fs::read_to_string(data_dir.join("nl/words/multipartcompounds.txt"))
            .unwrap_or_default();
        let mut variants: HashMap<String, String> = HashMap::new();
        let mut messages: HashMap<String, String> = HashMap::new();
        for line in text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut line_parts = line.split('|');
            let Some(word_parts) = line_parts.next() else {
                continue;
            };
            if !word_parts.contains(' ') {
                continue; // Java throws; the vendored data has no such line
            }
            let words: Vec<&str> = word_parts.split(' ').collect();
            let mut generated = std::collections::HashSet::new();
            generate_variants("", &words, &mut generated);
            let mut message = format!("Waarschijnlijk bedoelt u: {}", glue_parts(&words));
            if let Some(description) = line_parts.next() {
                message.push_str(&format!(" ({description})"));
            }
            let glued = glue_parts(&words);
            for variant in generated {
                variants.insert(variant, glued.clone());
            }
            messages.insert(glued, message);
        }
        Self {
            variants,
            normalized_compound2message: messages,
        }
    }

    /// `SpaceInCompoundRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let text: String = tokens.iter().map(|t| t.surface()).collect();
        let bytes = text.as_bytes();
        let mut hits: Vec<(usize, usize)> = Vec::new();
        for variant in self.variants.keys() {
            for (begin, _) in text.match_indices(variant.as_str()) {
                let end = begin + variant.len();
                if begin > 0 && !is_boundary(&text[begin - 1..begin]) {
                    continue;
                }
                if end < text.len() && !is_boundary(&text[end..end + 1]) {
                    continue;
                }
                hits.push((begin, end));
            }
        }
        let _ = bytes;
        // `AhoCorasickDoubleArrayTrie.parseText` emits hits by increasing end
        // position; keep that order.
        hits.sort_by_key(|&(begin, end)| (end, begin));
        hits.dedup();
        let mut out = Vec::new();
        for (begin, end) in hits {
            let covered = &text[begin..end];
            let covered_no_spaces = glue_parts(&covered.split(' ').collect::<Vec<&str>>());
            let Some(message) = self.normalized_compound2message.get(&covered_no_spaces) else {
                continue;
            };
            let specific_id = to_id(&format!("NL_SPACE_IN_COMPOUND_{covered}"));
            out.push(
                Match::new(
                    specific_id,
                    Option::<String>::None,
                    message.clone(),
                    None::<String>,
                    TextRange::new(sentence_offset + begin, sentence_offset + end),
                    vec![Suggestion {
                        value: covered_no_spaces,
                        short_description: None,
                    }],
                    "MISC",
                    "Diversen",
                )
                .with_metadata("Detecteert spatiefouten", "uncategorized", 0),
            );
        }
        out
    }
}

/// `SpaceInCompoundRule.isBoundary` (`[a-zA-Z]`, `matches()`).
fn is_boundary(s: &str) -> bool {
    !s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// `SpaceInCompoundRule.generateVariants`.
fn generate_variants(so_far: &str, words: &[&str], result: &mut std::collections::HashSet<String>) {
    if words.is_empty() {
        return;
    }
    if words.len() == 1 {
        if so_far.contains(' ') {
            result.insert(format!("{so_far}{}", words[0]));
        }
        result.insert(format!("{so_far} {}", words[0]));
    } else {
        let rest = &words[1..];
        generate_variants(&format!("{so_far}{}", words[0]), rest, result);
        if !so_far.is_empty() {
            generate_variants(&format!("{so_far} {}", words[0]), rest, result);
        }
    }
}
