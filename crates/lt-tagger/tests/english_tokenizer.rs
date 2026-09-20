//! Cross-crate contraction-splitting test.
//!
//! Moved here from `lt-tokenize` so that `lt-tokenize` does not need a
//! dev-dependency back on `lt-tagger`: dev-dependencies take part in
//! `cargo publish` resolution, and the back-edge would form an
//! `lt-tokenize` <-> `lt-tagger` publish cycle. `lt-tagger` already depends
//! on `lt-tokenize` normally, so the direction here is publishable.
//!
//! Run from the repository root or set `LT_DATA_DIR`.

use lt_data::PathExt as _;
use lt_tagger::{Dictionary, DictionaryInfo, EnglishTagger};
use lt_tokenize::EnglishWordTokenizer;

#[test]
fn splits_contractions_like_lt() {
    // with the real tagger, "n't" is a dictionary word and stays intact
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/dictionaries");
    if !dir.lt_exists() {
        eprintln!("skipping: no vendored data");
        return;
    }
    let info = DictionaryInfo::load(&dir.join("english.info")).unwrap();
    let dict = Dictionary::load(&dir.join("english.dict"), &info).unwrap();
    let tagger = EnglishTagger::new(dict);
    let callback = |w: &str| tagger.is_tagged(w);
    let tok = EnglishWordTokenizer::new(&callback);
    assert_eq!(tok.tokenize("don't"), vec!["do", "n't"]);
    assert_eq!(tok.tokenize("it's fine"), vec!["it", "'s", " ", "fine"]);
}
