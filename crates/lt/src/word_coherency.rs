//! Port of `AbstractWordCoherencyRule` / English `WordCoherencyRule`
//! (`EN_WORD_COHERENCY`): do not mix two admitted spellings within one text.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, Match, Suggestion, TextRange};

use crate::ca::helpers::reading_with_tag_regex;

const RULE_ID: &str = "EN_WORD_COHERENCY";
const DESCRIPTION: &str = "Coherent spelling of words with two admitted variants.";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Miscellaneous";
const DE_RULE_ID: &str = "DE_WORD_COHERENCY";
const DE_DESCRIPTION: &str =
    "Einheitliche Schreibweise für Wörter mit mehr als einer korrekten Schreibweise";
const DE_CATEGORY_NAME: &str = "Sonstiges";
const PT_RULE_ID: &str = "PT_WORD_COHERENCY";
const PT_DESCRIPTION: &str = "Consistência de palavras com grafias múltiplas";
const PT_CATEGORY_NAME: &str = "Estilo";
const NL_RULE_ID: &str = "NL_WORD_COHERENCY";
const NL_DESCRIPTION: &str = "Consistente spelling van woorden met meerdere correcte vormen.";
const NL_CATEGORY_NAME: &str = "Diversen";
const CA_RULE_ID: &str = "CA_WORD_COHERENCY";
const CA_VALENCIA_RULE_ID: &str = "CA_WORD_COHERENCY_VALENCIA";
const CA_DESCRIPTION: &str = "Detecta l'ús incoherent de diferents formes dins d'un text.";
const CA_SHORT: &str = "Coherència";
const CA_CATEGORY_NAME: &str = "Estil";

/// Which subclass's message/category/issue-type the loader applies.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CoherencyLang {
    English,
    German,
    Portuguese,
    Dutch,
    Catalan,
    Valencian,
}

pub struct WordCoherencyRule {
    /// both directions: each variant maps to the other ones
    word_map: HashMap<String, HashSet<String>>,
    rule_id: &'static str,
    description: &'static str,
    short_message: Option<&'static str>,
    category_name: &'static str,
    lang: CoherencyLang,
    /// Catalan subclasses synthesize the replacement with the marked
    /// token's POS tag (`WordCoherencyRule.createReplacement`).
    synth: Option<Arc<lt_tagger::CatalanSynthesizer>>,
}

