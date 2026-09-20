//! Paraguayan Guaraní (`gn`/`gug`) pipeline parts.
//!
//! Hand-authored language: upstream LanguageTool has no Guaraní module. The
//! tokenizer keeps the puso (glottal stop, `'`/`’`/`ʼ`) inside the word; the
//! speller is the LibreOffice `gug` Hunspell dictionary; the word-list rules
//! live in [`crate::gn::rules`].

use std::sync::Arc;

use lt_core::AnalyzedSentence;

pub mod accents;
pub mod context;
pub mod priorities;
pub mod rules;
pub mod spelling;

/// Guaraní pipeline state.
pub struct GuaraniPipeline {
    /// `XmlRuleDisambiguator` over `gn/disambiguation.xml` (+ global rules)
    /// when the file exists, empty otherwise.
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `GN_SPELLER` over the vendored `gug` Hunspell dictionary.
    pub spelling: Arc<crate::gn::spelling::GuaraniSpellingRule>,
    /// `GN_ACCENTS`: dictionary-driven accent/tilde restoration.
    pub accents: crate::gn::accents::GuaraniAccentRule,
    /// `GN_HARMONY`: dictionary-driven nasal-harmony alternations.
    pub harmony: crate::gn::context::GuaraniHarmonyRule,
    /// `GN_WORD_REPETITION`.
    pub repetition: crate::word_repetition::WordRepetitionRule,
}

impl GuaraniPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.disambiguator.apply(sentence);
    }
}

/// Guaraní sentence tokenization + surface readings. The base tokenizer
/// splits on apostrophes, so the puso is re-joined into the surrounding word
/// (`ha` + `'` + `u` → `ha'u`) without changing the byte lengths.
pub fn analyze_guarani_sentence(text: &str) -> AnalyzedSentence {
    crate::pipeline::surface_sentence_from_tokens(text, tokenize_guarani(text))
}

fn tokenize_guarani(text: &str) -> Vec<String> {
    let raw = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(
            text,
            &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
        ),
    );
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if i + 2 < raw.len()
            && is_puso(&raw[i + 1])
            && !raw[i].is_empty()
            && !raw[i + 2].is_empty()
            && !lt_core::is_whitespace(&raw[i])
            && !lt_core::is_whitespace(&raw[i + 2])
        {
            let mut fused =
                String::with_capacity(raw[i].len() + raw[i + 1].len() + raw[i + 2].len());
            fused.push_str(&raw[i]);
            fused.push_str(&raw[i + 1]);
            fused.push_str(&raw[i + 2]);
            out.push(fused);
            i += 3;
        } else {
            out.push(raw[i].clone());
            i += 1;
        }
    }
    out
}

fn is_puso(token: &str) -> bool {
    matches!(token, "'" | "\u{2019}" | "\u{02BC}")
}
