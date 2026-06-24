//! `nova-value` — NovaCore's structured value model.
//!
//! Every command in NovaCore consumes and produces [`Value`]s. Text is just one
//! *view* of a value (see [`View`]). Pipelines move typed data — records,
//! tables, and typed [`CustomValue`]s like [`FileEntry`] — not strings.

#![forbid(unsafe_code)]

pub mod custom;
pub mod record;
pub mod render;
pub mod table;

pub use custom::{CustomValue, FileEntry, FileKind, ProcessInfo};
pub use record::Record;
pub use table::Table;

use std::cmp::Ordering;
use std::sync::Arc;

/// A render hint. The UI maps the value's shape + this hint to a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Auto,
    Table,
    Tree,
    Grid,
    Cards,
    Timeline,
    Raw,
}

/// A structured, non-fatal error value (errors are first-class data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueError {
    pub kind: String,
    pub message: String,
}

/// The universal data type that flows through every pipeline stage.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    /// Nanoseconds.
    Duration(i64),
    /// Bytes; rendered human-readable (e.g. `1.2 MB`).
    Filesize(u64),
    /// Unix nanoseconds; rendered as a date-time.
    Date(i64),
    List(Vec<Value>),
    Record(Record),
    Table(Table),
    Error(ValueError),
    /// A typed object contributed by a builtin or plugin (FileEntry, Process…).
    Custom(Arc<dyn CustomValue>),
}

impl Value {
    /// A short type name, used in diagnostics and signatures.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Bytes(_) => "bytes",
            Value::Duration(_) => "duration",
            Value::Filesize(_) => "filesize",
            Value::Date(_) => "date",
            Value::List(_) => "list",
            Value::Record(_) => "record",
            Value::Table(_) => "table",
            Value::Error(_) => "error",
            Value::Custom(_) => "custom",
        }
    }

    /// Build a typed value from any [`CustomValue`].
    pub fn custom(c: impl CustomValue + 'static) -> Value {
        Value::Custom(Arc::new(c))
    }

    /// Numeric coercion for comparison/arithmetic (ints, floats, filesizes,
    /// durations, and dates all compare on a common numeric axis).
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Filesize(b) => Some(*b as f64),
            Value::Duration(d) => Some(*d as f64),
            Value::Date(d) => Some(*d as f64),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Filesize(b) => Some(*b as i64),
            Value::Duration(d) | Value::Date(d) => Some(*d),
            Value::Float(f) => Some(*f as i64),
            Value::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Truthiness for conditionals (`where`, `if`).
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Filesize(b) => *b != 0,
            Value::String(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Table(t) => !t.rows.is_empty(),
            Value::Error(_) => false,
            _ => true,
        }
    }

    /// Access a field by name on records and custom values; index into a list
    /// or table by numeric string.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Value> {
        match self {
            Value::Record(r) => r.get(key).cloned(),
            Value::Custom(c) => c.get(key),
            Value::Table(t) => key
                .parse::<usize>()
                .ok()
                .and_then(|i| t.rows.get(i))
                .map(|r| Value::Record(r.clone())),
            Value::List(l) => key.parse::<usize>().ok().and_then(|i| l.get(i)).cloned(),
            _ => None,
        }
    }

    /// Total-ish ordering used by `sort-by` and comparison operators. Returns
    /// `None` for incomparable pairs.
    #[must_use]
    pub fn compare(&self, other: &Value) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (self.as_number(), other.as_number()) {
            return a.partial_cmp(&b);
        }
        match (self, other) {
            (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
            (Value::Null, Value::Null) => Some(Ordering::Equal),
            (Value::Custom(a), _) => a.to_base().compare(other),
            (_, Value::Custom(b)) => self.compare(&b.to_base()),
            _ => None,
        }
    }

    /// Render this value to its text view (fallback for non-GUI contexts).
    #[must_use]
    pub fn to_text(&self) -> String {
        render::to_text(self)
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        use Value::*;
        match (self, other) {
            (Null, Null) => true,
            (Bool(a), Bool(b)) => a == b,
            (String(a), String(b)) => a == b,
            (Bytes(a), Bytes(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Record(a), Record(b)) => a == b,
            (Table(a), Table(b)) => a == b,
            (Error(a), Error(b)) => a == b,
            (Custom(a), Custom(b)) => a.to_base() == b.to_base(),
            (Custom(a), other) => &a.to_base() == other,
            (s, Custom(b)) => s == &b.to_base(),
            // numeric cross-type equality (Int == Filesize, etc.)
            _ => match (self.as_number(), other.as_number()) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}
impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::List(v.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_cross_type_comparison() {
        assert_eq!(
            Value::Int(1024).compare(&Value::Filesize(1024)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Value::Filesize(2048).compare(&Value::Int(1024)),
            Some(Ordering::Greater)
        );
        assert_eq!(Value::Int(1), Value::Filesize(1));
    }

    #[test]
    fn truthiness() {
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Int(3).is_truthy());
        assert!(!Value::String(String::new()).is_truthy());
        assert!(Value::from("x").is_truthy());
    }

    #[test]
    fn record_field_access() {
        let mut r = Record::new();
        r.push("name", Value::from("a"));
        r.push("size", Value::Filesize(10));
        let v = Value::Record(r);
        assert_eq!(v.get("size"), Some(Value::Filesize(10)));
        assert_eq!(v.get("missing"), None);
    }

    #[test]
    fn custom_value_flows_through_get_and_compare() {
        let f = FileEntry {
            name: "a.txt".into(),
            path: "/a.txt".into(),
            kind: FileKind::File,
            size: 100,
            modified: 0,
            readonly: false,
        };
        let v = Value::custom(f);
        assert_eq!(v.get("size"), Some(Value::Filesize(100)));
        assert_eq!(v.get("name"), Some(Value::from("a.txt")));
        // custom compares via its base record's field-less numeric? name string:
        assert_eq!(v.get("type"), Some(Value::from("file")));
    }
}
