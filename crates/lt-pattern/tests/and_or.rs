//! `<and>`/`<or>` group loading and matching (P2.1b).

use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::matcher::*;
use lt_pattern::{compile_pattern, compile_patterns, Grammar, PatternToken};

fn tr(surface: &str, readings: &[(&str, Option<&str>)]) -> AnalyzedTokenReadings {
    tr_chunk(surface, readings, &[])
}

fn tr_chunk(
    _surface: &str,
    readings: &[(&str, Option<&str>)],
    chunks: &[&str],
) -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: readings
            .iter()
            .map(|(surface, tag)| {
                AnalyzedToken::new(surface.to_string(), None, tag.map(String::from))
            })
            .collect(),
        chunk_tags: chunks.iter().map(|s| s.to_string()).collect(),
        whitespace_before: false,
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

static FILE_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn load(xml: &str, name: &str) -> Grammar {
    // unique file per call to keep parallel tests from clobbering each other
    let dir = std::env::temp_dir().join("lt-pattern-and-or-test");
    std::fs::create_dir_all(&dir).unwrap();
    let seq = FILE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("{}-{}-{}", std::process::id(), seq, name));
    std::fs::write(&path, xml).unwrap();
    Grammar::load_file(&path).unwrap()
}

const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="en">
  <category name="Test" id="TEST">
    <rule id="AND_RULE" name="and">
      <pattern>
        <token>its</token>
        <and>
          <token postag="NN.*" postag_regexp="yes" />
          <token postag="SENT_END" />
        </and>
      </pattern>
      <message>and</message>
    </rule>
    <rule id="OR_RULE" name="or">
      <pattern>
        <or>
          <token chunk="B-NP-singular" />
          <token chunk="B-NP-plural" />
        </or>
        <token>dog</token>
      </pattern>
      <message>or</message>
    </rule>
    <rule id="UNIFY_RULE" name="unify">
      <pattern>
        <unify negate="yes">
          <feature id="x"><type>y</type></feature>
          <token>a</token>
        </unify>
      </pattern>
      <message>unify</message>
    </rule>
  </category>
</rules>"#;

#[test]
fn loader_builds_groups_and_or_expands_at_compile() {
    let g = load(XML, "and_or.xml");
    assert_eq!(g.rules.len(), 3, "{g:?}");
    let and_rule = g.rules.iter().find(|r| r.id == "AND_RULE").unwrap();
    assert!(!and_rule.complex_pattern, "and must not be complex");
    assert_eq!(and_rule.pattern.tokens.len(), 2);
    let group = &and_rule.pattern.tokens[1].and_group;
    assert_eq!(group.len(), 1);
    assert_eq!(group[0].postag.as_deref(), Some("SENT_END"));

    let or_rule = g.rules.iter().find(|r| r.id == "OR_RULE").unwrap();
    assert!(!or_rule.complex_pattern, "or must not be complex");
    assert_eq!(or_rule.pattern.tokens.len(), 2);
    assert_eq!(or_rule.pattern.tokens[0].or_group.len(), 1);
    assert_eq!(
        or_rule.pattern.tokens[0].or_group[0].chunk.as_deref(),
        Some("B-NP-plural")
    );

    let unify_rule = g.rules.iter().find(|r| r.id == "UNIFY_RULE").unwrap();
    assert!(!unify_rule.complex_pattern, "unify is supported");
    assert!(
        unify_rule.pattern.tokens[0].unification.is_some(),
        "unify features are parsed"
    );
    assert!(unify_rule.pattern.tokens[0].last_in_unification);

    let compiled = compile_patterns(
        &or_rule.pattern.tokens,
        or_rule.pattern.marker_start,
        or_rule.pattern.marker_end,
    )
    .unwrap();
    assert_eq!(compiled.len(), 2, "one compiled pattern per OR alternative");
}

