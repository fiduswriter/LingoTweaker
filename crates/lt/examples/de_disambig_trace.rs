//! Debug helper: apply the German disambiguation rules one by one and print
//! every step that changes a token's readings (the German counterpart of
//! `disambig_trace`).
//!
//! Usage: cargo run --release -p lt --example de_disambig_trace -- "sentence"
fn main() {
    let text = std::env::args().nth(1).expect("text");
    let data = lt::DataDir::discover().unwrap();
    let pipeline = lt::Pipeline::new_german(&data, None, &[], None).unwrap();
    let mut sentences = pipeline.analyze(&text, false);
    let s = &mut sentences[0];
    let german = pipeline.german.as_ref().unwrap();
    german.multitoken_chunker.apply(s);
    german.global_chunker.apply(s);
    german.multitoken_suggest_chunker.apply(s);
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
                        .map(|r| {
                            format!(
                                "{}:{}",
                                r.stem.as_deref().unwrap_or("null"),
                                r.pos_tag.as_deref().unwrap_or("null")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect()
    };
    println!("start: {}", readings(s).join(" "));
    for i in 0..german.disambiguator.rules_len() {
        let before = readings(s);
        german.disambiguator.apply_rule_index(i, s);
        let after = readings(s);
        if before != after {
            let changed: Vec<String> = before
                .iter()
                .zip(after.iter())
                .filter(|(b, a)| b != a)
                .map(|(b, a)| format!("{b} => {a}"))
                .collect();
            println!(
                "[{}] {} : {}",
                i,
                german.disambiguator.rule_id(i).unwrap_or("?"),
                changed.join(" | ")
            );
        }
    }
    println!("final: {}", readings(s).join(" "));
}
