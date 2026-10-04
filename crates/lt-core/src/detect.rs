//! Language detection.
//!
//! Deciding which language a piece of text is written in, ahead of checking it.
//! Four layers, each present because of a measured failure:
//!
//! 1. **Lexicon** — a small set of words exclusive to one language. Nordum is
//!    decided here. It appears in no off-the-shelf language identifier, and a
//!    statistical model answers `da`/`no`/`sv` for it with up to 0.99 confidence,
//!    so no threshold can recover it afterwards.
//! 2. **Statistical model** — see [`Model`]. Behind a trait so the backing
//!    implementation (bundled fastText, a self-trained model, something else)
//!    is a release-time decision rather than an architectural one.
//! 3. **Refiner** — see [`Refiner`]. A second opinion for the languages the
//!    primary model is confidently wrong about.
//! 4. **Gates** — length, confidence and margin. Below them the answer is
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
/// Calibrated against the bundled models over the fixture corpus
/// (`crates/lt/tests/detect_model.rs` sweeps both tables below). The length gate
/// dominates: lowering `min_confidence` from 0.60 to 0.30 moves coverage by under
/// 4 points, while `min_chars` moves it by tens.
///
/// | `min_chars` | coverage | precision |
/// |---|---|---|
/// | 0 | 85.2 % | 93.4 % |
/// | **20** | **71.8 %** | **95.6 %** |
/// | 40 | 40.3 % | 97.3 % |
/// | 80 | 16.8 % | 98.4 % |
///
/// 40 was the original guess and is too high: it abstains on 82 % of real
/// sentences, which makes detection nearly inert. Precision is preferred over
/// coverage because a wrong answer silently checks text against the wrong rules,
/// while an abstention leaves the user on the language they already had.
///
/// `min_chars` is counted in [`significant_length`] units rather than raw
/// characters, which is what makes it usable at all outside Latin scripts — see
/// [`non_latin_weight`]. With the length gate counted in raw characters the
/// fixture's Japanese sentences, median **5** characters, were abstained on 399
/// times out of 401 *by a model that had them right 348 times*. That was the
/// single largest defect in the detector, and it was in the gate rather than in
/// the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gates {
    /// Minimum [`significant_length`] before any answer is given.
    pub min_chars: usize,
    /// Minimum confidence in the leading candidate.
    pub min_confidence: f32,
    /// Minimum ratio between the leading candidate and the runner-up.
    pub min_margin: f32,
    /// How many `min_chars` one non-Latin character is worth. See
    /// [`significant_length`]; `1` restores raw character counting.
    pub non_latin_weight: usize,
}

impl Default for Gates {
    fn default() -> Self {
        Gates {
            min_chars: 20,
            min_confidence: 0.60,
            min_margin: 1.5,
            non_latin_weight: NON_LATIN_WEIGHT,
        }
    }
}

/// How many Latin characters one non-Latin character counts as, when the length
/// gate asks whether there is enough text to be worth a decision.
///
/// The argument is information per character, not sentiment. In a
/// space-delimited Latin sentence, 20 characters is three or four words. In
/// Japanese or Chinese a character *is* a morpheme, so 20 characters is about 20
/// words; in Devanagari, Tamil, Khmer or Thai a cluster is a syllable and a
/// syllable is a morpheme; in Arabic and Hebrew a character carries more of the
/// word than a Latin letter does. Counting them all as one says a Japanese
/// sentence of seven characters and a Danish sentence of seven characters carry
/// the same evidence, which is not true in either direction.
///
/// Measured over the fixture (`non_latin_weight` swept by hand, discriminator
/// in place):
///
/// | weight | correct | abstained | wrong |
/// |---|---|---|---|
/// | 1 | 58.5 % | 39.0 % | 2.5 % |
/// | 3 | 67.1 % | 30.0 % | 3.0 % |
/// | **5** | **68.6 %** | **28.2 %** | 3.2 % |
/// | 8 | 68.8 % | 28.0 % | 3.2 % |
/// | 10 | 68.9 % | 27.8 % | 3.3 % |
///
/// 5 is where the curve flattens: it takes Japanese from 0.5 % to 54.9 %,
/// Chinese from 47.9 % to 83.2 %, Khmer from 53.2 % to 93.5 %, Arabic from 25.2 %
/// to 85.9 %, Persian from 0.4 % to 71.3 %, and the 35 Latin-script languages do
/// not move at all, because they contain no character this applies to. Pushing
/// to 10 buys three tenths of a point for another wrong answer.
///
/// It is a constant rather than a tuning knob on purpose: it encodes a property
/// of writing systems, and a caller who wants the old behaviour sets it to `1`.
pub const NON_LATIN_WEIGHT: usize = 5;

