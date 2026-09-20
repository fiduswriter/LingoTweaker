//! Galician Java-coded built-in rules (`Galician.getRelevantRules`):
//! `SimpleReplaceRule` (15) and `CastWordsRule` (16) extend the legacy
//! `AbstractSimpleReplaceRule`; `GalicianRedundancyRule` (17),
//! `GalicianWordinessRule` (18), `GalicianBarbarismsRule` (19) and
//! `GalicianWikipediaRule` (20) extend `AbstractSimpleReplaceRule2`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};

/// `SimpleReplaceDataLoader.loadWords`: `wrong=right1|right2` lines.
fn load_words(path: &Path) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if right.trim().is_empty() || right.contains('=') {
            continue;
        }
        let replacements: Vec<String> = right.split('|').map(|s| s.to_string()).collect();
        map.insert(left.to_string(), replacements);
    }
    map
}

/// One `AbstractSimpleReplaceRule` (legacy) Galician instance.
pub struct LegacyReplaceRule {
    rule_id: &'static str,
    description: &'static str,
    short: &'static str,
    /// Rendered from `(token, replacements)`.
    message: fn(&str, &[String]) -> String,
    category_id: &'static str,
    category_name: String,
    wrong_words: HashMap<String, Vec<String>>,
    synth: Arc<lt_tagger::GalicianSynthesizer>,
}

impl LegacyReplaceRule {
    pub fn rule_id(&self) -> &str {
        self.rule_id
    }

    /// `AbstractSimpleReplaceRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut rule_matches = Vec::new();
        for tr in &non_blank {
            if tr.is_sentence_start || tr.is_immunized || tr.is_ignore_spelling {
                continue;
            }
            let original = tr.surface().to_string();
            let token_string = original.to_lowercase();
            let is_all_uppercase = lt_tagger::is_all_uppercase(&original);
            let mut possible = self
                .wrong_words
                .get(&original)
                .or_else(|| self.wrong_words.get(&token_string))
                .cloned();
            if possible.is_none() {
                // lemma synthesis (`checkLemmas` default true)
                let mut lemmas: Vec<String> = Vec::new();
                for reading in &tr.readings {
                    if let Some(lemma) = &reading.stem {
                        if self.wrong_words.contains_key(lemma) && !lemmas.contains(lemma) {
                            lemmas.push(lemma.to_lowercase());
                        }
                    }
                }
                let mut synthesized: Vec<String> = Vec::new();
                for lemma in lemmas {
                    let Some(replacements) = self.wrong_words.get(&lemma) else {
                        continue;
                    };
                    for replacement_lemma in replacements {
                        for at in &tr.readings {
                            let Some(pos_tag) = at.pos_tag.as_deref() else {
                                continue;
                            };
                            let token = AnalyzedToken::new(
                                at.stem.clone().unwrap_or_default(),
                                Some(replacement_lemma.clone()),
                                at.pos_tag.clone(),
                            );
                            for form in self.synth.synthesize_plain(&token, pos_tag) {
                                if !synthesized.contains(&form) {
                                    synthesized.push(form);
                                }
                            }
                        }
                    }
                }
                if !synthesized.is_empty() {
                    synthesized.dedup();
                    possible = Some(synthesized);
                }
            }
            let Some(possible) = possible else {
                continue;
            };
            if possible.is_empty() {
                continue;
            }
            let mut replacements: Vec<String> = if is_all_uppercase {
                possible.iter().map(|s| s.to_uppercase()).collect()
            } else {
                possible
            };
            replacements.retain(|r| *r != original);
            if replacements.is_empty() {
                continue;
            }
            let message = (self.message)(&token_string, &replacements);
            if original.chars().next().is_some_and(char::is_uppercase) {
                for replacement in &mut replacements {
                    *replacement = lt_tagger::uppercase_first_char(replacement);
                }
            }
            rule_matches.push(
                Match::new(
                    self.rule_id,
                    Option::<String>::None,
                    message,
                    Some(self.short.to_string()),
                    TextRange::new(
                        sentence_offset + tr.start_pos,
                        sentence_offset + tr.start_pos + original.len(),
                    ),
                    replacements
                        .into_iter()
                        .map(|value| Suggestion {
                            value,
                            short_description: None,
                        })
                        .collect(),
                    self.category_id,
                    &self.category_name,
                )
                .with_metadata(self.description, "misspelling", 0)
                .with_match_type("Other"),
            );
        }
        rule_matches
    }
}

