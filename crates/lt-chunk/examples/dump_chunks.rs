//! Dump the raw OpenNLP tokenize/POS/chunk pipeline as TSV, matching the
//! `C` lines of `scripts/oracle/Dump.java` (oracle diff helper).
//!
//! Usage: cargo run --release -p lt-chunk --example dump_chunks <sentences.txt>

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/models");
    let chunker = lt_chunk::EnglishChunker::load(&dir).expect("load OpenNLP models");
    let path = std::env::args().nth(1).expect("usage: dump_chunks <file>");
    let text = std::fs::read_to_string(path).expect("read sentences");
    for (si, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        println!("S\t{}", si + 1);
        for (i, (tok, pos, chunk)) in chunker.opennlp_tag_sentence(line).into_iter().enumerate() {
            let q = |s: &str| s.replace('\t', "\\t").replace('\n', "\\n");
            println!("C\t{}\t{}\t{}\t{}", i, q(&tok), pos, chunk);
        }
    }
}
