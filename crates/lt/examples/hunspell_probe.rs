//! Dev probe: run the in-tree hunspell checker over a word list, one
//! `word<TAB>spell()` line per input (same format as
//! `scripts/oracle/de/probe-hunspell.sh` / `ProbeHunspell.java`).
//!
//! Usage: cargo run --release --example hunspell_probe -- <aff> <dic> <words.txt>

use std::io::{BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: hunspell_probe <aff> <dic> <words.txt>");
        std::process::exit(2);
    }
    let checker =
        lt_spell::hunspell::HunspellChecker::load(args[1].as_ref(), args[2].as_ref()).unwrap();
    let file = std::fs::File::open(&args[3]).unwrap();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for line in BufReader::new(file).lines() {
        let line = line.unwrap();
        if line.is_empty() {
            continue;
        }
        writeln!(out, "{line}\t{}", checker.spell(&line)).unwrap();
    }
}
