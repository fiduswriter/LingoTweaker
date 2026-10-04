//! Detection accuracy with the real bundled model.
//!
//! The unit tests in `lt_core::detect` pin behaviour against canned output.
//! This runs the actual `lid.176.ftz` over the generated fixture corpus and
//! reports what it gets right, which is what the gates in `lt_core::detect`
//! have to be calibrated against.
//!
//! Run with output: `cargo test -p lt --test detect_model -- --nocapture`

use std::collections::HashMap;
use std::path::Path;

use lt::{detect_model, detect_refiner};
use lt_core::detect::{self, Gates, Lexicon, Model, Refiner, Source};
use lt_core::Lang;
use serde::Deserialize;

#[derive(Deserialize)]
struct Buckets {
    valid: Vec<String>,
    with_errors: Vec<String>,
}

fn repo(relative: &str) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    base.join(relative).to_string_lossy().into_owned()
}

fn corpus() -> HashMap<String, Buckets> {
    let raw = std::fs::read_to_string(repo("crates/lt-core/tests/fixtures/detection_corpus.json"))
        .expect("fixture; run tools/detection/generate.py");
    serde_json::from_str(&raw).expect("fixture parses")
}

fn lexicon() -> Lexicon {
    let raw = std::fs::read_to_string(repo("data/nrd/detection/markers.txt"))
        .expect("markers; run tools/detection/generate.py");
    let mut lex = Lexicon::new();
    lex.load(Lang::Nrd, &raw);
    lex
}

struct Outcome {
    correct: usize,
    abstained: usize,
    wrong: Vec<(String, Lang)>,
}

#[test]
fn reports_accuracy_per_language() {
    let model = detect_model::bundled().expect("model loads");
    let refiner = detect_refiner::bundled();
    let lex = lexicon();
    let gates = Gates::default();
    let corpus = corpus();

    let mut lines = Vec::new();
    let mut total_correct = 0usize;
    let mut total_abstained = 0usize;
    let mut total = 0usize;
    let mut nrd: Option<Outcome> = None;
    let mut weak: Vec<(String, f64)> = Vec::new();

    for (code, buckets) in &corpus {
        let expected = match Lang::from_long_code(code) {
            Some(lang) => lang,
            None => continue,
        };
        let mut out = Outcome {
            correct: 0,
            abstained: 0,
            wrong: Vec::new(),
        };
        for sentence in buckets.valid.iter().chain(buckets.with_errors.iter()) {
            total += 1;
            match detect::detect(sentence, &lex, Some(model), refiner, &gates) {
                Some(got) if got.language == expected => {
                    out.correct += 1;
                    total_correct += 1;
                }
                Some(got) => out.wrong.push((sentence.clone(), got.language)),
                None => {
                    out.abstained += 1;
                    total_abstained += 1;
                }
            }
        }
        let seen = out.correct + out.abstained + out.wrong.len();
        let rate = if seen == 0 {
            0.0
        } else {
            out.correct as f64 / seen as f64
        };
        lines.push(format!(
            "  {code:5} {:5.1}%  ({:>3} correct, {:>3} abstained, {:>3} wrong)",
            rate * 100.0,
            out.correct,
            out.abstained,
            out.wrong.len()
        ));
        if code == "nrd" {
            nrd = Some(out);
        } else if seen > 0 {
            weak.push((code.clone(), rate));
        }
    }

    lines.sort_by_key(|l| l.to_string());
    println!(
        "per-language accuracy (valid + error-bearing sentences):\n{}",
        lines.join("\n")
    );

    let rate = total_correct as f64 / total as f64;
    println!(
        "\noverall: {}/{} correct ({:.1}%), {} abstained ({:.1}%)",
        total_correct,
        total,
        rate * 100.0,
        total_abstained,
        total_abstained as f64 / total as f64 * 100.0
    );

    weak.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let worst: Vec<String> = weak
        .iter()
        .take(8)
        .map(|(c, r)| format!("{c} {:.0}%", r * 100.0))
        .collect();
    println!("weakest: {}", worst.join(", "));

    // Nordum must be decided by the lexicon: no statistical model knows the
    // language, so a model-sourced Nordum answer is always a mistake.
    if let Some(out) = &nrd {
        for (sentence, got) in &out.wrong {
            assert_ne!(
                *got,
                Lang::Nrd,
                "Nordum answered from the model, which has no such label: {sentence:?}"
            );
        }
    }

    // The gate defaults have not been calibrated yet. Sweep them here so the
    // choice is made against measurements rather than a guess; the operating
    // point is pinned in `lt_core::detect`'s own test.
    for min_chars in [0usize, 20, 40, 60, 80] {
        for min_confidence in [0.60f32, 0.45, 0.30] {
            let gates = Gates {
                min_chars,
                min_confidence,
                ..Gates::default()
            };
            println!(
                "  chars>={min_chars:3} conf>={min_confidence:.2}  {}",
                sweep(&corpus, &lex, model, refiner, &gates, total)
            );
        }
    }

    // `non_latin_weight` is the one gate parameter that decides whether detection
    // works outside Latin scripts at all, and its calibration table lives in
    // `lt_core::detect`. Printed here so the table is reproducible rather than
    // asserted: at 1 the detector abstains on 399 of the fixture's 401 Japanese
    // sentences, having been asked of a model that had 348 of them right.
    println!("\nnon-Latin character weight:");
    for weight in [1usize, 3, 5, 8, 10] {
        let gates = Gates {
            non_latin_weight: weight,
            ..Gates::default()
        };
        println!(
            "  weight {weight:>2}  {}",
            sweep(&corpus, &lex, model, refiner, &gates, total)
        );
    }
    let _ = rate;
}

