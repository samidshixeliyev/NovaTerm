//! The `Command` trait and the per-call evaluation context.

use crate::error::CmdError;
use crate::eval::{eval_expr, Host};
use nova_lang::{Arg, Expr};
use nova_value::Value;
use std::path::PathBuf;

/// A command's metadata (name + one-line usage). Extended with typed
/// arg/flag specs in a later phase.
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub usage: String,
}

impl Signature {
    #[must_use]
    pub fn new(name: &str) -> Self {
        Signature {
            name: name.into(),
            usage: String::new(),
        }
    }
    #[must_use]
    pub fn usage(mut self, u: &str) -> Self {
        self.usage = u.into();
        self
    }
}

/// A built-in or plugin command. Consumes the pipeline `input` value and the
/// parsed `call`, returns a structured value.
pub trait Command: Send + Sync {
    fn signature(&self) -> Signature;
    fn run(&self, ctx: &mut EvalCtx<'_>, input: Value, call: &Call<'_>) -> Result<Value, CmdError>;
}

/// Per-invocation context: the working directory plus a handle to the engine
/// for evaluating expressions/sub-pipelines.
pub struct EvalCtx<'a> {
    pub cwd: PathBuf,
    pub host: &'a mut dyn Host,
}

impl EvalCtx<'_> {
    /// Evaluate an expression against `input` (the value bound to `$in`/`$it`).
    pub fn eval(&mut self, expr: &Expr, input: &Value) -> Result<Value, CmdError> {
        eval_expr(expr, input, self.host)
    }
}

/// The parsed arguments of one command invocation.
pub struct Call<'a> {
    pub name: String,
    pub args: &'a [Arg],
}

impl<'a> Call<'a> {
    #[must_use]
    pub fn new(name: impl Into<String>, args: &'a [Arg]) -> Self {
        Call {
            name: name.into(),
            args,
        }
    }

    /// The `i`-th positional argument expression.
    #[must_use]
    pub fn positional(&self, i: usize) -> Option<&'a Expr> {
        self.args
            .iter()
            .filter_map(|a| match a {
                Arg::Positional(e) => Some(e),
                _ => None,
            })
            .nth(i)
    }

    #[must_use]
    pub fn positional_count(&self) -> usize {
        self.args
            .iter()
            .filter(|a| matches!(a, Arg::Positional(_)))
            .count()
    }

    /// Whether `--name` (or `-name`) was passed.
    #[must_use]
    pub fn has_flag(&self, name: &str) -> bool {
        self.args.iter().any(|a| match a {
            Arg::Flag { name: n, .. } => n == name,
            Arg::Short(s) => s == name,
            _ => false,
        })
    }

    /// The value expression of `--name value`, if present.
    #[must_use]
    pub fn flag_value(&self, name: &str) -> Option<&'a Expr> {
        self.args.iter().find_map(|a| match a {
            Arg::Flag {
                name: n,
                value: Some(v),
            } if n == name => Some(v),
            _ => None,
        })
    }
}