#[test]
fn and_group_requires_all_members_on_the_same_token() {
    let g = load(XML, "and_or.xml");
    let rule = g.rules.iter().find(|r| r.id == "AND_RULE").unwrap();
    let c = compile_pattern(&rule.pattern.tokens, None, None).unwrap();

    // one token with two readings: the group member NN.* matches one, the
    // SENT_END member the other
    let both = [
        tr("its", &[("its", Some("PRP$"))]),
        tr("run", &[("run", Some("NNS")), ("run", Some("SENT_END"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = both.iter().collect();
    assert_eq!(find_matches(&c, &[], &refs).len(), 1);

    // member postag matches, SENT_END member does not
    let only_nn = [
        tr("its", &[("its", Some("PRP$"))]),
        tr("run", &[("run", Some("NNS"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = only_nn.iter().collect();
    assert_eq!(find_matches(&c, &[], &refs).len(), 0);
}

#[test]
fn or_group_matches_any_alternative() {
    let g = load(XML, "and_or.xml");
    let rule = g.rules.iter().find(|r| r.id == "OR_RULE").unwrap();
    let compiled = compile_patterns(&rule.pattern.tokens, None, None).unwrap();

    let plural = [
        tr_chunk("dogs", &[("dogs", Some("NNS"))], &["B-NP-plural"]),
        tr("dog", &[("dog", Some("NN"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = plural.iter().collect();
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches(c, &[], &refs).len())
        .sum();
    assert_eq!(hits, 1);

    let singular = [
        tr_chunk("dog", &[("dog", Some("NN"))], &["B-NP-singular"]),
        tr("dog", &[("dog", Some("NN"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = singular.iter().collect();
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches(c, &[], &refs).len())
        .sum();
    assert_eq!(hits, 1);

    let other = [
        tr_chunk("cat", &[("cat", Some("NN"))], &["B-VP"]),
        tr("dog", &[("dog", Some("NN"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = other.iter().collect();
    let hits: usize = compiled
        .iter()
        .map(|c| find_matches(c, &[], &refs).len())
        .sum();
    assert_eq!(hits, 0);
}

#[test]
fn or_in_sequence_keeps_element_positions() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="en">
  <category name="Test" id="TEST">
    <rule id="SEQ" name="seq">
      <pattern>
        <token>x</token>
        <or>
          <token>a</token>
          <token>b</token>
        </or>
        <token>y</token>
      </pattern>
      <message>seq</message>
    </rule>
  </category>
</rules>"#;
    let g = load(xml, "seq.xml");
    let rule = &g.rules[0];
    let compiled = compile_patterns(&rule.pattern.tokens, None, None).unwrap();
    let toks = [
        tr("x", &[("x", Some("NN"))]),
        tr("b", &[("b", Some("NN"))]),
        tr("y", &[("y", Some("NN"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    let m: Vec<_> = compiled
        .iter()
        .flat_map(|c| find_matches(c, &[], &refs))
        .collect();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].positions, vec![Some(0), Some(1), Some(2)]);
}

#[test]
fn and_members_see_negation_and_missing_readings() {
    let pt = [
        PatternToken {
            text: Some("x".into()),
            ..Default::default()
        },
        PatternToken {
            postag: Some("NN".into()),
            ..Default::default()
        },
    ];
    let mut with_group = pt[1].clone();
    with_group.and_group = vec![PatternToken {
        postag: Some("VB".into()),
        ..Default::default()
    }];
    let tokens = vec![pt[0].clone(), with_group];
    let c = compile_pattern(&tokens, None, None).unwrap();

    // "x run": run has NN but not VB -> no match
    let nn_only = [
        tr("x", &[("x", Some("NN"))]),
        tr("run", &[("run", Some("NN"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = nn_only.iter().collect();
    assert_eq!(find_matches(&c, &[], &refs).len(), 0);

    // run has both NN and VB readings -> match
    let both = [
        tr("x", &[("x", Some("NN"))]),
        tr("run", &[("run", Some("NN")), ("run", Some("VB"))]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = both.iter().collect();
    assert_eq!(find_matches(&c, &[], &refs).len(), 1);
}