/// One line of gate-sweep output: coverage, precision and the three counts.
fn sweep(
    corpus: &HashMap<String, Buckets>,
    lex: &Lexicon,
    model: &dyn Model,
    refiner: Option<&dyn Refiner>,
    gates: &Gates,
    total: usize,
) -> String {
    let (mut correct, mut abstained, mut wrong) = (0usize, 0usize, 0usize);
    for (code, buckets) in corpus {
        let Some(expected) = Lang::from_long_code(code) else {
            continue;
        };
        for sentence in buckets.valid.iter().chain(buckets.with_errors.iter()) {
            match detect::detect(sentence, lex, Some(model), refiner, gates) {
                Some(got) if got.language == expected => correct += 1,
                Some(_) => wrong += 1,
                None => abstained += 1,
            }
        }
    }
    let answered = correct + wrong;
    format!(
        "coverage {:5.1}%  precision {:5.1}%  ({correct} correct, {wrong} wrong, {abstained} abstained)",
        answered as f64 / total as f64 * 100.0,
        correct as f64 / answered.max(1) as f64 * 100.0,
    )
}

#[test]
fn the_lexicon_outranks_the_model_for_nordum() {
    let model = detect_model::bundled().expect("model loads");
    let lex = lexicon();
    let text = "Jei vet at hun arbeider i dag, og det er viktig å lære språket.";
    let got = detect::detect(
        text,
        &lex,
        Some(model),
        detect_refiner::bundled(),
        &Gates::default(),
    )
    .expect("decided");
    assert_eq!(got.language, Lang::Nrd);
    assert_eq!(got.source, Source::Lexicon);
}

