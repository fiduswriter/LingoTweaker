//! Debug helper: apply the Spanish disambiguation rules one by one and print
//! every step that changes a token's readings. Used to compare the rule
//! application order/state against Java (`scripts/oracle/es/DebugDisambig.java`
//! has the same view, but Java's `RuleSet` also filters by text hints).
//!
//! Usage: cargo run --release -p lt --example disambig_trace_es -- "sentence"
fn main() {
    let text = std::env::args().nth(1).expect("text");
    let data = lt::DataDir::discover().unwrap();
    let pipeline = lt::Pipeline::new_spanish(&data, None, &[], None).unwrap();
    let spanish = pipeline.spanish.as_ref().unwrap();
    let mut sentences = pipeline.analyze(&text, false);
    let s = &mut sentences[0];
    spanish.global_chunker.apply(s);
    spanish.multiwords_chunker.apply(s);
    let readings = |s: &lt::AnalyzedSentence| -> Vec<String> {
        s.tokens
            .iter()
            .filter(|t| !t.is_whitespace)
            .map(|t| {
                format!(
                    "{}[{}]",
                    t.surface(),
                    t.readings
                        .iter()
                        .map(|r| r.pos_tag.as_deref().unwrap_or("null"))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect()
    };
    println!("start: {}", readings(s).join(" "));
    for i in 0..spanish.disambiguator.rules_len() {
        let before = readings(s);
        spanish.disambiguator.apply_rule_index(i, s);
        let after = readings(s);
        if before != after {
            let changed: Vec<String> = before
                .iter()
                .zip(after.iter())
                .filter(|(b, a)| b != a)
                .map(|(b, a)| format!("{b} => {a}"))
                .collect();
            println!(
                "[{i}] {} : {}",
                spanish.disambiguator.rule_id(i).unwrap_or("?"),
                changed.join("; ")
            );
        }
    }
    println!("final: {}", readings(s).join(" "));
}
