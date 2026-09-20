//! Portuguese pipeline parts: tokenizer/tagger, the
//! `PortugueseHybridDisambiguator` chunker order and (incrementally) the
//! Java-coded Portuguese built-in rules.
//!
//! Stage 1 (XML rules) wires the plain `BaseTagger`/`BaseSynthesizer`
//! foundations over the vendored `portuguese*.dict` dictionaries and the
//! hybrid chunker-disambiguator (`core/spelling_global.txt` → `pt/multiwords.txt`
//! → XML rules). `PortugueseTagger`'s extra heuristics (number expressions,
//! `-mente` adverbs, prefixed verbs), `PortugueseWordTokenizer` and the
//! speller follow in stages 2/3.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod accentuation;
pub mod date_filters;
pub mod filters;
pub mod legacy_simple_replace;
pub mod priorities;
pub mod rules;
pub mod simple_replace;
pub mod spelling;

/// `PortugueseHybridDisambiguator`: global chunker → `pt/multiwords.txt`
/// chunker (`removePreviousTags`) → XML rules (+ global rules).
pub struct PortuguesePipeline {
    /// Long code of the requested variant (`pt-PT` default, `pt-BR`,
    /// `pt-AO`, `pt-MZ`); selects the core built-in message bundle.
    pub variant: String,
    pub tagger: Arc<lt_tagger::PortugueseTagger>,
    pub synthesizer: Arc<lt_tagger::PortugueseSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<PortugueseSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/spelling_global.txt", false, true,
    /// true, "NPCN000")` with `setIgnoreSpelling(true)`
    pub global_chunker: lt_disambig::MultiWordChunker,
    /// `MultiWordChunker.getInstance("/pt/multiwords.txt", true, true, true)`
    /// with `setRemovePreviousTags(true)` and `setIgnoreSpelling(true)`
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikPortugueseSpellerRule` (`MORFOLOGIK_RULE_PT_*`)
    pub spelling: Arc<crate::pt::spelling::PortugueseSpellingRule>,
    /// `PortugueseMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// Legacy `AbstractSimpleReplaceRule` instances (stage 3d).
    pub legacy_replace: Vec<crate::pt::legacy_simple_replace::LegacyReplaceRule>,
    /// `AbstractSimpleReplaceRule2` instances (stage 3d).
    pub replace_rule2: Vec<crate::simple_replace::SimpleReplaceRule>,
    /// The merged execution order of the replace family plus
    /// `DoublePunctuationRule` (Java's `getRelevantRules` interleaves the two
    /// families; ties in the overlap filter depend on this order).
    pub replace_order: Vec<ReplaceInstance>,
    /// `AbstractCompoundRule` instances in Java's rule-list order: the common
    /// `PT_COMPOUNDS_POST_REFORM` (16) + `PT_COLOUR_HYPHENATION` (17), then
    /// the variant additions (pt-PT/pt-BR repeat the post-reform rule;
    /// pt-AO/pt-MZ add `PT_COMPOUNDS_PRE_REFORM`).
    pub compounds: Vec<crate::compound::CompoundRule>,
    /// `AbstractDashRule` instances (`PT_POSAO_DASH_RULE` for pt-PT/pt-BR,
    /// `PT_PREAO_DASH_RULE` for pt-AO/pt-MZ).
    pub dashes: Vec<crate::dash::DashRule>,
    /// `PortugueseWrongWordInContextRule` (`PORTUGUESE_WRONG_WORD_IN_CONTEXT`)
    pub wrong_word_in_context: Option<crate::wrong_word_in_context::WrongWordInContextRule>,
    /// `PortugueseWordCoherencyRule` (`PT_WORD_COHERENCY`, text level)
    pub word_coherency: crate::word_coherency::WordCoherencyRule,
    /// `PortugueseAccentuationCheckRule` (`ACCENTUATION_CHECK_PT`, default
    /// off)
    pub accentuation: crate::pt::accentuation::AccentuationCheckRule,
    /// `PortugueseReadabilityRule` difficult + simple (text level, both
    /// default off)
    pub readability: Vec<crate::readability::ReadabilityRule>,
}

