//! NovaLang — the NovaCore command language: lexer, AST, and parser.
//!
//! ```
//! use nova_lang::parse_str;
//! let pipeline = parse_str("ls | where size > 0 | sort-by name | first 3").unwrap();
//! assert_eq!(pipeline.stages.len(), 4);
//! ```

#![forbid(unsafe_code)]

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use lexer::{lex, LexError, Token};
pub use parser::{parse, ParseError};

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum NovaLangError {
    #[error(transparent)]
    Lex(#[from] LexError),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// Lex + parse source into a [`Pipeline`].
pub fn parse_str(src: &str) -> Result<Pipeline, NovaLangError> {
    let toks = lex(src)?;
    Ok(parse(toks)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_command(p: &Pipeline) -> &Command {
        match &p.stages[0] {
            Expr::Command(c) => c,
            other => panic!("expected command, got {other:?}"),
        }
    }

    #[test]
    fn simple_command() {
        let p = parse_str("ls").unwrap();
        assert_eq!(p.stages.len(), 1);
        assert_eq!(first_command(&p).name, "ls");
    }

    #[test]
    fn pipeline_stage_count() {
        let p = parse_str("ls | where size > 0 | sort-by name | first 3").unwrap();
        assert_eq!(p.stages.len(), 4);
    }

    #[test]
    fn where_parses_row_condition() {
        let p = parse_str("ls | where size > 1mb").unwrap();
        let Expr::Command(c) = &p.stages[1] else {
            panic!()
        };
        assert_eq!(c.name, "where");
        let Arg::Positional(Expr::Binary(l, op, r)) = &c.args[0] else {
            panic!("{:?}", c.args)
        };
        assert_eq!(**l, Expr::Bareword("size".into()));
        assert_eq!(*op, BinOp::Gt);
        assert_eq!(**r, Expr::Literal(Literal::Filesize(1024 * 1024)));
    }

    #[test]
    fn flags_and_values() {
        let p = parse_str("upload --to prod --force").unwrap();
        let c = first_command(&p);
        assert_eq!(c.args.len(), 2);
        assert_eq!(
            c.args[0],
            Arg::Flag {
                name: "to".into(),
                value: Some(Expr::Bareword("prod".into()))
            }
        );
        assert_eq!(
            c.args[1],
            Arg::Flag {
                name: "force".into(),
                value: None
            }
        );
    }

    #[test]
    fn block_argument() {
        let p = parse_str("ls | where { $it.size > 0 }").unwrap();
        let Expr::Command(c) = &p.stages[1] else {
            panic!()
        };
        let Arg::Positional(Expr::Block(b)) = &c.args[0] else {
            panic!("{:?}", c.args)
        };
        let Expr::Binary(l, op, _) = &b.stages[0] else {
            panic!()
        };
        assert_eq!(**l, Expr::Var(vec!["it".into(), "size".into()]));
        assert_eq!(*op, BinOp::Gt);
    }

    #[test]
    fn literals_units_and_collections() {
        let p = parse_str(r#"echo 1mb 200ms 3.5 "hi" true [1 2 3] {a: 1}"#).unwrap();
        let c = first_command(&p);
        assert_eq!(
            c.args[0],
            Arg::Positional(Expr::Literal(Literal::Filesize(1024 * 1024)))
        );
        assert_eq!(
            c.args[1],
            Arg::Positional(Expr::Literal(Literal::Duration(200_000_000)))
        );
        assert_eq!(
            c.args[2],
            Arg::Positional(Expr::Literal(Literal::Float(3.5)))
        );
        assert_eq!(
            c.args[3],
            Arg::Positional(Expr::Literal(Literal::Str("hi".into())))
        );
        assert_eq!(
            c.args[4],
            Arg::Positional(Expr::Literal(Literal::Bool(true)))
        );
        assert!(matches!(c.args[5], Arg::Positional(Expr::List(_))));
        assert!(matches!(c.args[6], Arg::Positional(Expr::Record(_))));
    }

    #[test]
    fn subexpression_and_negative() {
        // negative number lexes as Int(-1)
        let p2 = parse_str("first -1").unwrap();
        assert_eq!(
            first_command(&p2).args[0],
            Arg::Positional(Expr::Literal(Literal::Int(-1)))
        );
        // a parenthesized pipeline is a single value stage
        let p3 = parse_str("(ls | first 1)").unwrap();
        assert_eq!(p3.stages.len(), 1);
        assert!(matches!(p3.stages[0], Expr::Subexpr(_)));
    }

    #[test]
    fn comments_and_strings() {
        let p = parse_str("echo \"a b\" # trailing comment").unwrap();
        assert_eq!(first_command(&p).args.len(), 1);
    }
}
