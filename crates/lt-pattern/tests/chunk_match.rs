use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::matcher::*;
use lt_pattern::{compile_pattern, PatternToken};

fn tr(surface: &str, tag: &str, chunk_tags: &[&str]) -> AnalyzedTokenReadings {
    AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(surface, None, Some(tag.into()))],
        chunk_tags: chunk_tags.iter().map(|s| s.to_string()).collect(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: surface.is_empty(),
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

#[test]
fn chunk_attrs_gate_matching() {
    // D-002: chunk="B-NP" requires the token to carry that chunk tag
    let pt = vec![
        PatternToken {
            text: Some("big".into()),
            chunk: Some("B-NP".into()),
            ..Default::default()
        },
        PatternToken {
            text: Some("cat".into()),
            ..Default::default()
        },
    ];
    let c = compile_pattern(&pt, None, None).unwrap();
    let with = [tr("big", "JJ", &["B-NP"]), tr("cat", "NN", &[])];
    let without = [tr("big", "JJ", &[]), tr("cat", "NN", &[])];
    let refs: Vec<&AnalyzedTokenReadings> = with.iter().collect();
    assert_eq!(find_matches(&c, &[], &refs).len(), 1);
    let refs: Vec<&AnalyzedTokenReadings> = without.iter().collect();
    assert_eq!(find_matches(&c, &[], &refs).len(), 0);

    // chunk_re=".-NP.*" with full-match semantics
    let pt_re = vec![PatternToken {
        text: Some("cat".into()),
        chunk_re: Some(".-NP.*".into()),
        ..Default::default()
    }];
    let c_re = compile_pattern(&pt_re, None, None).unwrap();
    let toks = [tr("cat", "NN", &["B-NP-singular"])];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    assert_eq!(find_matches(&c_re, &[], &refs).len(), 1);
    let toks = [tr("cat", "NN", &["B-VP"])];
    let refs: Vec<&AnalyzedTokenReadings> = toks.iter().collect();
    assert_eq!(find_matches(&c_re, &[], &refs).len(), 0);
}
