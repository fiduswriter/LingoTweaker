//! Malayalam (`ml`) pipeline parts. `Malayalam` is a plain (upstream-deprecated)
//! `Language` with a `BaseTagger` (`MalayalamTagger`) over the in-tree FSA5
//! `malayalam.dict`, the `MalayalamWordTokenizer` (a small delimiter
//! `StringTokenizer` set) and the base no-op disambiguator (there is no
//! `ml/disambiguation.xml`). There is no synthesizer.
//!
//! `Malayalam.getRelevantRules` returns exactly seven classes and does **not**
//! call `super`: the generic `CommaWhitespaceRule`, `DoublePunctuationRule`,
//! `GenericUnpairedBracketsRule`, `UppercaseSentenceStartRule`,
//! `WordRepeatRule` and `MultipleWhitespaceRule` plus the Morfologik speller
//! `MorfologikMalayalamSpellerRule` (`MORFOLOGIK_RULE_ML_IN`). There are no
//! ml-specific rule classes and no XML `<filter>`.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

pub mod spelling;

/// `Malayalam` pipeline parts.
pub struct MalayalamPipeline {
    /// `MalayalamTagger` (`BaseTagger` over `ml/dictionaries/malayalam.dict`).
    pub tagger: Arc<lt_tagger::MalayalamTagger>,
    /// `MorfologikMalayalamSpellerRule` (`MORFOLOGIK_RULE_ML_IN`). `None` only
    /// when the vendored `ml_IN` dictionary cannot be read.
    pub spelling: Option<Arc<crate::ml::spelling::MalayalamSpellingRule>>,
    /// `WordRepeatRule` (`WORD_REPEAT_RULE`, base English strings).
    pub word_repeat: crate::word_repeat::WordRepeatRule,
}

/// `MalayalamWordTokenizer`: the explicit `StringTokenizer` delimiter set
/// (whitespace block plus the hand-written punctuation tail). Note this is
/// **not** the base `WordTokenizer.getTokenizingCharacters()` set: the Java
/// class overrides `tokenize` with its own string, so characters such as
/// `=`/`*`/`|` are *not* split. Unlike `KhmerWordTokenizer`, the Java method
/// does **not** call `joinEMailsAndUrls`.
pub fn tokenizing_characters() -> String {
    String::from("\u{0020}\u{00A0}\u{115f}\u{1160}\u{1680},.;()[]{}!?:\"'’‘„“”…\\/\t\n")
}

/// Tokenize + tag one sentence like Java's `Malayalam`: the
/// `MalayalamWordTokenizer` followed by the `MalayalamTagger` (`BaseTagger`).
/// The token stream concatenates back to the original text exactly and the
/// byte offsets accumulate.
pub fn analyze_malayalam_sentence(ml: &MalayalamPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::string_tokenize(text, &tokenizing_characters());
    let tagged = ml.tagger.tag(&raw_tokens);

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

/// `WordRepeatRule` (`WORD_REPEAT_RULE`) with the base English strings: the
/// generic base class used by `Malayalam.getRelevantRules` (there is no
/// `MessagesBundle_ml`, so Java falls back to the base bundle).
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Word repetition (e.g. 'will will')",
        message: "Possible typo: you repeated a word.",
        short_message: "Word repetition",
        category_name: "Miscellaneous",
    })
}
