fn main() {
    let data = lt_data::DataDir::discover().unwrap();
    let engine = lt::Engine::builder(lt::Lang::Fr)
        .unwrap()
        .data_dir(data)
        .build()
        .unwrap();
    println!("active rules: {}", engine.active_rule_count());
    println!("disambig rules: {}", engine.disambig_rule_count());
    println!("skipped: {:?}", engine.skipped_counts());
    let failures = engine.compile_failures();
    println!("compile failures: {}", failures.len());
    for (id, err) in failures.iter().take(30) {
        println!("  {id}: {err}");
    }
}
