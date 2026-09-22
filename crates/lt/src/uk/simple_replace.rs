//! Port of `org.languagetool.rules.uk.SimpleReplaceRule` (`UK_SIMPLE_REPLACE`,
//! barbarisms from `uk/rules/replace.txt`) with its custom `isTagged` /
//! `findMatches` (active participle, derivats and the speller `:bad`
//! fallback).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "UK_SIMPLE_REPLACE";
const DESCRIPTION: &str = "Пошук помилкових слов";
const SHORT: &str = "Помилка?";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";

static ACTV_BAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?adjp:actv.*?:bad.*$").unwrap());

pub struct SimpleReplaceRule {
    wrong_words: HashMap<String, Vec<String>>,
}

impl SimpleReplaceRule {
    pub fn load(rules_dir: &Path) -> Self {
        Self {
            wrong_words: load_lists(&rules_dir.join("replace.txt")),
        }
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    fn is_good_pos_tag(pos_tag: &str) -> bool {
        pos_tag != "PARA_END"
            && pos_tag != "SENT_END"
            && !pos_tag.contains(":bad")
            && !pos_tag.contains("subst")
            && !pos_tag.starts_with('<')
    }

    fn is_tagged(tr: &AnalyzedTokenReadings) -> bool {
        for at in &tr.readings {
            let Some(pos_tag) = at.pos_tag.as_deref() else {
                return false;
            };
            if Self::is_good_pos_tag(pos_tag) {
                return true;
            }
        }
        false
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        derivs: &HashMap<String, HashSet<String>>,
        speller_suggestions: impl Fn(&str) -> Vec<String>,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut out = Vec::new();
        for tr in tokens.iter().filter(|t| !t.is_whitespace) {
            let sent_start =
                tr.readings.first().and_then(|r| r.pos_tag.as_deref()) == Some("SENT_START");
            if sent_start || tr.is_immunized || tr.is_ignore_spelling || Self::is_tagged(tr) {
                continue;
            }
            out.extend(self.find_matches(tr, derivs, &speller_suggestions, sentence_offset));
        }
        out
    }

    fn find_matches(
        &self,
        tr: &AnalyzedTokenReadings,
        derivs: &HashMap<String, HashSet<String>>,
        speller_suggestions: &impl Fn(&str) -> Vec<String>,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let original = tr.surface().to_string();
        let mut possible = self.base_replacements(tr);
        if possible.as_ref().is_some_and(|p| p.is_empty()) {
            possible = None;
        }
        if let Some(replacements) = possible {
            return vec![self.create_match(
                tr,
                &original,
                &replacements,
                &self.message(&original, &replacements),
                sentence_offset,
            )];
        }
        let token_string = original.to_lowercase();

        // active participles
        if tr.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .is_some_and(|t| ACTV_BAD.is_match(t).unwrap_or(false))
        }) {
            let mut msg = "Активні дієприкметники не властиві українській мові.".to_string();
            let lemma = tr
                .readings
                .first()
                .and_then(|r| r.stem.clone())
                .unwrap_or_default();
            if lemma.ends_with("ший") {
                msg.push_str(" Їх можна замінити на що + дієслово (випавший сніг - сніг, що випав), або на форму з суфіксом -л- (промокший - промоклий)");
            } else {
                msg.push_str(" Їх можна замінити питомими словами в різний спосіб: що + дієслово (роблячий  - що робить), дієслівний корінь+ суфікси -льн-, -лив- тощо (збираючий - збиральний, обтяжуючий - обтяжливий), заміна іменником (завідуючий - завідувач), заміна прикметником із відповідним значенням (діюча модель - робоча модель), зміна конструкції (з наступаючим Новим роком - з настанням Нового року) тощо.");
            }
            return vec![self.create_match(tr, &original, &[], &msg, sentence_offset)];
        }

        // derivats
        let derivat_suggestions = self.find_in_deriv(&token_string, derivs);
        if !derivat_suggestions.is_empty() {
            return vec![self.create_match(
                tr,
                &original,
                &derivat_suggestions,
                "Неправильне слово.",
                sentence_offset,
            )];
        }

        let has_bad = tr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(":bad")));
        let is_number = tr.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .is_some_and(|t| t.starts_with("number"))
        });
        if has_bad && !is_number {
            let mut suggestions = speller_suggestions(&original);
            suggestions.retain(|s| !s.contains(' '));
            return vec![self.create_match(
                tr,
                &original,
                &suggestions,
                "Неправильно написане слово.",
                sentence_offset,
            )];
        }

        Vec::new()
    }

    /// The `AbstractSimpleReplaceRule.findMatches` base (single token +
    /// lowercase + `checkLemmas`), returning case-adjusted replacements.
    fn base_replacements(&self, tr: &AnalyzedTokenReadings) -> Option<Vec<String>> {
        base_replacements_in(&self.wrong_words, tr)
    }

    fn find_in_deriv(&self, word: &str, derivs: &HashMap<String, HashSet<String>>) -> Vec<String> {
        let Some(verbs) = derivs.get(word) else {
            return Vec::new();
        };
        let ending: String = {
            let chars: Vec<char> = word.chars().collect();
            chars[chars.len().saturating_sub(3)..].iter().collect()
        };
        let mut suggestions = Vec::new();
        for d in verbs {
            let Some(replacements) = self.wrong_words.get(d) else {
                continue;
            };
            for t in replacements {
                // find a derivative whose value contains `t` and whose key ends
                // with the same 3 letters
                let mapped = derivs
                    .iter()
                    .find(|(key, value)| value.contains(t) && key.ends_with(&ending))
                    .map(|(key, _)| key.clone())
                    .unwrap_or_else(|| t.clone());
                suggestions.push(mapped);
            }
        }
        suggestions
    }

    fn message(&self, token: &str, replacements: &[String]) -> String {
        format!(
            "«{token}» - помилкове слово, виправлення: {}.",
            replacements.join(", ")
        )
    }

    fn create_match(
        &self,
        tr: &AnalyzedTokenReadings,
        original: &str,
        replacements: &[String],
        message: &str,
        sentence_offset: usize,
    ) -> Match {
        let mut replacements: Vec<String> = replacements.to_vec();
        if original.chars().next().is_some_and(|c| c.is_uppercase()) {
            for r in &mut replacements {
                *r = uppercase_first_char(r);
            }
        }
        Match::new(
            RULE_ID,
            Option::<String>::None,
            message,
            Some(SHORT.to_string()),
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
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "misspelling", 0)
    }
}

