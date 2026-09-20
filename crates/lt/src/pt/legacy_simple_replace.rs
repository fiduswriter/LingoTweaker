//! Portuguese `AbstractSimpleReplaceRule` (legacy, single-token) instances:
//! `PortugueseReplaceRule`, `PortugueseOrthographyReplaceRule`,
//! `PortugueseAgreementReplaceRule` (pt-PT only) and
//! `EnglishContractionSpellingRule`.
//!
//! Unlike the `AbstractSimpleReplaceRule2` family this base class matches
//! one token at a time, falls back to dictionary lemmas (synthesizing the
//! replacement for the token's POS tag) and renders its message from the
//! replacement list without `<suggestion>` tags.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::pt::simple_replace::categories;
use crate::simple_replace::to_id;

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

/// The `getMessage(token, replacements)` variants of the four rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyMessage {
    /// `PortugueseReplaceRule.getMessage`.
    ForeignWord,
    /// `PortugueseOrthographyReplaceRule` (`messages.getString("spelling")`).
    Orthography(String),
    /// `PortugueseAgreementReplaceRule.getMessage`.
    Agreement,
    /// `EnglishContractionSpellingRule.getMessage` (first replacement only).
    EnglishContraction,
}

impl LegacyMessage {
    fn render(&self, token: &str, replacements: &[String]) -> String {
        match self {
            LegacyMessage::ForeignWord => format!(
                "'{token}' é um estrangeirismo. Em Português é mais comum usar: {}.",
                replacements.join(", ")
            ),
            LegacyMessage::Orthography(message) => message.clone(),
            LegacyMessage::Agreement => format!(
                "'{token}' é uma forma do antigo acordo ortográfico. No novo acordo ortográfico, a palavra escreve-se assim: {}.",
                replacements.join(", ")
            ),
            LegacyMessage::EnglishContraction => format!(
                "Caso seja uma contração da língua inglesa, prefira \"{}\".",
                replacements.first().cloned().unwrap_or_default()
            ),
        }
    }
}

pub struct LegacyReplaceConfig {
    pub rule_id: &'static str,
    /// may contain `$match`
    pub description: String,
    pub short: String,
    pub message: LegacyMessage,
    pub case_sensitive: bool,
    pub check_lemmas: bool,
    pub sub_rule_specific_ids: bool,
    pub category_id: &'static str,
    pub category_name: String,
    pub issue_type: &'static str,
}

/// `org.languagetool.rules.pt` legacy `AbstractSimpleReplaceRule` subclass.
pub struct LegacyReplaceRule {
    config: LegacyReplaceConfig,
    wrong_words: HashMap<String, Vec<String>>,
    synth: Arc<lt_tagger::PortugueseSynthesizer>,
}

impl LegacyReplaceRule {
    pub fn rule_id(&self) -> &str {
        self.config.rule_id
    }

    pub fn category_id(&self) -> &str {
        self.config.category_id
    }

    fn cleanup(&self, word: &str) -> String {
        if self.config.case_sensitive {
            word.to_string()
        } else {
            word.to_lowercase()
        }
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
            let token_string = self.cleanup(&original);
            let mut possible = self
                .wrong_words
                .get(&original)
                .or_else(|| self.wrong_words.get(&token_string))
                .cloned();
            if possible.is_none() && self.config.check_lemmas {
                let mut lemmas: Vec<String> = Vec::new();
                for reading in &tr.readings {
                    if let Some(lemma) = &reading.stem {
                        if self.wrong_words.contains_key(lemma) && !lemmas.contains(lemma) {
                            lemmas.push(self.cleanup(lemma));
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
                            let token = lt_core::AnalyzedToken::new(
                                at.stem.clone().unwrap_or_default(),
                                Some(replacement_lemma.clone()),
                                at.pos_tag.clone(),
                            );
                            let Some(tag) = at.pos_tag.as_deref() else {
                                continue;
                            };
                            for form in self.synth.synthesize(&token, tag, false) {
                                if !synthesized.contains(&form) {
                                    synthesized.push(form);
                                }
                            }
                        }
                    }
                }
                if !synthesized.is_empty() {
                    possible = Some(synthesized);
                }
            }
            let Some(possible) = possible else {
                continue;
            };
            if possible.is_empty() {
                continue;
            }
            let is_all_uppercase = lt_tagger::is_all_uppercase(&original);
            let mut replacements: Vec<String> = if is_all_uppercase {
                possible.iter().map(|s| s.to_uppercase()).collect()
            } else {
                possible
            };
            replacements.retain(|r| *r != original);
            if replacements.is_empty() {
                continue;
            }
            let message = self.message(&original, &replacements);
            if !self.config.case_sensitive && starts_with_uppercase(&original) {
                for replacement in &mut replacements {
                    *replacement = lt_tagger::uppercase_first_char(replacement);
                }
            }
            let description = self.config.description.replace("$match", &original);
            let rule_id = if self.config.sub_rule_specific_ids {
                to_id(&format!("{}_{}", self.config.rule_id, original))
            } else {
                self.config.rule_id.to_string()
            };
            rule_matches.push(
                Match::new(
                    &rule_id,
                    Option::<String>::None,
                    &message,
                    Some(self.config.short.clone()),
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
                    self.config.category_id,
                    &self.config.category_name,
                )
                .with_metadata(&description, self.config.issue_type, 0)
                .with_match_type("Other"),
            );
        }
        rule_matches
    }

    fn message(&self, token: &str, replacements: &[String]) -> String {
        self.config.message.render(token, replacements)
    }
}

