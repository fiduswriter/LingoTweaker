//! Raw morfologik speller probe (development helper):
//! `word<TAB>isMisspelled<TAB>suggestion/weight|...` for each input line,
//! using the binary dictionary alone (`MorfologikSpeller`).
//!
//! Usage: cargo run --release -p lt --example morfo_probe -- <dict> <info> <words.txt> [edits]

use std::io::BufRead;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dict = PathBuf::from(&args[0]);
    let info = PathBuf::from(&args[1]);
    let edits: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2);
    let speller = lt_spell::MorfologikSpeller::from_dict_file(&dict, &info, edits).unwrap();
    for line in std::io::BufReader::new(std::fs::File::open(&args[2]).unwrap()).lines() {
        let line = line.unwrap();
        if line.is_empty() {
            continue;
        }
        let misspelled = speller.is_misspelled(&line);
        let suggestions: Vec<String> = speller
            .get_suggestions(&line)
            .into_iter()
            .map(|s| format!("{}/{}", s.word, s.weight))
            .collect();
        println!("{line}\t{misspelled}\t{}", suggestions.join("|"));
    }
}
