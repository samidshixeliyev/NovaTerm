//! Text rendering of values — the fallback "view" used in tests and headless
//! contexts. The GUI (`nova-ui`) renders the same values graphically.

use crate::{Table, Value};

/// Render any value to a human-readable string.
#[must_use]
pub fn to_text(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{f}"),
        Value::String(s) => s.clone(),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
        Value::Duration(d) => format_duration(*d),
        Value::Filesize(b) => format_filesize(*b),
        Value::Date(d) => format_date(*d),
        Value::Error(e) => format!("error[{}]: {}", e.kind, e.message),
        Value::List(items) => {
            // A list of records/customs renders as a table; scalars as lines.
            if items
                .iter()
                .all(|i| matches!(i, Value::Record(_) | Value::Custom(_)))
                && !items.is_empty()
            {
                render_table(&Table::from_values(items))
            } else {
                items.iter().map(to_text).collect::<Vec<_>>().join("\n")
            }
        }
        Value::Record(r) => r
            .iter()
            .map(|(k, val)| format!("{k}: {}", to_text(val)))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Table(t) => render_table(t),
        Value::Custom(c) => to_text(&c.to_base()),
    }
}

/// Render a table as a bordered ASCII grid.
#[must_use]
pub fn render_table(t: &Table) -> String {
    if t.columns.is_empty() {
        return String::new();
    }
    // Compute column widths from header + cells.
    let mut widths: Vec<usize> = t.columns.iter().map(|c| c.chars().count()).collect();
    let cell = |row: &crate::Record, col: &str| -> String {
        row.get(col).map(to_text).unwrap_or_default()
    };
    for row in &t.rows {
        for (i, col) in t.columns.iter().enumerate() {
            widths[i] = widths[i].max(cell(row, col).chars().count());
        }
    }

    let sep = |left: &str, mid: &str, right: &str| -> String {
        let mut s = String::from(left);
        for (i, w) in widths.iter().enumerate() {
            s.push_str(&"─".repeat(w + 2));
            s.push_str(if i + 1 == widths.len() { right } else { mid });
        }
        s
    };
    let line = |fields: &[String]| -> String {
        let mut s = String::from("│");
        for (i, f) in fields.iter().enumerate() {
            let pad = widths[i] - f.chars().count();
            s.push(' ');
            s.push_str(f);
            s.push_str(&" ".repeat(pad + 1));
            s.push('│');
        }
        s
    };

    let mut out = String::new();
    out.push_str(&sep("┌", "┬", "┐"));
    out.push('\n');
    out.push_str(&line(&t.columns.clone()));
    out.push('\n');
    out.push_str(&sep("├", "┼", "┤"));
    out.push('\n');
    for row in &t.rows {
        let fields: Vec<String> = t.columns.iter().map(|c| cell(row, c)).collect();
        out.push_str(&line(&fields));
        out.push('\n');
    }
    out.push_str(&sep("└", "┴", "┘"));
    out
}

/// Human-readable filesize (binary units).
#[must_use]
pub fn format_filesize(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

/// Human-readable duration from nanoseconds.
#[must_use]
pub fn format_duration(nanos: i64) -> String {
    let ms = nanos as f64 / 1_000_000.0;
    if ms < 1.0 {
        format!("{nanos} ns")
    } else if ms < 1000.0 {
        format!("{ms:.1} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

/// Format unix nanoseconds as `YYYY-MM-DD HH:MM:SS` (UTC), dependency-free.
#[must_use]
pub fn format_date(unix_nanos: i64) -> String {
    let secs = unix_nanos.div_euclid(1_000_000_000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

/// Howard Hinnant's `civil_from_days` algorithm (days since 1970-01-01 → Y/M/D).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileEntry, FileKind, Value};

    #[test]
    fn filesize_formatting() {
        assert_eq!(format_filesize(512), "512 B");
        assert_eq!(format_filesize(1024), "1.0 KB");
        assert_eq!(format_filesize(1_572_864), "1.5 MB");
    }

    #[test]
    fn date_formatting() {
        // 2021-01-01T00:00:00Z = 1_609_459_200 s
        assert_eq!(
            format_date(1_609_459_200 * 1_000_000_000),
            "2021-01-01 00:00:00"
        );
    }

    #[test]
    fn table_render_has_borders_and_columns() {
        let files = vec![
            Value::custom(FileEntry {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                kind: FileKind::File,
                size: 1024,
                modified: 0,
                readonly: false,
            }),
            Value::custom(FileEntry {
                name: "src".into(),
                path: "/src".into(),
                kind: FileKind::Dir,
                size: 0,
                modified: 0,
                readonly: false,
            }),
        ];
        let text = to_text(&Value::List(files));
        assert!(text.contains("name"));
        assert!(text.contains("a.txt"));
        assert!(text.contains("1.0 KB"));
        assert!(text.contains("dir"));
        assert!(text.contains('│'));
    }
}