/// `SimpleReplaceRule.getMessage` (`GL_SIMPLE_REPLACE`).
fn simple_replace_message(token: &str, replacements: &[String]) -> String {
    format!(
        "'{token}' non existe en galego. Talvez quería vostede dicir: {}.",
        replacements.join(", ")
    )
}

/// `CastWordsRule.getMessage` (`GL_CAST_WORDS`).
fn cast_words_message(token: &str, replacements: &[String]) -> String {
    format!(
        "'{token}' é un castelanismo. Empregue no seu sitio: {}.",
        replacements.join(", ")
    )
}

/// The two legacy rules in Java's order (15–16).
pub fn legacy_replace_instances(
    data_dir: &Path,
    synth: Arc<lt_tagger::GalicianSynthesizer>,
) -> Vec<LegacyReplaceRule> {
    let rules = data_dir.join("gl/rules");
    vec![
        LegacyReplaceRule {
            rule_id: "GL_SIMPLE_REPLACE",
            description: "Corrección de erros léxicos (barbarismos).",
            short: "Erros léxicos",
            message: simple_replace_message,
            category_id: "MISC",
            category_name: "Miscelánea".to_string(),
            wrong_words: load_words(&rules.join("words.txt")),
            synth: Arc::clone(&synth),
        },
        LegacyReplaceRule {
            rule_id: "GL_CAST_WORDS",
            description: "Corrección de erros léxicos (castelanismos).",
            short: "Castelanismos léxicos",
            message: cast_words_message,
            category_id: "MISC",
            category_name: "Miscelánea".to_string(),
            wrong_words: load_words(&rules.join("spanish.txt")),
            synth,
        },
    ]
}

/// The four `AbstractSimpleReplaceRule2` instances in Java's order (17–20).
#[allow(clippy::vec_init_then_push)]
pub fn rule2_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules = data_dir.join("gl/rules");
    let mut instances = Vec::new();
    // `GalicianRedundancyRule` (17)
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("redundancies.txt")],
        SimpleReplaceConfig {
            rule_id: "GL_REDUNDANCY_REPLACE",
            description: "1. Pleonasmos e redundancias",
            short: "Pleonasmo",
            message: "'$match' é un pleonasmo. É preferible dicir $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "Redundancia",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `GalicianWordinessRule` (18)
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("wordiness.txt")],
        SimpleReplaceConfig {
            rule_id: "GL_WORDINESS_REPLACE",
            description: "2. Expresións prolixas",
            short: "Expresión prolixa",
            message: "'$match' é unha expresión innecesariamente complexa. É preferíbel dicir $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "Redundancia",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `GalicianBarbarismsRule` (19)
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("barbarisms.txt")],
        SimpleReplaceConfig {
            rule_id: "GL_BARBARISM_REPLACE",
            description: "Palabras de orixe estranxeira evitábeis",
            short: "Xenismo",
            message: "'$match' é un xenismo. É preferíbel dicir $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "Estilo",
            issue_type: "locale-violation",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `GalicianWikipediaRule` (20)
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("wikipedia.txt")],
        SimpleReplaceConfig {
            rule_id: "GL_WIKIPEDIA_COMMON_ERRORS",
            description: "Erros frecuentes nos artigos da Wikipedia",
            short: "Erro gramatical ou de normativa",
            message: "'$match' é un erro. Considere utilizar $suggestions",
            suggestions_separator: " ou ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "WIKIPEDIA",
            category_name: "Regras específicas da Wikipedia",
            issue_type: "grammar",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    Ok(instances)
}
