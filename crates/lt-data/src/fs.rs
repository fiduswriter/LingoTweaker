//! File-system dispatch for vendored data.
//!
//! Engine code reads its data through these helpers instead of calling
//! `std::fs` directly. Native builds behave exactly like before (the helpers
//! delegate to `std::fs`); a build without a file system — `wasm32-unknown-unknown`
//! in a browser — can instead register an in-memory mount built from a data
//! pack (`DataDir::from_pack`), and the same loaders then read from memory.
//!
//! Only the read-side subset the engine uses is wrapped:
//! [`read`], [`read_to_string`], [`open`], [`exists`], [`is_dir`],
//! [`is_file`], plus the [`PathExt`] convenience trait used by call sites
//! that previously wrote `.exists()` / `.is_dir()`.

use std::collections::HashMap;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// An in-memory data tree registered under a virtual base path.
pub(crate) struct Mount {
    pub(crate) base: PathBuf,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) index: HashMap<PathBuf, (usize, usize)>,
}

static MOUNTS: OnceLock<RwLock<Vec<Mount>>> = OnceLock::new();
static NEXT_MOUNT: AtomicUsize = AtomicUsize::new(0);

fn mounts() -> &'static RwLock<Vec<Mount>> {
    MOUNTS.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register a parsed pack and return the virtual base path it is mounted at.
pub(crate) fn register(bytes: Vec<u8>, index: HashMap<PathBuf, (usize, usize)>) -> PathBuf {
    let id = NEXT_MOUNT.fetch_add(1, Ordering::Relaxed);
    let base = PathBuf::from(format!("/lt-mem/{id}"));
    mounts().write().expect("mount lock poisoned").push(Mount {
        base: base.clone(),
        bytes: Arc::from(bytes.into_boxed_slice()),
        index,
    });
    base
}

enum Backend {
    /// File inside a mount, with its `(offset, len)` in the pack.
    PackFile(Arc<[u8]>, usize, usize),
    /// Directory inside a mount (the mount root or a prefix of packed files).
    PackDir,
    /// Path under a mount that holds no such file or directory.
    PackMissing,
    /// Not under any mount: use the real file system.
    Fs,
}

fn classify(path: &Path) -> Backend {
    let mounts = mounts().read().expect("mount lock poisoned");
    // The path must live under exactly one mount's base (the `DataDir`
    // handle); split packs mount a base pack plus sidecars (models, variant
    // dictionaries), and a resource missing in the base pack falls through
    // to the sidecar mounts by its mount-relative path.
    let mut rel_path: Option<PathBuf> = None;
    for mount in mounts.iter() {
        if let Ok(rel) = path.strip_prefix(&mount.base) {
            if rel.as_os_str().is_empty() {
                return Backend::PackDir;
            }
            rel_path = Some(rel.to_path_buf());
            break;
        }
    }
    let Some(rel) = rel_path else {
        return Backend::Fs;
    };
    for mount in mounts.iter() {
        if let Some(&(offset, len)) = mount.index.get(&rel) {
            return Backend::PackFile(Arc::clone(&mount.bytes), offset, len);
        }
        if mount.index.keys().any(|key| key.starts_with(&rel)) {
            return Backend::PackDir;
        }
    }
    Backend::PackMissing
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: not found in data pack", path.display()),
    )
}

fn is_a_directory(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::IsADirectory,
        format!("{}: is a directory in data pack", path.display()),
    )
}

/// Read a file from a data pack or, when the path is not mounted, from disk.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let path = path.as_ref();
    match classify(path) {
        Backend::PackFile(bytes, offset, len) => Ok(bytes[offset..offset + len].to_vec()),
        Backend::PackDir => Err(is_a_directory(path)),
        Backend::PackMissing => Err(not_found(path)),
        Backend::Fs => std::fs::read(path),
    }
}

/// Read a UTF-8 text file from a data pack or disk.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let path = path.as_ref();
    let bytes = read(path)?;
    String::from_utf8(bytes).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}: stream did not contain valid UTF-8: {e}",
                path.display()
            ),
        )
    })
}

/// Open a file for buffered reading (`zip`, XML readers).
pub fn open(path: impl AsRef<Path>) -> io::Result<Cursor<Vec<u8>>> {
    Ok(Cursor::new(read(path)?))
}

/// Whether the path exists as a file or directory in a data pack or on disk.
pub fn exists(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    match classify(path) {
        Backend::PackFile(..) | Backend::PackDir => true,
        Backend::PackMissing => false,
        Backend::Fs => path.exists(),
    }
}

/// Whether the path is a directory in a data pack or on disk.
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    match classify(path) {
        Backend::PackDir => true,
        Backend::PackFile(..) | Backend::PackMissing => false,
        Backend::Fs => path.is_dir(),
    }
}

/// Whether the path is a file in a data pack or on disk.
pub fn is_file(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    match classify(path) {
        Backend::PackFile(..) => true,
        Backend::PackDir | Backend::PackMissing => false,
        Backend::Fs => path.is_file(),
    }
}

/// Mount-aware counterparts of `Path::exists` / `Path::is_dir` /
/// `Path::is_file`. Import anonymously (`use lt_data::PathExt as _;`) where a
/// loader previously called the `std` methods.
pub trait PathExt {
    fn lt_exists(&self) -> bool;
    fn lt_is_dir(&self) -> bool;
    fn lt_is_file(&self) -> bool;
}

impl PathExt for Path {
    fn lt_exists(&self) -> bool {
        exists(self)
    }

    fn lt_is_dir(&self) -> bool {
        is_dir(self)
    }

    fn lt_is_file(&self) -> bool {
        is_file(self)
    }
}
