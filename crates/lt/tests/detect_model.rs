//! Detection accuracy with the real bundled model.
//!
//! The unit tests in `lt_core::detect` pin behaviour against canned output.
//! This runs the actual `lid.176.ftz` over the generated fixture corpus and
//! reports what it gets right, which is what the thresholds in
//! `docs/language-detection-plan.md` §3.4 have to be calibrated against.
//!
//! Run with output: `cargo test -p lt --test detect_model -- --nocapture`

use std::collections::HashMap;
use std::path::Path;

use lt::detect_model;
use lt_core::detect::{self, Gates, Lexicon, Source};
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
            match detect::detect(sentence, &lex, Some(model), &gates) {
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
            "  {code:5} {rate:5.1}%  ({:>3} correct, {:>3} abstained, {:>3} wrong)",
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
            let mut correct = 0usize;
            let mut abstained = 0usize;
            let mut wrong = 0usize;
            for (code, buckets) in &corpus {
                let Some(expected) = Lang::from_long_code(code) else {
                    continue;
                };
                for sentence in buckets.valid.iter().chain(buckets.with_errors.iter()) {
                    match detect::detect(sentence, &lex, Some(model), &gates) {
                        Some(got) if got.language == expected => correct += 1,
                        Some(_) => wrong += 1,
                        None => abstained += 1,
                    }
                }
            }
            let answered = correct + wrong;
            println!(
                "  chars>={min_chars:3} conf>={min_confidence:.2}  coverage {:5.1}%  precision {:5.1}%  ({correct} correct, {wrong} wrong, {abstained} abstained)",
                answered as f64 / total as f64 * 100.0,
                correct as f64 / answered.max(1) as f64 * 100.0,
            );
        }
    }
    let _ = rate;
}

#[test]
fn the_lexicon_outranks_the_model_for_nordum() {
    let model = detect_model::bundled().expect("model loads");
    let lex = lexicon();
    let text = "Jei vet at hun arbeider i dag, og det er viktig å lære språket.";
    let got = detect::detect(text, &lex, Some(model), &Gates::default()).expect("decided");
    assert_eq!(got.language, Lang::Nrd);
    assert_eq!(got.source, Source::Lexicon);
}
