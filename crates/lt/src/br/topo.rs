//! Port of `org.languagetool.rules.br.TopoReplaceRule` (`BR_TOPO`): French
//! place names in `/br/topo.txt` that should be written in Breton. Loads
//! multiword keys (the n-th map holds the n-word phrases) and matches the
//! longest phrase first, case-sensitively.

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

pub const RULE_ID: &str = "BR_TOPO";

pub struct TopoReplaceRule {
    /// `wrongWords`: the n-th map holds the `(n+1)`-word phrases.
    wrong_words: Vec<HashMap<String, String>>,
}

impl TopoReplaceRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("br/rules/topo.txt");
        let text = lt_data::fs::read_to_string(&path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display()))
        })?;
        let mut list: Vec<HashMap<String, String>> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((left, right)) = line.split_once('=') else {
                return Err(lt_core::CoreError::Parse(
                    "topo".into(),
                    format!("format error, line: {line}"),
                ));
            };
            for wrong_form in left.split('|') {
                let word_count = lt_tokenize::BretonWordTokenizer::new()
                    .tokenize(wrong_form)
                    .iter()
                    .filter(|t| !lt_core::is_whitespace(t))
                    .count();
                if word_count == 0 {
                    continue;
                }
                while list.len() < word_count {
                    list.push(HashMap::new());
                }
                list[word_count - 1].insert(wrong_form.to_string(), right.to_string());
            }
        }
        Ok(Self { wrong_words: list })
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "anvioù-lec’h e brezhoneg"
    }

    fn short(&self) -> &'static str {
        "anvioù lec’h"
    }

    fn suggestion(&self) -> &'static str {
        " zo un anv lec’h gallek. Ha fellout a rae deoc’h skrivañ "
    }

    fn suggestions_separator(&self) -> &'static str {
        " pe "
    }

    /// `TopoReplaceRule.match` over one sentence's token view
    /// (`getTokensWithoutWhitespace`).
    #[allow(clippy::needless_range_loop)]
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
        if self.wrong_words.is_empty() {
            return rule_matches;
        }
        let mut prev_tokens: VecDeque<&AnalyzedTokenReadings> = VecDeque::new();

        for i in 1..non_blank.len() {
            prev_tokens.push_back(non_blank[i]);
            if prev_tokens.len() > self.wrong_words.len() {
                prev_tokens.pop_front();
            }
            let prev_list: Vec<&AnalyzedTokenReadings> = prev_tokens.iter().copied().collect();
            let mut variants: Vec<String> = Vec::new();
            let mut sb = String::new();
            for j in (0..prev_list.len()).rev() {
                if j != prev_list.len() - 1 && prev_list[j + 1].whitespace_before {
                    sb.insert(0, ' ');
                }
                sb.insert_str(0, prev_list[j].surface());
                variants.insert(0, sb.clone());
            }
            let len = variants.len();
            for j in 0..len {
                let crt_word_count = len - j;
                let first_index = len - crt_word_count;
                if prev_list[first_index].is_immunized {
                    continue;
                }
                let crt = &variants[j];
                let Some(crt_match) = self.wrong_words[crt_word_count - 1].get(crt) else {
                    continue;
                };
                let replacements: Vec<String> =
                    crt_match.split('|').map(|s| s.to_string()).collect();
                let mut msg = format!("{crt}{}", self.suggestion());
                for (k, repl) in replacements.iter().enumerate() {
                    if k > 0 {
                        msg.push_str(if k == replacements.len() - 1 {
                            self.suggestions_separator()
                        } else {
                            ", "
                        });
                    }
                    msg.push_str("<suggestion>");
                    msg.push_str(repl);
                    msg.push_str("</suggestion>");
                }
                msg.push('?');
                let start_pos = sentence_offset + prev_list[first_index].start_pos;
                let end_pos = sentence_offset + prev_list[len - 1].end_pos();
                let suggestions: Vec<Suggestion> = replacements
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect();
                let m = Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    msg,
                    Some(self.short().to_string()),
                    TextRange::new(start_pos, end_pos),
                    suggestions,
                    "MISC",
                    "Reolennoù diazez",
                )
                .with_metadata(self.description(), "Other", 0)
                .with_match_type("Other");
                rule_matches.push(m);
                break;
            }
        }
        rule_matches
    }
}
