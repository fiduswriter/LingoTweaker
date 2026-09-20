//! Polish Java-coded built-in rules (`Polish.getRelevantRules`):
//! `WordRepeatRule` (generic base, stage 1), `PolishWordRepeatRule`
//! (`AdvancedWordRepeatRule`) and `SimpleReplaceRule`
//! (`AbstractSimpleReplaceRule` v1).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use regex::Regex;

use crate::wordutil::{eq_ignore_case, is_word};

// ---------------------------------------------------------------------------
// WordRepeatRule (`WORD_REPEAT_RULE`), the generic base class
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_RULE_ID: &str = "WORD_REPEAT_RULE";
pub const WORD_REPEAT_DESCRIPTION: &str = "Powtórzenie wyrazu (np. „jest jest”)";
pub const WORD_REPEAT_SHORT: &str = "Powtórzenie wyrazu";
pub const WORD_REPEAT_MESSAGE: &str = "Prawdopodobna literówka: powtórzony wyraz";

/// `WordRepeatRule.ignore`: the fixed base name list.
fn base_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if position > 0
            && tokens[position - 1].surface() == name
            && tokens[position].surface() == name
        {
            return true;
        }
    }
    false
}

/// `WordRepeatRule.match` over one sentence with the Polish
/// `MessagesBundle_pl` strings. The generic base class has no language
/// override, so the detection is the `WordRepeatRule` behaviour unchanged.
pub struct WordRepeatSentenceRule;

impl WordRepeatSentenceRule {
    pub fn new() -> Self {
        Self
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut prev_token = String::new();
        for (i, item) in view.iter().enumerate().skip(1) {
            let token = item.surface().to_string();
            if view[i].is_immunized {
                prev_token.clear();
                continue;
            }
            if is_word(&token) && eq_ignore_case(&prev_token, &token) && !base_ignore(&view, i) {
                let prev_pos = view[i - 1].start_pos;
                let pos = view[i].start_pos;
                rule_matches.push(
                    Match::new(
                        WORD_REPEAT_RULE_ID,
                        Option::<String>::None,
                        WORD_REPEAT_MESSAGE,
                        Some(WORD_REPEAT_SHORT.to_string()),
                        TextRange::new(
                            sentence_offset + prev_pos,
                            sentence_offset + pos + prev_token.len(),
                        ),
                        vec![Suggestion {
                            value: prev_token.clone(),
                            short_description: None,
                        }],
                        "MISC",
                        "Błędy różne",
                    )
                    .with_metadata(WORD_REPEAT_DESCRIPTION, "duplication", 1),
                );
            }
            prev_token = token;
        }
        rule_matches
    }
}

impl Default for WordRepeatSentenceRule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PolishWordRepeatRule (`PL_WORD_REPEAT`, AdvancedWordRepeatRule, default off)
// ---------------------------------------------------------------------------

pub const POLISH_WORD_REPEAT_ID: &str = "PL_WORD_REPEAT";
pub const POLISH_WORD_REPEAT_DESCRIPTION: &str =
    "Powtórzenia wyrazów w zdaniu (monotonia stylistyczna)";
pub const POLISH_WORD_REPEAT_MESSAGE: &str = "Powtórzony wyraz w zdaniu";
pub const POLISH_WORD_REPEAT_SHORT: &str = "Powtórzenie wyrazu";

/// `PolishWordRepeatRule.EXC_WORDS`.
fn pl_exc_words() -> &'static HashSet<&'static str> {
    static SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        [
            "nie", "tuż", "aż", "to", "siebie", "być", "ani", "ni", "albo", "lub", "czy", "bądź",
            "jako", "zł", "np", "coraz", "bardzo", "bardziej", "proc", "ten", "jak", "mln", "tys",
            "swój", "mój", "twój", "nasz", "wasz", "i", "zbyt", "się",
        ]
        .into_iter()
        .collect()
    });
    &SET
}

/// `PolishWordRepeatRule.EXC_POS` = `prep:.*|ppron.*` (full match).
static PL_EXC_POS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:prep:.*|ppron.*)$").unwrap());
/// `PolishWordRepeatRule.EXC_NONWORDS` (full match).
static PL_EXC_NONWORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:&quot|&gt|&lt|&amp|[0-9].*|M*(D?C{0,3}|C[DM])(L?X{0,3}|X[LC])(V?I{0,3}|I[VX]))$",
    )
    .unwrap()
});

/// `PolishWordRepeatRule` (`AdvancedWordRepeatRule`, `tags` none, default
/// off): reports a word form repeated elsewhere in the same sentence.
pub struct PolishWordRepeatRule;

