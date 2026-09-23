//! Dev probe: per-stage timing for the main languages over a sentences file
//! (one line per sentence). Times engine build, analyze-only, full check,
//! full check with the spelling rule disabled, and spell-only (rules disabled,
//! speller enabled).
//!
//! Usage: cargo run --release -p lt --example stage_bench -- <lang en|de|es|pt|fr> <file> [lines]

use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let lang = args.next().expect("lang en|de|es|pt|fr");
    let file = args.next().expect("file");
    let lines_n: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(1000);

    let (lt_lang, speller_rule): (lt::Lang, &str) = match lang.as_str() {
        "en" => (lt::Lang::En, "MORFOLOGIK_RULE_EN_US"),
        "de" => (lt::Lang::De, "GERMAN_SPELLER_RULE"),
        "es" => (lt::Lang::Es, "MORFOLOGIK_RULE_ES"),
        "fr" => (lt::Lang::Fr, "FR_SPELLING_RULE"),
        "pt" => (lt::Lang::Pt, "MORFOLOGIK_RULE_PT_PT"),
        _ => panic!("unknown lang {lang}"),
    };

    let text = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(lines_n)
        .collect();

    let t0 = Instant::now();
    let engine = lt::Engine::builder(lt_lang).unwrap().build().unwrap();
    let build = t0.elapsed();

    // warmup
    for line in lines.iter().take(50) {
        let _ = engine.check(line).unwrap();
    }

    let bench = |name: &str, options: &lt::EngineOptions| {
        let t = Instant::now();
        let mut matches = 0usize;
        for line in &lines {
            matches += engine
                .check_with_options(line, options)
                .unwrap()
                .matches
                .len();
        }
        let run = t.elapsed();
        let per = run.as_secs_f64() * 1e3 / lines.len() as f64;
        eprintln!(
            "{name}: {per:.3} ms/line ({:.0} lines/s, {matches} matches)",
            lines.len() as f64 / run.as_secs_f64()
        );
        per
    };

    let t = Instant::now();
    for line in &lines {
        let _ = engine.analyze(line);
    }
    let analyze = t.elapsed();
    eprintln!(
        "analyze: {:.3} ms/line",
        analyze.as_secs_f64() * 1e3 / lines.len() as f64
    );

    let full = bench("check(full)", &lt::EngineOptions::default());
    let nospell = bench(
        "check(no-speller)",
        &lt::EngineOptions {
            disabled_rules: vec![speller_rule.to_string()],
            ..Default::default()
        },
    );
    let spellonly = bench(
        "check(spell-only)",
        &lt::EngineOptions {
            enabled_rules: vec![speller_rule.to_string()],
            enabled_only: true,
            ..Default::default()
        },
    );
    eprintln!(
        "build: {:.2} s | lines: {} | rules ~= {:.3} ms/line | speller ~= {:.3} ms/line",
        build.as_secs_f64(),
        lines.len(),
        (nospell - spellonly).max(0.0),
        (full - nospell).max(0.0)
    );
}
