//! Load a fastText model and print predictions, so a model trained by the C++
//! trainer can be checked against that trainer's own output before it is wired
//! into the engine.
//!
//! Output is one line per input line, `label probability` pairs, in the same
//! shape as `fasttext predict-prob <model> - <k>` but at full `f32` precision so
//! the comparison is numeric rather than a diff of two rounded renderings
//! (`verify_fasttext.sh` does the comparing).
//!
//! ```sh
//! cargo run --release --manifest-path tools/detection/ftz-probe/Cargo.toml -- model.ftz < sentences.txt
//! ```

use std::io::{BufRead, Cursor};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        eprintln!("usage: ftz-probe <model.bin|model.ftz> [top-k]");
        std::process::exit(2);
    });
    let k: usize = args
        .next()
        .map(|a| a.parse().expect("top-k is a number"))
        .unwrap_or(1);

    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let model = fasttext_pure_rs::FastText::load_from_reader(Cursor::new(bytes))
        .unwrap_or_else(|e| panic!("load {path}: {e}"));

    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap_or_else(|e| panic!("read stdin: {e}"));
        if line.trim().is_empty() {
            continue;
        }
        let rendered = model
            .predict(&line, k, 0.0)
            .unwrap_or_else(|e| panic!("predict: {e}"))
            .iter()
            .map(|p| format!("{} {}", p.label, p.probability))
            .collect::<Vec<_>>()
            .join(" ");
        println!("{rendered}");
    }
}
