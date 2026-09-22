//! Port of `org.languagetool.tagging.disambiguation.uk.SimpleDisambiguator`:
//! removes dictionary readings listed in `uk/words/disambig_remove.txt` and the
//! duplicate-lemma readings from `uk/words/disambig_dups.txt`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

static DASHED_PARTICLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*-(?:то|от|таки|бо|но))$").unwrap());

/// One `MatcherEntry`: a lemma (`*` = any) plus a full-match POS tag regex.
struct MatcherEntry {
    lemma: String,
    tag_regex: Regex,
}

impl MatcherEntry {
    fn matches(&self, token: &AnalyzedToken) -> bool {
        let Some(tag) = token.pos_tag.as_deref() else {
            return false;
        };
        (self.lemma == "*" || token.stem.as_deref() == Some(self.lemma.as_str()))
            && self.tag_regex.is_match(tag).unwrap_or(false)
    }
}

struct TokenMatcher {
    matchers: Vec<MatcherEntry>,
}

impl TokenMatcher {
    fn matches(&self, token: &AnalyzedToken) -> bool {
        self.matchers.iter().any(|m| m.matches(token))
    }
}

pub struct SimpleDisambiguator {
    remove_map: HashMap<String, TokenMatcher>,
    dups_map: HashMap<String, Vec<String>>,
}

impl SimpleDisambiguator {
    pub fn load(words_dir: &Path) -> Self {
        Self {
            remove_map: load_remove_map(&words_dir.join("disambig_remove.txt")),
            dups_map: load_dups_map(&words_dir.join("disambig_dups.txt")),
        }
    }

    /// `SimpleDisambiguator.removeRareForms`.
    pub fn remove_rare_forms(&self, sentence: &mut AnalyzedSentence) {
        for token_index in 0..sentence.tokens.len() {
            if sentence.tokens[token_index].is_whitespace {
                continue;
            }
            // `getTokensWithoutWhitespace()` starts at the SENT_START token,
            // which the Java loop skips.
            if token_index == 0 {
                continue;
            }
            let surface = sentence.tokens[token_index].surface().to_string();
            if surface.is_empty() {
                continue;
            }
            let mut token = surface;
            if token.chars().next().is_some_and(|c| c.is_lowercase()) {
                token = token.to_lowercase();
            }

            let mut matcher = self.remove_map.get(&token);
            if matcher.is_none() {
                let lower = token.to_lowercase();
                matcher = self.remove_map.get(&lower);
                if matcher.is_none() {
                    if let Some(idx) = token.rfind('-') {
                        if idx > 0 && DASHED_PARTICLES.is_match(&token).unwrap_or(false) {
                            matcher = self.remove_map.get(&token[..idx]);
                        }
                    }
                }
            }

            if let Some(matcher) = matcher {
                sentence.tokens[token_index]
                    .readings
                    .retain(|at| !matcher.matches(at));
            }

            // duplicate lemmas
            let lemmas: HashSet<&str> = sentence.tokens[token_index]
                .readings
                .iter()
                .filter_map(|a| a.stem.as_deref())
                .collect();
            let present: Vec<&String> = self
                .dups_map
                .keys()
                .filter(|k| lemmas.contains(k.as_str()))
                .collect();
            if !present.is_empty() {
                let to_remove: HashSet<&String> = present
                    .iter()
                    .flat_map(|k| self.dups_map[*k].iter())
                    .collect();
                sentence.tokens[token_index].readings.retain(|a| {
                    !a.stem
                        .as_deref()
                        .is_some_and(|l| to_remove.contains(&l.to_string()))
                });
            }

            rebuild_flags(&mut sentence.tokens[token_index]);
        }
    }
}

fn rebuild_flags(token: &mut AnalyzedTokenReadings) {
    token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
}

fn load_remove_map(path: &Path) -> HashMap<String, TokenMatcher> {
    let mut result = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return result;
    };
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let line = line.split(" #").next().unwrap_or(line);
        let line = line.trim();
        let Some((word, matchers)) = line.split_once(' ') else {
            continue;
        };
        let entries: Vec<MatcherEntry> = matchers
            .split('|')
            .filter_map(|m| {
                let mut parts = m.split(' ');
                let lemma = parts.next()?.to_string();
                let tag = parts.next()?;
                Regex::new(&format!("^(?:{tag})$"))
                    .ok()
                    .map(|tag_regex| MatcherEntry { lemma, tag_regex })
            })
            .collect();
        result.insert(word.to_string(), TokenMatcher { matchers: entries });
    }
    result
}

fn load_dups_map(path: &Path) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return result;
    };
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let line = line.split(" #").next().unwrap_or(line);
        let parts: Vec<String> = line.trim().split(' ').map(|s| s.to_string()).collect();
        if let Some((key, rest)) = parts.split_first() {
            result.insert(key.clone(), rest.to_vec());
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn readings(surface: &str, entries: &[(&str, &str)]) -> AnalyzedTokenReadings {
        let mut tr = AnalyzedTokenReadings::new(
            entries
                .iter()
                .map(|(l, t)| AnalyzedToken::new(surface, Some(l.to_string()), Some(t.to_string())))
                .collect(),
        );
        tr.is_tagged = true;
        tr
    }

    fn sentence(tokens: Vec<AnalyzedTokenReadings>) -> AnalyzedSentence {
        AnalyzedSentence {
            text: String::new(),
            offset: 0,
            tokens,
            pre_disambig_tokens: Vec::new(),
            pre_disambig_detached: Vec::new(),
        }
    }

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    /// `SimpleDisambiguator.removeRareForms`: `бабій`'s rare verb reading is
    /// removed (`disambig_remove.txt`), the noun kept.
    #[test]
    fn removes_rare_forms() {
        if !data_dir().is_dir() {
            return;
        }
        let simple = SimpleDisambiguator::load(&data_dir().join("uk/words"));
        let mut start = readings("", &[("", "SENT_START")]);
        start.is_sentence_start = true;
        let mut sent = sentence(vec![
            start,
            readings(
                "бабій",
                &[
                    ("бабій", "noun:anim:m:v_naz"),
                    ("бабіти", "verb:imperf:impr:s:2"),
                ],
            ),
        ]);
        simple.remove_rare_forms(&mut sent);
        let got: Vec<String> = sent.tokens[1]
            .readings
            .iter()
            .map(|a| {
                format!(
                    "{}:{}",
                    a.stem.as_deref().unwrap_or(""),
                    a.pos_tag.as_deref().unwrap_or("")
                )
            })
            .collect();
        assert_eq!(got, ["бабій:noun:anim:m:v_naz"]);
    }

    /// `disambig_dups.txt`: when the first lemma is present, the mapped
    /// duplicate is removed.
    #[test]
    fn removes_duplicate_lemmas() {
        if !data_dir().is_dir() {
            return;
        }
        let simple = SimpleDisambiguator::load(&data_dir().join("uk/words"));
        let mut start = readings("", &[("", "SENT_START")]);
        start.is_sentence_start = true;
        let mut sent = sentence(vec![
            start,
            readings(
                "Ангола",
                &[
                    ("Ангола", "noun:inanim:f:v_naz:prop:geo"),
                    ("ангол", "noun:anim:m:v_rod:arch"),
                ],
            ),
        ]);
        simple.remove_rare_forms(&mut sent);
        let got: Vec<String> = sent.tokens[1]
            .readings
            .iter()
            .map(|a| a.stem.clone().unwrap_or_default())
            .collect();
        assert_eq!(got, ["Ангола"]);
    }
}
