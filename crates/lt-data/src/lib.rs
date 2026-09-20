//! Discovery and verification of vendored upstream data.
//!
//! Resolution order for the data directory:
//! 1. explicit builder override (`lt::EngineBuilder::data_dir`)
//! 2. `LT_DATA_DIR` environment variable
//! 3. `./data` relative to the current directory
//! 4. `../data` (running from a crate inside the workspace)
//!
//! Data may also come from an in-memory pack (`DataDir::from_pack`), which is
//! how wasm builds without a file system are served (see [`pack`]); engine
//! loaders read through [`fs`] so both sources behave identically. Manifest
//! verification (`Manifest::verify`) still needs the real files on disk.

use std::path::{Path, PathBuf};

use lt_core::{CoreError, Lang, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub mod fs;
pub mod pack;
pub use fs::PathExt;

#[derive(Debug, Clone)]
pub struct DataDir(PathBuf);

impl DataDir {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(path.as_ref().to_path_buf())
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
}
