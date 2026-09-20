//! Debug helper: compile and match a disambiguation rule's antipatterns.
//! Usage: cargo run --release -p lt --example antipattern_probe -- RULE_ID "sentence"
fn main() {
    let rule_id = std::env::args().nth(1).expect("rule id");
    let text = std::env::args().nth(2).expect("text");
    let data = lt::DataDir::discover().unwrap();
    let grammar =
        lt_pattern::Grammar::load_file(data.path().join("en/disambiguation.xml")).unwrap();
    let rule = grammar.rules.iter().find(|r| r.id == rule_id).unwrap();
    println!(
        "rule {} has {} antipatterns",
        rule.id,
        rule.antipatterns.len()
    );
    let mut compiled = Vec::new();
    for (i, ap) in rule.antipatterns.iter().enumerate() {
        match lt_pattern::compile_patterns(&ap.tokens, ap.marker_start, ap.marker_end) {
            Ok(cs) => {
                println!(
                    "AP{i}: {} compiled patterns, {} raw tokens",
                    cs.len(),
                    ap.tokens.len()
                );
                compiled.push((i, cs));
            }
            Err(e) => println!("AP{i}: COMPILE ERROR: {e}"),
        }
    }
    let pipeline = lt::Pipeline::new_english(&data, None, &[], None).unwrap();
    let mut sentences = pipeline.analyze(&text, false);
    let s = &mut sentences[0];
    pipeline.global_chunker.apply(s);
    pipeline.multiword_chunker.apply(s);
    let view: Vec<&lt_core::AnalyzedTokenReadings> = s
        .tokens
        .iter()
        .filter(|t| {
            !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
        })
        .collect();
    for (i, cs) in &compiled {
        for c in cs {
            let ms =
                lt_pattern::matcher::find_matches_with_synth(c, &[], &[], &view, None, None, None);
            if !ms.is_empty() {
                println!(
                    "AP{i} MATCHES: {:?}",
                    ms.iter()
                        .map(|m| (m.start_tok(), m.end_tok()))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    // prefix analysis for one antipattern (debugging why it does not match)
    if let Ok(idx) = std::env::var("AP_PREFIX") {
        let idx: usize = idx.parse().unwrap();
        let ap = &rule.antipatterns[idx];
        println!(
            "VIEW: {:?}",
            view.iter()
                .map(|t| t.surface().to_string())
                .collect::<Vec<_>>()
        );
        for (k, t) in ap.tokens.iter().enumerate() {
            println!(
                "tok{k}: text={:?} regexp={} postag={:?}",
                t.text, t.regexp, t.postag
            );
        }
        for k in 1..=ap.tokens.len() {
            let prefix = &ap.tokens[..k];
            match lt_pattern::compile_patterns(prefix, None, None) {
                Ok(cs) => {
                    let hit = cs.iter().any(|c| {
                        !lt_pattern::matcher::find_matches_with_synth(
                            c,
                            &[],
                            &[],
                            &view,
                            None,
                            None,
                            None,
                        )
                        .is_empty()
                    });
                    println!("prefix {k}: {}", if hit { "MATCH" } else { "no match" });
                }
                Err(e) => println!("prefix {k}: compile error {e}"),
            }
        }
    }
}
