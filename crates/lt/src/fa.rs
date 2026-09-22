//! Persian (`fa`) pipeline parts. `Persian` is a plain `Language`: it does not
//! override `createDefaultTagger` (so the base `DemoTagger` gives every token
//! a single null-POS reading), `createDefaultDisambiguator` (the base
//! `DemoDisambiguator`, a no-op), `createDefaultSynthesizer` (none) or
//! `createDefaultSpellingRule` (none). The analyzed sentence is therefore the
//! surface tokenization only, like the hand-authored languages.
//!
//! `Persian.getRelevantRules` adds five Persian-specific classes on top of the
//! generic built-ins:
//! `PERSIAN_WORD_REPEAT_BEGINNING_RULE`, `FA_WORD_COHERENCY`,
//! `PERSIAN_WORD_REPEAT_RULE`, `FA_SIMPLE_REPLACE` and
//! `FA_SPACE_BEFORE_CONJUNCTION`; the two punctuation subclasses reuse the
//! shared `comma_whitespace`/`double_punctuation` machinery.

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

/// `Persian` pipeline parts.
pub struct PersianPipeline {
    /// `PersianSpaceBeforeRule` (`FA_SPACE_BEFORE_CONJUNCTION`, default off).
    pub space_before: crate::space_before::SpaceBeforeRule,
    /// `SimpleReplaceRule` (`FA_SIMPLE_REPLACE`).
    pub simple_replace: PersianSimpleReplaceRule,
    /// `WordCoherencyRule` (`FA_WORD_COHERENCY`).
    pub word_coherency: crate::word_coherency::WordCoherencyRule,
}

/// `PersianWordTokenizer`: `WordTokenizer.getTokenizingCharacters()` plus the
/// Arabic comma `،` (`U+060C`), question mark `؟` (`U+061F`) and semicolon `؛`
/// (`U+061B`).
pub fn tokenizing_characters() -> String {
    let mut chars = lt_tokenize::wordtokenizer::base_tokenizing_characters();
    chars.push_str("\u{060C}\u{061F}\u{061B}");
    chars
}

/// Tokenize + tag one sentence like Java's `Persian`: the `PersianWordTokenizer`
/// followed by the `DemoTagger` (a single null-POS reading per token).
pub fn analyze_persian_sentence(text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(text, &tokenizing_characters()),
    );
    crate::pipeline::surface_sentence_from_tokens(text, raw_tokens)
}

// ---------------------------------------------------------------------------
// SimpleReplaceRule (`FA_SIMPLE_REPLACE`)
// ---------------------------------------------------------------------------

const SIMPLE_REPLACE_ID: &str = "FA_SIMPLE_REPLACE";
const SIMPLE_REPLACE_DESCRIPTION: &str = "اشتباه محتمل املائی";
const SIMPLE_REPLACE_MESSAGE_PREFIX: &str = "اشتباه محتمل املائی پیداشده: ";

/// Port of the older `AbstractSimpleReplaceRule` used by
/// `fa.SimpleReplaceRule` (`FA_SIMPLE_REPLACE`): a per-token, case-insensitive
/// replacement lookup over `fa/rules/replace.txt` (the newer
/// `AbstractSimpleReplaceRule2` in `crate::simple_replace` differs in the
/// multi-token scan and the `$suggestions` message markup, so it is not
/// reused). Persian has no synthesizer, so the lemma/synthesis path of the
/// Java rule is inert.
pub struct PersianSimpleReplaceRule {
    /// wrong form -> replacements (in file order)
    wrong_words: HashMap<String, Vec<String>>,
}

