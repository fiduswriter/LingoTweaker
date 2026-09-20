//! Catalan pipeline parts: tokenizer/tagger, the `CatalanHybridDisambiguator`
//! chunker order and (incrementally) the Java-coded Catalan built-in rules.
//!
//! Stage 1 (XML rules) wires the plain `BaseTagger`/`BaseSynthesizer`
//! foundations over the vendored `ca-ES*.dict` dictionaries and the hybrid
//! chunker-disambiguator (`core/spelling_global.txt` with the `NPCN000`
//! default tag → `ca/multiwords.txt` with `removePreviousTags` → XML rules).
//! `CatalanTagger`'s heuristics, `CatalanWordTokenizer`, the speller,
//! `CatalanMultitokenDisambiguator` and the Java rule classes follow in
//! stages 2/3 (internal development notes).

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::Synthesizer;

pub mod adapt;
pub mod date_filters;
pub mod diacritics;
pub mod dnv_replace;
pub mod donarseli;
pub mod filters;
pub mod gender_number;
pub mod helpers;
pub mod legacy_simple_replace;
pub mod match_filter;
pub mod multitoken_disambig;
pub mod postponed;
pub mod priorities;
pub mod que_inici;
pub mod remote;
pub mod rules;
pub mod simple_replace;
pub mod small_filters;
pub mod spelling;
pub mod synth_filters;
pub mod verb_filters;

/// `CatalanHybridDisambiguator`: global chunker (`NPCN000`) →
/// `ca/multiwords.txt` chunker (`removePreviousTags`) → XML rules (+ global
/// rules).
pub struct CatalanPipeline {
    /// Long code of the requested variant (`ca-ES` default, `ca-ES-balear`,
    /// `ca-ES-valencia`); selects the variant rule directory and the
    /// Valencian/Balearic message-variant behavior.
    pub variant: String,
    pub tagger: Arc<lt_tagger::CatalanTagger>,
    pub synthesizer: Arc<lt_tagger::CatalanSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<CatalanSynthesizerAdapter>,
    /// `MultiWordChunker.getInstance("/spelling_global.txt", false, true,
    /// false, "NPCN000")`
    pub global_chunker: lt_disambig::MultiWordChunker,
    /// `MultiWordChunker.getInstance("/ca/multiwords.txt", true, true,
    /// false)` with `setRemovePreviousTags(true)`
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikCatalanSpellerRule` (`MORFOLOGIK_RULE_CA_ES`, rule 9)
    pub spelling: Arc<crate::ca::spelling::CatalanSpellingRule>,
    /// `CatalanMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// `CatalanMorfologikMultitokenSpeller.getSpeller` for
    /// `CatalanMultitokenDisambiguator`.
    pub multitoken_dict_speller: Option<Arc<lt_spell::morfologik::MorfologikSpeller>>,
    /// `CatalanWordCoherencyRule` (29) over `ca/rules/coherency.txt`.
    pub word_coherency: crate::word_coherency::WordCoherencyRule,
    /// `WordCoherencyValencianRule` (ca-ES-valencia addition).
    pub word_coherency_valencia: Option<crate::word_coherency::WordCoherencyRule>,
    /// `CatalanWrongWordInContextRule` (12).
    pub wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule,
    /// Legacy `AbstractSimpleReplaceRule` family (13–18, 22).
    pub legacy_replace: Vec<crate::ca::legacy_simple_replace::LegacyReplaceRule>,
    /// `SimpleReplaceMultiwordsRule` (16).
    pub multiwords: crate::simple_replace::SimpleReplaceRule,
    /// `SimpleReplaceAnglicism` (19, with the gender/number post filter).
    pub anglicism: crate::simple_replace::SimpleReplaceRule,
    /// `CheckCaseRule` (21).
    pub check_case: crate::simple_replace::SimpleReplaceRule,
    /// DNV lemma rules (26–28).
    pub dnv_replace: Vec<crate::ca::dnv_replace::DnvReplaceRule>,
    /// `CompoundRule` (24, `CA_COMPOUNDS`).
    pub compound: crate::compound::CompoundRule,
    /// Shared filter environment (kept so the engine can set the outer
    /// pipeline back-reference for `SuppressIfAnyRuleMatchesFilter`).
    pub filter_env: Arc<crate::ca::filters::CaFilterEnv>,
}

impl CatalanPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguate_with_snapshot(sentence, false);
    }

    /// `snapshot_after_chunkers`: take the `raw_pos="yes"` pre-disambiguation
    /// snapshot after `spelling_global` + `ca/multiwords.txt` (`removePreviousTags`).
    /// Java's `MultiWordChunker` replaces the tokens in place, so
    /// `getPreDisambigTokensWithoutWhitespace` sees the chunker state; the
    /// XML disambiguator's own in-place mutations are mirrored by
    /// `lt_disambig::mirror_pre`.
    pub fn disambiguate_with_snapshot(
        &self,
        sentence: &mut AnalyzedSentence,
        snapshot_after_chunkers: bool,
    ) {
        self.global_chunker.apply(sentence);
        self.multiwords_chunker.apply(sentence);
        if snapshot_after_chunkers {
            sentence.pre_disambig_tokens = sentence.tokens.clone();
            sentence.pre_disambig_detached = Vec::new();
        }
        self.disambiguator.apply(sentence);
        if let Some(speller) = &self.multitoken_dict_speller {
            crate::ca::multitoken_disambig::disambiguate_multitoken(sentence, speller);
        }
    }
}

/// Adapter exposing the Catalan synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct CatalanSynthesizerAdapter {
    pub synth: Arc<lt_tagger::CatalanSynthesizer>,
    pub tagger: Arc<lt_tagger::CatalanTagger>,
}

impl CatalanSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::CatalanSynthesizer {
        &self.synth
    }
}

impl Synthesizer for CatalanSynthesizerAdapter {
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

    fn sort_target_pos_tags(&self, pos_tags: &mut Vec<String>) {
        self.synth.sort_pos_tags(pos_tags);
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`
        !self
            .tagger
            .tag_word(word)
            .into_iter()
            .next()
            .is_some_and(|r| r.stem.is_none() && r.pos_tag.is_none())
    }
}

/// Catalan sentence tokenization + tagger. `CatalanWordTokenizer` keeps
/// apostrophes inside words (`l'home`) and splits unknown hyphenated words
/// with the dictionary check; the tagger applies the `CatalanTagger.tag()`
/// heuristics.
pub fn analyze_catalan_sentence(catalan: &CatalanPipeline, text: &str) -> AnalyzedSentence {
    let is_tagged = |w: &str| catalan.tagger.is_tagged_word(w);
    let raw_tokens = lt_tokenize::CatalanWordTokenizer::new(&is_tagged).tokenize(text);
    let tagged = catalan.tagger.tag(&raw_tokens);

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
        // `CatalanTagger` sets the typographic-apostrophe flag (the tagger
        // copies it onto each reading)
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    });

    // `CatalanWordTokenizer` preprocesses the text (hyphen/apostrophe/
    // ela-geminada substitutions) before splitting it. Those substitutions
    // are one character for one character, so the concatenated token
    // surfaces have the same *character* count as the sentence; map every
    // token back onto the original text by characters to keep the byte
    // offsets correct (a U+2011 is 3 UTF-8 bytes but the token surface "-"
    // is one).
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
