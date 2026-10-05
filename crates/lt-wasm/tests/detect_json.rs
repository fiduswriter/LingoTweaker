//! The detection binding, end to end on the host target.
//!
//! `wasm-bindgen` compiles the exported functions for the host as well, so the
//! JSON a browser gets is asserted here — including the embedded model and the
//! embedded Nordum marker list, neither of which a browser can read from the
//! file system.
//!
//! The accuracy sweep against the whole fixture corpus lives in `lt`
//! (`tests/detect_model.rs`); this pins the shape and the outcomes a caller has
//! to handle: a settled model answer, a lexicon answer, and an abstention.
//! Error paths are unit-tested in `src/detect.rs` instead — `JsError::new`
//! panics off wasm, so they cannot be asserted from here.

use serde_json::Value;

/// Sentences long enough to clear the length gate and unambiguous enough to
/// clear the confidence and margin gates. The Norwegian one runs its runner-up
/// (`da`) far below the margin, so the answer does not depend on the default
/// thresholds sitting where they do today.
const SWEDISH: &str = "Jag arbetar inte i dag, men jag kommer hem efter jobbet.";
const NORWEGIAN: &str = "Det er ikke noe poeng i å gjøre det på den måten; vi må gjøre det slik.";

/// The one word that decides Nordum: it is in no other language we ship.
const NORDUM: &str = "Jei vet at hun arbeider i dag, og det er viktig å lære språket.";

fn detect(text: &str) -> Value {
    serde_json::from_str(&lt_wasm::detect_json(text, None).expect("detection succeeds"))
        .expect("JSON")
}

#[test]
fn a_swedish_sentence_resolves_to_sv_from_the_model() {
    let report = detect(SWEDISH);
    assert_eq!(report["resolved"], "sv");
    assert_eq!(report["detected"]["language"], "sv");
    assert_eq!(report["detected"]["source"], "fasttext");
    assert!(
        report["detected"]["confidence"]
            .as_f64()
            .expect("confidence")
            > 0.6,
        "the default confidence gate let this through: {report}"
    );
    // The ranking is reported alongside the decision, best first.
    assert_eq!(
        report["candidates"][0]["language"],
        report["detected"]["language"]
    );
    assert!(report["candidates"].as_array().expect("array").len() > 1);
}

#[test]
fn a_norwegian_sentence_resolves_to_no() {
    let report = detect(NORWEGIAN);
    assert_eq!(report["resolved"], "no");
    assert_eq!(report["detected"]["source"], "fasttext");
    // Danish is the confusable neighbour and comes second, which is what the
    // margin gate is there to reject when the two run close.
    assert_eq!(report["candidates"][1]["language"], "da");
}

/// The `source` strings are wire values, and this binding is one of two surfaces
/// that emit them. `/v3/detect` sends the same three for the same three layers,
/// and both now come from `lt_core::detect::source_name`, so a client can compare
/// an HTTP response with a wasm result field by field. `lexicon` and
/// `discriminator` are covered by the sentences below; `fasttext` is what every
/// statistical decision reports.
#[test]
fn the_source_strings_are_the_projects_own() {
    use lt_core::detect::{
        source_name, Source, SOURCE_DISCRIMINATOR, SOURCE_LEXICON, SOURCE_MODEL,
    };
    assert_eq!(SOURCE_LEXICON, "lexicon");
    assert_eq!(SOURCE_MODEL, "fasttext");
    assert_eq!(SOURCE_DISCRIMINATOR, "discriminator");
    assert_eq!(source_name(Source::Lexicon), "lexicon");
    assert_eq!(source_name(Source::Model), "fasttext");
    assert_eq!(source_name(Source::Discriminator), "discriminator");

    assert_eq!(
        detect(SWEDISH)["detected"]["source"],
        source_name(lt_core::detect::Source::Model)
    );
    assert_eq!(
        detect(NORDUM)["detected"]["source"],
        source_name(lt_core::detect::Source::Lexicon)
    );
}

#[test]
fn a_nordum_sentence_resolves_to_nrd_from_the_lexicon() {
    // No statistical model knows the language: it answers da/no/sv here, so the
    // lexicon has to be the layer that decides it.
    let report = detect(NORDUM);
    assert_eq!(report["resolved"], "nrd");
    assert_eq!(report["detected"]["language"], "nrd");
    assert_eq!(report["detected"]["source"], "lexicon");
    assert_eq!(report["candidates"].as_array().map(Vec::len), Some(1));
}

#[test]
fn abstention_is_reported_as_a_null_resolved() {
    // Under the length gate with no marker in it: not enough evidence, which the
    // caller must be able to tell apart from a wrong answer.
    let report = detect("Jag kommer.");
    assert_eq!(report["resolved"], Value::Null);
    assert_eq!(report["detected"], Value::Null);
    // The ranking survives an abstention, so a UI can offer it as a suggestion.
    assert_eq!(report["candidates"][0]["language"], "sv");
}

#[test]
fn empty_text_abstains_even_when_the_model_proposes_something() {
    // fastText labels an empty string without complaint; the gates are what
    // refuses the answer, which is the whole reason they exist.
    let report = detect("");
    assert_eq!(report["resolved"], Value::Null);
    assert_eq!(report["detected"], Value::Null);
    assert!(report["candidates"].is_array());
}

#[test]
fn gates_can_be_overridden_from_the_caller() {
    // Same text, gates lowered: the same call shape decides what the default
    // gates abstained on.
    let opened = lt_wasm::detect_json(
        "Han lot meg gå, og boka lå igjen på bordet.",
        Some(r#"{"minChars": 5, "minConfidence": 0.1, "minMargin": 1.0}"#.to_string()),
    )
    .expect("detection succeeds");
    let opened: Value = serde_json::from_str(&opened).expect("JSON");
    assert_eq!(
        detect("Han lot meg gå, og boka lå igjen på bordet.")["resolved"],
        Value::Null
    );
    assert_eq!(opened["resolved"], "no");

    // Gates the text cannot clear: abstention with the ranking still reported.
    let closed = lt_wasm::detect_json(SWEDISH, Some(r#"{"minConfidence": 1.1}"#.to_string()))
        .expect("detection succeeds");
    let closed: Value = serde_json::from_str(&closed).expect("JSON");
    assert_eq!(closed["resolved"], Value::Null);
    assert!(!closed["candidates"].as_array().expect("array").is_empty());
}

#[test]
fn empty_gates_are_the_default_gates() {
    let empty = lt_wasm::detect_json(SWEDISH, Some("{}".to_string())).expect("detection succeeds");
    let none = lt_wasm::detect_json(SWEDISH, None).expect("detection succeeds");
    assert_eq!(empty, none);
}
