//! Dutch speller probe mirroring `scripts/oracle/nl/SpellerProbe.java`:
//! `word<TAB>getSpellingSuggestions` per argument.
//!
//! Usage: cargo run --release --example spell_probe_nl -- word1 word2 ...

fn main() {
    let data = lt::DataDir::discover().unwrap();
    let rule = lt::spelling_nl_probe(&data);
    for word in std::env::args().skip(1) {
        println!("{word}\t{}", rule.suggestions(&word).join("|"));
    }
}
