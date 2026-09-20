//! Dumps raw Italian tagger readings in the same TSV format as
//! `scripts/oracle/it/DumpTags.java` (tagger verification).
//!
//! Usage: cargo run -p lt-tagger --example dump_tags_it -- <sentences.txt>

use std::io::BufRead;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: dump_tags_it <sentences.txt>"));
    let data = lt_data::DataDir::discover().expect("data dir");
    let file = std::fs::File::open(&path).expect("open input");
    let tagger = lt_tagger::ItalianTagger::load(data.path()).expect("tagger");
    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("read line");
        if line.is_empty() {
            continue;
        }
        println!("S\t{line}");
        let tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
            lt_tokenize::wordtokenizer::string_tokenize(
                &line,
                &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
            ),
        );
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
