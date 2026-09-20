//! Raw German tagger dump (development helper, mirrors
//! `scripts/oracle/de/DumpTags.java`): `T<TAB>token<TAB>lemma:tag|...` per
//! input line, using `GermanTagger::tag(tokens, true)`.
//!
//! Usage: cargo run --release -p lt --example dump_tags -- <sentences.txt> [de-DE|de-CH]

use lt_tagger::GermanTagger;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let variant = args.get(1).cloned().unwrap_or_else(|| "de-DE".to_string());
    let data = lt::DataDir::discover().unwrap();
    let tagger = GermanTagger::load(data.path()).unwrap();
    let _ = variant;
    for line in std::io::BufRead::lines(std::io::BufReader::new(
        std::fs::File::open(&args[0]).unwrap(),
    )) {
        let line = line.unwrap();
        if line.is_empty() {
            continue;
        }
        let tokens = lt_tokenize::GermanWordTokenizer::new().tokenize(&line);
        println!("S\t{line}");
        for readings in tagger.tag(&tokens, true) {
            let mut parts = Vec::new();
            for r in &readings.readings {
                parts.push(format!(
                    "{}:{}",
                    r.stem.as_deref().unwrap_or("null"),
                    r.pos_tag.as_deref().unwrap_or("null")
                ));
            }
            println!("T\t{}\t{}", readings.surface(), parts.join("|"));
        }
    }
}
