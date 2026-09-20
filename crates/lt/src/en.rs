//! English-specific rule modules.
//!
//! Modules whose Java classes live only in the English language module
//! (`languagetool-language-modules/en/.../rules/en/`). Rule families that are
//! shared with other languages — paragraph, readability, long-sentence,
//! unpaired brackets/quotes, whitespace, word coherency, style-too-often,
//! comma whitespace, dash rules, unit conversion, repeated words, the
//! synthesizer facade — stay at the crate root and expose per-language entry
//! points (`*_de`, `symbols_de()`, `strings_de()`, …).

pub(crate) mod avs_an;
pub(crate) mod consistent_apostrophes;
pub(crate) mod contractions;
pub(crate) mod filters;
pub(crate) mod priorities;
pub(crate) mod specific_case;
pub(crate) mod spelling;
pub(crate) mod spelling_data;
pub(crate) mod synthesizer;
pub(crate) mod word_repeat;
pub(crate) mod word_repeat_beginning;