fn starts_with_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
}

/// The four legacy rules of the requested variant, in Java's order
/// (`Portuguese.getRelevantRules` then the variant additions).
pub fn pt_legacy_replace_instances(
    data_dir: &Path,
    variant: &str,
    synth: Arc<lt_tagger::PortugueseSynthesizer>,
) -> Vec<LegacyReplaceRule> {
    let rules = data_dir.join("pt/rules");
    let c = categories(variant);
    let make = |config: LegacyReplaceConfig, path: &Path| LegacyReplaceRule {
        config,
        wrong_words: load_words(path),
        synth: Arc::clone(&synth),
    };
    let mut instances = Vec::new();

    // `PortugueseOrthographyReplaceRule`, all variants (18).
    instances.push(make(
        LegacyReplaceConfig {
            rule_id: "PT_SIMPLE_REPLACE_ORTHOGRAPHY",
            description: if variant == "pt-PT" || variant == "pt-BR" {
                "Possível erro ortográfico".to_string()
            } else {
                "Possible spelling mistake".to_string()
            },
            short: if variant == "pt-PT" || variant == "pt-BR" {
                "Erro ortográfico".to_string()
            } else {
                "Spelling mistake".to_string()
            },
            message: LegacyMessage::Orthography(if variant == "pt-PT" || variant == "pt-BR" {
                "Possível erro ortográfico".to_string()
            } else {
                "Possible spelling mistake found.".to_string()
            }),
            case_sensitive: false,
            check_lemmas: true,
            sub_rule_specific_ids: true,
            category_id: "TYPOS",
            category_name: c.typos.to_string(),
            issue_type: "misspelling",
        },
        &rules.join("replace_orthography.txt"),
    ));
    // `PortugueseReplaceRule` (`/pt/replace.txt`), all variants (19).
    instances.push(make(
        LegacyReplaceConfig {
            rule_id: "PT_SIMPLE_REPLACE",
            description: "Palavras estrangeiras facilmente confundidas em Português".to_string(),
            short: "Estrangeirismo".to_string(),
            message: LegacyMessage::ForeignWord,
            case_sensitive: false,
            check_lemmas: true,
            sub_rule_specific_ids: true,
            category_id: "STYLE",
            category_name: c.style.to_string(),
            issue_type: "locale-violation",
        },
        &rules.join("replace.txt"),
    ));
    // `PortugueseAgreementReplaceRule` (`pt-PT` variant only).
    if variant == "pt-PT" {
        instances.push(make(
            LegacyReplaceConfig {
                rule_id: "PT_AGREEMENT_REPLACE",
                description: "Palavras alteradas pelo Acordo Ortográfico de 90".to_string(),
                short: "Forma do Acordo Ortográfico de 45.".to_string(),
                message: LegacyMessage::Agreement,
                case_sensitive: false,
                check_lemmas: true,
                sub_rule_specific_ids: true,
                category_id: "TYPOS",
                category_name: c.typos.to_string(),
                issue_type: "misspelling",
            },
            &rules.join("AOreplace.txt"),
        ));
    }
    instances.push(make(
        LegacyReplaceConfig {
            rule_id: "PT_ENGLISH_CONTRACTION_ORTHOGRAPHY",
            description: "Ortografia de contrações inglesas".to_string(),
            short: "Erro de ortografia inglesa".to_string(),
            message: LegacyMessage::EnglishContraction,
            case_sensitive: true,
            check_lemmas: false,
            sub_rule_specific_ids: false,
            category_id: "TYPOS",
            category_name: c.typos.to_string(),
            issue_type: "misspelling",
        },
        &rules.join("english_contractions.txt"),
    ));
    instances
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn parses_wrong_word_files() {
        // cargo runs unit tests from the crate dir; resolve the vendored
        // data root like the other data-dependent tests.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let path = root.join("pt/rules/english_contractions.txt");
        if !path.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let map = load_words(&path);
        assert_eq!(map.get("aint"), Some(&vec!["ain't".to_string()]));
    }
}
