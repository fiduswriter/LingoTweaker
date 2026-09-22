//! Port of `org.languagetool.rules.uk.MissingHyphenRule` (`UK_MISSING_HYPHEN`):
//! a dash prefix written with a space instead of a hyphen.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "UK_MISSING_HYPHEN";
const DESCRIPTION: &str = "Пропущений дефіс";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";
const UA_1992_TAG_PART: &str = ":alt";

static ALL_LOWER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[а-яіїєґ'-]+$").unwrap());
static GEO_OR_PENINSULA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:країни|півострова)$").unwrap());

pub struct MissingHyphenRule {
    dash_prefixes: HashMap<String, String>,
}

impl MissingHyphenRule {
    pub fn load(words_dir: &Path) -> Self {
        let mut dash_prefixes = HashMap::new();
        if let Ok(text) = lt_data::fs::read_to_string(words_dir.join("dash_prefixes.txt")) {
            for line in text.lines() {
                let mut parts = line.split(' ');
                if let Some(key) = parts.next() {
                    let value = parts.next().unwrap_or("");
                    dash_prefixes.insert(key.to_string(), value.to_string());
                }
            }
        }
        dash_prefixes.remove("блок");
        dash_prefixes.remove("рейтинг");
        dash_prefixes.retain(|key, value| {
            ALL_LOWER.is_match(key).unwrap_or(false) && !value.contains(":bad")
        });
        Self { dash_prefixes }
    }

    fn prefix_extra_tag(&self, token: &str, is_capitalized: bool) -> Option<String> {
        let token = if is_capitalized {
            uncapitalize(token)
        } else {
            token.to_string()
        };
        self.dash_prefixes.get(&token).cloned()
    }

    /// `MissingHyphenRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        tagger: &lt_tagger::UkrainianTagger,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view: Vec<&AnalyzedTokenReadings> =
            tokens.iter().filter(|t| !t.is_whitespace).collect();
        let mut out = Vec::new();
        for i in 1..view.len().saturating_sub(1) {
            let tr = view[i];
            let next = view[i + 1];
            let next_surface = next.surface();
            let next_has_noun = next
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("noun")));
            let next_has_pron = next
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains("pron")));
            if !(next_has_noun
                && !next_has_pron
                && ALL_LOWER.is_match(next_surface).unwrap_or(false))
            {
                continue;
            }
            let surface = tr.surface().to_string();
            let is_capitalized = lt_tagger::uk_helpers::is_capitalized(&surface);
            let extra_tag = self.prefix_extra_tag(&surface, is_capitalized);
            let tim_aut = surface.to_lowercase() == "тайм"
                && next
                    .readings
                    .iter()
                    .any(|r| r.stem.as_deref() == Some("аут"));
            if extra_tag.is_none() && !tim_aut {
                continue;
            }
            if surface.eq_ignore_ascii_case("медіа")
                && GEO_OR_PENINSULA.is_match(next_surface).unwrap_or(false)
            {
                continue;
            }
            if surface.eq_ignore_ascii_case("шоу") && next_surface.contains('-') {
                continue;
            }
            let (suggested, message) = if extra_tag.as_deref() == Some(UA_1992_TAG_PART) {
                (
                    format!("{surface}{next_surface}"),
                    "Можливо, зайвий пробіл?",
                )
            } else {
                (
                    format!("{surface}-{next_surface}"),
                    "Можливо, пропущено дефіс?",
                )
            };
            let token_to_check = if is_capitalized {
                uncapitalize(&suggested)
            } else {
                suggested.clone()
            };
            let known = !tagger.lookup(&token_to_check).is_empty();
            let alt = extra_tag.as_deref() == Some(UA_1992_TAG_PART)
                && next.readings.iter().any(|r| {
                    r.pos_tag
                        .as_deref()
                        .is_some_and(|t| t.contains(UA_1992_TAG_PART))
                });
            if known || alt {
                out.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        message,
                        Some(DESCRIPTION.to_string()),
                        TextRange::new(
                            sentence_offset + tr.start_pos,
                            sentence_offset + next.end_pos(),
                        ),
                        vec![Suggestion {
                            value: suggested,
                            short_description: None,
                        }],
                        CATEGORY_ID,
                        CATEGORY_NAME,
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
            }
        }
        out
    }
}

fn uncapitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
