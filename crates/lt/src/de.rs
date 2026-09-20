//! German-specific rule modules.
//!
//! Modules whose Java classes live only in the German language module
//! (`languagetool-language-modules/de/.../rules/de/`). Rule families that are
//! shared with other languages stay at the crate root and expose per-language
//! entry points (`german()`, `check_sentence_de`, `is_article_de`, …).

pub(crate) mod agreement;
pub(crate) mod agreement_antipatterns;
pub(crate) mod agreement_suggestor;
pub(crate) mod case_rule;
pub(crate) mod case_rule_antipatterns;
pub(crate) mod compound_infinitiv;
pub(crate) mod date_filters;
pub(crate) mod filters;
pub(crate) mod german_helper;
pub(crate) mod language_names;
pub(crate) mod line_expander;
pub(crate) mod missing_comma;
pub(crate) mod old_spelling;
pub(crate) mod pipeline;
pub(crate) mod preposition_to_cases;
pub(crate) mod priorities;
pub(crate) mod repeat;
pub(crate) mod rules;
pub(crate) mod speller_data;
pub(crate) mod spelling;
pub(crate) mod spelling_patterns;
pub(crate) mod statistic;
pub(crate) mod style;
pub(crate) mod style_repeated_word;
pub(crate) mod subject_verb_agreement;
pub(crate) mod synthesizer;
pub(crate) mod util;
pub(crate) mod verb_agreement;
