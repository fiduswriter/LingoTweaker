//! Experimental Lindera-backed segmentation for Japanese/Chinese.
//!
//! This is a spike-branch crate: it exists to prove that Lindera's dictionary
//! I/O can be routed through [`lt_data::fs`] (via the patched
//! `lindera-dictionary` in `vendor/`) so a compiled dictionary can be loaded
//! from an in-memory data pack and therefore on wasm.
//!
//! The POS/lemma mapping mirrors LanguageTool's `JapaneseWordTokenizer` +
//! `JapaneseTagger`: IPADIC details are `[品詞, 細分類1, 細分類2, 細分類3,
//! 活用型, 活用形, 原形, 読み, 発音]`, the POS tag is the first four fields
//! joined with `-` (skipping `*`), and the basic form is field 6 unless it is
//! `*`, in which case the surface is used.

use std::borrow::Cow;
use std::path::Path;

use lindera::dictionary::load_fs_dictionary_with_options;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;

pub use lindera::mode::Mode as LinderaMode;

/// One segmented token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CjkToken {
    pub surface: String,
    /// POS tag (`名詞-代名詞-一般`, …) built from the IPADIC detail fields.
    pub pos: String,
    /// Lemma / base form.
    pub basic_form: String,
}

/// A Lindera segmenter whose dictionary was loaded through `lt_data::fs`.
pub struct CjkSegmenter {
    segmenter: Segmenter,
}

impl CjkSegmenter {
    /// Load a compiled Lindera dictionary directory through `lt_data::fs`.
    ///
    /// `dict_path` may be a real directory or a path inside a mounted data
    /// pack (`/lt-mem/<id>/…`); both go through the same read helpers.
    pub fn from_dict_dir(dict_path: &Path) -> Result<Self, String> {
        // `use_mmap = false`: mmap cannot read from an in-memory mount (nor is
        // it available on wasm), so always take the plain-read path.
        let dictionary =
            load_fs_dictionary_with_options(dict_path, false).map_err(|e| e.to_string())?;
        Ok(Self {
            segmenter: Segmenter::new(Mode::Normal, dictionary, None),
        })
    }

    /// Segment `text` into tokens, with POS and basic form.
    pub fn tokenize(&self, text: &str) -> Result<Vec<CjkToken>, String> {
        let mut tokens = self
            .segmenter
            .segment(Cow::Borrowed(text))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(tokens.len());
        for token in tokens.iter_mut() {
            let details: Vec<String> = token.details().iter().map(|d| d.to_string()).collect();
            let surface = token.surface.as_ref().to_string();
            let pos = details
                .iter()
                .take(4)
                .filter(|f| f.as_str() != "*" && !f.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join("-");
            let pos = if pos.is_empty() {
                details.first().cloned().unwrap_or_default()
            } else {
                pos
            };
            let raw_base = details.get(6).cloned().unwrap_or_else(|| surface.clone());
            let basic_form = if raw_base.eq_ignore_ascii_case("*") || raw_base.is_empty() {
                surface.clone()
            } else {
                raw_base
            };
            out.push(CjkToken {
                surface,
                pos,
                basic_form,
            });
        }
        Ok(out)
    }

    /// Segment `text` for the Chinese (jieba) dictionary.
    ///
    /// The jieba details are `[pos, "CHINESE", pinyin, traditional, simplified,
    /// gloss, length, …]`, so the POS is `details[0]`; there is no lemma
    /// (the basic form is the surface), matching LanguageTool's
    /// `ChineseTagger` (`new AnalyzedToken(word, pos, null)`).
    pub fn tokenize_zh(&self, text: &str) -> Result<Vec<CjkToken>, String> {
        let mut tokens = self
            .segmenter
            .segment(Cow::Borrowed(text))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(tokens.len());
        for token in tokens.iter_mut() {
            let surface = token.surface.as_ref().to_string();
            let pos = token
                .details()
                .first()
                .map(|d| d.to_string())
                .unwrap_or_default();
            out.push(CjkToken {
                basic_form: surface.clone(),
                surface,
                pos,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine stores a segmenter in a shared engine that must be
    /// `Send + Sync`.
    #[test]
    fn segmenter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CjkSegmenter>();
    }
}
