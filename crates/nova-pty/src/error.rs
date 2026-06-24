use thiserror::Error;

/// Errors produced by the PTY layer.
#[derive(Debug, Error)]
pub enum PtyError {
    #[error("pseudoconsole creation failed: {0}")]
    CreatePseudoConsole(String),

    #[error("failed to create pipe: {0}")]
    CreatePipe(String),

    #[error("failed to spawn child process `{program}`: {reason}")]
    Spawn { program: String, reason: String },

    #[error("failed to resize pseudoconsole: {0}")]
    Resize(String),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("the child process has already exited")]
    AlreadyExited,

    #[error("this platform does not support ConPTY")]
    Unsupported,
}
