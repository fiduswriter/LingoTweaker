//! Spanish language module: pipeline, filters and (incrementally) the
//! Java-coded Spanish built-in rules.
//!
//! Rule families that carry per-language entry points stay at the crate root
//! (`comma_whitespace`, `unpaired_brackets`, …); Spanish-only implementations
//! live here, mirroring `crates/lt/src/de/`.

pub mod date_filters;
pub mod filters;
pub mod pipeline;
pub mod priorities;
pub mod rules;
pub mod simple_replace;
pub mod spelling;
pub mod synthesizer;
