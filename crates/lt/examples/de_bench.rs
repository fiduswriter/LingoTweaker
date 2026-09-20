//! Dev probe: German per-stage timing (analysis vs rules vs speller).
//! Usage: cargo run --release -p lt --example de_bench -- <file> <mode>
//!   modes: analyze | analyze_raw | check | check_nospell | check_norules
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = args.get(1).expect("file");
    let mode = args.get(2).map(String::as_str).unwrap_or("check");
    let text = std::fs::read_to_string(file).unwrap();
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(100)
        .collect();
    let run_options = match mode {
        "check_nospell" => lt::EngineOptions {
            disabled_rules: vec!["GERMAN_SPELLER_RULE".into()],
            ..Default::default()
        },
        "check_norules" => lt::EngineOptions {
            enabled_rules: vec!["__none__".into()],
            enabled_only: true,
            ..Default::default()
        },
        _ => lt::EngineOptions::default(),
    };
    let t0 = Instant::now();
    let engine = lt::Engine::builder(lt::Lang::De).unwrap().build().unwrap();
    eprintln!("build: {:?}", t0.elapsed());
    let t1 = Instant::now();
    let mut matches = 0usize;
    for line in &lines {
        match mode {
            "analyze" => {
                let _ = engine.analyze(line);
            }
            "analyze_raw" => {
                let _ = engine.analyze_raw(line);
            }
            _ => {
                matches += engine
                    .check_with_options(line, &run_options)
                    .unwrap()
                    .matches
                    .len();
            }
        }
    }
    let run = t1.elapsed();
    println!(
        "mode={mode} lines={} matches={matches} run={run:?} per_line={:?}",
        lines.len(),
        run / lines.len() as u32
    );
}
