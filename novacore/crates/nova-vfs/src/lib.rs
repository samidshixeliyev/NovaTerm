//! `nova-vfs` — NovaCore's virtual filesystem layer.
//!
//! Built-in file operations and search are written against [`VfsProvider`], so
//! `ls`/`cp`/`find`/`cat` work uniformly over local disk, archives, remote SSH,
//! or in-memory overlays. Paths are scheme-prefixed URIs (`file://…`,
//! `ssh://host/…`); a bare path implies the `file` scheme.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VfsError {
    #[error("no provider for scheme `{0}`")]
    NoProvider(String),
    #[error("io error: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    /// Unix nanoseconds.
    pub modified: i64,
    pub readonly: bool,
}

/// A backend that resolves one URI scheme.
pub trait VfsProvider: Send + Sync {
    fn scheme(&self) -> &str;
    fn read_dir(&self, path: &str) -> Result<Vec<Entry>, VfsError>;
    fn exists(&self, path: &str) -> bool;
    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError>;
}

/// Routes paths to providers by scheme.
#[derive(Default)]
pub struct Vfs {
    providers: HashMap<String, Box<dyn VfsProvider>>,
}

impl Vfs {
    #[must_use]
    pub fn new() -> Self {
        let mut vfs = Vfs::default();
        vfs.register(Box::new(LocalFs));
        vfs
    }

    pub fn register(&mut self, provider: Box<dyn VfsProvider>) {
        self.providers
            .insert(provider.scheme().to_string(), provider);
    }

    fn resolve(&self, path: &str) -> Result<(&dyn VfsProvider, String), VfsError> {
        if let Some((scheme, rest)) = path.split_once("://") {
            let p = self
                .providers
                .get(scheme)
                .ok_or_else(|| VfsError::NoProvider(scheme.into()))?;
            Ok((p.as_ref(), rest.to_string()))
        } else {
            let p = self
                .providers
                .get("file")
                .ok_or_else(|| VfsError::NoProvider("file".into()))?;
            Ok((p.as_ref(), path.to_string()))
        }
    }

    pub fn read_dir(&self, path: &str) -> Result<Vec<Entry>, VfsError> {
        let (p, rest) = self.resolve(path)?;
        p.read_dir(&rest)
    }
    pub fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        let (p, rest) = self.resolve(path)?;
        p.read(&rest)
    }
    pub fn exists(&self, path: &str) -> bool {
        self.resolve(path)
            .map(|(p, rest)| p.exists(&rest))
            .unwrap_or(false)
    }
}

/// The local-disk provider (scheme `file`).
pub struct LocalFs;

impl VfsProvider for LocalFs {
    fn scheme(&self) -> &str {
        "file"
    }

    fn read_dir(&self, path: &str) -> Result<Vec<Entry>, VfsError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)
            .map_err(|e| VfsError::Io(e.to_string()))?
            .flatten()
        {
            let Ok(meta) = entry.metadata() else { continue };
            let kind = if meta.is_dir() {
                EntryKind::Dir
            } else if meta.file_type().is_symlink() {
                EntryKind::Symlink
            } else if meta.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(0);
            out.push(Entry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().display().to_string(),
                kind,
                size: meta.len(),
                modified,
                readonly: meta.permissions().readonly(),
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        std::fs::read(path).map_err(|e| VfsError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_read_dir_and_exists() {
        let dir = std::env::temp_dir().join(format!("novavfs_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello.txt");
        std::fs::write(&file, b"hi there").unwrap();

        let vfs = Vfs::new();
        let entries = vfs.read_dir(&dir.display().to_string()).unwrap();
        assert!(entries
            .iter()
            .any(|e| e.name == "hello.txt" && e.kind == EntryKind::File && e.size == 8));
        assert!(vfs.exists(&file.display().to_string()));
        assert_eq!(vfs.read(&file.display().to_string()).unwrap(), b"hi there");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_scheme_errors() {
        let vfs = Vfs::new();
        assert!(matches!(
            vfs.read_dir("ssh://host/x"),
            Err(VfsError::NoProvider(_))
        ));
    }
}
