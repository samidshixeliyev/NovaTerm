use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CmdError {
    #[error("command not found: {0}")]
    NotFound(String),
    #[error("missing argument: {0}")]
    MissingArg(&'static str),
    #[error("type error in {ctx}: expected {expected}, got {got}")]
    Type {
        ctx: &'static str,
        expected: &'static str,
        got: &'static str,
    },
    #[error("{0}")]
    Msg(String),
}

impl CmdError {
    pub fn msg(s: impl Into<String>) -> Self {
        CmdError::Msg(s.into())
    }
}
