//! Raw German `MorfologikMultiSpeller.getSuggestions` probe (development
//! helper, mirrors `scripts/oracle/de/ProbeMorfo.java`):
//! `word<TAB>isMisspelled<TAB>suggestion/weight|...`.
//!
//! Usage: cargo run --release -p lt --example morfo_rule_probe -- <words.txt> [de-DE|de-AT|de-CH]

use std::io::BufRead;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let variant = args.get(1).cloned().unwrap_or_else(|| "de-DE".to_string());
    let data = lt::DataDir::discover().unwrap();
    let rule = lt::spelling_de_probe(&data, &variant);
    for line in std::io::BufReader::new(std::fs::File::open(&args[0]).unwrap()).lines() {
        let line = line.unwrap();
        if line.is_empty() {
            continue;
        }
        let misspelled = rule.is_misspelled(&line);
        let suggestions: Vec<String> = rule
            .probe_morfo_suggestions(&line)
            .into_iter()
            .map(|(word, weight)| format!("{word}/{weight}"))
            .collect();
        println!("{line}\t{misspelled}\t{}", suggestions.join("|"));
    }
}
