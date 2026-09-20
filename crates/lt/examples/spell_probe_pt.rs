//! Portuguese speller probe mirroring `scripts/oracle/pt/SpellerProbe.java`:
//! `word<TAB>getSpellingSuggestions` per argument.
//!
//! Usage: cargo run --release --example spell_probe_pt -- [pt-PT|pt-BR|pt-AO|pt-MZ] word1 word2 ...

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next().unwrap_or_default();
    let (variant, words): (String, Vec<String>) = if first.starts_with("pt") {
        (first, args.collect())
    } else {
        let mut all = vec![first];
        all.extend(args);
        ("pt-PT".to_string(), all)
    };
    let data = lt::DataDir::discover().unwrap();
    let rule = lt::spelling_pt_probe(&data, &variant);
    for word in words {
        println!("{word}\t{}", rule.suggestions(&word).join("|"));
    }
}
