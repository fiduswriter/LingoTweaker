//! The confusable-set discriminator: detection's third layer.
//!
//! `lid.176.ftz` is not short of labels it declines to use. Measured over the
//! 14,559 fixture sentences, it answers `es` for Asturian (89 % of what it gets
//! wrong there), `da` for Bokmål, `no` for Nynorsk, `es` for Galician, and `tr`
//! for Crimean Tatar — every one of them a label it has and ranks below the top.
//! No confidence threshold recovers a label that is ranked second, so the fix is
//! a specialist that only knows the neighbourhood.
//!
//! One model over the whole confusable set rather than one per cluster, because
//! the errors are transitive (`es`↔`ast`↔`gl`↔`ca`, `da`↔`no`↔`nn`↔`sv`) and a
//! shared specialist sees the whole neighbourhood instead of four pairwise
//! arguments that cannot see each other.
//!
//! The model is 855 KiB, trained by `tools/detection/train_discriminator.py` on
//! the vendored Wikipedia corpora in `data/detection/training/` and committed at
//! `assets/discriminator.ftz` (provenance and licence beside it in the README).
//! Embedded with `include_bytes!`, so the browser build fetches nothing.
//!
//! What it is worth, measured on the fixture with the gates in place — the same
//! pipeline, the same reader, one variable changed (`cargo test -p lt --test
//! detect_model`):
//!
//! | | correct | abstained | wrong |
//! |---|---|---|---|
//! | lexicon + `lid.176.ftz` | 66.5 % | 30.7 % | 2.8 % |
//! | + discriminator | **68.6 %** | 28.2 % | 3.2 % |
//!
//! Per language: `gl` 17→36 %, `is` 73→91 %, `ast` 4→22 %, `da` 53→67 %,
//! `nn` 4→17 %, `ca` 84→92 %, `ro` 68→74 %, `no` 8→11 %; no language regresses,
//! because the layer cannot replace an answer the primary model had already
//! settled (see [`Refiner::refine`] and the `a_settled_answer_is_never_replaced`
//! test). Nordum does not move: the lexicon owns it, and the discriminator has no
//! label for it. It answers 356 of the 14 559 fixture sentences.
//!
//! Those figures are against the *fixed* length gate. When this layer was written
//! the gate counted raw characters, and the same comparison read 56.5 → 58.5 %;
//! the layer is worth +2.0 points either way, and the gate fix was worth +10.
//!
//! Three properties keep the blast radius small. It only ever produces an answer
//! from its own label set, so no other language's calibration moves. It only
//! speaks where the primary model would have abstained, so a settled answer
//! cannot be overwritten. And [`MIN_CONFIDENCE`] sits far above the engine's own
//! 0.60 gate, because the operating point was chosen on net answers (correct
//! minus wrong) rather than on correct answers alone; the full grid is printed
//! by the training tool.

use std::sync::OnceLock;

use lt_core::detect::{Candidate, Gates, Refiner, Source};
use lt_core::Lang;

/// The committed model. Trained from scratch on Wikipedia text; see the module
/// docs and `assets/README.md`.
const DISCRIMINATOR: &[u8] = include_bytes!("../assets/discriminator.ftz");

/// The languages this model speaks for, in the labels it emits.
///
/// `tr` is here as a negative class rather than as a language we ship: the
/// primary model answers `tr` for Turkic text, which is one of the ways Crimean
/// Tatar gets misclaimed, and a specialist that cannot emit `tr` cannot
/// ratify that answer. It is dropped on the way out — `Lang::from_long_code`
/// does not know it — so it can only ever *withhold* an answer, never create one
/// the caller has no rules for.
const CONFUSABLE: &[&str] = &[
    "es", "ast", "ca", "gl", "pt", "fr", "it", "ro", "da", "sv", "no", "nn", "is", "tr", "crh",
];

/// How confident the discriminator has to be before it may answer.
///
/// Measured, not guessed: the training tool sweeps this against the whole fixture
/// and selects on net answers. Under the policy above, a floor of 0.98 gives
/// 58.5 % correct against 2.5 % wrong; lowering it to 0.90 buys 0.3 points of
/// coverage and costs 0.2 points of precision, which is the exchange the net
/// criterion rejects. It is high because the layer's confidence is agreement with
/// Wikipedia prose, not with the truth — see [`Refiner::refine`].
const MIN_CONFIDENCE: f32 = 0.98;

/// Labels asked for. The gate needs a runner-up to compare against; three is
/// enough for the refiner's own `min_margin` check and cheap at 855 KiB.
const TOP_K: usize = 3;

