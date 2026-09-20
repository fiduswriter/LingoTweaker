//! Spanish pipeline parts: tokenizer/tagger, the
//! `SpanishHybridDisambiguator` chunker order and (incrementally) the
//! Java-coded Spanish built-in rules.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_tokenize::SpanishWordTokenizer;

/// `SpanishHybridDisambiguator`: spelling_global chunker → `es/multiwords.txt`
/// chunker (removePreviousTags) → XML rules (+ global rules).
pub struct SpanishPipeline {
    pub tagger: Arc<lt_tagger::SpanishTagger>,
    pub synthesizer: Arc<lt_tagger::SpanishSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<crate::es::synthesizer::SpanishSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/spelling_global.txt", false, true,
    /// false, "NPCN000")`
    pub global_chunker: lt_disambig::MultiWordChunker,
    /// `MultiWordChunker.getInstance("/es/multiwords.txt", true, true, false)`
    /// with `setRemovePreviousTags(true)`
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikSpanishSpellerRule` (`MORFOLOGIK_RULE_ES`, rule 5)
    pub spelling: Arc<crate::es::spelling::SpanishSpellingRule>,
    /// `es.SimpleReplaceRule` (13, default on)
    pub simple_replace: crate::es::simple_replace::SpanishSimpleReplaceRule,
    /// `es.SimpleReplaceVerbsRule` (14, default on)
    pub simple_replace_verbs: crate::es::simple_replace::SpanishSimpleReplaceVerbsRule,
    /// `SpanishWrongWordInContextRule` (10, default on)
    pub wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule,
    /// `es.CompoundRule` (`ES_COMPOUNDS`, 16, default on)
    pub compound: crate::compound::CompoundRule,
    /// `SpanishRepeatedWordsRule` (`ES_REPEATEDWORDS`, 17, `tags="picky"`)
    pub repeated_words: crate::repeated_words::RepeatedWordsRule,
    /// `SpanishMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
}

impl SpanishPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.global_chunker.apply(sentence);
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// Spanish sentence tokenization + tagger. The Spanish tokenizer consults the
/// tagger for hyphenated words; the tagger needs the whole token list.
pub fn analyze_spanish_sentence(spanish: &SpanishPipeline, text: &str) -> AnalyzedSentence {
    let is_tagged = |w: &str| spanish.tagger.is_tagged_word(w);
    let tokenizer = SpanishWordTokenizer::new(&is_tagged);
    let raw_tokens = tokenizer.tokenize(text);
    let tagged = spanish.tagger.tag(&raw_tokens);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(tagged.len() + 1);
    // synthetic sentence-start entry (LT: AnalyzedToken("", "SENT_START", null))
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
        // Java `AnalyzedTokenReadings.isTagged()` counts SENT_START as a real
        // POS tag (only SENT_END/PARA_END/null are "no real tag")
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
        let has_typographic_apostrophe = raw.chars().count() > 1 && raw.contains('’');
        reading.has_typographic_apostrophe = has_typographic_apostrophe;
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
