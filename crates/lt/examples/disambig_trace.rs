//! Debug helper: apply the English disambiguation rules one by one and print
//! every step that changes a token's readings. Used to compare the rule
//! application order/state against Java.
//!
//! Usage: cargo run --release -p lt --example disambig_trace -- "sentence"
fn main() {
    let text = std::env::args().nth(1).expect("text");
    let data = lt::DataDir::discover().unwrap();
    let pipeline = lt::Pipeline::new_english(&data, None, &[], None).unwrap();
    let mut sentences = pipeline.analyze(&text, false);
    let s = &mut sentences[0];
    pipeline.global_chunker.apply(s);
    pipeline.multiword_chunker.apply(s);
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
    for i in 0..pipeline.disambiguator.rules_len() {
        let before = readings(s);
        pipeline.disambiguator.apply_rule_index(i, s);
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
                pipeline.disambiguator.rule_id(i).unwrap_or("?"),
                changed.join("; ")
            );
        }
    }
    println!("final: {}", readings(s).join(" "));
}
