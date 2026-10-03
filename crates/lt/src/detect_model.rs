//! The bundled statistical language identifier.
//!
//! Implements [`lt_core::detect::Model`] on top of fastText's `lid.176.ftz`,
//! committed at `assets/lid.176.ftz` (see the README beside it for provenance and
//! licence). The model is embedded with `include_bytes!`, so the browser build
//! never fetches it.
//!
//! It is loaded lazily: parsing 2 M bucket vectors costs ~8 ms and memory, which
//! a caller that only ever needs the Nordum lexicon should not pay. `OnceLock`
//! means the cost is paid once per process and the 900 kB is stored once.

use std::sync::OnceLock;

use lt_core::detect::Model;

/// The model, quantised and compressed by its authors. Unmodified.
const LID_176: &[u8] = include_bytes!("../assets/lid.176.ftz");

/// Labels to ask for. The full 176 would be wasted work; the caller only ever
/// displays a handful.
const TOP_K: usize = 5;

struct FastTextModel(fasttext_pure_rs::FastText);

fn model() -> Option<&'static FastTextModel> {
    static MODEL: OnceLock<Option<FastTextModel>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            fasttext_pure_rs::FastText::load_from_reader(std::io::Cursor::new(LID_176))
                .ok()
                .map(FastTextModel)
        })
        .as_ref()
}

/// The bundled model, or `None` if it could not be parsed.
///
/// A failure here is not fatal: detection still works through the lexicon, and
/// every gate in `lt_core::detect` abstains rather than guessing when this is
/// unavailable.
pub fn bundled() -> Option<&'static dyn Model> {
    model().map(|inner| inner as &dyn Model)
}

impl Model for FastTextModel {
    fn predict(&self, text: &str, k: usize) -> Vec<(String, f32)> {
        let k = k.clamp(1, TOP_K);
        self.0
            .predict(text, k, 0.0)
            .map(|predictions| {
                predictions
                    .into_iter()
                    .map(|p| {
                        let label = p.label.strip_prefix("__label__").unwrap_or(&p.label);
                        (label.to_string(), p.probability)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_loads() {
        assert!(model().is_some(), "bundled lid.176.ftz failed to parse");
    }

    #[test]
    fn predicts_a_confident_language_for_long_text() {
        let model = bundled().expect("model");
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        let got = model.predict(text, TOP_K);
        assert_eq!(got.first().map(|(l, _)| l.as_str()), Some("sv"));
        assert!(got.first().is_some_and(|(_, p)| *p > 0.5));
    }

    #[test]
    fn labels_have_no_prefix() {
        let model = bundled().expect("model");
        for (label, _) in model.predict("Hej och välkommen till vår butik.", TOP_K) {
            assert!(!label.starts_with("__"), "prefix left on {label}");
        }
    }
}