impl WordCoherencyRule {
    /// Rule id (`CA_WORD_COHERENCY`, `CA_WORD_COHERENCY_VALENCIA`, …).
    pub fn rule_id(&self) -> &'static str {
        self.rule_id
    }

    pub fn from_data(data_dir: &Path) -> Self {
        Self::load(data_dir, "en/rules/coherency.txt", CoherencyLang::English)
    }

    /// `de.WordCoherencyRule`: `de/rules/coherency.txt`, German message.
    pub fn german(data_dir: &Path) -> Self {
        Self::load(data_dir, "de/rules/coherency.txt", CoherencyLang::German)
    }

    /// `PortugueseWordCoherencyRule`: `pt/rules/coherency.txt`, Portuguese
    /// message, category STYLE, issue type inconsistency.
    pub fn portuguese(data_dir: &Path) -> Self {
        Self::load(
            data_dir,
            "pt/rules/coherency.txt",
            CoherencyLang::Portuguese,
        )
    }

    /// `Dutch WordCoherencyRule`: `nl/rules/coherency.txt`, Dutch message,
    /// category MISC (Diversen), issue type misspelling.
    pub fn dutch(data_dir: &Path) -> Self {
        Self::load(data_dir, "nl/rules/coherency.txt", CoherencyLang::Dutch)
    }

    /// `ca.WordCoherencyRule`: `ca/rules/coherency.txt`, Catalan message,
    /// category STYLE.
    pub fn catalan(data_dir: &Path, synth: Arc<lt_tagger::CatalanSynthesizer>) -> Self {
        Self::load_ca(data_dir, "ca/rules/coherency.txt", CA_RULE_ID, synth)
    }

    /// `WordCoherencyValencianRule`: `ca/rules/coherency-valencia.txt`, the
    /// valencia variant addition of `ValencianCatalan.getRelevantRules`.
    pub fn valencian(data_dir: &Path, synth: Arc<lt_tagger::CatalanSynthesizer>) -> Self {
        Self::load_ca(
            data_dir,
            "ca/rules/coherency-valencia.txt",
            CA_VALENCIA_RULE_ID,
            synth,
        )
    }

    fn load_ca(
        data_dir: &Path,
        path: &str,
        rule_id: &'static str,
        synth: Arc<lt_tagger::CatalanSynthesizer>,
    ) -> Self {
        let mut rule = Self::load(
            data_dir,
            path,
            if rule_id == CA_VALENCIA_RULE_ID {
                CoherencyLang::Valencian
            } else {
                CoherencyLang::Catalan
            },
        );
        rule.synth = Some(synth);
        rule
    }

    fn load(data_dir: &Path, path: &str, lang: CoherencyLang) -> Self {
        let mut word_map: HashMap<String, HashSet<String>> = HashMap::new();
        let text = lt_data::fs::read_to_string(data_dir.join(path)).unwrap_or_default();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() != 2 {
                continue;
            }
            word_map
                .entry(parts[0].to_string())
                .or_default()
                .insert(parts[1].to_string());
            word_map
                .entry(parts[1].to_string())
                .or_default()
                .insert(parts[0].to_string());
        }
        let (rule_id, description, short_message, category_name) = match lang {
            CoherencyLang::English => (RULE_ID, DESCRIPTION, None, CATEGORY_NAME),
            CoherencyLang::German => (DE_RULE_ID, DE_DESCRIPTION, None, DE_CATEGORY_NAME),
            CoherencyLang::Portuguese => (PT_RULE_ID, PT_DESCRIPTION, None, PT_CATEGORY_NAME),
            CoherencyLang::Dutch => (NL_RULE_ID, NL_DESCRIPTION, None, NL_CATEGORY_NAME),
            CoherencyLang::Catalan | CoherencyLang::Valencian => {
                (CA_RULE_ID, CA_DESCRIPTION, Some(CA_SHORT), CA_CATEGORY_NAME)
            }
        };
        let rule_id = if lang == CoherencyLang::Valencian {
            CA_VALENCIA_RULE_ID
        } else {
            rule_id
        };
        Self {
            word_map,
            rule_id,
            description,
            short_message,
            category_name,
            lang,
            synth: None,
        }
    }

    /// Catalan `createReplacement`: synthesize the other spelling with the
    /// marked token's POS tag (`[VAND].*`); falls back to the base
    /// replacement. Java passes an empty lemma, so the dictionary lookup is
    /// usually empty and the fallback applies.
    fn catalan_replacement(
        &self,
        marked: &str,
        token: &str,
        other_spelling: &str,
        token_readings: &lt_core::AnalyzedTokenReadings,
    ) -> String {
        let Some(synth) = &self.synth else {
            return create_replacement(marked, token, other_spelling);
        };
        if let Some(atr) = reading_with_tag_regex(token_readings, "[VAND].*") {
            if let Some(tag) = atr.pos_tag.clone() {
                let probe =
                    AnalyzedToken::new("", Some(String::new()), Some(other_spelling.to_string()));
                let forms = synth.synthesize(&probe, &tag, false);
                if let Some(first) = forms.first() {
                    if !first.is_empty() {
                        return first.clone();
                    }
                }
            }
        }
        create_replacement(marked, token, other_spelling)
    }

    /// `AbstractWordCoherencyRule.match` over all sentences of the text.
    pub fn check(&self, sentences: &[AnalyzedSentence]) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let mut should_not_appear_word: HashMap<String, String> = HashMap::new();
        for sentence in sentences {
            let tokens: Vec<&lt_core::AnalyzedTokenReadings> = sentence
                .tokens
                .iter()
                .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
                .collect();
            for token_readings in tokens {
                if token_readings.readings.is_empty() {
                    continue;
                }
                let baseforms: Vec<&str> =
                    token_readings.readings.iter().map(|r| r.lemma()).collect();
                for baseform in baseforms {
                    let token = baseform.to_string();
                    let from_pos = sentence.offset + token_readings.start_pos;
                    let to_pos = sentence.offset + token_readings.end_pos();
                    if let Some(other_spelling) = should_not_appear_word.get(&token) {
                        let message = match self.lang {
                            CoherencyLang::English => format!(
                                "Do not mix variants of the same word ('{token}' and '{other_spelling}') within a single text."
                            ),
                            CoherencyLang::German => format!(
                                "'{token}' und '{other_spelling}' sollten nicht gleichzeitig benutzt werden."
                            ),
                            CoherencyLang::Portuguese => format!(
                                "Não deve utilizar formas distintas de palavras com dupla grafia no mesmo texto. Escolha entre '{token}' e '{other_spelling}'."
                            ),
                            CoherencyLang::Dutch => format!(
                                "Gebruik liever niet '{token}' en '{other_spelling}' door elkaar in een tekst."
                            ),
                            CoherencyLang::Catalan | CoherencyLang::Valencian => format!(
                                "No és coherent usar '{token}' i '{other_spelling}' dins d'un mateix text."
                            ),
                        };
                        let marked = sentence
                            .text
                            .get(token_readings.start_pos..token_readings.end_pos())
                            .unwrap_or("")
                            .to_string();
                        let mut replacement = match self.lang {
                            CoherencyLang::Catalan | CoherencyLang::Valencian => self
                                .catalan_replacement(
                                    &marked,
                                    &token,
                                    other_spelling,
                                    token_readings,
                                ),
                            _ => create_replacement(&marked, &token, other_spelling),
                        };
                        if starts_with_uppercase(token_readings.surface()) {
                            replacement = uppercase_first(&replacement);
                        }
                        let mut m = Match::new(
                            self.rule_id,
                            Option::<String>::None,
                            message,
                            self.short_message.map(str::to_string),
                            TextRange::new(from_pos, to_pos),
                            Vec::<Suggestion>::new(),
                            if matches!(
                                self.lang,
                                CoherencyLang::Catalan | CoherencyLang::Valencian
                            ) {
                                "STYLE"
                            } else {
                                CATEGORY_ID
                            },
                            self.category_name,
                        )
                        .with_metadata(
                            self.description,
                            if matches!(
                                self.lang,
                                CoherencyLang::Portuguese
                                    | CoherencyLang::Catalan
                                    | CoherencyLang::Valencian
                            ) {
                                "inconsistency"
                            } else {
                                "misspelling"
                            },
                            0,
                        )
                        .with_match_type("Other");
                        if !marked.eq_ignore_ascii_case(&replacement) {
                            m.suggestions = vec![Suggestion {
                                value: replacement,
                                short_description: None,
                            }];
                            rule_matches.push(m);
                        }
                        break;
                    } else if let Some(should_not_appear) = self.word_map.get(&token) {
                        for word in should_not_appear {
                            should_not_appear_word.insert(word.clone(), token.clone());
                        }
                    }
                }
            }
        }
        rule_matches
    }
}

/// `createReplacement`: `marked.replaceFirst("(?i)" + token, otherSpelling)`.
fn create_replacement(marked: &str, token: &str, other_spelling: &str) -> String {
    let lower_marked = marked.to_lowercase();
    let lower_token = token.to_lowercase();
    if let Some(pos) = lower_marked.find(&lower_token) {
        let mut out = String::new();
        out.push_str(&marked[..pos]);
        out.push_str(other_spelling);
        out.push_str(&marked[pos + token.len()..]);
        out
    } else {
        marked.to_string()
    }
}

fn starts_with_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
