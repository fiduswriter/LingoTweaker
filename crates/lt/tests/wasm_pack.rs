//! Wasm data packs: build an engine from an in-memory pack, with no data
//! directory on disk (`DataDir::from_pack`).
//!
//! The tests skip themselves when the vendored `data/` directory is missing,
//! like the other data-dependent tests.

use std::path::PathBuf;

fn repo_data_dir() -> Option<PathBuf> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    data.is_dir().then_some(data)
}

fn pack_for(lang: &str) -> Option<Vec<u8>> {
    let data = repo_data_dir()?;
    let files = lt_data::pack::collect_language(&data, lang).expect("collect language data");
    Some(lt_data::pack::write(&files))
}

fn check_from_pack(lang: lt::Lang, code: &str) -> Option<usize> {
    let pack = pack_for(code)?;
    let data = lt::DataDir::from_pack(&pack).expect("pack parses");
    let engine = lt::Engine::builder(lang)
        .expect("builder")
        .data_dir(data)
        .build()
        .expect("engine builds from pack");
    let result = engine.check("This are a test.").expect("check runs");
    Some(result.matches.len())
}

#[test]
fn checks_italian_from_memory_pack() {
    let Some(matches) = check_from_pack(lt::Lang::It, "it") else {
        eprintln!("skipping: no vendored data directory found");
        return;
    };
    assert!(matches > 0, "English text must trigger the Italian speller");
}

#[test]
fn checks_guarani_from_memory_pack() {
    let Some(matches) = check_from_pack(lt::Lang::Gn, "gn") else {
        eprintln!("skipping: no vendored data directory found");
        return;
    };
    assert!(matches > 0, "English text must trigger the Guaraní speller");
}

#[test]
fn mount_read_semantics() {
    let pack = lt_data::pack::write(&[
        (PathBuf::from("core/x.txt"), b"hi".to_vec()),
        (PathBuf::from("en/rules/grammar.xml"), b"<rules/>".to_vec()),
    ]);
    let data = lt::DataDir::from_pack(&pack).expect("pack parses");

    assert_eq!(
        lt_data::fs::read_to_string(data.path().join("core/x.txt")).unwrap(),
        "hi"
    );
    assert!(lt_data::fs::exists(
        data.path().join("en/rules/grammar.xml")
    ));
    assert!(lt_data::fs::is_dir(data.path().join("en/rules")));
    assert!(lt_data::fs::is_dir(data.path()));
    assert!(!lt_data::fs::is_dir(data.path().join("core/x.txt")));

    let missing = lt_data::fs::read(data.path().join("core/missing.txt")).unwrap_err();
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    assert!(!lt_data::fs::exists(data.path().join("core/missing.txt")));

    // A path outside every mount still uses the real file system.
    assert!(lt_data::fs::exists("Cargo.toml"));
}
