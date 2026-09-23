//! Khmer (`km`) pipeline parts. `Khmer` is a plain `Language` with a
//! `BaseTagger` (`KhmerTagger`) over the in-tree FSA5 `khmer.dict`, the
//! `KhmerWordTokenizer` (a delimiter `WordTokenizer` subclass) and the plain
//! `XmlRuleDisambiguator` (`km/disambiguation.xml` + global). There is no
//! synthesizer; the speller is `KhmerHunspellRule` (`isLatinScript() = false`).
//!
//! `Khmer.getRelevantRules` returns exactly five classes and does **not** call
//! `super`: `KhmerHunspellRule` (`HUNSPELL_RULE`), `KhmerSimpleReplaceRule`
//! (`KM_SIMPLE_REPLACE`), `KhmerWordRepeatRule` (`KM_WORD_REPEAT_RULE`),
//! `KhmerUnpairedBracketsRule` (`KM_UNPAIRED_BRACKETS`) and
//! `KhmerSpaceBeforeRule` (`KM_SPACE_BEFORE_CONJUNCTION`). The generic
//! built-ins are absent.

use std::sync::Arc;

use lt_core::{
    AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange,
};

pub mod spelling;

use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};
use crate::wordutil::eq_ignore_case;

/// `Khmer` pipeline parts.
pub struct KhmerPipeline {
    /// `KhmerTagger` (`BaseTagger` over `km/dictionaries/khmer.dict`).
    pub tagger: Arc<lt_tagger::KhmerTagger>,
    /// `Khmer.createDefaultDisambiguator` is a plain `XmlRuleDisambiguator`
    /// (XML rules + `core/disambiguation-global.xml`); Khmer has no chunker.
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `KhmerHunspellRule` (`HUNSPELL_RULE`). `None` only when the vendored
    /// `km_KH` dictionary cannot be read.
    pub spelling: Option<Arc<crate::km::spelling::KhmerSpellingRule>>,
    /// `KhmerSpaceBeforeRule` (`KM_SPACE_BEFORE_CONJUNCTION`).
    pub space_before: crate::space_before::SpaceBeforeRule,
}

impl KhmerPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}

/// `KhmerWordTokenizer`: the base `WordTokenizer.getTokenizingCharacters()`
/// whitespace block plus the Khmer signs `។` (`U+17D4`) / `៕` (`U+17D5`) and
/// the hand-written punctuation tail. Note this is **not** the full base
/// tokenizing set: the Java class overrides `tokenize` with its own delimiter
/// string, so characters such as `=`/`*`/`|` are *not* split.
pub fn tokenizing_characters() -> String {
    let mut chars = String::from(
        "\u{17D4}\u{17D5}\u{0020}\u{00A0}\u{115f}\u{1160}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{200B}\u{200c}\u{200d}\u{200e}\u{200f}\u{2028}\u{2029}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{202f}\u{205F}\u{2060}\u{2061}\u{2062}\u{2063}\u{206A}\u{206b}\u{206c}\u{206d}\u{206E}\u{206F}\u{3000}\u{3164}\u{feff}\u{ffa0}\u{fff9}\u{fffa}\u{fffb}",
    );
    chars.push_str(",.;()[]{}«»!?:\"'’‘„“”…\\/\t\n");
    chars
}

