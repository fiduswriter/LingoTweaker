//! Dev probe: time the German spellers over a word list (one word per line):
//! - the in-tree hunspell port (`lt_spell::hunspell::HunspellChecker`)
//! - the morfologik binary speller (`lt_spell::MorfologikSpeller`) used by
//!   the actual German speller rule
//!
//! Usage: cargo run --release -p lt --example spell_bench_de -- <words.txt> [--suggest N]
//! Prints ops/sec and ms/op to stderr, one block per engine.

use std::io::BufRead;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let suggest_n: usize = match args.iter().position(|a| a == "--suggest") {
        Some(i) => {
            let n = args[i + 1].parse().unwrap();
            args.remove(i + 1);
            args.remove(i);
            n
        }
        None => 500,
    };
    let words: Vec<String> = std::io::BufReader::new(std::fs::File::open(&args[0]).unwrap())
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.is_empty())
        .collect();
    let misspelled: Vec<String> = words
        .iter()
        .filter(|w| w.ends_with('x') || w.ends_with('z') && w.len() > 4)
        .take(suggest_n)
        .cloned()
        .collect();
    let misspelled: Vec<String> = if misspelled.len() >= suggest_n {
        misspelled
    } else {
        words
            .iter()
            .skip(words.len() / 2)
            .take(suggest_n)
            .cloned()
            .collect()
    };

    let data = PathBuf::from(std::env::var("LT_DATA_DIR").unwrap_or_else(|_| "data".into()));

    // --- hunspell port (de_DE) ---
    let hs_dir = data.join("de/hunspell");
    let t0 = Instant::now();
    let checker = lt_spell::hunspell::HunspellChecker::load(
        hs_dir.join("de_DE.aff").as_path(),
        hs_dir.join("de_DE.dic").as_path(),
    )
    .unwrap();
    let load_hs = t0.elapsed();
    let t0 = Instant::now();
    let n_ok = words.iter().filter(|w| checker.spell(w)).count();
    let spell_hs = t0.elapsed();
    let t0 = Instant::now();
    let n_sug: usize = misspelled.iter().map(|w| checker.suggest(w).len()).sum();
    let sug_hs = t0.elapsed();
    report(
        "hunspell-rust",
        &words,
        &misspelled,
        load_hs,
        spell_hs,
        sug_hs,
        n_ok,
        n_sug,
    );

    // --- morfologik (german.dict) ---
    let dict = data.join("de/dictionaries/german.dict");
    let info = data.join("de/dictionaries/german.info");
    let t0 = Instant::now();
    let speller = lt_spell::MorfologikSpeller::from_dict_file(&dict, &info, 2).unwrap();
    let load_mo = t0.elapsed();
    let t0 = Instant::now();
    let n_ok = words.iter().filter(|w| !speller.is_misspelled(w)).count();
    let spell_mo = t0.elapsed();
    let t0 = Instant::now();
    let n_sug: usize = misspelled
        .iter()
        .map(|w| speller.get_suggestions(w).len())
        .sum();
    let sug_mo = t0.elapsed();
    report(
        "morfologik-rust",
        &words,
        &misspelled,
        load_mo,
        spell_mo,
        sug_mo,
        n_ok,
        n_sug,
    );

    let _ = (n_ok, n_sug);
}

#[allow(clippy::too_many_arguments)]
fn report(
    name: &str,
    words: &[String],
    misspelled: &[String],
    load: std::time::Duration,
    spell: std::time::Duration,
    sug: std::time::Duration,
    _n_ok: usize,
    _n_sug: usize,
) {
    let spell_us = spell.as_secs_f64() * 1e6 / words.len() as f64;
    let sug_ms = sug.as_secs_f64() * 1e3 / misspelled.len().max(1) as f64;
    eprintln!(
        "{name}: load {load_ms:.0} ms | spell {n}/{} words {spell_ms:.3} ms/word ({sps:.0}/s) | suggest {m}/{} words {sug_ms:.3} ms/word",
        words.len(),
        misspelled.len(),
        name = name,
        load_ms = load.as_secs_f64() * 1e3,
        n = words.len(),
        spell_ms = spell_us / 1000.0,
        sps = words.len() as f64 / spell.as_secs_f64(),
        m = misspelled.len(),
        sug_ms = sug_ms,
    );
}