/// The discriminator can only add answers, never replace one.
///
/// That is the guarantee the layer is built on, and it is exact rather than
/// statistical: the refiner is only consulted when the primary model would have
/// abstained, so a settled answer cannot change. It matters more than the
/// aggregate, because the permissive alternative — let the discriminator overrule
/// a confident primary answer whenever it is more confident itself — scores
/// *better* on the fixture (59.0% correct against 58.5%) while turning
/// "Jag arbetar inte i dag, men jag kommer hem efter jobbet." from Swedish into
/// Danish. The aggregate hides that because it overrules wrong answers too.
///
/// This walks the whole fixture and requires every settled answer to survive.
#[test]
fn a_settled_answer_is_never_replaced() {
    let model = detect_model::bundled().expect("model loads");
    let refiner = detect_refiner::bundled().expect("refiner loads");
    let lex = lexicon();
    let gates = Gates::default();
    let corpus = corpus();

    let mut settled = 0usize;
    let mut from_refiner = 0usize;
    for buckets in corpus.values() {
        for sentence in buckets.valid.iter().chain(buckets.with_errors.iter()) {
            let without = detect::detect(sentence, &lex, Some(model), None, &gates);
            let with = detect::detect(sentence, &lex, Some(model), Some(refiner), &gates);
            match (without, with) {
                (Some(before), Some(after)) => {
                    assert_eq!(
                        before.language, after.language,
                        "refiner replaced a settled answer: {sentence:?}"
                    );
                    if after.source == Source::Discriminator {
                        from_refiner += 1;
                    }
                    settled += 1;
                }
                (None, Some(after)) => {
                    assert_eq!(
                        after.source,
                        Source::Discriminator,
                        "abstention became an answer without the refiner: {sentence:?}"
                    );
                    from_refiner += 1;
                }
                (Some(_), None) => {
                    panic!("refiner turned a settled answer into an abstention: {sentence:?}")
                }
                (None, None) => {}
            }
        }
    }
    println!("{settled} settled answers kept, {from_refiner} of them plus abstentions answered by the refiner");
    assert!(
        from_refiner > 200,
        "the refiner barely fired: {from_refiner}"
    );
}

/// The discriminator may only speak for the languages it was trained on.
///
/// Not "is accurate on them" — *may only*. Its safety argument is that it can
/// only ever produce an answer from its own label set, so a language it knows
/// nothing about can never be answered *because of it*, and no other language's
/// calibration moves.
///
/// What it cannot promise is that it is never consulted about one. It fires on
/// what the primary model said, not on what the truth is, so Breton text the
/// primary model ranks `gl` first does reach it — and comes back `gl`. This walks
/// the whole fixture and pins both halves: every discriminator-sourced answer is
/// inside the label set, and those false firings stay rare enough to be a
/// regression guard rather than a hope.
#[test]
fn the_discriminator_only_answers_for_its_own_languages() {
    let model = detect_model::bundled().expect("model loads");
    let refiner = detect_refiner::bundled().expect("refiner loads");
    let lex = lexicon();
    let gates = Gates::default();
    let corpus = corpus();
    let confusable = [
        Lang::Es,
        Lang::Ast,
        Lang::Ca,
        Lang::Gl,
        Lang::Pt,
        Lang::Fr,
        Lang::It,
        Lang::Ro,
        Lang::Da,
        Lang::Sv,
        Lang::No,
        Lang::Nn,
        Lang::Is,
        Lang::Crh,
    ];

    let mut outside = 0usize;
    let mut outside_total = 0usize;
    for (code, buckets) in &corpus {
        let Some(expected) = Lang::from_long_code(code) else {
            continue;
        };
        for sentence in buckets.valid.iter().chain(buckets.with_errors.iter()) {
            let hit = detect::detect(sentence, &lex, Some(model), Some(refiner), &gates);
            if hit.as_ref().map(|hit| hit.source) != Some(Source::Discriminator) {
                continue;
            }
            let answer = hit.expect("source implies an answer").language;
            assert!(
                confusable.contains(&answer),
                "discriminator answered {code} text with {answer:?}, which is outside its label set"
            );
            if !confusable.contains(&expected) {
                outside += 1;
                outside_total += 1;
            }
        }
        if !confusable.contains(&expected) {
            outside_total += buckets.valid.len() + buckets.with_errors.len();
        }
    }

    let share = outside as f64 / outside_total.max(1) as f64 * 100.0;
    println!(
        "discriminator answered {outside} of {outside_total} non-confusable sentences ({share:.2}%)"
    );
    assert!(
        share < 1.0,
        "discriminator fired on {share:.2}% of sentences for languages it does not cover"
    );
}
