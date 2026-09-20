//! `<unify>` loading and matching.

use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::matcher::*;
use lt_pattern::{EquivalenceConfig, Grammar};

fn tr(readings: &[(&str, &str)]) -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: readings
            .iter()
            .map(|(w, t)| {
                AnalyzedToken::new(w.to_string(), Some(w.to_lowercase()), Some(t.to_string()))
            })
            .collect(),
        chunk_tags: Vec::new(),
        whitespace_before: true,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: false,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    }
}

const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="en">
  <unification feature="number">
    <equivalence type="sg"><token postag="NNP"/></equivalence>
    <equivalence type="pl"><token postag="NNS"/></equivalence>
  </unification>
  <category name="Test" id="TEST">
    <rule id="NUM" name="number">
      <pattern>
        <unify>
          <feature id="number"/>
          <token postag="NNP|NNS" postag_regexp="yes"/>
          <token postag="NNP|NNS" postag_regexp="yes"/>
        </unify>
      </pattern>
      <message>number</message>
    </rule>
  </category>
</rules>"#;

static FILE_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn load(name: &str) -> Grammar {
    let dir = std::env::temp_dir().join("lt-pattern-unify-test");
    std::fs::create_dir_all(&dir).unwrap();
    let seq = FILE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("{}-{}-{}", std::process::id(), seq, name));
    std::fs::write(&path, XML).unwrap();
    Grammar::load_file(&path).unwrap()
}

#[test]
fn parses_definitions_and_features() {
    let g = load("defs.xml");
    assert_eq!(g.equivalence_defs.len(), 2);
    assert_eq!(g.equivalence_defs[0].feature, "number");
    assert_eq!(g.equivalence_defs[0].type_name, "sg");
    assert_eq!(g.equivalence_defs[1].type_name, "pl");
    let rule = &g.rules[0];
    assert!(rule.pattern.tokens[0].unification.is_some());
    assert!(rule.pattern.tokens[1].last_in_unification);
    assert!(!rule.complex_pattern);
}

#[test]
fn unify_matches_only_consistent_features() {
    let g = load("match.xml");
    let cfg = EquivalenceConfig::from_defs(&g.equivalence_defs).unwrap();
    let compiled = lt_pattern::compile_patterns(&g.rules[0].pattern.tokens, None, None).unwrap();

    let same = [tr(&[("Sam", "NNP")]), tr(&[("Pete", "NNP")])];
    let refs: Vec<&AnalyzedTokenReadings> = same.iter().collect();
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches_with_unify(c, &[], &refs, None, Some(&cfg)).len())
        .sum();
    assert_eq!(hits, 1);

    let mixed = [tr(&[("Sam", "NNP")]), tr(&[("cats", "NNS")])];
    let refs: Vec<&AnalyzedTokenReadings> = mixed.iter().collect();
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches_with_unify(c, &[], &refs, None, Some(&cfg)).len())
        .sum();
    assert_eq!(hits, 0, "sg vs pl must not unify");

    let plural = [tr(&[("cats", "NNS")]), tr(&[("dogs", "NNS")])];
    let refs: Vec<&AnalyzedTokenReadings> = plural.iter().collect();
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches_with_unify(c, &[], &refs, None, Some(&cfg)).len())
        .sum();
    assert_eq!(hits, 1);

    // without a config the unification test is not applied (unit-test mode)
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches_with_unify(c, &[], &refs, None, None).len())
        .sum();
    assert_eq!(hits, 1);
}

#[test]
fn unified_readings_are_feature_filtered() {
    let g = load("readings.xml");
    let cfg = EquivalenceConfig::from_defs(&g.equivalence_defs).unwrap();
    let compiled = lt_pattern::compile_patterns(&g.rules[0].pattern.tokens, None, None).unwrap();
    let toks = [tr(&[("Sam", "NNP")]), tr(&[("Pete", "NNP")])];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    let m = find_matches_with_unify(&compiled[0], &[], &refs, None, Some(&cfg));
    assert_eq!(m.len(), 1);
    let unified = m[0].unified.as_ref().expect("unified readings");
    assert_eq!(unified.len(), 2);
    assert_eq!(unified[0][0].pos_tag.as_deref(), Some("NNP"));
}

#[test]
fn disambiguation_file_defs_load() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/disambiguation.xml");
    if !path.exists() {
        eprintln!("skipping: no vendored data");
        return;
    }
    let g = Grammar::load_file(&path).unwrap();
    let number = g
        .equivalence_defs
        .iter()
        .filter(|d| d.feature == "number")
        .count();
    assert_eq!(number, 2, "sg/pl equivalence definitions");
    let unify_rules = g
        .rules
        .iter()
        .filter(|r| r.pattern.tokens.iter().any(|t| t.unification.is_some()))
        .count();
    assert_eq!(unify_rules, 2, "DT_NN_SENT_END subrules use <unify>");
    assert!(g.rules.iter().all(|r| !r.complex_pattern));
}
