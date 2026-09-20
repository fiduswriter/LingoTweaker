//! Spanish `AbstractSimpleReplaceRule` subclasses (`SimpleReplaceRule`,
//! `SimpleReplaceVerbsRule`) — the single-token base class that predates
//! `AbstractSimpleReplaceRule2` (see `crate::simple_replace`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

pub const SIMPLE_REPLACE_RULE_ID: &str = "ES_SIMPLE_REPLACE_SIMPLE";
pub const SIMPLE_REPLACE_VERBS_RULE_ID: &str = "ES_SIMPLE_REPLACE_VERBS";
const SHORT_SIMPLE: &str = "Palabra incorrecta";
const DESCRIPTION_SIMPLE: &str = "Palabra incorrecta: $match";
const SHORT_VERBS: &str = "Verbo incorrecto";
const DESCRIPTION_VERBS: &str = "Detecta verbos incorrectos y propone sugerencias.";
const CATEGORY_ID: &str = "TYPOS";
const CATEGORY_NAME: &str = "Posible error ortográfico";

/// `SimpleReplaceDataLoader.loadWords`: `wrong=right` lines (multiple wrong
/// forms separated by `|`, multiple replacements separated by `|`).
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
        for wrong_form in left.split('|') {
            map.insert(wrong_form.to_string(), replacements.clone());
        }
    }
    map
}