/// Length of `text` for the length gate, in [`Gates::min_chars`] units.
///
/// Non-whitespace characters count [`Gates::non_latin_weight`] each when they
/// belong to a script that does not delimit words with spaces and writes a
/// morpheme per character or syllable; everything else counts one.
///
/// Cyrillic and Greek are deliberately *not* weighted. They are alphabetic and
/// space-delimited like Latin, their fixture medians are 31 and 46 characters,
/// and their accuracy is already high — weighting them would be fitting noise.
pub fn significant_length(text: &str, weight: usize) -> usize {
    let mut total = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += if is_non_latin(ch) { weight } else { 1 };
    }
    total
}

/// Whether `ch` belongs to a script where one character carries roughly one
/// morpheme: CJK, Hangul, Khmer, the Brahmic andIndic scripts, Thai, Lao,
/// Myanmar, and the Arabic and Hebrew ranges.
///
/// Half-width and full-width CJK *punctuation* is included with the CJK ranges,
/// which is deliberate: `、` and `。` end clauses the way a Latin full stop does,
/// so text made of them is still text.
fn is_non_latin(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x11FF      // Hangul Jamo
        | 0x0600..=0x06FF    // Arabic
        | 0x0750..=0x077F    // Arabic Supplement
        | 0x0900..=0x097F    // Devanagari
        | 0x0B80..=0x0BFF    // Tamil
        | 0x0C00..=0x0C7F    // Telugu
        | 0x0D00..=0x0D7F    // Malayalam
        | 0x0E00..=0x0E7F    // Thai
        | 0x1000..=0x109F    // Myanmar
        | 0x1780..=0x17FF    // Khmer
        | 0x3040..=0x30FF    // Hiragana, Katakana
        | 0x3400..=0x4DBF    // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF    // CJK Unified Ideographs
        | 0xAC00..=0xD7AF    // Hangul Syllables
        | 0xF900..=0xFAFF    // CJK Compatibility Ideographs
        | 0xFB50..=0xFDFF    // Arabic Presentation Forms-A
        | 0xFE70..=0xFEFF    // Arabic Presentation Forms-B
        | 0xFF00..=0xFF60    // Full-width forms
        | 0xFFE0..=0xFFE6
    )
}

/// Which layer produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An exclusive-word hit; authoritative for that language.
    Lexicon,
    /// The statistical model.
    Model,
    /// A refiner's second opinion, after the model named a language the refiner
    /// covers.
    Discriminator,
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

