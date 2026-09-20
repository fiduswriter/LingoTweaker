//! Galician engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Offsets are UTF-8 bytes (the engine
//! format); the Java probes print UTF-16 code units, converted in the comments.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Galician engines hold the tagger dictionary.
fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Gl).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Gl)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(gl) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    gl.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage-3 state: 295 active XML rules (the Java-only replace family is not
/// part of the XML rule count), 285 disambiguation rules and
/// `compile_failures()` = 0. The `HunspellRule` speller loads the `FLAG num`
/// `gl_ES` dictionary.
#[test]
fn galician_engine_state() {
    let _guard = engine_guard();
    let Some(gl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(gl.active_rule_count(), 295);
    assert_eq!(gl.disambig_rule_count(), 285);
    assert_eq!(gl.skipped_counts().filters, 0);
    assert_eq!(gl.skipped_counts().off_by_default, 13);
    assert!(
        gl.compile_failures().is_empty(),
        "{:?}",
        gl.compile_failures()
    );
}

/// `UppercaseSentenceStartRule` (Galician.java example): the second sentence
/// starts lowercase.
#[test]
fn galician_uppercase_sentence_start() {
    let matches = one(
        "Esta casa é vella. foi construida en 1950.",
        "UPPERCASE_SENTENCE_START",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    let m = &matches[0];
    assert_eq!(m.range.start, 20);
    assert_eq!(m.range.end, 23);
    assert_eq!(m.suggestions[0].value, "Foi");
}

/// `SimpleReplaceRule` / `CastWordsRule` (legacy `AbstractSimpleReplaceRule`,
/// getMessage bodies from `Galician`+`CastWordsRule`).
#[test]
fn galician_legacy_replace_rules() {
    let matches = one("O achádego é vello.", "GL_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].suggestions[0].value, "achado");

    let matches = one("A acera é longa.", "GL_CAST_WORDS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].suggestions[0].value, "beirarrúa");
}

/// `HunspellRule` (`HUNSPELL_RULE`) over the `FLAG num` `gl_ES` dictionary:
/// a clear misspelling fires with the `MessagesBundle_gl` message. The
/// suggestion list still comes from the shared bounded search, not upstream's
/// native `hunspell.suggest` ranking (internal notes/D-195).
#[test]
fn galician_speller() {
    let matches = one("O mundo zzz.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].message, "Atopouse un posíbel erro ortográfico");
    let clean = one("O mundo.", "HUNSPELL_RULE");
    assert!(clean.is_empty(), "{clean:?}");
}

/// Standalone `é` is accepted: it is derived from the allomorph entry
/// `érer` via the empty-append suffix `SFX 322 rer 0/… érer`, whose condition
/// is byte-reversed UTF-8 (`test_condition_suffix` must advance past the
/// reversed lead byte).
#[test]
fn galician_speller_accepts_affix_derived_e_acute() {
    let matches = one("O ano é bo.", "HUNSPELL_RULE");
    assert!(matches.is_empty(), "{matches:?}");
}

/// `GENERAL_GENDER_AGREEMENT_ERRORS` (sub-rule 1) `<match postag_replace>`
/// suggestions: the shared pattern engine must render both the inflected form
/// (`Unha vaca`) and the parenthesized failed match (`Un (vaca)`) exactly like
/// the legacy engine, which needs the Galician synthesizer wired into
/// `Pipeline::synthesizer()`.
#[test]
fn galician_gender_agreement_suggestions() {
    let matches = one("Un vaca está no prado.", "GENERAL_GENDER_AGREEMENT_ERRORS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    let values: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(values, ["Unha vaca", "Un (vaca)"]);
}

/// `AbstractSimpleReplaceRule2` family (17–20), one match each with the
/// rule's first suggestion.
#[test]
fn galician_replace2_rules() {
    let cases: [(&str, &str, &str); 4] = [
        ("Vimos unha duna de area.", "GL_REDUNDANCY_REPLACE", "duna"),
        (
            "Raramente é o caso en que acontece isto.",
            "GL_WORDINESS_REPLACE",
            "Raramente acontece",
        ),
        (
            "O curriculum vitae está listo.",
            "GL_BARBARISM_REPLACE",
            "currículo",
        ),
        (
            "Iso é a efectos de proba.",
            "GL_WIKIPEDIA_COMMON_ERRORS",
            "para os efectos de",
        ),
    ];
    for (text, rule, suggestion) in cases {
        let matches = one(text, rule);
        assert_eq!(matches.len(), 1, "{rule}: {matches:?}");
        assert_eq!(matches[0].suggestions[0].value, suggestion, "{rule}");
    }
}
