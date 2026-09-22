//! Persian (`fa`) pipeline parts. `Persian` is a plain `Language`: it does not
//! override `createDefaultTagger` (so the base `DemoTagger` gives every token
//! a single null-POS reading), `createDefaultDisambiguator` (the base
//! `DemoDisambiguator`, a no-op), `createDefaultSynthesizer` (none) or
//! `createDefaultSpellingRule` (none). The analyzed sentence is therefore the
//! surface tokenization only, like the hand-authored languages.

use lt_core::AnalyzedSentence;

/// `Persian` pipeline parts. Stage 3 adds the Persian-specific Java rule
/// instances (`PERSIAN_WORD_REPEAT_RULE`, `PERSIAN_WORD_REPEAT_BEGINNING_RULE`,
/// `FA_SIMPLE_REPLACE`, `FA_WORD_COHERENCY`, `FA_SPACE_BEFORE_CONJUNCTION`).
pub struct PersianPipeline {}

/// `PersianWordTokenizer`: `WordTokenizer.getTokenizingCharacters()` plus the
/// Arabic comma `،` (`U+060C`), question mark `؟` (`U+061F`) and semicolon `؛`
/// (`U+061B`).
pub fn tokenizing_characters() -> String {
    let mut chars = lt_tokenize::wordtokenizer::base_tokenizing_characters();
    chars.push_str("\u{060C}\u{061F}\u{061B}");
    chars
}

/// Tokenize + tag one sentence like Java's `Persian`: the `PersianWordTokenizer`
/// followed by the `DemoTagger` (a single null-POS reading per token).
pub fn analyze_persian_sentence(text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(text, &tokenizing_characters()),
    );
    crate::pipeline::surface_sentence_from_tokens(text, raw_tokens)
}
