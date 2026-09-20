//! Dumps raw German tagger readings in the same TSV format as
//! `scripts/oracle/de/DumpTags.java` (tagger verification).
//!
//! Usage: cargo run -p lt-tagger --example dump_tags -- <sentences.txt> [de-CH]

use std::io::BufRead;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .expect("usage: dump_tags <sentences.txt> [de-CH]"),
    );
    let variant = args.next().unwrap_or_else(|| "de-DE".to_string());
    let data = lt_data::DataDir::discover().expect("data dir");
    let file = std::fs::File::open(&path).expect("open input");
    let tokenizer = lt_tokenize::GermanWordTokenizer::new();
    let swiss = variant == "de-CH";
    let tagger = if swiss {
        None
    } else {
        Some(lt_tagger::GermanTagger::load(data.path()).expect("tagger"))
    };
    let swiss_tagger = if swiss {
        Some(lt_tagger::SwissGermanTagger::load(data.path()).expect("swiss tagger"))
    } else {
        None
    };
    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("read line");
        if line.is_empty() {
            continue;
        }
        println!("S\t{line}");
        let tokens = tokenizer.tokenize(&line);
        let readings = match (&tagger, &swiss_tagger) {
            (Some(t), _) => t.tag(&tokens, true),
            (_, Some(t)) => t.tag(&tokens, true),
            _ => unreachable!(),
        };
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
