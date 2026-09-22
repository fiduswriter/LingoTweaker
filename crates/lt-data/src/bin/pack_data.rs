//! Build a data pack for one language.
//!
//! ```sh
//! cargo run -p lt-data --bin pack_data -- data gn /tmp/lt-gn.pack
//! ```
//!
//! The pack contains `core/**` and `data/<lang>/**`; see
//! [`lt_data::pack::collect_language`] for why the set is deliberately
//! conservative (and why `manifest.json`/`messages/**` stay out).

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(data_dir), Some(lang), Some(out)) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: pack_data <data-dir> <lang> <out.pack>");
        std::process::exit(2);
    };
    let data_dir = PathBuf::from(data_dir);
    let files = lt_data::pack::collect_language(&data_dir, &lang)?;
    let raw: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
    let pack = lt_data::pack::write(&files);
    std::fs::write(&out, &pack)?;
    println!(
        "{lang}: {} files, {raw} bytes raw, {} bytes packed -> {out}",
        files.len(),
        pack.len()
    );
    Ok(())
}
