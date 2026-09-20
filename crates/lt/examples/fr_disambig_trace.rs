//! Debug helper: apply the French disambiguation steps one by one and print
//! every step that changes a token's reading list (the French counterpart of
//! `disambig_trace`/`de_disambig_trace`).
//!
//! Usage: cargo run --release -p lt --example fr_disambig_trace -- "sentence"

fn readings(s: &lt::AnalyzedSentence) -> Vec<String> {
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
}

fn main() {
    let text = std::env::args().nth(1).expect("text");
    let data = lt::DataDir::discover().unwrap();
    let pipeline = lt::Pipeline::new_french(&data, None, &[], None).unwrap();
    let french = pipeline.french.as_ref().unwrap();
    let mut sentences = pipeline.analyze(&text, false);
    let s = &mut sentences[0];
    french.global_chunker.apply(s);
    french.multiwords_chunker.apply(s);
    println!("chunkers: {}", readings(s).join(" "));
    for i in 0..french.disambiguator.rules_len() {
        let before = readings(s);
        french.disambiguator.apply_rule_index(i, s);
        let after = readings(s);
        if before != after {
            let changed: Vec<String> = before
                .iter()
                .zip(after.iter())
                .filter(|(b, a)| b != a)
                .map(|(b, a)| format!("{b} -> {a}"))
                .collect();
            println!(
                "rule {i} ({}): {}",
                french.disambiguator.rule_id(i).unwrap_or("?"),
                changed.join("; ")
            );
        }
    }
}
