//! Typed values contributed by builtins/plugins. A `CustomValue` still flows
//! through `get`, `where`, `sort-by`, and the renderers via field reflection.

use crate::{Record, Value};

pub trait CustomValue: std::fmt::Debug + Send + Sync {
    /// Type name, e.g. `"file"`, `"process"`, `"git-status"`.
    fn type_name(&self) -> &str;

    /// The field names this value exposes (column/order for views).
    fn fields(&self) -> Vec<String>;

    /// Read a single field as a [`Value`].
    fn get(&self, field: &str) -> Option<Value>;

    /// Project to a plain [`Record`]-backed value (default: from fields+get).
    fn to_base(&self) -> Value {
        let mut r = Record::new();
        for f in self.fields() {
            if let Some(v) = self.get(&f) {
                r.push(f, v);
            }
        }
        Value::Record(r)
    }
}

/// Kind of a filesystem entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    Other,
}

impl FileKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            FileKind::File => "file",
            FileKind::Dir => "dir",
            FileKind::Symlink => "symlink",
            FileKind::Other => "other",
        }
    }
}

/// A filesystem entry — the element type of `ls`'s output table.
#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    /// Unix nanoseconds.
    pub modified: i64,
    pub readonly: bool,
}

impl CustomValue for FileEntry {
    fn type_name(&self) -> &str {
        "file"
    }

    fn fields(&self) -> Vec<String> {
        ["name", "type", "size", "modified", "readonly", "path"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn get(&self, field: &str) -> Option<Value> {
        Some(match field {
            "name" => Value::String(self.name.clone()),
            "type" => Value::String(self.kind.as_str().to_string()),
            "size" => Value::Filesize(self.size),
            "modified" => Value::Date(self.modified),
            "readonly" => Value::Bool(self.readonly),
            "path" => Value::String(self.path.clone()),
            _ => return None,
        })
    }
}

/// A tracked process — the element type of `ps`'s output table.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub status: String,
    pub cpu: f64,
    pub mem: u64,
}

impl CustomValue for ProcessInfo {
    fn type_name(&self) -> &str {
        "process"
    }

    fn fields(&self) -> Vec<String> {
        ["pid", "name", "status", "cpu", "mem"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn get(&self, field: &str) -> Option<Value> {
        Some(match field {
            "pid" => Value::Int(self.pid as i64),
            "name" => Value::String(self.name.clone()),
            "status" => Value::String(self.status.clone()),
            "cpu" => Value::Float(self.cpu),
            "mem" => Value::Filesize(self.mem),
            _ => return None,
        })
    }
}
