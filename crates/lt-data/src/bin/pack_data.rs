//! Build a data pack for one language (or a subset of it).
//!
//! ```sh
//! cargo run -p lt-data --bin pack_data -- data gn /tmp/lt-gn.pack
//! # base pack without optional resources (split packs):
//! pack_data data en /tmp/en.pack --exclude en/models en/hunspell/en_GB en/hunspell/en_AU ...
//! # sidecar with only the optional resources:
//! pack_data data en /tmp/en.models.pack --only en/models
//! ```
//!
//! The default pack contains `core/**` and `data/<lang>/**`; see
//! [`lt_data::pack::collect_language`] for why the set is deliberately
//! conservative (and why `manifest.json`/`messages/**` stay out).

use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1);
    let mut positional: Vec<String> = Vec::new();
    let mut only: Vec<PathBuf> = Vec::new();
    let mut exclude: Vec<PathBuf> = Vec::new();
    let mut flag: Option<&'static str> = None;
    for arg in args {
        match arg.as_str() {
            "--only" => flag = Some("only"),
            "--exclude" => flag = Some("exclude"),
            _ => match flag {
                Some("only") => only.push(PathBuf::from(arg)),
                Some("exclude") => exclude.push(PathBuf::from(arg)),
                Some(_) | None => positional.push(arg),
            },
        }
    }
    let (Some(data_dir), Some(lang), Some(out)) = (
        positional.first().cloned(),
        positional.get(1).cloned(),
        positional.get(2).cloned(),
    ) else {
        eprintln!(
            "usage: pack_data <data-dir> <lang> <out.pack> [--only <rel>...] [--exclude <rel>...]"
        );
        std::process::exit(2);
    };
    let data_dir = PathBuf::from(data_dir);
    let files = if only.is_empty() && exclude.is_empty() {
        lt_data::pack::collect_language(&data_dir, &lang)?
    } else {
        // `--only` packs exactly those paths (a sidecar); otherwise the
        // default set, minus any `--exclude` prefixes (the split base pack)
        let roots: Vec<&Path> = if only.is_empty() {
            vec![Path::new("core"), Path::new(&lang)]
        } else {
            only.iter().map(|p| p.as_path()).collect()
        };
        let exclude_refs: Vec<&Path> = exclude.iter().map(|p| p.as_path()).collect();
        lt_data::pack::collect_paths(&data_dir, &roots, &exclude_refs)?
    };
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
