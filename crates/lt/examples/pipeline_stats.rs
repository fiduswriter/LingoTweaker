fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lang = if args.first().map(|s| s == "de").unwrap_or(false) {
        lt::Lang::De
    } else {
        lt::Lang::En
    };
    let mut builder = lt::Engine::builder(lang).unwrap();
    if let Some(variant) = args.get(1) {
        builder = builder.variant(variant.clone());
    }
    let engine = builder.build().unwrap();
    println!("lang: {:?}", engine.lang());
    println!("active rules: {}", engine.active_rule_count());
    println!("disambiguation rules: {}", engine.disambig_rule_count());
    println!("skipped: {:?}", engine.skipped_counts());
    let failures = engine.compile_failures();
    println!("compile failures: {}", failures.len());
    let mut by_reason: std::collections::HashMap<String, usize> = Default::default();
    for (_, reason) in failures {
        *by_reason.entry(reason.clone()).or_default() += 1;
    }
    let mut entries: Vec<_> = by_reason.into_iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    for (reason, count) in entries.iter().take(30) {
        println!("  {count:5}  {reason}");
    }
}
