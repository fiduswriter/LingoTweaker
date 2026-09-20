fn main() {
    let text = std::env::args().nth(1).expect("text");
    let engine = lt::Engine::builder(lt::Lang::De).unwrap().build().unwrap();
    for s in engine.analyze(&text) {
        println!("SENTENCE offset={} text={:?}", s.offset, s.text);
        for t in &s.tokens {
            if t.is_whitespace && !t.is_sentence_start {
                continue;
            }
            let tag = t
                .readings
                .iter()
                .map(|r| {
                    format!(
                        "{}:{}",
                        r.stem.as_deref().unwrap_or("null"),
                        r.pos_tag.as_deref().unwrap_or("null")
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            println!("  {:?} {} tagged={}", t.surface(), tag, t.is_tagged);
        }
    }
}
