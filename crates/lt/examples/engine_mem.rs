//! Dev probe: steady-state RSS of a built engine per language.
//! Usage: cargo run --release -p lt --example engine_mem -- en-US de-DE
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
    for code in std::env::args().skip(1) {
        let lang = lt::Lang::from_long_code(&code).expect("lang");
        println!("{code}: before {} MB", rss_mb());
        let engine = lt::Engine::builder(lang).unwrap().build().unwrap();
        println!("{code}: steady {} MB", rss_mb());
        drop(engine);
        println!("{code}: dropped {} MB", rss_mb());
    }
}