/// `StringTools.toId` (Spanish short code: no umlaut mapping).
fn to_id(input: &str) -> String {
    input
        .trim()
        .to_uppercase()
        .replace(' ', "_")
        .replace('\'', "_Q_")
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase()
                || ('\u{c0}'..='\u{d6}').contains(&c)
                || ('\u{d8}'..='\u{de}').contains(&c)
            {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `AbstractSimpleReplaceRule` (case-insensitive, sub-rule ids, tagged words
/// ignored) with the Spanish `SimpleReplaceRule` messages.
pub struct SpanishSimpleReplaceRule {
    wrong_words: HashMap<String, Vec<String>>,
}

impl SpanishSimpleReplaceRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let rules_dir = data_dir.join("es/rules");
        let mut wrong_words = load_words(&rules_dir.join("replace.txt"));
        wrong_words.extend(load_words(&rules_dir.join("replace_custom.txt")));
        Ok(Self { wrong_words })
    }

    pub fn rule_id(&self) -> &str {
        SIMPLE_REPLACE_RULE_ID
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
        let mut matches = Vec::new();
        for tr in &non_blank {
            if tr.is_sentence_start || tr.is_immunized || tr.is_ignore_spelling || tr.is_tagged {
                continue;
            }
            let original = tr.surface().to_string();
            let lower = original.to_lowercase();
            let is_all_uppercase = lt_spell::morfologik::is_all_uppercase(&original);
            let possible = self
                .wrong_words
                .get(&original)
                .or_else(|| self.wrong_words.get(&lower));
            let Some(possible) = possible else {
                continue;
            };
            let mut replacements: Vec<String> = if is_all_uppercase {
                possible.iter().map(|s| s.to_uppercase()).collect()
            } else {
                possible.clone()
            };
            replacements.retain(|r| *r != original);
            if replacements.is_empty() {
                continue;
            }
            matches.push(self.create_match(tr, replacements, sentence_offset, &original));
        }
        matches
    }

    /// `AbstractSimpleReplaceRule.createRuleMatch` with `subRuleSpecificIds`.
    fn create_match(
        &self,
        tr: &AnalyzedTokenReadings,
        mut replacements: Vec<String>,
        sentence_offset: usize,
        original: &str,
    ) -> Match {
        let id = to_id(&format!("{SIMPLE_REPLACE_RULE_ID}_{original}"));
        let description = DESCRIPTION_SIMPLE.replace("$match", original);
        let message = match replacements.first() {
            Some(first) => format!("¿Quería decir «{first}»?"),
            None => SHORT_SIMPLE.to_string(),
        };
        if original.chars().next().is_some_and(char::is_uppercase) {
            for r in &mut replacements {
                *r = uppercase_first(r);
            }
        }
        Match::new(
            &id,
            Option::<String>::None,
            &message,
            Some(SHORT_SIMPLE.to_string()),
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
        .with_metadata(&description, "misspelling", 0)
        .with_match_type("Other")
    }
}

/// `SimpleReplaceVerbsRule.match` (all inflected forms of a wrong verb).
pub struct SpanishSimpleReplaceVerbsRule {
    wrong_words: HashMap<String, Vec<String>>,
    synth: Arc<lt_tagger::SpanishSynthesizer>,
    tagger: Arc<lt_tagger::SpanishTagger>,
}

impl SpanishSimpleReplaceVerbsRule {
    pub fn load(
        data_dir: &Path,
        synth: Arc<lt_tagger::SpanishSynthesizer>,
        tagger: Arc<lt_tagger::SpanishTagger>,
    ) -> Result<Self> {
        let wrong_words = load_words(&data_dir.join("es/rules/replace_verbs.txt"));
        Ok(Self {
            wrong_words,
            synth,
            tagger,
        })
    }

    pub fn rule_id(&self) -> &str {
        SIMPLE_REPLACE_VERBS_RULE_ID
    }

    fn desinencies() -> &'static fancy_regex::Regex {
        static RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
            const ENDINGS: &str = "a|aba|abais|aban|abas|ad|ada|adas|ado|ados|amos|an|ando|ar|ara|arais|aran|aras|are|areis|aremos|aren|ares|aron|ará|arán|arás|aré|aréis|aría|aríais|aríamos|arían|arías|as|ase|aseis|asen|ases|aste|asteis|e|emos|en|es|o|ábamos|áis|áramos|áremos|ásemos|é|éis|ó|arse|arme|arte|arlos|arles|arlas|arnos|aros";
            fancy_regex::Regex::new(&format!(r"^(.+?)({ENDINGS})$")).unwrap()
        });
        &RE
    }

    fn desinencies1() -> &'static fancy_regex::Regex {
        static RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
            const ENDINGS: &str = "a|aba|abais|aban|abas|ad|ada|adas|ado|ados|amos|an|ando|ar|ara|arais|aran|aras|are|areis|aremos|aren|ares|aron|ará|arán|arás|aré|aréis|aría|aríais|aríamos|arían|arías|as|ase|aseis|asen|ases|aste|asteis|e|emos|en|es|o|ábamos|áis|áramos|áremos|ásemos|é|éis|ó|arse|arme|arte|arlos|arles|arlas|arnos|aros";
            fancy_regex::Regex::new(&format!(r"^(.+)({ENDINGS})$")).unwrap()
        });
        &RE
    }

    /// `SimpleReplaceVerbsRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut rule_matches: Vec<Match> = Vec::new();
        for tr in &non_blank {
            let original = tr.surface().to_string();
            if tr.is_tagged {
                continue;
            }
            let token_string = original.to_lowercase();
            let mut analyzed: Option<Vec<lt_core::AnalyzedToken>> = None;
            let mut infinitive: Option<String> = None;
            let mut i = 0;
            while i < 2 && analyzed.is_none() {
                let m = if i == 0 {
                    Self::desinencies().captures(&token_string)
                } else {
                    Self::desinencies1().captures(&token_string)
                };
                if let Ok(Some(caps)) = m {
                    let mut lexeme = caps.get(1).unwrap().as_str().to_string();
                    let desinence = caps.get(2).unwrap().as_str();
                    if desinence.starts_with(['e', 'é', 'i', 'ï']) {
                        if let Some(stripped) = lexeme.strip_suffix('c') {
                            lexeme = format!("{stripped}z");
                        } else if let Some(stripped) = lexeme.strip_suffix("qu") {
                            lexeme = format!("{stripped}c");
                        } else if let Some(stripped) = lexeme.strip_suffix('g') {
                            lexeme = format!("{stripped}j");
                        } else if let Some(stripped) = lexeme.strip_suffix("gü") {
                            lexeme = format!("{stripped}gu");
                        } else if let Some(stripped) = lexeme.strip_suffix("gu") {
                            lexeme = format!("{stripped}g");
                        }
                    }
                    let candidate = format!("{lexeme}ar");
                    if self.wrong_words.contains_key(&candidate) {
                        let word = format!("am{desinence}");
                        let tagged = self.tagger.tag(std::slice::from_ref(&word));
                        if let Some(first) = tagged.into_iter().next() {
                            if first.readings.first().is_some_and(|r| r.pos_tag.is_some()) {
                                analyzed = Some(first.readings);
                            }
                        }
                    }
                    infinitive = Some(candidate);
                }
                i += 1;
            }
            let (Some(analyzed), Some(infinitive)) = (analyzed, infinitive) else {
                continue;
            };
            let Some(replacement_infinitives) = self.wrong_words.get(&infinitive) else {
                continue;
            };
            let mut possible: Vec<String> = Vec::new();
            for replacement_infinitive in replacement_infinitives {
                if replacement_infinitive.starts_with('(') {
                    possible.push(replacement_infinitive.clone());
                    continue;
                }
                let parts: Vec<&str> = replacement_infinitive.split(' ').collect();
                let token = lt_core::AnalyzedToken::new(
                    parts[0],
                    Some(parts[0].to_string()),
                    Some("V.*".to_string()),
                );
                for at in &analyzed {
                    let Some(tag) = at.pos_tag.as_deref() else {
                        continue;
                    };
                    let tag = if parts[0] == "haver" && tag.len() > 2 {
                        format!("VA{}", &tag[2..])
                    } else {
                        tag.to_string()
                    };
                    for mut s in self.synth.synthesize(&token, &tag, false) {
                        for part in parts.iter().skip(1) {
                            s.push(' ');
                            s.push_str(part);
                        }
                        if !possible.contains(&s) {
                            possible.push(s);
                        }
                    }
                }
            }
            if possible.is_empty() {
                continue;
            }
            // Java's legacy `AbstractSimpleReplaceRule` only expands `$match`
            // in the description, so Java's message keeps the literal `$match`
            // (a Java bug, docs/differences.md #2); expand it like Java's own
            // description and like `AbstractSimpleReplaceRule2` does.
            let message = "Verbo incorrecto: $match".replace("$match", &original);
            let id = to_id(&format!("{SIMPLE_REPLACE_VERBS_RULE_ID}_{original}"));
            let description = DESCRIPTION_VERBS.replace("$match", &original);
            rule_matches.push(
                Match::new(
                    &id,
                    Option::<String>::None,
                    &message,
                    Some(SHORT_VERBS.to_string()),
                    TextRange::new(
                        sentence_offset + tr.start_pos,
                        sentence_offset + tr.start_pos + original.len(),
                    ),
                    possible
                        .into_iter()
                        .map(|value| Suggestion {
                            value,
                            short_description: None,
                        })
                        .collect(),
                    CATEGORY_ID,
                    CATEGORY_NAME,
                )
                .with_metadata(&description, "misspelling", 0)
                .with_match_type("Other"),
            );
        }
        rule_matches
    }
}
