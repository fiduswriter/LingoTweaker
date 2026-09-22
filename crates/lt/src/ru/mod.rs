//! Russian pipeline parts: the `RussianTagger` (stress-mark normalization and
//! the `MayMissingYO` chunk tag) + `RussianWordTokenizer`, the
//! `RussianHybridDisambiguator` order (`ru/multiwords.txt` chunker → XML
//! disambiguation) and the post-disambiguation `RussianChunker`.
//! `Russian.getRelevantRules` adds the generic built-ins plus the language's
//! Java rule classes (ported in stage 3).

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod filters;
pub mod priorities;
pub mod spelling;

/// Russian pipeline parts.
pub struct RussianPipeline {
    pub tagger: Arc<lt_tagger::RussianTagger>,
    /// `Russian.createDefaultSynthesizer` (`/ru/russian_synth.dict`).
    pub synthesizer: Arc<lt_tagger::RussianSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<RussianSynthesizerAdapter>,
    /// `RussianHybridDisambiguator`'s first stage:
    /// `MultiWordChunker.getInstance("/ru/multiwords.txt")`.
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    /// `new XmlRuleDisambiguator(Russian.getInstance())` (no global rules).
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `Russian.createDefaultPostDisambiguationChunker`.
    pub post_chunker: lt_chunk::RussianChunker,
    /// `MorfologikRussianSpellerRule` (`MORFOLOGIK_RULE_RU_RU`); `None` only
    /// when the vendored `ru_RU` dictionary cannot be read.
    pub spelling: Option<Arc<crate::ru::spelling::RussianSpellingRule>>,
    /// `MorfologikRussianYOSpellerRule` (`MORFOLOGIK_RULE_RU_RU_YO`, default
    /// off).
    pub spelling_yo: Option<Arc<crate::ru::spelling::RussianYOSpellingRule>>,
}

impl RussianPipeline {
    /// `RussianHybridDisambiguator.disambiguate`: the multiword chunker first,
    /// then the XML disambiguator (used by `RussianPartialPosTagFilter`, which
    /// calls the language's default disambiguator without the
    /// post-disambiguation chunker).
    pub fn disambiguate_hybrid(&self, sentence: &mut AnalyzedSentence) {
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }

    /// Full pipeline disambiguation: hybrid + post-disambiguation chunker.
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguate_hybrid(sentence);
        self.post_chunker.add_chunk_tags(&mut sentence.tokens);
    }
}

/// Adapter exposing the Russian synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct RussianSynthesizerAdapter {
    pub synth: Arc<lt_tagger::RussianSynthesizer>,
    pub tagger: Arc<lt_tagger::RussianTagger>,
}

impl RussianSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::RussianSynthesizer {
        &self.synth
    }
}

impl Synthesizer for RussianSynthesizerAdapter {
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
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`.
        self.tagger.is_tagged_word(word)
    }
}

/// Russian sentence tokenization (`RussianWordTokenizer`) + `RussianTagger`.
pub fn analyze_russian_sentence(russian: &RussianPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::russian::RussianWordTokenizer::new().tokenize(text);
    let tagged = russian.tagger.tag(&raw_tokens);

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
