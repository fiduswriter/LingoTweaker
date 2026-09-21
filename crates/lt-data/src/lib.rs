//! Discovery and verification of vendored upstream data.
//!
//! Resolution order for the data directory:
//! 1. explicit builder override (`lt::EngineBuilder::data_dir`)
//! 2. `LT_DATA_DIR` environment variable
//! 3. `./data` relative to the current directory
//! 4. `../data` (running from a crate inside the workspace)
//!
//! Both a directory tree and a single pack file are accepted: if the override
//! or `LT_DATA_DIR` names a `.pack`/`.pack.gz` file, it is registered as an
//! in-memory pack (see [`DataDir::from_pack_path`]) and every loader reads
//! through [`fs`] exactly as for a directory.
//!
//! Data may also come from an in-memory pack (`DataDir::from_pack`), which is
//! how wasm builds without a file system are served (see [`pack`]); engine
//! loaders read through [`fs`] so both sources behave identically. Manifest
//! verification (`Manifest::verify`) still needs the real files on disk.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use lt_core::{CoreError, Lang, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub mod fs;
pub mod pack;
pub use fs::PathExt;

#[derive(Debug, Clone)]
pub struct DataDir(PathBuf);

/// Mounts are immutable and content-addressed by their path, so a pack file is
/// parsed once even when several engines are built from it.
static PACK_CACHE: OnceLock<Mutex<HashMap<PathBuf, DataDir>>> = OnceLock::new();

