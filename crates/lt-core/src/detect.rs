//! Language detection.
//!
//! Deciding which language a piece of text is written in, ahead of checking it.
//! Three layers, each present because of a measured failure:
//!
//! 1. **Lexicon** — a small set of words exclusive to one language. Nordum is
//!    decided here. It appears in no off-the-shelf language identifier, and a
//!    statistical model answers `da`/`no`/`sv` for it with up to 0.99 confidence,
//!    so no threshold can recover it afterwards.
//! 2. **Statistical model** — see [`Model`]. Behind a trait so the backing
//!    implementation (bundled fastText, a self-trained model, something else)
//!    is a release-time decision rather than an architectural one.
//! 3. **Gates** — length, confidence and margin. Below them the answer is
//!    "unknown" rather than a guess.
//!
//! The common case is deliberately conservative: a grammar checker sees
//! fragments, not documents, and trigram models are weakest exactly there. An
//! abstention keeps the caller's existing language; a wrong answer silently
//! checks the text against the wrong rule set.

use std::collections::{HashMap, HashSet};

use crate::Lang;

/// Confidence reported for a lexicon hit.
///
/// The marker sets are exclusive by construction — the Nordum list is
/// `nrd_core.dic` minus the Bokmål, Nynorsk, Danish and Swedish Hunspell word
/// lists, so none of the 734 words is valid in any of them. That makes a hit
/// strong evidence rather than a hint, but it is still not the statistical
/// certainty a well-trained model reports, so it is capped below 1.0.
pub const LEXICON_CONFIDENCE: f32 = 0.95;

/// Thresholds below which detection abstains rather than guessing.
///
/// Defaults are proposals calibrated against
/// `tests/fixtures/detection_corpus.json`; see `docs/language-detection-plan.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gates {
    /// Minimum non-whitespace characters before any answer is given.
    pub min_chars: usize,
    /// Minimum confidence in the leading candidate.
    pub min_confidence: f32,
    /// Minimum ratio between the leading candidate and the runner-up.
    pub min_margin: f32,
}

impl Default for Gates {
    fn default() -> Self {
        Gates {
            min_chars: 40,
            min_confidence: 0.60,
            min_margin: 1.5,
        }
    }
}

/// Which layer produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An exclusive-word hit; authoritative for that language.
    Lexicon,
    /// The statistical model.
    Model,
}

/// One scored language.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub lang: Lang,
    pub confidence: f32,
    pub source: Source,
}

/// A settled answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub language: Lang,
    pub confidence: f32,
    pub source: Source,
}

/// Words that identify a language outright.
///
/// Only languages that no statistical model can be expected to get right belong
/// here. Adding a language whose model output is merely *low confidence* would
/// make this layer authoritative over evidence it does not deserve.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    markers: HashMap<Lang, HashSet<String>>,
}

impl Lexicon {
    pub fn new() -> Self {
        Lexicon::default()
    }

    /// Register markers for a language. Entries are lowercased and any that are
    /// empty are ignored.
    pub fn add(&mut self, lang: Lang, words: impl IntoIterator<Item = String>) {
        let entry = self.markers.entry(lang).or_default();
        for word in words {
            let word = word.trim().to_lowercase();
            if !word.is_empty() {
                entry.insert(word);
            }
        }
    }

    /// Load markers from a newline-separated blob.
    ///
    /// Blank lines are skipped and everything from a `#` onwards is treated as a
    /// comment, so the generated file can be committed with a header and checked
    /// in without pre-processing.
    pub fn load(&mut self, lang: Lang, text: &str) {
        self.add(
            lang,
            text.lines().filter_map(|line| {
                let word = line.split('#').next().unwrap_or("").trim();
                (!word.is_empty()).then(|| word.to_string())
            }),
        );
    }

    pub fn is_empty(&self) -> bool {
        self.markers.values().all(HashSet::is_empty)
    }

    /// Languages with any marker registered.
    pub fn languages(&self) -> Vec<Lang> {
        let mut langs: Vec<Lang> = self
            .markers
            .iter()
            .filter(|(_, words)| !words.is_empty())
            .map(|(lang, _)| *lang)
            .collect();
        langs.sort_by_key(|lang| lang.base_code());
        langs
    }