impl PolishWordRepeatRule {
    pub fn new() -> Self {
        Self
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut rule_matches = Vec::new();
        let mut repetition = false;
        let mut inflected: BTreeSet<String> = BTreeSet::new();
        let mut cur_token = 0usize;
        for (i, item) in view.iter().enumerate().skip(1) {
            let token = item.surface().to_string();
            let mut is_word = true;
            let mut has_lemma = true;
            if token.chars().count() < 2 {
                is_word = false;
            }
            for reading in &item.readings {
                match &reading.pos_tag {
                    Some(pos_tag) => {
                        if pos_tag.is_empty() {
                            is_word = false;
                            break;
                        }
                        let Some(lemma) = &reading.stem else {
                            has_lemma = false;
                            break;
                        };
                        if pl_exc_words().contains(lemma.as_str()) {
                            is_word = false;
                            break;
                        }
                        if PL_EXC_POS.is_match(pos_tag) {
                            is_word = false;
                            break;
                        }
                    }
                    None => has_lemma = false,
                }
            }
            if is_word && PL_EXC_NONWORDS.is_match(item.surface()) {
                is_word = false;
            }

            let mut prev_lemma = String::new();
            if is_word {
                let mut not_sent_end = false;
                for reading in &item.readings {
                    if let Some(pos) = &reading.pos_tag {
                        not_sent_end |= pos == "SENT_END";
                    }
                    if has_lemma {
                        let cur_lemma = reading.stem.clone().unwrap_or_default();
                        if prev_lemma != cur_lemma && !not_sent_end {
                            if inflected.contains(&cur_lemma) && cur_token != i {
                                repetition = true;
                            } else {
                                inflected.insert(cur_lemma.clone());
                                cur_token = i;
                            }
                        }
                        prev_lemma = cur_lemma;
                    } else {
                        let surface = item.surface().to_string();
                        if inflected.contains(&surface) && !not_sent_end {
                            repetition = true;
                        } else {
                            inflected.insert(surface);
                        }
                    }
                }
            }

            if repetition {
                rule_matches.push(
                    Match::new(
                        POLISH_WORD_REPEAT_ID,
                        Option::<String>::None,
                        POLISH_WORD_REPEAT_MESSAGE,
                        Some(POLISH_WORD_REPEAT_SHORT.to_string()),
                        TextRange::new(
                            sentence_offset + item.start_pos,
                            sentence_offset + item.end_pos(),
                        ),
                        Vec::<Suggestion>::new(),
                        "MISC",
                        "Błędy różne",
                    )
                    .with_metadata(POLISH_WORD_REPEAT_DESCRIPTION, "style", 0),
                );
                repetition = false;
            }
        }
        rule_matches
    }
}

impl Default for PolishWordRepeatRule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SimpleReplaceRule (`PL_SIMPLE_REPLACE`, AbstractSimpleReplaceRule v1)
// ---------------------------------------------------------------------------

pub const SIMPLE_REPLACE_ID: &str = "PL_SIMPLE_REPLACE";

/// `SimpleReplaceDataLoader.loadWords` (`wrong=right1|right2`, both sides may
/// contain `|`-separated forms).
fn load_legacy_words(path: &Path) -> Result<HashMap<String, Vec<String>>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut map = HashMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() != 2 || parts[1].trim().is_empty() {
            return Err(lt_core::CoreError::Data(format!(
                "could not load simple replacement data from {}: bad line '{line}'",
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

/// `pl.SimpleReplaceRule` (legacy `AbstractSimpleReplaceRule`,
/// `checkLemmas=false`, case-insensitive).
pub struct PolishSimpleReplaceRule {
    wrong_words: HashMap<String, Vec<String>>,
}

impl PolishSimpleReplaceRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            wrong_words: load_legacy_words(&data_dir.join("pl/rules/replace.txt"))?,
        })
    }

    pub fn rule_id(&self) -> &str {
        SIMPLE_REPLACE_ID
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
            let possible = self
                .wrong_words
                .get(&original)
                .or_else(|| self.wrong_words.get(&token_string))
                .cloned();
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
            if original.chars().next().is_some_and(char::is_uppercase) {
                for replacement in &mut replacements {
                    *replacement = lt_tagger::uppercase_first_char(replacement);
                }
            }
            let message = format!(
                "Wyraz „{original}” to najczęściej literówka; poprawnie pisze się: {}.",
                replacements.join(", ")
            );
            rule_matches.push(
                Match::new(
                    SIMPLE_REPLACE_ID,
                    Option::<String>::None,
                    message,
                    Some("Literówka".to_string()),
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
                    "PRAWDOPODOBNE_LITEROWKI",
                    "Prawdopodobne literówki",
                )
                .with_metadata(
                    "Typowe literówki i niepoprawne wyrazy (domowi, sie, niewiadomo, duh, cie…)",
                    "misspelling",
                    0,
                ),
            );
        }
        rule_matches
    }
}