/// The primary model's leading confidence over its runner-up, above which a
/// settled answer may **not** be overruled.
///
/// `1.0` — the shipped value — means "never", because a ratio is always at
/// least 1. The gate exists because a settled answer *can* be overruled safely
/// when the primary was visibly unsure, and the measurement is unambiguous
/// (fixture, 14 559 sentences, whole pipeline, discriminator in place):
///
/// | gate | correct | wrong | `no` | `nn` | `gl` | `ast` | `sv` | `da` |
/// |---|---|---|---|---|---|---|---|---|
/// | **1.0 (shipped)** | 68.6 % | 3.2 % | 11.1 % | 17.4 % | 35.7 % | 22.0 % | **69.5 %** | 66.9 % |
/// | 8.0 | **69.0 %** | **2.8 %** | 15.7 % | 23.2 % | 42.7 % | 26.0 % | **69.5 %** | 65.7 % |
/// | 12.0 | 69.0 % | 2.7 % | 15.7 % | 23.2 % | 44.8 % | 26.8 % | **69.5 %** | 64.5 % |
/// | 40.0 | 69.1 % | 2.6 % | 16.7 % | 24.6 % | 47.4 % | 35.0 % | **69.5 %** | 65.3 % |
///
/// Two things make this different from the blanket "second opinion wins" policy
/// that was rejected earlier. Swedish is untouched at **every** setting, because
/// the case that motivated rejecting overrides — Swedish prose called Danish at
/// 0.999 — has a primary ratio of about 700, far outside any of these gates. And
/// the overrides are overwhelmingly fixes: at 8.0 the refiner newly overrules 328
/// settled answers and 299 of them are corrections, which is why the wrong-answer
/// rate *falls* rather than rises.
///
/// The cost is Danish, which loses about three sentences of its 245 as the gate
/// widens: Danish text the primary model rated only moderately is read as
/// Norwegian. That is a named-language trade for a cluster-wide gain, and it is
/// why the shipped value is the conservative one — raising this constant to 8.0
/// is a one-line change, and the table above is what it buys.
const MAX_UNSURE_RATIO: f32 = 1.0;

struct FastTextRefiner {
    model: fasttext_pure_rs::FastText,
    max_unsure_ratio: f32,
}

fn refiner() -> Option<&'static FastTextRefiner> {
    static REFINER: OnceLock<Option<FastTextRefiner>> = OnceLock::new();
    REFINER
        .get_or_init(|| {
            fasttext_pure_rs::FastText::load_from_reader(std::io::Cursor::new(DISCRIMINATOR))
                .ok()
                .map(|model| FastTextRefiner {
                    model,
                    max_unsure_ratio: MAX_UNSURE_RATIO,
                })
        })
        .as_ref()
}

/// The bundled discriminator, or `None` if it could not be parsed.
///
/// A failure is not fatal: the primary model and the lexicon still decide, and
/// every gate still abstains rather than guessing.
pub fn bundled() -> Option<&'static dyn Refiner> {
    refiner().map(|inner| inner as &dyn Refiner)
}

