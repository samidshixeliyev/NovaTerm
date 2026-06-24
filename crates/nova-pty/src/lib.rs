//! `nova-pty` — a thin, native wrapper over the Windows **pseudoconsole**
//! (ConPTY) API.
//!
//! Unlike crates that shell out or wrap `conhost`, this talks directly to
//! `CreatePseudoConsole` / `ResizePseudoConsole` / `ClosePseudoConsole` and
//! spawns the child with `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`. That gives us
//! exact control over resize semantics and handle lifetimes — important for a
//! flicker-free, zombie-free terminal.
//!
//! The public surface is intentionally small and OS-agnostic in shape:
//!
//! * [`CommandBuilder`] — describe the program to run.
//! * [`PtySize`] — grid dimensions.
//! * [`Pty`] — a spawned pseudoconsole; hands out a [`PtyReader`] and
//!   [`PtyWriter`] (both `std::io` + `Send`) and supports `resize`, `wait`,
//!   and `kill`.

use std::collections::BTreeMap;

mod error;
pub use error::PtyError;

#[cfg(windows)]
mod conpty;
#[cfg(windows)]
pub use conpty::{Pty, PtyReader, PtyWriter};

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, PtyError>;

/// Pseudoconsole dimensions, in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        PtySize { cols: 80, rows: 24 }
    }
}

/// Describes the child program to launch inside the pseudoconsole.
#[derive(Debug, Clone, Default)]
pub struct CommandBuilder {
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: BTreeMap<String, String>,
    /// When true, the process inherits the parent environment (and `env` is
    /// applied on top). When false, only `env` is used.
    inherit_env: bool,
}

impl CommandBuilder {
    /// Start building a command for `program` (an executable name or path).
    #[must_use]
    pub fn new(program: impl Into<String>) -> Self {
        CommandBuilder {
            program: program.into(),
            inherit_env: true,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn cwd(mut self, dir: impl Into<String>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    #[must_use]
    pub fn env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.env.insert(key.into(), val.into());
        self
    }

    #[must_use]
    pub fn inherit_env(mut self, yes: bool) -> Self {
        self.inherit_env = yes;
        self
    }

    /// The full command line as a single string, with arguments quoted per the
    /// Windows `CommandLineToArgvW` rules.
    #[must_use]
    pub fn command_line(&self) -> String {
        let mut out = quote_arg(&self.program);
        for a in &self.args {
            out.push(' ');
            out.push_str(&quote_arg(a));
        }
        out
    }

    pub fn cwd_ref(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Effective environment map (merged with parent if `inherit_env`).
    pub fn env_iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.env.iter()
    }

    #[must_use]
    pub fn inherits_env(&self) -> bool {
        self.inherit_env
    }
}

/// Quote a single argument following the Windows command-line escaping rules
/// understood by `CommandLineToArgvW`.
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| matches!(c, ' ' | '\t' | '"')) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => {
                backslashes += 1;
            }
            '"' => {
                // Escape all pending backslashes, then the quote.
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                out.push(c);
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quoting() {
        let c = CommandBuilder::new("pwsh.exe").arg("-NoLogo");
        assert_eq!(c.command_line(), "pwsh.exe -NoLogo");

        let c = CommandBuilder::new("C:/Program Files/Git/bin/bash.exe").arg("-l");
        assert_eq!(c.command_line(), "\"C:/Program Files/Git/bin/bash.exe\" -l");

        let c = CommandBuilder::new("app").arg(r#"a "b" c"#);
        assert_eq!(c.command_line(), r#"app "a \"b\" c""#);
    }

    #[test]
    fn default_size() {
        assert_eq!(PtySize::default(), PtySize { cols: 80, rows: 24 });
    }
}
