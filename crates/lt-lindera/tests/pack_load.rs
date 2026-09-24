//! Proves Lindera's dictionary I/O goes through `lt_data::fs`, including for a
//! dictionary that lives only inside an in-memory data pack (the wasm path).
//!
//! The test needs a compiled Lindera dictionary under
//! `<LT_LINDERA_TEST_DATA>/ja/dictionary` (see the branch notes). It is skipped
//! when that directory is absent so the normal workspace test run is unaffected.

use std::path::{Path, PathBuf};

use lt_lindera::CjkSegmenter;

fn test_data_root() -> PathBuf {
    std::env::var_os("LT_LINDERA_TEST_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/kilo/lt-lindera-testdata"))
}

fn assert_tokens(seg: &CjkSegmenter) {
    let tokens = seg.tokenize("これはペンです。").expect("segment");
    let got: Vec<(String, String, String)> = tokens
        .iter()
        .map(|t| (t.surface.clone(), t.pos.clone(), t.basic_form.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (
                "これ".to_string(),
                "名詞-代名詞-一般".to_string(),
                "これ".to_string()
            ),
            (
                "は".to_string(),
                "助詞-係助詞".to_string(),
                "は".to_string()
            ),
            (
                "ペン".to_string(),
                "名詞-一般".to_string(),
                "ペン".to_string()
            ),
            ("です".to_string(), "助動詞".to_string(), "です".to_string()),
            ("。".to_string(), "記号-句点".to_string(), "。".to_string()),
        ]
    );
}

/// Load straight from a directory on disk.
#[test]
fn loads_dictionary_from_disk_directory() {
    let root = test_data_root();
    let dict = root.join("ja").join("dictionary");
    if !dict.is_dir() {
        eprintln!("skipping: {} not present", dict.display());
        return;
    }
    let seg = CjkSegmenter::from_dict_dir(&dict).expect("load dictionary from disk");
    assert_tokens(&seg);
}

/// Build a `.pack` in memory from the dictionary directory, mount it, and load
/// it through the virtual path. The virtual base (`/lt-mem/<id>`) does not
/// exist on disk, so success proves the reads came from the pack.
#[test]
fn loads_dictionary_from_in_memory_pack() {
    let root = test_data_root();
    if !root.join("ja").join("dictionary").is_dir() {
        eprintln!("skipping: {} not present", root.display());
        return;
    }

    let files = lt_data::pack::collect_paths(&root, &[Path::new("ja")], &[])
        .expect("collect dictionary files");
    let pack = lt_data::pack::write(&files);
    let data = lt_data::DataDir::from_pack_bytes(pack).expect("mount pack");

    let base = data.path();
    assert!(
        base.starts_with("/lt-mem/"),
        "expected a virtual mount base, got {}",
        base.display()
    );
    let dict = base.join("ja").join("dictionary");
    assert!(!dict.exists(), "virtual path must not exist on the real fs");

    let seg = CjkSegmenter::from_dict_dir(&dict).expect("load dictionary from pack");
    assert_tokens(&seg);
}