/// Tokenize + tag one sentence like Java's `Khmer`: the `KhmerWordTokenizer`
/// followed by the `KhmerTagger` (`BaseTagger`). The token stream concatenates
/// back to the original text exactly and the byte offsets accumulate.
pub fn analyze_khmer_sentence(khmer: &KhmerPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(text, &tokenizing_characters()),
    );
    let tagged = khmer.tagger.tag(&raw_tokens);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(tagged.len() + 1);
    tokens.push(AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(
            "",
            None,
            Some(crate::pipeline::sentence_start_tag().to_string()),
        )],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: true,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    });

    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for (raw, mut reading) in raw_tokens.iter().zip(tagged) {
        let is_whitespace = lt_core::is_whitespace(raw);
        reading.whitespace_before = prev_was_whitespace;
        reading.start_pos = byte_pos;
        reading.raw_byte_len = raw.len();
        reading.is_whitespace = is_whitespace;
        reading.is_tagged = reading.readings.iter().any(|r| r.pos_tag.is_some());
        tokens.push(reading);
        if !is_whitespace {
            last_non_ws_idx = Some(tokens.len() - 1);
        }
        byte_pos += raw.len();
        prev_was_whitespace = is_whitespace;
    }

    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("SENT_END"))
        {
            let surface = tr
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_default();
            let lemma = tr.readings.first().and_then(|r| r.stem.clone());
            tr.add_reading(AnalyzedToken::new(
                surface,
                lemma,
                Some(crate::pipeline::sentence_end_tag().to_string()),
            ));
        }
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        if !tr.is_sentence_end {
            tr.add_reading(AnalyzedToken::new(
                tr.surface().to_string(),
                tr.readings.first().and_then(|r| r.stem.clone()),
                Some(crate::pipeline::sentence_end_tag().to_string()),
            ));
            tr.is_sentence_end = true;
        }
        if tr.is_linebreak() {
            tr.set_paragraph_end();
        }
    }

    AnalyzedSentence {
        text: text.to_string(),
        offset: 0,
        tokens,
        pre_disambig_tokens: Vec::new(),
        pre_disambig_detached: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// KhmerSimpleReplaceRule (`KM_SIMPLE_REPLACE`)
// ---------------------------------------------------------------------------

const SIMPLE_REPLACE_CATEGORY_NAME: &str = "របស់ផ្សេងៗ";

/// `KhmerSimpleReplaceRule` (`KM_SIMPLE_REPLACE`): the `AbstractSimpleReplaceRule2`
/// instance over `/km/coherency.txt` (case-insensitive, category `MISC`,
/// `getSuggestionsSeparator() = " or "`).
#[allow(clippy::vec_init_then_push)]
pub fn simple_replace_instances(
    data_dir: &std::path::Path,
) -> lt_core::Result<Vec<SimpleReplaceRule>> {
    let mut instances = Vec::new();
    instances.push(SimpleReplaceRule::from_files(
        &[data_dir.join("km/rules/coherency.txt")],
        SimpleReplaceConfig {
            rule_id: "KM_SIMPLE_REPLACE",
            description: "Words or groups of words that are incorrect or obsolete",
            short: "Consider following the spelling of Chuon Nath",
            message: " Consider following the spelling of Chuon Nath ",
            suggestions_separator: " or ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: SIMPLE_REPLACE_CATEGORY_NAME,
            issue_type: "uncategorized",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    Ok(instances)
}

// ---------------------------------------------------------------------------
// KhmerWordRepeatRule (`KM_WORD_REPEAT_RULE`)
// ---------------------------------------------------------------------------

const WORD_REPEAT_ID: &str = "KM_WORD_REPEAT_RULE";
const WORD_REPEAT_DESCRIPTION: &str =
    "មាន\u{200b}\u{200b}វារេណវាក្យ\u{200b}ពាក្យ\u{200b} (ឧទាហរណ៍ «បាន\u{200b}បាន»)";
const WORD_REPEAT_MESSAGE: &str = "កំហុសវាយអក្សរ: អ្នកធ្វើឲ្យពាក្យជាន់គ្នា";
const WORD_REPEAT_SHORT: &str = "មាន\u{200b}\u{200b}វារេណវាក្យ\u{200b}ពាក្យ";

/// `KhmerWordRepeatRule.isWord`: a one-character token must be a letter.
fn is_word_km(token: &str) -> bool {
    if token.chars().count() == 1 {
        return token.chars().next().is_some_and(char::is_alphabetic);
    }
    true
}

/// `KhmerWordRepeatRule.ignore`: skip when the token immediately before the
/// current one in the *full* token stream (whitespace included) is a regular
/// space (`sentence.getOriginalPosition(position)` + `"\u0020"`).
fn ignore_km(tokens: &[AnalyzedTokenReadings], orig_pos: usize) -> bool {
    orig_pos >= 1 && tokens[orig_pos - 1].surface() == " "
}

/// `KhmerWordRepeatRule.match` over one sentence. Java iterates
/// `getTokensWithoutWhitespace()` (keeping `SENT_START`/`SENT_END`), reports
/// the range `prevPos … pos + prevToken.length()` and offers the three
/// replacements `X X`, `X` and `Xៗ`.
pub fn check_word_repeat_km(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
) -> Vec<Match> {
    let view: Vec<(usize, &AnalyzedTokenReadings)> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.is_whitespace)
        .collect();
    let mut rule_matches = Vec::new();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let (orig_pos, tr) = view[i];
        let token = tr.surface().to_string();
        if is_word_km(&token) && eq_ignore_case(&prev_token, &token) && !ignore_km(tokens, orig_pos)
        {
            let prev_pos = view[i - 1].1.start_pos;
            let pos = tr.start_pos;
            let replacements = vec![
                Suggestion {
                    value: format!("{prev_token} {token}"),
                    short_description: None,
                },
                Suggestion {
                    value: prev_token.clone(),
                    short_description: None,
                },
                Suggestion {
                    value: format!("{prev_token}ៗ"),
                    short_description: None,
                },
            ];
            rule_matches.push(
                Match::new(
                    WORD_REPEAT_ID,
                    Option::<String>::None,
                    WORD_REPEAT_MESSAGE,
                    Some(WORD_REPEAT_SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    replacements,
                    "MISC",
                    SIMPLE_REPLACE_CATEGORY_NAME,
                )
                .with_metadata(WORD_REPEAT_DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}