fn base_replacements_in(
    wrong_words: &HashMap<String, Vec<String>>,
    tr: &AnalyzedTokenReadings,
) -> Option<Vec<String>> {
    let original = tr.surface().to_string();
    let token_string = original.to_lowercase();
    let is_all_uppercase =
        original.chars().all(|c| !c.is_lowercase()) && original.chars().any(|c| c.is_uppercase());
    let mut possible = wrong_words
        .get(&original)
        .or_else(|| wrong_words.get(&token_string))
        .cloned();
    if possible.is_none() {
        let mut lemmas: Vec<String> = Vec::new();
        for r in &tr.readings {
            if let Some(lemma) = r.stem.as_deref() {
                let lemma = lemma.to_lowercase();
                if wrong_words.contains_key(&lemma) && !lemmas.contains(&lemma) {
                    lemmas.push(lemma);
                }
            }
        }
        let mut acc: Vec<String> = Vec::new();
        for lemma in &lemmas {
            if let Some(repl) = wrong_words.get(lemma) {
                for r in repl {
                    if !acc.contains(r) {
                        acc.push(r.clone());
                    }
                }
            }
        }
        if !acc.is_empty() {
            possible = Some(acc);
        }
    }
    let possible = possible?;
    let mut replacements: Vec<String> = if is_all_uppercase {
        possible.iter().map(|s| s.to_uppercase()).collect()
    } else {
        possible
    };
    replacements.retain(|r| r != &original);
    Some(replacements)
}

pub const SOFT_RULE_ID: &str = "UK_SIMPLE_REPLACE_SOFT";
const SOFT_DESCRIPTION: &str = "Пошук нерекомендованих слів";
const SOFT_SHORT: &str = "Нерекомендоване слово";

/// `SimpleReplaceSoftRule` (`uk/rules/replace_soft.txt`): the `ctx:` entries
/// are contexts, not suggestions.
pub struct SimpleReplaceSoftRule {
    wrong_words: HashMap<String, Vec<String>>,
}

impl SimpleReplaceSoftRule {
    pub fn load(rules_dir: &Path) -> Self {
        Self {
            wrong_words: load_lists(&rules_dir.join("replace_soft.txt")),
        }
    }

    pub fn rule_id(&self) -> &str {
        SOFT_RULE_ID
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut out = Vec::new();
        for tr in tokens.iter().filter(|t| !t.is_whitespace) {
            let sent_start =
                tr.readings.first().and_then(|r| r.pos_tag.as_deref()) == Some("SENT_START");
            if sent_start || tr.is_immunized || tr.is_ignore_spelling || tr.surface() == "завидна"
            {
                continue;
            }
            let Some(replacements) = base_replacements_in(&self.wrong_words, tr) else {
                continue;
            };
            if replacements.is_empty() {
                continue;
            }
            let original = tr.surface().to_string();
            let mut contexts: Vec<String> = Vec::new();
            let mut plain: Vec<String> = Vec::new();
            for r in &replacements {
                if let Some(ctx) = r.strip_prefix("ctx:") {
                    for c in ctx.trim().split(',') {
                        contexts.push(c.trim().to_string());
                    }
                } else {
                    plain.push(r.clone());
                }
            }
            let mut replacements = plain;
            if replacements.is_empty() {
                continue;
            }
            let message = if !contexts.is_empty() {
                format!(
                    "«{original}» вживається лише в таких контекстах: {}, можливо, мали на увазі: {}?",
                    contexts.join(", "),
                    replacements.join(", ")
                )
            } else {
                format!(
                    "«{original}» — нерекомендоване слово, кращий варіант: {}.",
                    replacements.join(", ")
                )
            };
            if original.chars().next().is_some_and(|c| c.is_uppercase()) {
                for r in &mut replacements {
                    *r = uppercase_first_char(r);
                }
            }
            out.push(
                Match::new(
                    SOFT_RULE_ID,
                    Option::<String>::None,
                    message,
                    Some(SOFT_SHORT.to_string()),
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
                    CATEGORY_ID,
                    CATEGORY_NAME,
                )
                .with_metadata(SOFT_DESCRIPTION, "style", 0),
            );
        }
        out
    }
}

fn uppercase_first_char(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn load_lists(path: &Path) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return result;
    };
    let split_re = Regex::new(r" *= *|\|").unwrap();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<String> = split_re
            .split(line)
            .filter_map(|s| s.ok())
            .map(|s| s.to_string())
            .collect();
        if parts.len() >= 2 {
            result.insert(parts[0].clone(), parts[1..].to_vec());
        }
    }
    result
}
