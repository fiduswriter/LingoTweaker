//! Dumps raw Spanish tagger readings in the same TSV format as
//! `scripts/oracle/es/DumpTags.java` (tagger verification).
//!
//! Usage: cargo run -p lt-tagger --example dump_tags_es -- <sentences.txt>

use std::io::BufRead;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: dump_tags_es <sentences.txt>"));
    let data = lt_data::DataDir::discover().expect("data dir");
    let file = std::fs::File::open(&path).expect("open input");
    let tagger = lt_tagger::SpanishTagger::load(data.path()).expect("tagger");
    let is_tagged = |w: &str| tagger.is_tagged_word(w);
    let tokenizer = lt_tokenize::SpanishWordTokenizer::new(&is_tagged);
    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("read line");
        if line.is_empty() {
            continue;
        }
        println!("S\t{line}");
        let tokens = tokenizer.tokenize(&line);
        let readings = tagger.tag(&tokens);
        for (token, atr) in tokens.iter().zip(readings.iter()) {
            let mut sb = String::from("T\t");
            sb.push_str(token);
            sb.push('\t');
            for (j, r) in atr.readings.iter().enumerate() {
                if j > 0 {
                    sb.push('|');
                }
                sb.push_str(r.stem.as_deref().unwrap_or("null"));
                sb.push(':');
                sb.push_str(r.pos_tag.as_deref().unwrap_or("null"));
            }
            println!("{sb}");
        }
    }
}