/// A second opinion on the model's answer.
///
/// The primary model is not short of labels it declines to use: it answers `es`
/// for Asturian, `da` for Bokmål, `no` for Nynorsk, `es` for Galician, and `tr`
/// for Crimean Tatar. Those are all *available* labels it ranks second or lower,
/// so no confidence threshold recovers them — which is what a specialist is for.
///
/// A refiner is consulted with the model's ranking and the caller's gates, and
/// returns a **replacement** ranking, highest first, tagged
/// [`Source::Discriminator`]. Returning `None` keeps the model's ranking, and
/// that has to be the easy answer: a refiner may improve an answer or decline to,
/// but it must not turn a settled answer into an abstention. So an implementation
/// checks its own candidate against `gates` first and declines when the
/// replacement would not clear them — which is also why the gates are passed in
/// rather than baked into the implementation.
///
/// The engine applies no further policy: what the refiner is for, when it may
/// fire and how confident it has to be are all inside it, because those are
/// measured decisions about one model rather than rules of the pipeline.
pub trait Refiner {
    /// A ranking to use instead of `primary`, or `None` to keep it.
    fn refine(&self, text: &str, primary: &[Candidate], gates: &Gates) -> Option<Vec<Candidate>>;
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
///
/// `refiner` gets the model's ranking and may replace it; see [`Refiner`].
pub fn candidates(
    text: &str,
    lexicon: &Lexicon,
    model: Option<&dyn Model>,
    refiner: Option<&dyn Refiner>,
    gates: &Gates,
) -> Vec<Candidate> {
    if let Some((lang, _)) = lexicon.identify(text) {
        return vec![Candidate {
            lang,
            confidence: LEXICON_CONFIDENCE,
            source: Source::Lexicon,
        }];
    }
    let Some(model) = model else {
        return Vec::new();
    };
    let primary = model_candidates(model, text, 5);
    match refiner.and_then(|refiner| refiner.refine(text, &primary, gates)) {
        Some(refined) if !refined.is_empty() => refined,
        _ => primary,
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
    refiner: Option<&dyn Refiner>,
    gates: &Gates,
) -> Option<Detection> {
    cleared_candidates(
        &candidates(text, lexicon, model, refiner, gates),
        text,
        gates,
    )
}

/// The leading candidate of an existing ranking, if it clears the gates.
///
/// Split out from [`detect`] for callers that already hold a ranking — a
/// server that reports the candidates alongside the decision, or restricts them
/// first, must not pay for a second prediction or keep a second copy of the
/// gate rules.
pub fn cleared_candidates(ranked: &[Candidate], text: &str, gates: &Gates) -> Option<Detection> {
    let best = ranked.first()?;

    // A lexicon hit is decided by words that occur in no other language we ship,
    // so it is evidence at any length: "Jei." alone is Nordum. The length gate
    // exists because a *statistical* model is unreliable on the short fragments
    // a grammar checker sees, and it applies to both statistical layers — the
    // discriminator is a statistical model and its short-fragment behaviour is
    // the reason `Gates` prefers abstention there.
    if best.source != Source::Lexicon {
        if significant_length(text, gates.non_latin_weight) < gates.min_chars {
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
    fn no_model_and_no_markers_yields_nothing() {
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(
            detect(text, &Lexicon::new(), None, None, &Gates::default()),
            None
        );
    }

    struct FakeRefiner(Vec<(&'static str, f32)>);

    impl Refiner for FakeRefiner {
        fn refine(
            &self,
            _text: &str,
            _primary: &[Candidate],
            _gates: &Gates,
        ) -> Option<Vec<Candidate>> {
            Some(
                self.0
                    .iter()
                    .map(|(label, confidence)| Candidate {
                        lang: Lang::from_long_code(label).expect("known label"),
                        confidence: *confidence,
                        source: Source::Discriminator,
                    })
                    .collect(),
            )
        }
    }

    #[test]
    fn a_refiner_replaces_the_ranking() {
        // The model is sure and wrong; the refiner is sure and right. Whichever
        // ranking survives is the one the gates are then applied to, which is why
        // the refiner's own confidence has to clear them too.
        let model = FakeModel(vec![("da", 0.99)]);
        let refiner = FakeRefiner(vec![("no", 0.99)]);
        let text = "Flere ganger har jeg sagt det.";
        let got = detect(
            text,
            &lexicon(),
            Some(&model),
            Some(&refiner),
            &Gates::default(),
        )
        .unwrap();
        assert_eq!(got.language, Lang::No);
        assert_eq!(got.source, Source::Discriminator);
    }

    #[test]
    fn a_refiner_that_declines_leaves_the_ranking_alone() {
        let model = FakeModel(vec![("sv", 0.93), ("da", 0.04)]);
        struct Silent;
        impl Refiner for Silent {
            fn refine(
                &self,
                _text: &str,
                _primary: &[Candidate],
                _gates: &Gates,
            ) -> Option<Vec<Candidate>> {
                None
            }
        }
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        let got = detect(
            text,
            &lexicon(),
            Some(&model),
            Some(&Silent),
            &Gates::default(),
        )
        .unwrap();
        assert_eq!(got.language, Lang::Sv);
        assert_eq!(got.source, Source::Model);
    }

    #[test]
    fn the_length_gate_applies_to_a_refiner_too() {
        // The discriminator is a statistical model, so it is exactly as unreliable
        // on a fragment as the primary one is. Applying the gate to the lexicon
        // as well would cost every short Nordum marker hit.
        let model = FakeModel(vec![("da", 0.99)]);
        let refiner = FakeRefiner(vec![("no", 0.99)]);
        assert_eq!(
            detect(
                "Kort.",
                &lexicon(),
                Some(&model),
                Some(&refiner),
                &Gates::default()
            ),
            None
        );
        assert_eq!(
            detect(
                "Jei.",
                &lexicon(),
                Some(&model),
                Some(&refiner),
                &Gates::default()
            )
            .map(|d| d.language),
            Some(Lang::Nrd)
        );
    }

    #[test]
    fn short_text_is_enough_for_a_lexicon_hit() {
        // An exclusive marker is evidence at any length, which is what separates
        // the lexicon from the model path.
        assert_eq!(
            detect("Jei.", &lexicon(), None, None, &Gates::default()).map(|d| d.language),
            Some(Lang::Nrd)
        );
    }

    #[test]
    fn low_confidence_abstains() {
        let model = FakeModel(vec![("sv", 0.4)]);
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(
            detect(text, &lexicon(), Some(&model), None, &Gates::default()),
            None
        );
    }

    #[test]
    fn close_call_abstains() {
        let model = FakeModel(vec![("sv", 0.70), ("da", 0.65)]);
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(
            detect(text, &lexicon(), Some(&model), None, &Gates::default()),
            None
        );
    }

    #[test]
    fn clear_winner_is_accepted() {
        let model = FakeModel(vec![("sv", 0.93), ("da", 0.04)]);
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        let got = detect(text, &lexicon(), Some(&model), None, &Gates::default()).unwrap();
        assert_eq!(got.language, Lang::Sv);
        assert_eq!(got.source, Source::Model);
    }

    #[test]
    fn lexicon_overrides_the_model() {
        // The model confidently says Danish; Nordum markers must still win.
        let model = FakeModel(vec![("da", 0.99)]);
        let text = "Jei vet at hun arbeider i dag, og det er viktig å lære språket.";
        let got = detect(text, &lexicon(), Some(&model), None, &Gates::default()).unwrap();
        assert_eq!(got.language, Lang::Nrd);
        assert_eq!(got.source, Source::Lexicon);
    }

    #[test]
    fn cleared_candidates_gates_an_existing_ranking() {
        // The same gate rules `detect` applies, over a ranking the caller built
        // itself (a server restricts and re-rates candidates before deciding).
        let ranking = [
            Candidate {
                lang: Lang::Sv,
                confidence: 0.93,
                source: Source::Model,
            },
            Candidate {
                lang: Lang::Da,
                confidence: 0.04,
                source: Source::Model,
            },
        ];
        let long = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        assert_eq!(
            cleared_candidates(&ranking, long, &Gates::default()).map(|d| d.language),
            Some(Lang::Sv)
        );
        // too short for the model path, whatever the ranking says
        assert_eq!(
            cleared_candidates(&ranking, "Jag.", &Gates::default()),
            None
        );
        // and a lexicon ranking still needs no length
        let lexicon_ranking = [Candidate {
            lang: Lang::Nrd,
            confidence: LEXICON_CONFIDENCE,
            source: Source::Lexicon,
        }];
        assert_eq!(
            cleared_candidates(&lexicon_ranking, "Jei.", &Gates::default()).map(|d| d.language),
            Some(Lang::Nrd)
        );
        // an empty ranking abstains
        assert_eq!(cleared_candidates(&[], long, &Gates::default()), None);
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
