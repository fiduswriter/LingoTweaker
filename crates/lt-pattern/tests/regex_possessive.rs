//! Java possessive-quantifier regex semantics (`{m,n}+`, `*+`, `++`, `?+`).
//!
//! Java parses `\p{Lu}{1,2}+[i]*\p{Lu}` as a possessive quantifier (greedy,
//! no backtracking); the Rust `regex` crate parses the same text as a
//! repetition of the quantified group, matching strictly more. Patterns with
//! possessive quantifiers must therefore keep Java's semantics (former
//! docs/differences.md #9: pl `SUBST_ADJ_UNIFY`'s abbreviation exception
//! wrongly consumed `OKARA`).

use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::matcher::*;
use lt_pattern::{EquivalenceConfig, Grammar};

fn tr(readings: &[(&str, &str, &str)]) -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: readings
            .iter()
            .map(|(w, l, t)| {
                AnalyzedToken::new(w.to_string(), Some(l.to_string()), Some(t.to_string()))
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

fn load(xml: &str) -> Grammar {
    let dir = std::env::temp_dir().join("lt-pattern-possessive-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}-{}.xml", std::process::id(), xml.len()));
    std::fs::write(&path, xml).unwrap();
    Grammar::load_file(&path).unwrap()
}

fn rule_xml(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="pl">
  <unification feature="number">
    <equivalence type="sg"><token postag=".*\bsg\b.*|SENT_END" postag_regexp="yes"/></equivalence>
    <equivalence type="pl"><token postag=".*\bpl(?:tant)?\b.*|SENT_END" postag_regexp="yes"/></equivalence>
  </unification>
  <unification feature="gender">
    <equivalence type="f"><token postag=".*\bf\b.*|SENT_END" postag_regexp="yes"/></equivalence>
    <equivalence type="m1"><token postag=".*\bm1\b.*|SENT_END" postag_regexp="yes"/></equivalence>
    <equivalence type="m2"><token postag=".*\bm2\b.*|SENT_END" postag_regexp="yes"/></equivalence>
    <equivalence type="m3"><token postag=".*\bm3\b.*|SENT_END" postag_regexp="yes"/></equivalence>
  </unification>
  <unification feature="case">
    <equivalence type="nom"><token postag="(?i).*\bnom\b.*|SENT_END" postag_regexp="yes"/></equivalence>
    <equivalence type="acc"><token postag="(?i).*\bacc\b.*|SENT_END" postag_regexp="yes"/></equivalence>
    <equivalence type="voc"><token postag="(?i).*\bvoc\b.*|SENT_END" postag_regexp="yes"/></equivalence>
  </unification>
  <category name="T" id="T">
    <rule id="R" name="r">{inner}</rule>
  </category>
</rules>"#
    )
}

fn matches_for(xml: &str, tokens: &[AnalyzedTokenReadings]) -> usize {
    let g = load(xml);
    let cfg = EquivalenceConfig::from_defs(&g.equivalence_defs).unwrap();
    let rule = &g.rules[0];
    let compiled = lt_pattern::compile_patterns(
        &rule.pattern.tokens,
        rule.pattern.marker_start,
        rule.pattern.marker_end,
    )
    .unwrap();
    let refs: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
    find_matches_with_unify(&compiled[0], &[], &refs, None, Some(&cfg)).len()
}

/// Java whole-match semantics with the possessive quantifier: at most two
/// Lu chars, then optional `i`, then one Lu — 5 uppercase letters do not
/// whole-match even under `CASE_INSENSITIVE | UNICODE_CASE`.
#[test]
fn possessive_quantifier_does_not_overmatch() {
    let okara = tr(&[("OKARA", "okara", "subst:sg:nom:f")]);
    let plain = r#"<pattern><token postag="(?:depr|ger|subst):.*" postag_regexp="yes"/></pattern><message>m</message>"#;
    assert_eq!(matches_for(&rule_xml(plain), &[okara.clone()]), 1);

    let with_exception = r#"<pattern><token postag="(?:depr|ger|subst):.*" postag_regexp="yes"><exception regexp="yes">\p{Lu}{1,2}+[i]*\p{Lu}</exception></token></pattern><message>m</message>"#;
    assert_eq!(
        matches_for(&rule_xml(with_exception), &[okara]),
        1,
        "the abbreviation exception must not consume OKARA (possessive quantifier, no backtracking)"
    );

    // short abbreviations still match: OKA is excluded
    let oka = tr(&[("OKA", "oka", "subst:sg:nom:f")]);
    assert_eq!(matches_for(&rule_xml(with_exception), &[oka]), 0);
}

/// End-to-end: the SUBST_ADJ_UNIFY main pattern over the real pl grammar.xml
/// with the disambiguated readings of `OKARA przedłużający o` (Java fires
/// the rule here; the possessive exception must not kill the noun).
#[test]
fn subst_adj_unify_matches_szampon_readings() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/pl/rules/grammar.xml");
    if !path.exists() {
        eprintln!("skipping: no vendored data");
        return;
    }
    let g = Grammar::load_file(&path).unwrap();
    let rule = g
        .rules
        .iter()
        .find(|r| r.id == "SUBST_ADJ_UNIFY")
        .expect("SUBST_ADJ_UNIFY found");
    let cfg = EquivalenceConfig::from_defs(&g.equivalence_defs).unwrap();
    let compiled = lt_pattern::compile_patterns(
        &rule.pattern.tokens,
        rule.pattern.marker_start,
        rule.pattern.marker_end,
    )
    .unwrap();
    let toks = [
        tr(&[("OKARA", "okara", "subst:sg:nom:f")]),
        tr(&[
            (
                "przedłużający",
                "przedłużać",
                "pact:pl:nom.voc:m1.p1:imperf:aff:refl.nonrefl",
            ),
            (
                "przedłużający",
                "przedłużać",
                "pact:sg:acc:m3:imperf:aff:refl.nonrefl",
            ),
            (
                "przedłużający",
                "przedłużać",
                "pact:sg:nom.voc:m1.m2.m3:imperf:aff:refl.nonrefl",
            ),
        ]),
        tr(&[("o", "o", "prep:acc"), ("o", "o", "prep:loc")]),
    ];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    assert!(
        compiled
            .iter()
            .any(|c| !find_matches_with_unify(c, &[], &refs, None, Some(&cfg)).is_empty()),
        "SUBST_ADJ_UNIFY must match OKARA przedłużający"
    );
}

/// The same possessive pattern as a token selector: `OKA` matches, `OKARA`
/// does not (Java `StringMatcher` whole-match + possessive quantifier).
#[test]
fn possessive_regex_direct_token() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="pl">
  <category name="T" id="T">
    <rule id="R" name="r">
      <pattern>
        <token regexp="yes">\p{Lu}{1,2}+[i]*\p{Lu}</token>
      </pattern>
      <message>m</message>
    </rule>
  </category>
</rules>"##;
    let g = load(xml);
    let rule = &g.rules[0];
    let compiled = lt_pattern::compile_patterns(&rule.pattern.tokens, None, None).unwrap();
    let cfg = EquivalenceConfig::from_defs(&g.equivalence_defs).unwrap();
    let toks = [tr(&[("OKARA", "okara", "subst:sg:nom:f")])];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    assert_eq!(
        find_matches_with_unify(&compiled[0], &[], &refs, None, Some(&cfg)).len(),
        0,
        "OKARA is longer than the possessive bound"
    );
    let toks = [tr(&[("OKA", "oka", "subst:sg:nom:f")])];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    assert_eq!(
        find_matches_with_unify(&compiled[0], &[], &refs, None, Some(&cfg)).len(),
        1
    );
}
