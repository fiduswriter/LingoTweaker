//! German pipeline parts: tokenizer/tagger/disambiguator order, the
//! `GermanRuleDisambiguator` multiword chunkers and (incrementally) the
//! Java-coded German built-in rules.
use lt_data::PathExt as _;

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, CoreError, Result};
use lt_tagger::{GermanTagger, SwissGermanTagger};
use lt_tokenize::{GermanWordTokenizer, SrxDocument, SrxTokenizer};

/// The German tagger variant (`GermanTagger` vs `SwissGermanTagger`).
#[derive(Clone)]
pub enum GermanTaggerKind {
    De(Arc<GermanTagger>),
    Swiss(Arc<SwissGermanTagger>),
}

impl GermanTaggerKind {
    /// The wrapped `GermanTagger` (filters and rules that use
    /// `GermanTagger.INSTANCE`).
    pub fn base(&self) -> &GermanTagger {
        match self {
            Self::De(t) => t,
            Self::Swiss(t) => t.inner(),
        }
    }

    pub fn tag(&self, sentence_tokens: &[String], ignore_case: bool) -> Vec<AnalyzedTokenReadings> {
        match self {
            Self::De(t) => t.tag(sentence_tokens, ignore_case),
            Self::Swiss(t) => t.tag(sentence_tokens, ignore_case),
        }
    }
}

/// `GermanRuleDisambiguator`'s chunker order: multitoken-ignore →
/// spelling_global → multitoken-suggest → XML rules.
pub struct GermanPipeline {
    pub tagger: GermanTaggerKind,
    pub synthesizer: Arc<lt_tagger::GermanSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<crate::de::synthesizer::GermanSynthesizerAdapter>,
    pub global_chunker: lt_disambig::MultiWordChunker,
    pub multitoken_chunker: lt_disambig::MultiWordChunker,
    pub multitoken_suggest_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `German`'s `createDefaultPostDisambiguationChunker` — runs after the
    /// disambiguator (`JLanguageTool.getAnalyzedSentence`)
    pub chunker: lt_chunk::german::GermanChunker,
    /// `GermanSpellerRule` for the variant (runs after the XML rules)
    pub spelling: Arc<crate::de::spelling::GermanSpellingRule>,
    /// `OLD_SPELLING_RULE` (12, default on)
    pub old_spelling: crate::de::old_spelling::OldSpellingRule,
    /// `COMPOUND_INFINITIV_RULE` (44, default on)
    pub compound_infinitiv: crate::de::compound_infinitiv::CompoundInfinitivRule,
    /// `DE_AGREEMENT` (19, default on)
    pub agreement: crate::de::agreement::AgreementRule,
    /// `DE_AGREEMENT2` (20, default on)
    pub agreement2: crate::de::agreement::AgreementRule2,
    /// `DE_CASE` (21, default on)
    pub case_rule: crate::de::case_rule::CaseRule,
    /// `DE_COMPOUNDS` (GermanyGerman/AustrianGerman/NonSwissGerman) or
    /// `DE_CH_COMPOUNDS` (SwissGerman), default on
    pub compound: crate::compound::CompoundRule,
    /// `READABILITY_RULE_DIFFICULT_DE`/`_SIMPLE_DE` (both default off)
    pub readability: Vec<crate::readability::ReadabilityRule>,
    /// `DE_REPEATEDWORDS` (`GermanRepeatedWordsRule`, text level, default on)
    pub repeated_words: crate::repeated_words::RepeatedWordsRule,
    /// `DE_VERBAGREEMENT` (23, text level, default on)
    pub verb_agreement: crate::de::verb_agreement::VerbAgreementRule,
    /// `DE_SUBJECT_VERB_AGREEMENT` (24, sentence level, default on)
    pub subject_verb_agreement: crate::de::subject_verb_agreement::SubjectVerbAgreementRule,
    /// `GERMAN_WRONG_WORD_IN_CONTEXT` (default on)
    pub wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule,
    /// `DE_WORD_COHERENCY` (text level)
    pub word_coherency: crate::word_coherency::WordCoherencyRule,
    /// `de-DE` (default) / `de-AT` / `de-CH`
    pub variant: String,
}

pub(crate) fn load_chunker(
    path: &std::path::Path,
    allow_first_capitalized: bool,
) -> lt_disambig::MultiWordChunker {
    lt_disambig::MultiWordChunker::load(
        path,
        // GermanRuleDisambiguator calls setIgnoreSpelling(true) on all three
        true,
        allow_first_capitalized,
        true,
        Some(lt_disambig::multiword::TAG_FOR_NOT_ADDING_TAGS.to_string()),
        false,
    )
    .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(true, false))
}

impl GermanPipeline {
    /// Build the German disambiguator order for `variant` (`de-DE`, `de-AT`,
    /// `de-CH`).
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.multitoken_chunker.apply(sentence);
        self.global_chunker.apply(sentence);
        self.multitoken_suggest_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// German sentence tokenization + tagger (`GermanWordTokenizer` produces the
/// token stream including whitespace; `GermanTagger.tag(List)` needs the whole
/// list at once for its case heuristics).
pub fn analyze_german_sentence(german: &GermanPipeline, text: &str) -> AnalyzedSentence {
    let tokenizer = GermanWordTokenizer::new();
    let raw_tokens = tokenizer.tokenize(text);
    let tagged = german.tagger.tag(&raw_tokens, true);

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

/// SRX tokenizer for German: the Java `singleLineBreaksMarksPara` default for
/// the German language class resolves to the `de` SRX rule set.
pub fn german_srx(data_dir: &lt_data::DataDir) -> Result<SrxTokenizer> {
    let srx_path = data_dir.path().join("core/segment.srx");
    if !srx_path.lt_exists() {
        return Err(CoreError::Data("missing core/segment.srx".into()));
    }
    let doc = SrxDocument::load_file(&srx_path)?;
    // `SRXSentenceTokenizer` sets `setSingleLineBreaksMarksParagraph(false)`
    // in its constructor, so `parCode` is "_two" for every language; the code
    // passed to the SRX matcher is the short code plus parCode (`de_two`,
    // matching `(DE|de).*` → the German rule group).
    SrxTokenizer::new(&doc, "de_two")
}