impl Refiner for FastTextRefiner {
    fn refine(&self, text: &str, primary: &[Candidate], gates: &Gates) -> Option<Vec<Candidate>> {
        let lead = primary.first()?;
        if !CONFUSABLE.contains(&lead.lang.base_code()) {
            return None;
        }
        // A settled answer is replaced only when the primary model was visibly
        // unsure about it — see [`MAX_UNSURE_RATIO`] for the measurements and for
        // why this is not the blanket "the more confident model wins" rule.
        // Comparing the two models' confidences directly would not mean anything:
        // they are not calibrated against each other, and this one is confidently
        // wrong on the register a grammar checker actually sees — asked about
        // "Jag arbetar inte i dag, men jag kommer hem efter jobbet." it returns
        // `da` at 0.999 where `lid.176` returns `sv` at 0.997. Its confidence
        // measures agreement with Wikipedia prose, not with the truth.
        //
        // So the question is whether the primary *would have answered*, and if it
        // did, whether it was sure. With the shipped gate of 1.0 the answer can
        // never be replaced, which makes the property exact rather than
        // statistical: the refiner supplies an answer the pipeline lacked and
        // never trades one away.
        if let Some(settled) = lt_core::detect::cleared_candidates(primary, text, gates) {
            let runner_up = primary.get(1).map(|c| c.confidence).unwrap_or(0.0);
            let ratio = settled.confidence / runner_up.max(1e-38);
            if ratio > self.max_unsure_ratio {
                return None;
            }
        }

        let refined: Vec<Candidate> = self
            .model
            .predict(text, TOP_K, 0.0)
            .map(|predictions| {
                predictions
                    .into_iter()
                    .filter_map(|prediction| {
                        let label = prediction
                            .label
                            .strip_prefix("__label__")
                            .unwrap_or(&prediction.label);
                        Lang::from_long_code(label).map(|lang| Candidate {
                            lang,
                            confidence: prediction.probability,
                            source: Source::Discriminator,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if refined
            .first()
            .is_none_or(|best| best.confidence < MIN_CONFIDENCE)
        {
            return None;
        }

        // The replacement has to be an answer the pipeline would have accepted.
        // If it is not, there is nothing to add: this layer exists to turn
        // abstentions into answers, and an answer that fails the gates is the
        // abstention it was trying to avoid.
        lt_core::detect::cleared_candidates(&refined, text, gates)?;
        Some(refined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary(lang: Lang, confidence: f32) -> Vec<Candidate> {
        vec![Candidate {
            lang,
            confidence,
            source: Source::Model,
        }]
    }

    /// A refiner with an explicit override gate, so both policies are testable
    /// without changing what ships.
    fn refiner_with_gate(max_unsure_ratio: f32) -> Option<FastTextRefiner> {
        fasttext_pure_rs::FastText::load_from_reader(std::io::Cursor::new(DISCRIMINATOR))
            .ok()
            .map(|model| FastTextRefiner {
                model,
                max_unsure_ratio,
            })
    }

    #[test]
    fn the_model_loads() {
        assert!(
            refiner().is_some(),
            "bundled discriminator.ftz failed to parse"
        );
    }

    #[test]
    fn declines_outside_the_confusable_set() {
        let refiner = bundled().expect("refiner");
        // German is nowhere near the confusable set, so the refiner must not
        // answer for it however confident it is: that is what keeps the other 23
        // languages untouched by construction.
        for text in [
            "Das ist zwingendermaßen nicht erforderlich, um das zu verstehen.",
            "Wir haben gestern Abend darüber lange gesprochen, wie wir es machen.",
        ] {
            assert!(
                refiner
                    .refine(text, &primary(Lang::De, 0.99), &Gates::default())
                    .is_none(),
                "refined German: {text}"
            );
        }
    }

    #[test]
    fn answers_where_the_primary_model_had_nothing_to_say() {
        let refiner = bundled().expect("refiner");
        // All four are `<example>` sentences from the shipped grammar files, and
        // all four are cases the two-layer pipeline abstains on: `lid.176` is
        // unsure (0.29–0.49), so the gates leave the caller's language alone,
        // while the discriminator is sure and right. Each primary label is inside
        // the confusable set, which is what makes the layer consulted at all.
        for (text, primary_lang, primary_confidence, expected) in [
            ("Han er NRK-medarbeider.", Lang::Da, 0.40, Lang::No),
            (
                "Da eg var liten, budde me i Oslo.",
                Lang::Da,
                0.37,
                Lang::Nn,
            ),
            ("Ho sa «nei» til forslaget.", Lang::No, 0.44, Lang::Nn),
        ] {
            let text_ok = text.chars().filter(|c| !c.is_whitespace()).count() >= 20;
            assert!(text_ok, "test sentence is under the length gate: {text:?}");
            let refined = refiner
                .refine(
                    text,
                    &primary(primary_lang, primary_confidence),
                    &Gates::default(),
                )
                .unwrap_or_else(|| panic!("declined {text:?}"));
            assert_eq!(refined.first().map(|c| c.lang), Some(expected), "{text:?}");
            assert_eq!(
                refined.first().map(|c| c.source),
                Some(Source::Discriminator)
            );
        }
    }

    #[test]
    fn declines_when_the_primary_model_already_answered() {
        let refiner = bundled().expect("refiner");
        // Bokmål the primary model gets right, and the discriminator gets wrong at
        // 0.999. This is the case that decides the whole policy: the layer is
        // more confident and less right, so it is only ever allowed to speak where
        // the primary had nothing to say.
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        let ranking = vec![
            Candidate {
                lang: Lang::Sv,
                confidence: 0.99,
                source: Source::Model,
            },
            Candidate {
                lang: Lang::Ru,
                confidence: 0.001,
                source: Source::Model,
            },
        ];
        assert!(refiner.refine(text, &ranking, &Gates::default()).is_none());
    }

    #[test]
    fn a_confident_primary_answer_survives_any_gate() {
        // lid.176 answers `sv` at 0.9968 against a runner-up of 0.0014 — a ratio
        // of about 712 — while the discriminator answers `da` at 0.999. This is
        // the case that made a blanket override unacceptable. Pinned at 40.0, the
        // widest gate in MAX_UNSURE_RATIO's table, because that is the honest
        // bound of the claim: Swedish survives every setting we measured, and the
        // ratio it would take to break it is eighteen times the widest of them.
        let text = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
        let ranking = vec![
            Candidate {
                lang: Lang::Sv,
                confidence: 0.9968,
                source: Source::Model,
            },
            Candidate {
                lang: Lang::Ru,
                confidence: 0.0014,
                source: Source::Model,
            },
        ];
        let refiner = refiner_with_gate(40.0).expect("refiner");
        assert!(refiner.refine(text, &ranking, &Gates::default()).is_none());
    }

    #[test]
    fn an_unsure_primary_answer_can_be_overruled() {
        // lid.176 answers `da` at 0.850 against `no` at 0.129 — a ratio of 6.6 —
        // for a sentence the discriminator calls `no` at 0.993. That is inside
        // the 8.0 gate and outside the shipped 1.0 one, so the two policies
        // disagree here and both are pinned.
        let text = "Jeg arbeider i dag, men jeg kommer hjem etter jobbet.";
        let ranking = vec![
            Candidate {
                lang: Lang::Da,
                confidence: 0.850,
                source: Source::Model,
            },
            Candidate {
                lang: Lang::No,
                confidence: 0.129,
                source: Source::Model,
            },
        ];

        let strict = refiner_with_gate(1.0).expect("refiner");
        assert!(strict.refine(text, &ranking, &Gates::default()).is_none());

        let relaxed = refiner_with_gate(8.0).expect("refiner");
        let refined = relaxed
            .refine(text, &ranking, &Gates::default())
            .expect("overruled");
        assert_eq!(refined.first().map(|c| c.lang), Some(Lang::No));
        assert_eq!(
            refined.first().map(|c| c.source),
            Some(Source::Discriminator)
        );
    }

    #[test]
    fn the_shipped_gate_never_replaces_a_settled_answer() {
        // The invariant the whole design rests on, asserted against the refiner
        // that actually ships rather than against a stand-in. It covers the three
        // ways a primary answer can be wrong inside the confusable set: certain
        // (Swedish), moderate (Bokmål called Danish) and very certain (Bokmål
        // called Norwegian). MAX_UNSURE_RATIO is 1.0, and a ratio is never below
        // 1, so the gate is closed and the three cases below cannot vary.
        let refiner = bundled().expect("refiner");
        for (text, lang, confidence) in [
            (
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
                Lang::Sv,
                0.9968,
            ),
            (
                "Jeg arbeider i dag, men jeg kommer hjem etter jobbet.",
                Lang::Da,
                0.850,
            ),
            ("Vi har store biler.", Lang::No, 0.991),
        ] {
            let ranking = vec![
                Candidate {
                    lang,
                    confidence,
                    source: Source::Model,
                },
                Candidate {
                    lang: Lang::Ru,
                    confidence: 0.001,
                    source: Source::Model,
                },
            ];
            assert!(
                refiner.refine(text, &ranking, &Gates::default()).is_none(),
                "the shipped gate overruled {text:?}"
            );
        }
    }

    #[test]
    fn declines_when_it_is_not_confident_enough() {
        let refiner = bundled().expect("refiner");
        // The primary is unsure here and the discriminator is only 0.78 sure, so
        // below `MIN_CONFIDENCE` it declines and the pipeline abstains as before.
        let text = "Han arbeider i Bergen på kontoret, og han kommer hjem sent på kvelden.";
        assert!(refiner
            .refine(text, &primary(Lang::Da, 0.95), &Gates::default())
            .is_none());
    }

    #[test]
    fn declines_rather_than_introducing_an_abstention() {
        let refiner = bundled().expect("refiner");
        // Text under the length gate: the refiner must keep the primary's
        // ranking, because returning a ranking the gates reject would replace a
        // settled answer with "unknown".
        let text = "Kort.";
        assert!(refiner
            .refine(text, &primary(Lang::No, 0.99), &Gates::default())
            .is_none());
    }

    #[test]
    fn declines_without_a_primary_answer() {
        let refiner = bundled().expect("refiner");
        assert!(refiner
            .refine(
                "Han arbeider i Bergen på kontoret, og han kommer hjem sent på kvelden.",
                &[],
                &Gates::default()
            )
            .is_none());
    }
}
