//! Detection against the real generated fixtures.
//!
//! The unit tests in `detect.rs` pin behaviour with canned model output. This
//! exercises the same code against authentic per-language sentences taken from
//! `data/*/rules/grammar.xml`, plus the committed Nordum marker list, and
//! answers the question the unit tests cannot: do the default gates actually
//! accept real text at the lengths a grammar checker sees?
//!
//! Regenerate the inputs with `python3 tools/detection/generate.py`.

use std::collections::HashMap;

use lt_core::detect::{self, Gates, Lexicon, Model};
use lt_core::Lang;
use serde::Deserialize;

#[derive(Deserialize)]
struct Buckets {
    valid: Vec<String>,
    with_errors: Vec<String>,
}

fn corpus() -> HashMap<String, Buckets> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/detection_corpus.json"
    );
    let raw = std::fs::read_to_string(path)
        .expect("detection_corpus.json; run tools/detection/generate.py");
    serde_json::from_str(&raw).expect("fixture parses")
}

fn nordum_lexicon() -> Lexicon {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/nrd/detection/markers.txt"
    );
    let raw = std::fs::read_to_string(path).expect("markers.txt; run tools/detection/generate.py");
    let mut lex = Lexicon::new();
    lex.load(Lang::Nrd, &raw);
    lex
}

/// A model that answers the truth for fixtures it knows and nothing otherwise.
struct Oracle {
    answers: HashMap<String, Vec<String>>,
}

impl Model for Oracle {
    fn predict(&self, text: &str, _k: usize) -> Vec<(String, f32)> {
        for (language, sentences) in &self.answers {
            if sentences.iter().any(|s| s == text) {
                return vec![(language.clone(), 0.95)];
            }
        }
        Vec::new()
    }
}

#[test]
fn nordum_is_identified_by_lexicon_alone() {
    let lex = nordum_lexicon();
    assert_eq!(
        lex.languages(),
        vec![Lang::Nrd],
        "only Nordum has markers today"
    );

    let corpus = corpus();
    let nrd = &corpus["nrd"];
    let checked = nrd.valid.len() + nrd.with_errors.len();
    assert!(checked > 20, "fixture looks truncated: {checked} sentences");

    // Recall is deliberately partial, and this records the measured value.
    //
    // Nordum draws its core vocabulary from all three source languages, so most
    // ordinary sentences are lexically indistinguishable from Scandinavian:
    // "Han arbeider i dag." and "Vi har ett stort hus." contain no word that is
    // not also Bokmål. Roughly half of authentic sentences carry an exclusive
    // word, so the lexicon cannot be the primary mechanism — it is a
    // high-precision gate, and primary detection is the trained model (which is
    // why Nordum is to be trained in rather than detected lexically).
    let gates = Gates::default();
    let hits = nrd
        .valid
        .iter()
        .chain(nrd.with_errors.iter())
        .filter(|sentence| detect::detect(sentence, &lex, None, &gates).is_some())
        .count();
    let rate = hits as f32 / checked as f32;
    assert!(
        (0.40..=0.75).contains(&rate),
        "Nordum lexicon recall moved to {rate:.0}% ({hits}/{checked}); the markers or the \
         gates changed, so this floor and ceiling need revisiting"
    );
}

#[test]
fn nordum_markers_do_not_fire_on_other_languages() {
    let lex = nordum_lexicon();
    let corpus = corpus();
    let gates = Gates::default();
    let mut checked = 0usize;
    let mut false_positives: Vec<(String, String)> = Vec::new();
    for (lang, buckets) in &corpus {
        if lang == "nrd" {
            continue;
        }
        for sentence in buckets.valid.iter().chain(buckets.with_errors.iter()) {
            checked += 1;
            if let Some(hit) = detect::detect(sentence, &lex, None, &gates) {
                false_positives.push((lang.clone(), sentence.clone()));
                let _ = hit;
            }
        }
    }
    assert!(
        checked > 5_000,
        "corpus looks truncated: {checked} sentences"
    );
    assert!(
        false_positives.is_empty(),
        "Nordum markers fired on {} other-language sentences, e.g. {:?}",
        false_positives.len(),
        &false_positives[..false_positives.len().min(5)]
    );
}

#[test]
fn the_length_gate_applies_to_the_model_path_only() {
    let gates = Gates::default();
    let short = "Jei vet att.";

    // Under the length gate, but it carries an exclusive marker, so the lexicon
    // decides it anyway.
    assert!(short.chars().filter(|c| !c.is_whitespace()).count() < gates.min_chars);
    assert_eq!(
        detect::detect(short, &nordum_lexicon(), None, &gates).map(|d| d.language),
        Some(Lang::Nrd)
    );

    // The same text with no markers and only a model to go on: abstains.
    let model = Oracle {
        answers: HashMap::new(),
    };
    assert_eq!(
        detect::detect(short, &Lexicon::new(), Some(&model), &gates),
        None
    );
}

/// The gates are proposals; this records the current operating point so a change
/// to them has to be a deliberate edit rather than a silent drift.
#[test]
fn gate_defaults_are_unchanged() {
    let gates = Gates::default();
    assert_eq!(gates.min_chars, 40);
    assert_eq!(gates.min_confidence, 0.60);
    assert_eq!(gates.min_margin, 1.5);
}

#[test]
fn model_answers_are_gated_not_taken_at_face_value() {
    // A confident model still loses to the length gate.
    let model = Oracle {
        answers: HashMap::new(),
    };
    let lex = Lexicon::new();
    let gates = Gates::default();
    assert_eq!(
        detect::detect("Jag kommer.", &lex, Some(&model), &gates),
        None
    );
}
