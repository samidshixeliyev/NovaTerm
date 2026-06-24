//! Built-in commands. Each produces structured [`Value`]s, never plain text.

use crate::command::{Call, Command, EvalCtx, Signature};
use crate::error::CmdError;
use crate::registry::Registry;
use nova_value::{FileEntry, FileKind, Value};
use std::cmp::Ordering;
use std::time::UNIX_EPOCH;

/// Register every built-in command.
pub fn register_builtins(reg: &mut Registry) {
    reg.register(Echo);
    reg.register(Pwd);
    reg.register(Ls);
    reg.register(Where);
    reg.register(SortBy);
    reg.register(First);
    reg.register(Get);
    reg.register(Lines);
    reg.register(Length);
}

/// Normalize any value into a row sequence for collection commands.
fn to_rows(v: &Value) -> Vec<Value> {
    match v {
        Value::List(items) => items.clone(),
        Value::Table(t) => t.rows.iter().cloned().map(Value::Record).collect(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    }
}

struct Echo;
impl Command for Echo {
    fn signature(&self) -> Signature {
        Signature::new("echo").usage("echo <values>... — emit values")
    }
    fn run(&self, ctx: &mut EvalCtx<'_>, input: Value, call: &Call<'_>) -> Result<Value, CmdError> {
        let mut vals = Vec::new();
        let mut i = 0;
        while let Some(e) = call.positional(i) {
            vals.push(ctx.eval(e, &Value::Null)?);
            i += 1;
        }
        Ok(match vals.len() {
            0 => input,
            1 => vals.pop().unwrap(),
            _ => Value::List(vals),
        })
    }
}

struct Pwd;
impl Command for Pwd {
    fn signature(&self) -> Signature {
        Signature::new("pwd").usage("pwd — print working directory")
    }
    fn run(
        &self,
        ctx: &mut EvalCtx<'_>,
        _input: Value,
        _call: &Call<'_>,
    ) -> Result<Value, CmdError> {
        Ok(Value::String(ctx.cwd.display().to_string()))
    }
}

struct Ls;
impl Command for Ls {
    fn signature(&self) -> Signature {
        Signature::new("ls").usage("ls [path] — list a directory as a table of files")
    }
    fn run(
        &self,
        ctx: &mut EvalCtx<'_>,
        _input: Value,
        call: &Call<'_>,
    ) -> Result<Value, CmdError> {
        let path = match call.positional(0) {
            // (`~` expansion will be handled by the engine/VFS in a later phase.)
            Some(e) => ctx.eval(e, &Value::Null)?.to_text(),
            None => ctx.cwd.display().to_string(),
        };
        let dir =
            std::fs::read_dir(&path).map_err(|e| CmdError::msg(format!("ls: {path}: {e}")))?;
        let mut entries = Vec::new();
        for entry in dir.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let kind = if meta.is_dir() {
                FileKind::Dir
            } else if meta.file_type().is_symlink() {
                FileKind::Symlink
            } else if meta.is_file() {
                FileKind::File
            } else {
                FileKind::Other
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(0);
            entries.push(Value::custom(FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().display().to_string(),
                kind,
                size: meta.len(),
                modified,
                readonly: meta.permissions().readonly(),
            }));
        }
        entries.sort_by(|a, b| {
            a.get("name")
                .unwrap_or(Value::Null)
                .compare(&b.get("name").unwrap_or(Value::Null))
                .unwrap_or(Ordering::Equal)
        });
        Ok(Value::List(entries))
    }
}

struct Where;
impl Command for Where {
    fn signature(&self) -> Signature {
        Signature::new("where")
            .usage("where <condition> — keep rows where the row condition is true")
    }
    fn run(&self, ctx: &mut EvalCtx<'_>, input: Value, call: &Call<'_>) -> Result<Value, CmdError> {
        let cond = call
            .positional(0)
            .ok_or(CmdError::MissingArg("condition"))?;
        let mut out = Vec::new();
        for row in to_rows(&input) {
            if ctx.eval(cond, &row)?.is_truthy() {
                out.push(row);
            }
        }
        Ok(Value::List(out))
    }
}

struct SortBy;
impl Command for SortBy {
    fn signature(&self) -> Signature {
        Signature::new("sort-by").usage("sort-by <field> — sort rows ascending by a field")
    }
    fn run(&self, ctx: &mut EvalCtx<'_>, input: Value, call: &Call<'_>) -> Result<Value, CmdError> {
        let key_expr = call.positional(0).ok_or(CmdError::MissingArg("field"))?;
        let key = ctx.eval(key_expr, &Value::Null)?.to_text();
        let mut rows = to_rows(&input);
        rows.sort_by(|a, b| {
            let av = a.get(&key).unwrap_or(Value::Null);
            let bv = b.get(&key).unwrap_or(Value::Null);
            av.compare(&bv).unwrap_or(Ordering::Equal)
        });
        Ok(Value::List(rows))
    }
}

struct First;
impl Command for First {
    fn signature(&self) -> Signature {
        Signature::new("first").usage("first [n] — take the first n rows (default 1)")
    }
    fn run(&self, ctx: &mut EvalCtx<'_>, input: Value, call: &Call<'_>) -> Result<Value, CmdError> {
        let n = match call.positional(0) {
            Some(e) => ctx.eval(e, &Value::Null)?.as_int().unwrap_or(1).max(0) as usize,
            None => 1,
        };
        let rows: Vec<Value> = to_rows(&input).into_iter().take(n).collect();
        Ok(Value::List(rows))
    }
}

struct Get;
impl Command for Get {
    fn signature(&self) -> Signature {
        Signature::new("get").usage("get <field> — project a field/column out of the input")
    }
    fn run(&self, ctx: &mut EvalCtx<'_>, input: Value, call: &Call<'_>) -> Result<Value, CmdError> {
        let field = ctx
            .eval(
                call.positional(0).ok_or(CmdError::MissingArg("field"))?,
                &Value::Null,
            )?
            .to_text();
        match &input {
            Value::List(_) | Value::Table(_) => {
                let col = to_rows(&input)
                    .iter()
                    .map(|r| r.get(&field).unwrap_or(Value::Null))
                    .collect();
                Ok(Value::List(col))
            }
            other => Ok(other.get(&field).unwrap_or(Value::Null)),
        }
    }
}

struct Lines;
impl Command for Lines {
    fn signature(&self) -> Signature {
        Signature::new("lines").usage("lines — split a string into a list of lines")
    }
    fn run(
        &self,
        _ctx: &mut EvalCtx<'_>,
        input: Value,
        _call: &Call<'_>,
    ) -> Result<Value, CmdError> {
        match input {
            Value::String(s) => Ok(Value::List(
                s.lines().map(|l| Value::String(l.to_string())).collect(),
            )),
            other => Err(CmdError::Type {
                ctx: "lines",
                expected: "string",
                got: other.type_name(),
            }),
        }
    }
}

struct Length;
impl Command for Length {
    fn signature(&self) -> Signature {
        Signature::new("length").usage("length — count the rows of the input")
    }
    fn run(
        &self,
        _ctx: &mut EvalCtx<'_>,
        input: Value,
        _call: &Call<'_>,
    ) -> Result<Value, CmdError> {
        Ok(Value::Int(to_rows(&input).len() as i64))
    }
}
