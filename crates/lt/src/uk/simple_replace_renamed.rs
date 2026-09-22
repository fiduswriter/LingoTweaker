//! Port of `org.languagetool.rules.uk.SimpleReplaceRenamedRule`
//! (`UK_SIMPLE_REPLACE_RENAMED`): suggests the current name for renamed
//! toponyms from `uk/rules/replace_renamed.txt`.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "UK_SIMPLE_REPLACE_RENAMED";
const DESCRIPTION: &str = "Пропозиція поточної назви для перейменованих власних назв";
const SHORT: &str = "Перейменована назва";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";

static GEO_POSTAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:noun:inanim.*?:prop.*|adj.*)$").unwrap());

pub struct SimpleReplaceRenamedRule {
    renamed: HashMap<String, Vec<String>>,
}

impl SimpleReplaceRenamedRule {
    pub fn load(rules_dir: &std::path::Path) -> Self {
        Self {
            renamed: load_lists(&rules_dir.join("replace_renamed.txt")),
        }
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    /// `SimpleReplaceRenamedRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut out = Vec::new();
        for tr in tokens.iter().filter(|t| !t.is_whitespace) {
            let mut renamed_lemmas: Vec<String> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            let mut broken = false;
            for reading in &tr.readings {
                let pos_tag = reading.pos_tag.as_deref();
                if pos_tag == Some("SENT_END") {
                    continue;
                }
                if let Some(lemma) = reading.stem.as_deref() {
                    if self.renamed.contains_key(lemma)
                        && pos_tag.is_some_and(|t| GEO_POSTAG.is_match(t).unwrap_or(false))
                    {
                        if seen.insert(lemma.to_string()) {
                            renamed_lemmas.push(lemma.to_string());
                        }
                    } else {
                        renamed_lemmas.clear();
                        broken = true;
                        break;
                    }
                }
            }
            let _ = broken;
            if renamed_lemmas.is_empty() {
                continue;
            }
            let mut info = String::new();
            let mut replacements = Vec::new();
            for lemma in &renamed_lemmas {
                let repl = &self.renamed[lemma];
                if let Some(first) = repl.first() {
                    replacements.push(first.clone());
                }
                for item in repl.iter().take(repl.len().saturating_sub(1)).skip(1) {
                    replacements.push(item.clone());
                }
                if info.is_empty() && repl.len() > 1 {
                    info = repl[repl.len() - 1].clone();
                }
            }
            let token_str = renamed_lemmas[0].clone();
            let mut msg = format!("«{token_str}» було перейменовано");
            if !info.is_empty() {
                msg.push_str(&format!(" ({info})"));
            }
            out.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    msg,
                    Some(SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + tr.start_pos,
                        sentence_offset + tr.end_pos(),
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
                .with_metadata(DESCRIPTION, "style", 0),
            );
        }
        out
    }
}

fn load_lists(path: &std::path::Path) -> HashMap<String, Vec<String>> {
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
#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    #[test]
    fn renamed_lists_and_match() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/uk/rules");
        if !dir.is_dir() {
            return;
        }
        let rule = SimpleReplaceRenamedRule::load(&dir);
        assert!(
            rule.renamed.contains_key("Альошинське"),
            "{:?}",
            rule.renamed.keys().take(3).collect::<Vec<_>>()
        );
        let mut tr = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "Альошинське",
            Some("Альошинське".to_string()),
            Some("noun:inanim:n:v_naz:prop:geo".to_string()),
        )]);
        tr.is_tagged = true;
        let matches = rule.check_sentence(&[tr], 0);
        assert_eq!(matches.len(), 1, "{matches:?}");
    }
}
