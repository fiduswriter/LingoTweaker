//! Serbian (`sr`, default variant `sr-RS`/ekavian) pipeline parts: the plain
//! `EkavianTagger`/`EkavianSynthesizer` over the vendored Morfologik
//! dictionaries, the `SerbianHybridDisambiguator` order (`sr/multiwords.txt`
//! chunker → `sr/disambiguation.xml`) and the XML rules.
//!
//! Stage 1 wires the XML rules, the tagger/synthesizer foundation and the
//! hybrid disambiguator order. The `MorfologikEkavianSpellerRule` and the two
//! ekavian replace rules follow in stages 2/3.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

/// `Serbian.createDefaultDisambiguator` is `SerbianHybridDisambiguator`:
/// `MultiWordChunker("/sr/multiwords.txt")` then
/// `new XmlRuleDisambiguator(new Serbian())` (no global rules). The
/// `multiwords.txt` list is empty upstream, but the chunker is still loaded.
pub struct SerbianPipeline {
    pub tagger: Arc<lt_tagger::EkavianTagger>,
    pub synthesizer: Arc<lt_tagger::EkavianSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<SerbianSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/sr/multiwords.txt")`.
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    /// `new XmlRuleDisambiguator(new Serbian())` (no global rules).
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `WordRepeatRule` (`WORD_REPEAT_RULE`, rule 7), the generic built-in
    /// with the `MessagesBundle_sr` strings.
    pub word_repeat: crate::word_repeat::WordRepeatRule,
}

impl SerbianPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// `WordRepeatRule` with the `MessagesBundle_sr` strings.
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Понављање речи (на пр. „хоћу хоћу“)",
        message: "Могућа грешка: поновили сте реч",
        short_message: "Понављање речи",
        category_name: "Разно",
    })
}

/// Adapter exposing the Serbian synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct SerbianSynthesizerAdapter {
    pub synth: Arc<lt_tagger::EkavianSynthesizer>,
    pub tagger: Arc<lt_tagger::EkavianTagger>,
}

impl SerbianSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::EkavianSynthesizer {
        &self.synth
    }
}

impl Synthesizer for SerbianSynthesizerAdapter {
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

/// Serbian sentence tokenization + tagger. `Serbian` overrides no word
/// tokenizer, so the default base `WordTokenizer` applies and the token stream
/// concatenates back to the original text (byte offsets accumulate).
pub fn analyze_serbian_sentence(serbian: &SerbianPipeline, text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(
            text,
            &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
        ),
    );
    let tagged = serbian.tagger.tag(&raw_tokens);

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
