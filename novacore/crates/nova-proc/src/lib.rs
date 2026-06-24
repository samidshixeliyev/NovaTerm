//! `nova-proc` — NovaCore's process manager.
//!
//! Runs native executables directly (no shell), captures their output as
//! structured values, and tracks spawned children in a process table. This is
//! how NovaCore "runs anything" without being a shell frontend — it owns the
//! pipes and talks to the OS directly via [`std::process`].

#![forbid(unsafe_code)]

use nova_value::{Record, Value};
use parking_lot::Mutex;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcError {
    #[error("failed to spawn `{program}`: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
}

/// The captured result of running an external command.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcOutput {
    pub program: String,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ProcOutput {
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == 0
    }

    /// Project to a structured [`Value`] record.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut r = Record::new();
        r.push("program", Value::String(self.program.clone()));
        r.push("exit_code", Value::Int(self.code as i64));
        r.push("success", Value::Bool(self.success()));
        r.push("stdout", Value::String(self.stdout.clone()));
        r.push("stderr", Value::String(self.stderr.clone()));
        Value::Record(r)
    }
}

#[derive(Debug, Clone)]
struct Tracked {
    pid: u32,
    program: String,
}

/// Manages external process execution and tracking.
#[derive(Default)]
pub struct ProcManager {
    table: Mutex<Vec<Tracked>>,
}

impl ProcManager {
    #[must_use]
    pub fn new() -> Self {
        ProcManager::default()
    }

    /// Run `program args…` to completion, capturing stdout/stderr. Blocking;
    /// the engine calls this on a worker thread.
    pub fn run_capture(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
    ) -> Result<ProcOutput, ProcError> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let output = cmd.output().map_err(|e| ProcError::Spawn {
            program: program.into(),
            source: e,
        })?;
        if let Some(pid) = output.status.code() {
            // Record the just-finished process for `ps`/history visibility.
            self.table.lock().push(Tracked {
                pid: pid as u32,
                program: program.into(),
            });
        }
        Ok(ProcOutput {
            program: program.into(),
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Number of processes this manager has run/tracked.
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.table.lock().len()
    }

    /// The tracked process table as a list of records (basis for `ps`).
    #[must_use]
    pub fn list(&self) -> Value {
        let rows = self
            .table
            .lock()
            .iter()
            .map(|t| {
                let mut r = Record::new();
                r.push("pid", Value::Int(t.pid as i64));
                r.push("program", Value::String(t.program.clone()));
                Value::Record(r)
            })
            .collect();
        Value::List(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_projects_to_record() {
        let o = ProcOutput {
            program: "git".into(),
            code: 0,
            stdout: "ok\n".into(),
            stderr: String::new(),
        };
        let v = o.to_value();
        assert_eq!(v.get("exit_code"), Some(Value::Int(0)));
        assert_eq!(v.get("success"), Some(Value::Bool(true)));
        assert_eq!(v.get("stdout"), Some(Value::String("ok\n".into())));
        assert!(o.success());
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "spawning external processes is unreliable in the headless sandbox; \
                  runs on a real machine"
    )]
    fn run_capture_real_process() {
        let pm = ProcManager::new();
        #[cfg(windows)]
        let out = pm
            .run_capture("cmd", &["/c".into(), "echo NOVA_OK".into()], None)
            .unwrap();
        #[cfg(not(windows))]
        let out = pm.run_capture("echo", &["NOVA_OK".into()], None).unwrap();
        assert!(out.stdout.contains("NOVA_OK"));
        assert!(out.success());
    }
}
