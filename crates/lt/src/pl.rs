//! Polish pipeline parts: tokenizer/tagger, the `PolishHybridDisambiguator`
//! order and (incrementally) the Java-coded Polish built-in rules.
//!
//! Stage 1 (XML rules) wires the `PolishTagger`/`PolishSynthesizer` over the
//! vendored PoliMorf 2.0 dictionaries and the hybrid chunker-disambiguator
//! (`XML rules` → `pl/multiwords.txt` chunker). The morfologik speller and
//! the Java rule classes follow in stages 2/3.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod filters;
pub mod rules;
pub mod spelling;

/// `PolishHybridDisambiguator` calls
/// `chunker.disambiguate(disambiguator.disambiguate(input))`: the XML rule
/// disambiguator runs first, then the `pl/multiwords.txt` chunker.
pub struct PolishPipeline {
    pub tagger: Arc<lt_tagger::PolishTagger>,
    pub synthesizer: Arc<lt_tagger::PolishSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<PolishSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/pl/multiwords.txt")`
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `WordRepeatRule` (3), the generic built-in over the Polish tokens.
    pub word_repeat: crate::pl::rules::WordRepeatSentenceRule,
    /// `MorfologikPolishSpellerRule` (`MORFOLOGIK_RULE_PL_PL`, rule 7).
    pub spelling: Option<Arc<crate::pl::spelling::PolishSpellingRule>>,
}

impl PolishPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
        self.multiwords_chunker.apply(sentence);
    }
}

/// Adapter exposing the Polish synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct PolishSynthesizerAdapter {
    pub synth: Arc<lt_tagger::PolishSynthesizer>,
    pub tagger: Arc<lt_tagger::PolishTagger>,
}

impl PolishSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::PolishSynthesizer {
        &self.synth
    }
}

impl Synthesizer for PolishSynthesizerAdapter {
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

    fn pos_tag_correction(&self, pos_tag: &str) -> String {
        self.synth.pos_tag_correction(pos_tag)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`, over the
        // full PolishTagger (heuristics included).
        let tagged = self.tagger.tag(std::slice::from_ref(&word.to_string()));
        tagged
            .first()
            .and_then(|tr| tr.readings.first())
            .is_some_and(|r| r.stem.is_some() || r.pos_tag.is_some())
    }
}

/// Polish sentence tokenization + tagger. `PolishWordTokenizer` uses the
/// tagger for its hybrid hyphen splitting, then the tagger produces the
/// readings and the byte offsets accumulate.
pub fn analyze_polish_sentence(polish: &PolishPipeline, text: &str) -> AnalyzedSentence {
    let tagger = Arc::clone(&polish.tagger);
    let raw_tokens = lt_tokenize::PolishWordTokenizer::new()
        .tokenize_with(text, Some(&|words: &[String]| tagger.tag(words)));
    let tagged = polish.tagger.tag(&raw_tokens);

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