    /// Return the language whose markers appear in `text`.
    ///
    /// First match wins if several appear, which in practice means the caller
    /// should keep the sets disjoint; they are disjoint by construction for the
    /// languages this is built for.
    pub fn identify(&self, text: &str) -> Option<(Lang, usize)> {
        let tokens = tokenize(text);
        if tokens.is_empty() {
            return None;
        }
        let mut best: Option<(Lang, usize)> = None;
        for (lang, words) in &self.markers {
            if words.is_empty() {
                continue;
            }
            let hits = tokens.iter().filter(|token| words.contains(*token)).count();
            if hits > 0 && best.is_none_or(|(_, seen)| hits > seen) {
                best = Some((*lang, hits));
            }
        }
        best
    }
}

/// A statistical language detector.
///
/// Takes the text and returns `(label, probability)` pairs, highest first.
/// Labels are engine-specific strings (`en`, `nn`, …) and are mapped through
/// [`Lang::from_long_code`]; unknown ones are dropped rather than guessed at.
pub trait Model {
    fn predict(&self, text: &str, k: usize) -> Vec<(String, f32)>;
}

/// Lowercase alphabetic tokens.
///
/// Diacritics are preserved rather than stripped: the markers contain `øi` and
/// `å`, and folding would merge words that the lexicon deliberately separates.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphabetic() {
            current.extend(ch.to_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Run the model and keep only candidates we actually ship.
///
/// Dropping unknown labels matters: it is what stops a 176-language model from
/// proposing a language with no pack behind it.
pub fn model_candidates(model: &dyn Model, text: &str, k: usize) -> Vec<Candidate> {
    let shipped: HashSet<Lang> = Lang::ALL.into_iter().collect();
    model
        .predict(text, k)
        .into_iter()
        .filter_map(|(label, confidence)| {
            let lang = Lang::from_long_code(&label)?;
            shipped.contains(&lang).then_some(Candidate {
                lang,
                confidence,
                source: Source::Model,
            })
        })
        .collect()
}

/// Score `text` without deciding, for callers that want the full ranking.
pub fn candidates(text: &str, lexicon: &Lexicon, model: Option<&dyn Model>) -> Vec<Candidate> {
    if let Some((lang, _)) = lexicon.identify(text) {
        return vec![Candidate {
            lang,
            confidence: LEXICON_CONFIDENCE,
            source: Source::Lexicon,
        }];
    }
    match model {
        Some(model) => model_candidates(model, text, 5),
        None => Vec::new(),
    }
}

/// Decide the language of `text`, or abstain.
///
/// `None` means "not enough evidence" and is a normal outcome, not a failure:
/// the caller should keep whatever language it was already using.
pub fn detect(
    text: &str,
    lexicon: &Lexicon,
    model: Option<&dyn Model>,
    gates: &Gates,
) -> Option<Detection> {
    let ranked = candidates(text, lexicon, model);
    let best = ranked.first()?;

    // A lexicon hit is decided by words that occur in no other language we ship,
    // so it is evidence at any length: "Jei." alone is Nordum. The length gate
    // exists because a *statistical* model is unreliable on the short fragments
    // a grammar checker sees, and it is applied only to that source.
    if best.source == Source::Model {
        if text.chars().filter(|ch| !ch.is_whitespace()).count() < gates.min_chars {
            return None;
        }
        if best.confidence < gates.min_confidence {
            return None;
        }
        if let Some(runner_up) = ranked.get(1) {
            let ratio = runner_up.confidence.max(f32::MIN_POSITIVE);
            if best.confidence / ratio < gates.min_margin {
                return None;
            }
        }
    }
    Some(Detection {
        language: best.lang,
        confidence: best.confidence,
        source: best.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeModel(Vec<(&'static str, f32)>);

    impl Model for FakeModel {
        fn predict(&self, _text: &str, _k: usize) -> Vec<(String, f32)> {
            self.0.iter().map(|(l, p)| ((*l).to_string(), *p)).collect()
        }
    }

    fn lexicon() -> Lexicon {
        let mut lex = Lexicon::new();
        lex.load(Lang::Nrd, "jei\n# a comment\n\nblir\n");
        lex
    }

    #[test]
    fn lexicon_identifies_nordum() {
        let (lang, hits) = lexicon()
            .identify("Jei vet at hun arbeider i dag.")
            .unwrap();
        assert_eq!(lang, Lang::Nrd);
        assert_eq!(hits, 1);
    }

    #[test]
    fn lexicon_loads_past_comments_and_blank_lines() {
        let lex = lexicon();
        // Markers on either side of a comment and a blank line still load.
        // Note the comment itself is inert either way: a stored "# a comment"
        // contains '#' and a space, so it can never equal an alphabetic token.
        // Stripping keeps the stored set honest for `languages()` and debugging,
        // not for matching.
        assert_eq!(lex.identify("jei").map(|(lang, _)| lang), Some(Lang::Nrd));
        assert_eq!(lex.identify("blir").map(|(lang, _)| lang), Some(Lang::Nrd));
        assert_eq!(lex.identify("comment"), None);
    }

    #[test]
    fn lexicon_does_not_claim_other_languages() {
        // 'jeg' is Bokmal/Danish, 'jag' is Swedish: neither is a Nordum marker.
        assert_eq!(lexicon().identify("Jeg kommer hjem i morgen"), None);
        assert_eq!(lexicon().identify("Jag kommer hem i morgon"), None);
    }

    #[test]
    fn unknown_model_labels_are_dropped() {
        let model = FakeModel(vec![("xx", 0.99), ("sw", 0.4), ("en", 0.8)]);
        let got = model_candidates(&model, "hello", 5);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].lang, Lang::En);
    }

    #[test]
    fn model_covers_aliases() {
        let model = FakeModel(vec![("nb", 0.9)]);
        assert_eq!(model_candidates(&model, "hei", 1)[0].lang, Lang::No);
        let model = FakeModel(vec![("nno", 0.9)]);
        assert_eq!(model_candidates(&model, "hei", 1)[0].lang, Lang::Nn);
    }

    #[test]
    fn short_text_abstains_for_the_model_path() {
        let model = FakeModel(vec![("sv", 0.99)]);
        assert_eq!(
            detect("Jag kommer.", &lexicon(), Some(&model), &Gates::default()),
            None
        );
    }

    #[test]
    fn short_text_is_enough_for_a_lexicon_hit() {
        // An exclusive marker is evidence at any length, which is what separates
        // the lexicon from the model path.
        assert_eq!(
            detect("Jei.", &lexicon(), None, &Gates::default()).map(|d| d.language),
            Some(Lang::Nrd)
        );
    }

    #[test]
    fn low_confidence_abstains() {
        let model = FakeModel(vec![("sv", 0.4)]);
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(
            detect(text, &lexicon(), Some(&model), &Gates::default()),
            None
        );
    }

    #[test]
    fn close_call_abstains() {
        let model = FakeModel(vec![("sv", 0.70), ("da", 0.65)]);
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(
            detect(text, &lexicon(), Some(&model), &Gates::default()),
            None
        );
    }

    #[test]
    fn clear_winner_is_accepted() {
        let model = FakeModel(vec![("sv", 0.93), ("da", 0.04)]);
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        let got = detect(text, &lexicon(), Some(&model), &Gates::default()).unwrap();
        assert_eq!(got.language, Lang::Sv);
        assert_eq!(got.source, Source::Model);
    }

    #[test]
    fn lexicon_overrides_the_model() {
        // The model confidently says Danish; Nordum markers must still win.
        let model = FakeModel(vec![("da", 0.99)]);
        let text = "Jei vet at hun arbeider i dag, og det er viktig å lære språket.";
        let got = detect(text, &lexicon(), Some(&model), &Gates::default()).unwrap();
        assert_eq!(got.language, Lang::Nrd);
        assert_eq!(got.source, Source::Lexicon);
    }

    #[test]
    fn no_model_and_no_markers_yields_nothing() {
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(detect(text, &Lexicon::new(), None, &Gates::default()), None);
    }

    #[test]
    fn tokenizer_keeps_diacritics() {
        assert_eq!(tokenize("Jei vet å lære"), vec!["jei", "vet", "å", "lære"]);
    }

    #[test]
    fn tokenizer_splits_on_punctuation_and_digits() {
        assert_eq!(tokenize("Hei, verden! 42"), vec!["hei", "verden"]);
    }
}
