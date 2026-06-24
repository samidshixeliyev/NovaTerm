//! A table: ordered columns + rows of [`Record`]s. The canonical output of
//! commands like `ls` and `ps`.

use crate::{Record, Value};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Record>,
}

impl Table {
    #[must_use]
    pub fn new(columns: Vec<String>) -> Self {
        Table {
            columns,
            rows: Vec::new(),
        }
    }

    /// Build a table from records, inferring the column set as the union of keys
    /// in first-seen order.
    #[must_use]
    pub fn from_records(rows: Vec<Record>) -> Self {
        let mut columns: Vec<String> = Vec::new();
        for row in &rows {
            for k in row.keys() {
                if !columns.iter().any(|c| c == k) {
                    columns.push(k.to_string());
                }
            }
        }
        Table { columns, rows }
    }

    /// Build a table from custom values (e.g. `FileEntry`s) by projecting each
    /// onto its base record.
    #[must_use]
    pub fn from_values(values: &[Value]) -> Table {
        let rows: Vec<Record> = values
            .iter()
            .map(|v| match v {
                Value::Record(r) => r.clone(),
                Value::Custom(c) => match c.to_base() {
                    Value::Record(r) => r,
                    other => {
                        let mut r = Record::new();
                        r.push("value", other);
                        r
                    }
                },
                other => {
                    let mut r = Record::new();
                    r.push("value", other.clone());
                    r
                }
            })
            .collect();
        Table::from_records(rows)
    }

    pub fn push(&mut self, row: Record) {
        for k in row.keys() {
            if !self.columns.iter().any(|c| c == k) {
                self.columns.push(k.to_string());
            }
        }
        self.rows.push(row);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
