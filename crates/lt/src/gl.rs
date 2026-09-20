//! Galician pipeline parts: tokenizer/tagger, the
//! `GalicianHybridDisambiguator` order and (incrementally) the Java-coded
//! Galician built-in rules.
//!
//! Stage 1 (XML rules) wires the `GalicianTagger`/`GalicianSynthesizer` over
//! the vendored Freeling/Apertium-derived dictionaries and the hybrid
//! chunker-disambiguator (`gl/multiwords.txt` → XML rules + global rules).
//! The hunspell speller and the Java rule classes follow in stages 2/3.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod filters;
pub mod priorities;
pub mod rules;
pub mod spelling;

/// `GalicianHybridDisambiguator`: `gl/multiwords.txt` chunker → XML rules
/// (+ global rules).
pub struct GalicianPipeline {
    pub tagger: Arc<lt_tagger::GalicianTagger>,
    pub synthesizer: Arc<lt_tagger::GalicianSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<GalicianSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/gl/multiwords.txt")`
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `HunspellRule` (`HUNSPELL_RULE`, rule 4). `None` when the vendored
    /// `gl_ES` dictionary cannot be parsed by the in-tree checker (it uses
    /// `FLAG num`, which `lt-spell` does not support yet) — the rest of the
    /// Galician engine still loads.
    pub spelling: Option<Arc<crate::gl::spelling::GalicianSpellingRule>>,
    /// Legacy `AbstractSimpleReplaceRule` instances (15–16).
    pub legacy_replace: Vec<crate::gl::rules::LegacyReplaceRule>,
    /// `AbstractSimpleReplaceRule2` instances (17–20).
    pub rule2: Vec<crate::simple_replace::SimpleReplaceRule>,
}

impl GalicianPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// Adapter exposing the Galician synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct GalicianSynthesizerAdapter {
    pub synth: Arc<lt_tagger::GalicianSynthesizer>,
    pub tagger: Arc<lt_tagger::GalicianTagger>,
}

impl GalicianSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::GalicianSynthesizer {
        &self.synth
    }
}

impl Synthesizer for GalicianSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synth.synthesize_plain(token, pos_tag)
    }

    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        self.synth.target_pos_tag(pos_tags, fallback)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`, over the
        // full GalicianTagger (heuristics included).
        let tagged = self.tagger.tag(std::slice::from_ref(&word.to_string()));
        tagged
            .first()
            .and_then(|tr| tr.readings.first())
            .is_some_and(|r| r.stem.is_some() || r.pos_tag.is_some())
    }
}

/// Galician sentence tokenization + tagger. `GalicianWordTokenizer` strips a
/// typewriter apostrophe onto the token surface and the tagger applies the
/// `GalicianTagger` heuristics (`-mente` adverbs, `auto`/`re` prefixed verbs).
pub fn analyze_galician_sentence(galician: &GalicianPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::GalicianWordTokenizer::new().tokenize(text);
    let tagged = galician.tagger.tag(&raw_tokens);

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

    // Map every token back onto the original text by characters, since the
    // tokenizer substitutes sentinels one char for one char; fall back to the
    // sequential byte position when a token does not fit (date collapse).
    let char_bytes: Vec<(usize, usize)> = {
        let mut ranges = Vec::with_capacity(text.len());
        let mut idx = 0usize;
        for ch in text.chars() {
            ranges.push((idx, idx + ch.len_utf8()));
            idx += ch.len_utf8();
        }
        ranges
    };
    let mut char_cursor = 0usize;
    let mut use_cursor = true;
    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for (raw, mut reading) in raw_tokens.iter().zip(tagged) {
        let is_whitespace = lt_core::is_whitespace(raw);
        reading.whitespace_before = prev_was_whitespace;
        let n_chars = raw.chars().count();
        let (start_pos, raw_byte_len) =
            if use_cursor && n_chars > 0 && char_cursor + n_chars <= char_bytes.len() {
                let start = char_bytes[char_cursor].0;
                let end = char_bytes[char_cursor + n_chars - 1].1;
                char_cursor += n_chars;
                (start, end - start)
            } else {
                use_cursor = false;
                (byte_pos, raw.len())
            };
        reading.start_pos = start_pos;
        reading.raw_byte_len = raw_byte_len;
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
