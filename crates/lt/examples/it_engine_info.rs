fn main() {
    let data = lt::DataDir::discover().unwrap();
    let engine = lt::Engine::builder(lt::Lang::It)
        .unwrap()
        .data_dir(data)
        .build()
        .unwrap();
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
