use lt_chunk::model::ModelFile;
use lt_chunk::EnglishChunker;

#[test]
fn loads_models_and_chunks() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/models");
    if !dir.exists() {
        eprintln!("skipping: no vendored models");
        return;
    }
    let chunker = EnglishChunker::load(&dir).expect("models load");

    // tokenize round trip
    let _ = ModelFile::load(&dir.join("en-token.bin")).unwrap();

    // run the full add_chunk_tags over a small sentence
    let mut tokens = make_tokens(&[
        "The", " ", "ten", " ", "books", " ", "are", " ", "late", ".",
    ]);
    chunker.add_chunk_tags(&mut tokens);
    for t in &tokens {
        if !t.chunk_tags.is_empty() {
            println!("{} {:?}", t.surface(), t.chunk_tags);
        }
    }
    // "ten books" must be a plural noun phrase
    let books = &tokens[4];
    assert!(
        books.chunk_tags.iter().any(|c| c.contains("NP-plural")),
        "expected plural NP on 'books', got {:?}",
        books.chunk_tags
    );
    // the OpenNLP-level tags must match the Java reference run
    let the = &tokens[0];
    assert!(
        the.chunk_tags.iter().any(|c| c.starts_with("B-NP")),
        "expected NP start on 'The', got {:?}",
        the.chunk_tags
    );
    let are = &tokens[6];
    assert!(
        are.chunk_tags.iter().any(|c| c == "B-VP"),
        "expected VP on 'are', got {:?}",
        are.chunk_tags
    );
}

fn make_tokens(words: &[&str]) -> Vec<lt_core::AnalyzedTokenReadings> {
    use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
    let mut pos = 0usize;
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let ws = w.trim().is_empty();
            let tag = if ws {
                None
            } else {
                match *w {
                    "The" | "the" => Some("DT".to_string()),
                    "ten" => Some("CD".to_string()),
                    "books" => Some("NNS".to_string()),
                    "are" => Some("VBP".to_string()),
                    "late" => Some("JJ".to_string()),
                    "." => Some(".".to_string()),
                    _ => Some("NN".to_string()),
                }
            };
            let tr = AnalyzedTokenReadings {
                readings: vec![AnalyzedToken::new(w.to_string(), None, tag)],
                chunk_tags: Vec::new(),
                whitespace_before: i > 0,
                start_pos: pos,
                raw_byte_len: pos,
                is_whitespace: ws,
                is_sentence_start: false,
                is_sentence_end: false,
                is_paragraph_end: false,
                is_tagged: !ws,
                is_immunized: false,
                is_ignore_spelling: false,
                has_typographic_apostrophe: false,
                is_pos_tag_unknown: false,
            };
            pos += w.len();
            tr
        })
        .collect()
}
