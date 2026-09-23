fn build(data: lt::DataDir, variant: Option<&str>) -> Result<usize, Box<dyn std::error::Error>> {
    let mut b = lt::Engine::builder(lt::Lang::En)?.data_dir(data);
    if let Some(v) = variant {
        b = b.variant(v);
    }
    let e = b.build()?;
    eprintln!(
        "rules: {}, failures: {}",
        e.active_rule_count(),
        e.compile_failures().len()
    );
    let r = e.check("This is a testte. He go to school yesterday.")?;
    eprintln!(
        "matches: {} {:?}",
        r.matches.len(),
        r.matches
            .iter()
            .map(|m| m.rule_id.clone())
            .collect::<Vec<_>>()
    );
    Ok(r.matches.len())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = &args[0];
    let (pack_args, variant) = match args.last().map(String::as_str) {
        Some(v) if !v.ends_with(".pack") && v.contains('-') => {
            (args[1..args.len() - 1].to_vec(), Some(v.to_string()))
        }
        _ => (args[1..].to_vec(), None),
    };
    let variant = variant.as_deref();
    let data = if mode == "full" {
        lt::DataDir::from_pack_path(&pack_args[0])?
    } else {
        let packs: Vec<Vec<u8>> = pack_args
            .iter()
            .map(|p| std::fs::read(p).unwrap())
            .collect();
        lt::DataDir::from_packs(packs)?
    };
    build(data, variant)?;
    Ok(())
}
