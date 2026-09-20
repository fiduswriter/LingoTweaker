//! Dev probe: RSS of the speller dictionary loads (`HunspellChecker` and the
//! morfologik `SpellChecker`) for one variant.
//! Usage: cargo run --release -p lt-spell --example load_probe
fn rss_mb() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    pages * 4 / 1024
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/de/hunspell");
    println!("start {} MB", rss_mb());
    let t = std::time::Instant::now();
    let h =
        lt_spell::hunspell::HunspellChecker::load(&dir.join("de_DE.aff"), &dir.join("de_DE.dic"))
            .unwrap();
    println!("hunspell {} MB in {:?}", rss_mb(), t.elapsed());
    let t = std::time::Instant::now();
    let s = lt_spell::SpellChecker::new(&dir.join("de_DE.dict"), &dir.join("de_DE.info")).unwrap();
    println!("morfo {} MB in {:?}", rss_mb(), t.elapsed());
    std::hint::black_box((&h, &s));
}