impl PersianSimpleReplaceRule {
    pub fn load(data_dir: &Path) -> Self {
        let text =
            lt_data::fs::read_to_string(data_dir.join("fa/rules/replace.txt")).unwrap_or_default();
        let mut wrong_words: HashMap<String, Vec<String>> = HashMap::new();
        for line in text.lines() {
            // `SimpleReplaceDataLoader`: skip empty and `#` lines; `split("=")`
            // must yield exactly two parts; the right side may not be empty.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() != 2 || parts[1].trim().is_empty() {
                continue;
            }
            let replacements: Vec<String> = parts[1].split('|').map(|s| s.to_string()).collect();
            for wrong_form in parts[0].split('|') {
                wrong_words.insert(wrong_form.to_string(), replacements.clone());
            }
        }
        Self { wrong_words }
    }

    /// `AbstractSimpleReplaceRule.match` over one sentence
    /// (`isCaseSensitive=false`, so the token is lowercased for the second
    /// lookup; the replacement compares against the original token).
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
        for token_readings in view {
            if token_readings.is_sentence_start
                || token_readings.is_immunized
                || token_readings.is_ignore_spelling
            {
                continue;
            }
            let original = token_readings.surface();
            if original.is_empty() {
                continue;
            }
            let lowered = original.to_lowercase();
            let Some(mut replacements) = self
                .wrong_words
                .get(original)
                .or_else(|| self.wrong_words.get(&lowered))
                .cloned()
            else {
                continue;
            };
            if lt_spell::morfologik::is_all_uppercase(original) {
                replacements = replacements.iter().map(|s| s.to_uppercase()).collect();
            }
            if let Some(pos) = replacements.iter().position(|r| r == original) {
                replacements.remove(pos);
            }
            if replacements.is_empty() {
                continue;
            }
            let message = format!(
                "{SIMPLE_REPLACE_MESSAGE_PREFIX}{}.",
                replacements.join("، ")
            );
            let start = sentence_offset + token_readings.start_pos;
            let end = start + original.len();
            rule_matches.push(
                Match::new(
                    SIMPLE_REPLACE_ID,
                    Option::<String>::None,
                    message,
                    Some(SIMPLE_REPLACE_DESCRIPTION.to_string()),
                    TextRange::new(start, end),
                    replacements
                        .into_iter()
                        .map(|value| Suggestion {
                            value,
                            short_description: None,
                        })
                        .collect(),
                    "CONFUSED_WORDS",
                    "Commonly confused words",
                )
                .with_metadata(SIMPLE_REPLACE_DESCRIPTION, "misspelling", 0)
                .with_match_type("Other"),
            );
        }
        rule_matches
    }
}

// ---------------------------------------------------------------------------
// PersianWordRepeatBeginningRule (`PERSIAN_WORD_REPEAT_BEGINNING_RULE`)
// ---------------------------------------------------------------------------

const WORD_REPEAT_BEGINNING_ID: &str = "PERSIAN_WORD_REPEAT_BEGINNING_RULE";
const WRB_DESCRIPTION: &str = "جملهٔ  بعدی هم  با کلمه‌ای مشابه شروع شده‌است";
const WRB_SHORT_ADV: &str = "دو جملهٔ پست سر هم با حرف ربط مشابه شروع شده‌اند";
const WRB_SHORT_WORD: &str = "سه جملهٔ پشت سر هم با کلمه‌ای مشابه شروع شده‌اند";
const WRB_THESAURUS: &str = "Consider rewording the sentence or use a thesaurus to find a synonym.";

/// `PersianWordRepeatBeginningRule.ADVERBS` (`isAdverb` override).
const ADVERBS: [&str; 9] = [
    "هم",
    "همچنین",
    "نیز",
    "از یک سو",
    "از یک طرف",
    "از طرف ديگر",
    "بنابراین",
    "حتی",
    "چنانچه",
];

fn wrb_is_exception(token: &str) -> bool {
    matches!(token, ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️")
}

/// `prevSentence.getText().trim().matches(".+[.?!]$")`.
fn trimmed_ends_like_sentence(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() > 1
        && trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '?' | '!'))
}

/// `PersianWordRepeatBeginningRule.match` over all sentences (text level).
/// Persian overrides only `isAdverb`; the base `getSuggestions` is empty.
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            token = tokens[1].surface().to_string();
            if tokens.len() > 3 {
                let mut is_word = true;
                if token.chars().count() == 1 {
                    is_word = token.chars().next().is_some_and(char::is_alphabetic);
                }
                if is_word
                    && last_token == token
                    && !wrb_is_exception(&token)
                    && !wrb_is_exception(tokens[2].surface())
                    && !wrb_is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(|p| trimmed_ends_like_sentence(&p.text))
                {
                    let short_msg = if ADVERBS.contains(&token.as_str()) {
                        Some(WRB_SHORT_ADV)
                    } else if before_last_token == token {
                        Some(WRB_SHORT_WORD)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {WRB_THESAURUS}");
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        rule_matches.push(
                            Match::new(
                                WORD_REPEAT_BEGINNING_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                Vec::<Suggestion>::new(),
                                "REPETITIONS_STYLE",
                                "Repetitions (Style)",
                            )
                            .with_metadata(WRB_DESCRIPTION, "style", 0),
                        );
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}