impl PortuguesePipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.global_chunker.apply(sentence);
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// One entry of the merged replace-family execution order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplaceInstance {
    Rule2(usize),
    Legacy(usize),
    /// `DoublePunctuationRule` (26), between wordiness (25) and wikipedia (27)
    DoublePunctuation,
}

/// Java's rule-list order for the Portuguese replace family (common list
/// first, then the variant additions, `EnglishContractionSpellingRule` last):
/// orthography (18) → replace (19) → Rule2 barbarisms/cliche/redundancy/
/// wordiness (20/22/24/25) → double punctuation (26) → wikipedia (27) →
/// diacritics (31) → pt-PT agreement (64) → variant replace (65) → variant
/// barbarisms/archaisms/cliche/redundancy/wordiness/wikipedia (66–71) →
/// English contractions (174).
pub fn replace_order(
    rule2: &[crate::simple_replace::SimpleReplaceRule],
    legacy: &[crate::pt::legacy_simple_replace::LegacyReplaceRule],
    variant: &str,
) -> Vec<ReplaceInstance> {
    let has_variant = variant == "pt-PT" || variant == "pt-BR";
    debug_assert!(rule2.len() >= 6, "rule2 vec too short: {}", rule2.len());
    debug_assert_eq!(
        rule2.len(),
        if has_variant { 13 } else { 6 },
        "unexpected Rule2 instance count for {variant}"
    );
    let english_index = legacy
        .iter()
        .position(|r| r.rule_id() == "PT_ENGLISH_CONTRACTION_ORTHOGRAPHY")
        .unwrap_or(legacy.len().saturating_sub(1));
    let agreement_index = legacy
        .iter()
        .position(|r| r.rule_id() == "PT_AGREEMENT_REPLACE");
    let mut order = Vec::with_capacity(rule2.len() + legacy.len() + 1);
    order.push(ReplaceInstance::Legacy(0)); // orthography (18)
    order.push(ReplaceInstance::Legacy(1)); // replace (19)
    for i in 0..6 {
        // barbarisms (20), cliche (22), redundancy (24), wordiness (25),
        // wikipedia (27), diacritics (31)
        if i == 3 {
            order.push(ReplaceInstance::DoublePunctuation); // (26)
        }
        order.push(ReplaceInstance::Rule2(i));
    }
    if let Some(agreement_index) = agreement_index {
        order.push(ReplaceInstance::Legacy(agreement_index)); // (64)
    }
    if has_variant {
        for i in 6..rule2.len() {
            order.push(ReplaceInstance::Rule2(i)); // (65)–(71)
        }
    }
    order.push(ReplaceInstance::Legacy(english_index)); // (174)
    order
}

/// Adapter exposing the Portuguese synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct PortugueseSynthesizerAdapter {
    pub synth: Arc<lt_tagger::PortugueseSynthesizer>,
    pub tagger: Arc<lt_tagger::PortugueseTagger>,
}

impl PortugueseSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::PortugueseSynthesizer {
        &self.synth
    }
}

impl Synthesizer for PortugueseSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        self.synth.target_pos_tag(pos_tags, fallback)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`. Java
        // calls `lang.getTagger().tag(list)`, i.e. the full tagger with the
        // Portuguese heuristics (ordinals, percent/degree expressions,
        // `-mente` adverbs); `tag_word` alone is a plain dictionary lookup
        // and wrongly marks synthesized ordinals like "3ª" as mistakes.
        let tagged = self.tagger.tag(std::slice::from_ref(&word.to_string()));
        tagged
            .first()
            .and_then(|tr| tr.readings.first())
            .is_some_and(|r| r.stem.is_some() || r.pos_tag.is_some())
    }
}

/// Portuguese sentence tokenization + tagger.
pub fn analyze_portuguese_sentence(
    portuguese: &PortuguesePipeline,
    text: &str,
) -> AnalyzedSentence {
    let is_tagged = |word: &str| portuguese.tagger.is_tagged_word(word);
    let raw_tokens = lt_tokenize::PortugueseWordTokenizer::new(&is_tagged).tokenize(text);
    let tagged = portuguese.tagger.tag(&raw_tokens);

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
        // `BaseTagger` never sets the typographic-apostrophe flag (only the
        // en/es/ca taggers do)
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
    }
}
