//! Serbian legacy `AbstractSimpleReplaceRule` instances that are not XML
//! rules: `SimpleGrammarEkavianReplaceRule`
//! (`SR_EKAVIAN_SIMPLE_GRAMMAR_REPLACE_RULE`, `sr/rules/ekavian/replace-grammar.txt`)
//! and `SimpleStyleEkavianReplaceRule`
//! (`SR_EKAVIAN_SIMPLE_STYLE_REPLACE_RULE`, `sr/rules/ekavian/replace-style.txt`).
//!
//! The base class is the classic (single-token) `AbstractSimpleReplaceRule`
//! with `isCaseSensitive() = true` (not overridden), no `getSynthesizer()`
//! override (so lemma hits add the replacement strings directly),
//! `checkLemmas = true`, `subRuleSpecificIds = false` and the default
//! `Uncategorized` issue type. Modelled on `gl::rules::LegacyReplaceRule`.

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, CoreError, Match, Result, Suggestion, TextRange};

/// `SimpleReplaceDataLoader.loadWords`: `word=suggestion1|suggestion2` lines,
/// `#` comments, `|` separates forms on both sides.
fn load_words(path: &Path) -> Result<HashMap<String, Vec<String>>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut map = HashMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() != 2 {
            return Err(CoreError::Data(format!(
                "Could not load simple replacement data from {}: error in line '{line}', \
                 expected format 'word=replacement'",
                path.display()
            )));
        }
        if parts[1].trim().is_empty() {
            return Err(CoreError::Data(format!(
                "Could not load simple replacement data from {}: error in line '{line}', \
                 replacement cannot be empty",
                path.display()
            )));
        }
        let replacements: Vec<String> = parts[1].split('|').map(str::to_string).collect();
        for wrong_form in parts[0].split('|') {
            map.insert(wrong_form.to_string(), replacements.clone());
        }
    }
    Ok(map)
}

/// One classic `AbstractSimpleReplaceRule` instance.
pub struct LegacyReplaceRule {
    rule_id: &'static str,
    description: &'static str,
    short: &'static str,
    /// Rendered from `(token, replacements)`.
    message: fn(&str, &[String]) -> String,
    category_id: &'static str,
    category_name: &'static str,
    wrong_words: HashMap<String, Vec<String>>,
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
            let is_all_uppercase = lt_tagger::is_all_uppercase(&original);

            // `isCaseSensitive() = true`, so `cleanup` is the identity: try the
            // original token only.
            let mut possible = self.wrong_words.get(&original).cloned();
            if possible.is_none() {
                // `checkLemmas` default true. `getSynthesizer()` is null, so
                // the replacements are added as-is.
                let mut lemmas: Vec<String> = Vec::new();
                for reading in &tr.readings {
                    if let Some(lemma) = &reading.stem {
                        if self.wrong_words.contains_key(lemma) && !lemmas.contains(lemma) {
                            lemmas.push(lemma.clone());
                        }
                    }
                }
                let mut direct: Vec<String> = Vec::new();
                for lemma in lemmas {
                    if let Some(replacements) = self.wrong_words.get(&lemma) {
                        for replacement in replacements {
                            if !direct.contains(replacement) {
                                direct.push(replacement.clone());
                            }
                        }
                    }
                }
                if !direct.is_empty() {
                    possible = Some(direct);
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
            let message = (self.message)(&original, &replacements);
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
                    self.category_name,
                )
                .with_metadata(self.description, "uncategorized", 0)
                .with_match_type("Other"),
            );
        }
        rule_matches
    }
}

/// `SimpleGrammarEkavianReplaceRule.getMessage`.
fn grammar_message(token: &str, replacements: &[String]) -> String {
    format!("Не каже се „{token}“ него „{}“.", replacements.join(", "))
}

/// `SimpleStyleEkavianReplaceRule.getMessage`.
fn style_message(token: &str, replacements: &[String]) -> String {
    format!(
        "Уместо израза „{token}“ било би боље да користите: {}.",
        replacements.join(", ")
    )
}

/// The two legacy rules in `Serbian.getRelevantRules` order (grammar, style).
pub fn legacy_replace_instances(data_dir: &Path) -> Result<Vec<LegacyReplaceRule>> {
    let rules = data_dir.join("sr/rules/ekavian");
    Ok(vec![
        LegacyReplaceRule {
            rule_id: "SR_EKAVIAN_SIMPLE_GRAMMAR_REPLACE_RULE",
            description: "Провера граматички погрешних речи или израза",
            short: "Граматички погрешна реч тј. израз",
            message: grammar_message,
            category_id: "MISC",
            category_name: "Разно",
            wrong_words: load_words(&rules.join("replace-grammar.txt"))?,
        },
        LegacyReplaceRule {
            rule_id: "SR_EKAVIAN_SIMPLE_STYLE_REPLACE_RULE",
            description: "Провера стилски лоших речи или израза",
            short: "Стилски лоша реч тј. израз",
            message: style_message,
            category_id: "MISC",
            category_name: "Разно",
            wrong_words: load_words(&rules.join("replace-style.txt"))?,
        },
    ])
}
