//! Stage-1/3 Portuguese engine report: active XML rules, disambiguation rules,
//! skipped counts and compile failures per variant.
//!
//! Usage: `cargo run -p lt --example pt_engine_info [pt-PT|pt-BR|pt-AO|pt-MZ]`

fn main() {
    let variant = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "pt-PT".to_string());
    let data = lt::DataDir::discover().unwrap();
    let engine = lt::Engine::builder(lt::Lang::Pt)
        .unwrap()
        .data_dir(data)
        .variant(variant.clone())
        .build()
        .unwrap();
    println!("variant: {variant}");
    println!("active rules: {}", engine.active_rule_count());
    println!("disambig rules: {}", engine.disambig_rule_count());
    println!("skipped: {:?}", engine.skipped_counts());
    let failures = engine.compile_failures();
    println!("compile failures: {}", failures.len());
    let mut by_class: std::collections::BTreeMap<String, usize> = Default::default();
    for (_id, err) in failures {
        *by_class.entry(err.clone()).or_default() += 1;
    }
    for (err, n) in by_class {
        println!("  {n} x {err}");
    }
}
