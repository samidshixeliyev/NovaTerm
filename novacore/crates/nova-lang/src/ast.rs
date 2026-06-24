//! NovaLang abstract syntax tree.

/// A source span (byte offsets) for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

/// A top-level pipeline: stages separated by `|`. Stage *N*'s output is stage
/// *N+1*'s `$in`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub stages: Vec<Expr>,
}

/// Binary operators (with Pratt precedence in the parser).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    /// Bytes.
    Filesize(u64),
    /// Nanoseconds.
    Duration(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    /// An unquoted word: a command name, a path, or (inside a row condition) a
    /// field reference. The evaluator decides based on context.
    Bareword(String),
    /// `$name` or `$name.field.field` (segments[0] is the variable).
    Var(Vec<String>),
    /// A command invocation: `name arg arg --flag val`.
    Command(Command),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    /// Unary `-x` / `!x`.
    Unary(UnaryOp, Box<Expr>),
    /// `( pipeline )`.
    Subexpr(Box<Pipeline>),
    /// `{ pipeline }` — a deferred block (e.g. a row condition for `where`).
    Block(Box<Pipeline>),
    List(Vec<Expr>),
    Record(Vec<(String, Expr)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub name: String,
    pub args: Vec<Arg>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Positional(Expr),
    /// `--name` or `--name value`.
    Flag {
        name: String,
        value: Option<Expr>,
    },
    /// `-x` (one or more short flags collapsed by the command).
    Short(String),
}
