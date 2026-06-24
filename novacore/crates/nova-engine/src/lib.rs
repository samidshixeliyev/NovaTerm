//! `nova-engine` — **NovaCore**: the runtime at the center of the platform.
//!
//! It owns the command [`Registry`], the current [`Session`] (cwd/env), the
//! [`History`], the [`EventBus`], the [`Vfs`], and the [`ProcManager`]. It
//! parses NovaLang, drives the structured-value pipeline (implementing
//! [`nova_cmd::Host`]), and falls back to running native executables for any
//! command that isn't a builtin/plugin. Shells are [`ShellAdapter`] plugins.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::Instant;

use nova_bus::{Event, EventBus};
use nova_cmd::{register_builtins, Call, EvalCtx, Host, Registry};
use nova_history::{History, HistoryEntry};
use nova_lang::{parse_str, Arg, Command as AstCommand, Expr, NovaLangError, Pipeline};
use nova_proc::ProcManager;
use nova_value::Value;
use nova_vfs::Vfs;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Parse(#[from] NovaLangError),
    #[error(transparent)]
    Command(#[from] nova_cmd::CmdError),
}

/// A running context: working directory + environment + identity.
pub struct Session {
    pub id: u64,
    pub cwd: PathBuf,
}

/// A plugin that wraps an external interpreter (pwsh/bash/zsh/nu/wsl). Optional
/// — never the core. Implementations live in `plugins/`.
pub trait ShellAdapter: Send + Sync {
    fn name(&self) -> &str;
    /// Feed a line to the shell; the engine surfaces its output as a stream
    /// value. (Wired in the plugin-runtime phase.)
    fn feed(&mut self, line: &str) -> Result<Value, nova_cmd::CmdError>;
}

/// NovaCore.
pub struct Engine {
    registry: Registry,
    session: Session,
    history: History,
    bus: EventBus,
    vfs: Vfs,
    proc: ProcManager,
}

impl Engine {
    /// Create an engine with the working directory of the current process.
    #[must_use]
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::with_cwd(cwd)
    }

    #[must_use]
    pub fn with_cwd(cwd: PathBuf) -> Self {
        let mut registry = Registry::new();
        register_builtins(&mut registry);
        Engine {
            registry,
            session: Session { id: 1, cwd },
            history: History::new(),
            bus: EventBus::new(),
            vfs: Vfs::new(),
            proc: ProcManager::new(),
        }
    }

    #[must_use]
    pub fn bus(&self) -> &EventBus {
        &self.bus
    }
    #[must_use]
    pub fn vfs(&self) -> &Vfs {
        &self.vfs
    }
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    #[must_use]
    pub fn history(&self) -> &History {
        &self.history
    }
    #[must_use]
    pub fn cwd(&self) -> &PathBuf {
        &self.session.cwd
    }

    /// Register a plugin/extra command.
    pub fn register_command(&mut self, command: impl nova_cmd::Command + 'static) {
        self.registry.register(command);
    }

    /// Parse and evaluate a line of NovaLang, returning the resulting value.
    pub fn eval(&mut self, source: &str) -> Result<Value, EngineError> {
        let pipeline = parse_str(source)?;
        let started = Instant::now();
        let result = self.run_pipeline(&pipeline, Value::Null);
        let (value, success) = match &result {
            Ok(v) => (v.type_name().to_string(), true),
            Err(_) => ("error".to_string(), false),
        };
        self.history.append(HistoryEntry {
            id: 0,
            session: self.session.id,
            source: source.to_string(),
            cwd: self.session.cwd.display().to_string(),
            result_type: value.clone(),
            success,
            duration_ns: started.elapsed().as_nanos() as i64,
        });
        self.bus.publish(Event::HistoryAppended {
            session: self.session.id,
            source: source.to_string(),
        });
        let v = result?;
        self.bus.publish(Event::ValueProduced {
            session: self.session.id,
            summary: summarize(&v),
        });
        Ok(v)
    }

    /// Run an external executable directly (no shell), surfacing stdout.
    fn run_external(&mut self, cmd: &AstCommand) -> Result<Value, nova_cmd::CmdError> {
        let mut argv = Vec::new();
        for arg in &cmd.args {
            match arg {
                Arg::Positional(e) => {
                    argv.push(nova_cmd::eval_expr(e, &Value::Null, self)?.to_text())
                }
                Arg::Flag { name, value } => {
                    argv.push(format!("--{name}"));
                    if let Some(v) = value {
                        argv.push(nova_cmd::eval_expr(v, &Value::Null, self)?.to_text());
                    }
                }
                Arg::Short(s) => argv.push(format!("-{s}")),
            }
        }
        let cwd = self.session.cwd.clone();
        match self.proc.run_capture(&cmd.name, &argv, Some(&cwd)) {
            Ok(out) => {
                self.bus.publish(Event::ProcessExited {
                    pid: 0,
                    code: out.code,
                });
                // Surface stdout as a text stream; structured lifting is a parser feature.
                Ok(Value::String(out.stdout.trim_end_matches('\n').to_string()))
            }
            Err(e) => Err(nova_cmd::CmdError::msg(e.to_string())),
        }
    }

    /// `cd` mutates session state, so the engine handles it (not a plain command).
    fn change_dir(&mut self, cmd: &AstCommand) -> Result<Value, nova_cmd::CmdError> {
        let target = match cmd.args.iter().find_map(|a| match a {
            Arg::Positional(e) => Some(e),
            _ => None,
        }) {
            Some(e) => nova_cmd::eval_expr(e, &Value::Null, self)?.to_text(),
            None => return Ok(Value::String(self.session.cwd.display().to_string())),
        };
        let mut next = PathBuf::from(&target);
        if next.is_relative() {
            next = self.session.cwd.join(next);
        }
        let canonical = next.canonicalize().unwrap_or(next);
        self.session.cwd = canonical;
        Ok(Value::Null)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Host for Engine {
    fn run_pipeline(
        &mut self,
        pipeline: &Pipeline,
        input: Value,
    ) -> Result<Value, nova_cmd::CmdError> {
        let mut cur = input;
        for (i, stage) in pipeline.stages.iter().enumerate() {
            cur = match stage {
                Expr::Command(c) if c.name == "cd" => self.change_dir(c)?,
                Expr::Command(c) => {
                    if let Some(cmd) = self.registry.get(&c.name) {
                        let cwd = self.session.cwd.clone();
                        let call = Call::new(c.name.clone(), &c.args);
                        let mut ctx = EvalCtx { cwd, host: self };
                        cmd.run(&mut ctx, cur, &call)?
                    } else {
                        self.run_external(c)?
                    }
                }
                // A non-command stage is a value expression. The first stage
                // ignores `$in`; later stages receive the previous value.
                other => {
                    let in_val = if i == 0 { Value::Null } else { cur };
                    nova_cmd::eval_expr(other, &in_val, self)?
                }
            };
        }
        Ok(cur)
    }
}

fn summarize(v: &Value) -> String {
    match v {
        Value::List(items) => format!("list[{}]", items.len()),
        Value::Table(t) => format!("table[{}x{}]", t.rows.len(), t.columns.len()),
        other => other.type_name().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir_with_files() -> PathBuf {
        // Unique per call: tests run in parallel and must not share a directory.
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("novacore_engine_{}_{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), vec![0u8; 10]).unwrap();
        fs::write(dir.join("b.txt"), vec![0u8; 20]).unwrap();
        fs::write(dir.join("c.txt"), vec![0u8; 30]).unwrap();
        fs::write(dir.join("empty.txt"), b"").unwrap();
        fs::create_dir_all(dir.join("subdir")).unwrap();
        dir
    }

    #[test]
    fn end_to_end_structured_pipeline() {
        let dir = temp_dir_with_files();
        let mut eng = Engine::with_cwd(dir.clone());

        // The headline gate: a real structured pipeline over the filesystem.
        let v = eng
            .eval("ls | where size > 0 | sort-by name | first 3")
            .unwrap();
        let Value::List(rows) = v else {
            panic!("expected list, got {v:?}")
        };

        // empty.txt (0 bytes) and subdir (0 bytes) are filtered out; a/b/c remain.
        assert_eq!(rows.len(), 3);
        let names: Vec<String> = rows
            .iter()
            .map(|r| r.get("name").unwrap().to_text())
            .collect();
        assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
        // and every survivor really is non-empty
        assert!(rows
            .iter()
            .all(|r| r.get("size").unwrap().as_int().unwrap() > 0));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn get_column_and_length() {
        let dir = temp_dir_with_files();
        let mut eng = Engine::with_cwd(dir.clone());
        let names = eng
            .eval("ls | where type == file | get name | sort-by name")
            .unwrap();
        let Value::List(items) = names else { panic!() };
        assert!(items.iter().any(|v| v.to_text() == "a.txt"));

        let count = eng.eval("ls | where type == file | length").unwrap();
        assert_eq!(count, Value::Int(4)); // a, b, c, empty

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn history_and_pipeline_value() {
        let mut eng = Engine::with_cwd(std::env::temp_dir());
        let v = eng.eval("echo hello | length").unwrap();
        assert_eq!(v, Value::Int(1));
        assert_eq!(eng.eval("[1 2 3] | length").unwrap(), Value::Int(3));
        assert!(eng.history().len() >= 2);
        assert_eq!(eng.history().recent(1)[0].source, "[1 2 3] | length");
    }

    #[test]
    fn literal_and_record_stage() {
        let mut eng = Engine::with_cwd(std::env::temp_dir());
        assert_eq!(eng.eval("1mb").unwrap(), Value::Filesize(1024 * 1024));
        let rec = eng.eval("{name: \"x\", size: 5}").unwrap();
        assert_eq!(rec.get("size"), Some(Value::Int(5)));
    }
}
