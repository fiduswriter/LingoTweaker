//! German speller probe mirroring `scripts/oracle/de/ProbeSpeller.java`:
//! `word<TAB>isMisspelled<TAB>ranges<TAB>suggestions` per input line, so the
//! output can be diffed against the pinned Java build
//! (`scripts/oracle/de/probe-speller.sh`).
//!
//! Usage: cargo run --release --example spell_probe -- <words.txt> [de-DE|de-AT|de-CH]

use std::io::BufRead;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = &args[0];
    let variant = args.get(1).cloned().unwrap_or_else(|| "de-DE".to_string());
    let data = lt::DataDir::discover().unwrap();
    let rule = lt::spelling_de_probe(&data, &variant);
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
        let line = line.unwrap();
        if line.is_empty() {
            continue;
        }
        let misspelled = rule.is_misspelled(&line);
        let (ranges, suggestions) = if misspelled {
            let utf16_len = line.encode_utf16().count();
            (
                format!("0-{utf16_len}"),
                rule.probe_match_suggestions(&line).join("|"),
            )
        } else {
            (String::new(), String::new())
        };
        println!("{line}\t{misspelled}\t{ranges}\t{suggestions}");
    }
}