fn pack_cache() -> &'static Mutex<HashMap<PathBuf, DataDir>> {
    PACK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl DataDir {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        // A pack file path is mounted in memory; a directory (or any error,
        // which surfaces on the first read) is used as-is.
        if Self::is_pack_path(path) && path.is_file() {
            if let Ok(dir) = Self::from_pack_path(path) {
                return dir;
            }
        }
        Self(path.to_path_buf())
    }

    /// Whether `path` names a data pack (`.pack`, optionally `.pack.gz`).
    pub fn is_pack_path(path: &Path) -> bool {
        path.to_str()
            .is_some_and(|s| s.ends_with(".pack") || s.ends_with(".pack.gz"))
    }

    /// Load a pack file and register it as an in-memory mount.
    ///
    /// Gzip is detected from the `.gz` suffix. Repeated calls for the same
    /// path reuse the existing mount.
    pub fn from_pack_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(dir) = pack_cache().lock().expect("pack cache poisoned").get(&key) {
            return Ok(dir.clone());
        }
        let raw = std::fs::read(path).map_err(|e| {
            CoreError::Data(format!("cannot read data pack {}: {e}", path.display()))
        })?;
        let bytes = if path.extension().and_then(|e| e.to_str()) == Some("gz") {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(raw.as_slice())
                .read_to_end(&mut out)
                .map_err(|e| {
                    CoreError::Data(format!("cannot gunzip data pack {}: {e}", path.display()))
                })?;
            out
        } else {
            raw
        };
        let dir = Self::from_pack(&bytes)?;
        pack_cache()
            .lock()
            .expect("pack cache poisoned")
            .insert(key, dir.clone());
        Ok(dir)
    }

    /// A data directory backed by an in-memory pack (see [`pack`]).
    ///
    /// The pack is registered as a mount and the returned handle points at a
    /// virtual base path, so all existing `join`/`&Path` plumbing works while
    /// reads resolve from memory.
    pub fn from_pack(bytes: &[u8]) -> Result<Self> {
        let index = pack::parse(bytes)?;
        let base = fs::register(bytes.to_vec(), index);
        Ok(Self(base))
    }

    pub fn discover() -> Result<Self> {
        if let Some(p) = std::env::var_os("LT_DATA_DIR") {
            let p = PathBuf::from(p);
            if p.is_dir() {
                return Ok(Self(p));
            }
            if Self::is_pack_path(&p) && p.is_file() {
                return Self::from_pack_path(&p);
            }
        }
        for candidate in [
            PathBuf::from("data"),
            PathBuf::from("../data"),
            PathBuf::from("../../data"),
        ] {
            if candidate.is_dir() {
                return Ok(Self(candidate));
            }
        }
        Err(CoreError::Data(
            "no data directory found; set LT_DATA_DIR or run from the repository root".into(),
        ))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.0.join("manifest.json")
    }

    pub fn load_manifest(&self) -> Result<Manifest> {
        Manifest::load(&self.manifest_path())
    }

    pub fn grammar_path(&self, lang: Lang) -> PathBuf {
        self.0
            .join(lang.base_code())
            .join("rules")
            .join("grammar.xml")
    }

    pub fn style_path(&self, lang: Lang) -> PathBuf {
        self.0
            .join(lang.base_code())
            .join("rules")
            .join("style.xml")
    }

    pub fn disambiguation_path(&self, lang: Lang) -> PathBuf {
        self.0.join(lang.base_code()).join("disambiguation.xml")
    }

    pub fn pos_dict_path(&self, lang: Lang) -> PathBuf {
        let name = match lang {
            Lang::En => "english.dict",
            Lang::De => "german.dict",
            Lang::Es => "es-ES.dict",
            Lang::Fr => "french.dict",
            Lang::It => "italian.dict",
            Lang::Pt => "portuguese.dict",
            Lang::Nl => "dutch.dict",
            Lang::Ca => "ca-ES.dict",
            Lang::Gl => "galician.dict",
            Lang::Ro => "romanian.dict",
            Lang::Pl => "polish.dict",
            Lang::Sk => "slovak.dict",
            _ => "",
        };
        self.0
            .join(lang.base_code())
            .join("dictionaries")
            .join(name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestSource {
    pub kind: String,
    #[serde(default)]
    pub upstream_path: Option<String>,
    #[serde(default)]
    pub coords: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub source: ManifestSource,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub file_count: usize,
    pub files: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let text = crate::fs::read_to_string(path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
        serde_json::from_str(&text)
            .map_err(|e| CoreError::Parse(path.display().to_string(), e.to_string()))
    }

    /// Verify every entry's size and sha256 against the files on disk.
    pub fn verify(&self, data_dir: &Path) -> Result<VerifyReport> {
        let mut report = VerifyReport::default();
        for entry in &self.files {
            let path = data_dir.join(&entry.path);
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(_) => {
                    report.missing.push(entry.path.clone());
                    continue;
                }
            };
            if bytes.len() as u64 != entry.size {
                report.size_mismatch.push(entry.path.clone());
                continue;
            }
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let digest = format!("{:x}", hasher.finalize());
            if digest != entry.sha256 {
                report.hash_mismatch.push(entry.path.clone());
            } else {
                report.ok += 1;
            }
        }
        Ok(report)
    }
}

#[derive(Debug, Default)]
pub struct VerifyReport {
    pub ok: usize,
    pub missing: Vec<String>,
    pub size_mismatch: Vec<String>,
    pub hash_mismatch: Vec<String>,
}

impl VerifyReport {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.size_mismatch.is_empty() && self.hash_mismatch.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_data() -> Option<DataDir> {
        DataDir::discover().ok()
    }

    #[test]
    fn manifest_verifies_against_vendored_data() {
        let Some(data) = repo_data() else {
            eprintln!("skipping: no vendored data directory found");
            return;
        };
        let manifest = data.load_manifest().expect("manifest loads");
        assert!(
            manifest.file_count > 100,
            "manifest should list vendored files"
        );
        let report = manifest.verify(data.path()).expect("verify runs");
        assert!(
            report.is_clean(),
            "vendored data must match manifest: {report:?}"
        );
    }

    #[test]
    fn known_paths_resolve() {
        let Some(data) = repo_data() else {
            eprintln!("skipping: no vendored data directory found");
            return;
        };
        assert!(data.grammar_path(Lang::En).exists());
        assert!(data.pos_dict_path(Lang::En).exists());
        assert!(data.disambiguation_path(Lang::En).exists());
        assert!(data.grammar_path(Lang::Fr).exists());
    }

    #[test]
    fn pack_file_loads_from_disk() {
        use std::io::Write as _;

        let pack = pack::write(&[
            (PathBuf::from("en/rules/grammar.xml"), b"<rules/>".to_vec()),
            (PathBuf::from("core/segment.srx"), b"<srx/>".to_vec()),
        ]);
        let dir = std::env::temp_dir().join(format!("lt-pack-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Plain `.pack`.
        let plain = dir.join("data.pack");
        std::fs::write(&plain, &pack).unwrap();
        let data = DataDir::new(&plain);
        assert!(fs::is_file(data.grammar_path(Lang::En)));
        assert_eq!(fs::read(data.grammar_path(Lang::En)).unwrap(), b"<rules/>");

        // Gzipped `.pack.gz`.
        let gz = dir.join("data.pack.gz");
        let mut enc = flate2::write::GzEncoder::new(
            std::fs::File::create(&gz).unwrap(),
            flate2::Compression::default(),
        );
        enc.write_all(&pack).unwrap();
        enc.finish().unwrap();
        let data = DataDir::new(&gz);
        assert!(fs::is_file(data.grammar_path(Lang::En)));
        assert_eq!(fs::read(data.grammar_path(Lang::En)).unwrap(), b"<rules/>");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
