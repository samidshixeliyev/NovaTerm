//! NovaLang parser: tokens → [`Pipeline`] AST.
//!
//! Pipelines are parsed recursive-descent; argument/condition expressions use a
//! small Pratt (precedence-climbing) expression parser.

use crate::ast::*;
use crate::lexer::{Spanned, Token};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("unexpected token {0:?}")]
    Unexpected(Token),
    #[error("expected {expected}, found {found:?}")]
    Expected {
        expected: &'static str,
        found: Token,
    },
}

pub fn parse(toks: Vec<Spanned>) -> Result<Pipeline, ParseError> {
    let mut p = Parser { toks, pos: 0 };
    let pipe = p.pipeline()?;
    if let Some(s) = p.peek() {
        return Err(ParseError::Unexpected(s.tok.clone()));
    }
    Ok(pipe)
}

struct Parser {
    toks: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Spanned> {
        self.toks.get(self.pos)
    }
    fn peek_tok(&self) -> Option<&Token> {
        self.peek().map(|s| &s.tok)
    }
    fn bump(&mut self) -> Option<Spanned> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// `stage ('|' stage)*`
    fn pipeline(&mut self) -> Result<Pipeline, ParseError> {
        let mut stages = vec![self.stage()?];
        while matches!(self.peek_tok(), Some(Token::Pipe)) {
            self.bump();
            stages.push(self.stage()?);
        }
        Ok(Pipeline { stages })
    }

    /// A pipeline stage: a command (bareword head) or a value expression.
    fn stage(&mut self) -> Result<Expr, ParseError> {
        match self.peek_tok() {
            Some(Token::Ident(name)) if !is_literal_word(name) => self.command(),
            _ => self.expr(0),
        }
    }

    fn command(&mut self) -> Result<Expr, ParseError> {
        let head = self.bump().ok_or(ParseError::UnexpectedEof)?;
        let start = head.span.start;
        let Token::Ident(name) = head.tok else {
            unreachable!("command head is Ident")
        };
        let mut args = Vec::new();

        loop {
            match self.peek_tok() {
                None
                | Some(Token::Pipe)
                | Some(Token::RParen)
                | Some(Token::RBrace)
                | Some(Token::RBracket) => break,
                Some(Token::LongFlag(_)) => {
                    let Some(Spanned {
                        tok: Token::LongFlag(flag),
                        ..
                    }) = self.bump()
                    else {
                        unreachable!()
                    };
                    let value = if self.starts_value() {
                        Some(self.expr(0)?)
                    } else {
                        None
                    };
                    args.push(Arg::Flag { name: flag, value });
                }
                Some(Token::ShortFlag(_)) => {
                    let Some(Spanned {
                        tok: Token::ShortFlag(s),
                        ..
                    }) = self.bump()
                    else {
                        unreachable!()
                    };
                    args.push(Arg::Short(s));
                }
                _ => args.push(Arg::Positional(self.expr(0)?)),
            }
        }
        let end = self
            .toks
            .get(self.pos.saturating_sub(1))
            .map_or(start, |s| s.span.end);
        Ok(Expr::Command(Command {
            name,
            args,
            span: Span::new(start, end),
        }))
    }

    /// True if the next token can begin a value expression (for flag values).
    fn starts_value(&self) -> bool {
        matches!(
            self.peek_tok(),
            Some(Token::Int(_))
                | Some(Token::Float(_))
                | Some(Token::Str(_))
                | Some(Token::Filesize(_))
                | Some(Token::Duration(_))
                | Some(Token::Ident(_))
                | Some(Token::Var(_))
                | Some(Token::LParen)
                | Some(Token::LBrace)
                | Some(Token::LBracket)
                | Some(Token::Minus)
                | Some(Token::Bang)
        )
    }

    /// Pratt expression parser.
    fn expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut left = self.prefix()?;
        while let Some((op, bp)) = self.peek_tok().and_then(as_binop) {
            if bp < min_bp {
                break;
            }
            self.bump();
            let right = self.expr(bp + 1)?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr, ParseError> {
        let s = self.bump().ok_or(ParseError::UnexpectedEof)?;
        Ok(match s.tok {
            Token::Minus => Expr::Unary(UnaryOp::Neg, Box::new(self.expr(7)?)),
            Token::Bang => Expr::Unary(UnaryOp::Not, Box::new(self.expr(7)?)),
            Token::Int(i) => Expr::Literal(Literal::Int(i)),
            Token::Float(f) => Expr::Literal(Literal::Float(f)),
            Token::Str(s) => Expr::Literal(Literal::Str(s)),
            Token::Filesize(b) => Expr::Literal(Literal::Filesize(b)),
            Token::Duration(d) => Expr::Literal(Literal::Duration(d)),
            Token::Var(segments) => Expr::Var(segments),
            Token::Ident(name) => match name.as_str() {
                "true" => Expr::Literal(Literal::Bool(true)),
                "false" => Expr::Literal(Literal::Bool(false)),
                "null" => Expr::Literal(Literal::Null),
                _ => Expr::Bareword(name),
            },
            Token::LParen => {
                let pipe = self.pipeline()?;
                self.expect(&Token::RParen, ")")?;
                Expr::Subexpr(Box::new(pipe))
            }
            Token::LBracket => self.list()?,
            Token::LBrace => self.brace()?,
            other => return Err(ParseError::Unexpected(other)),
        })
    }

    fn list(&mut self) -> Result<Expr, ParseError> {
        let mut items = Vec::new();
        while !matches!(self.peek_tok(), Some(Token::RBracket) | None) {
            items.push(self.expr(0)?);
            if matches!(self.peek_tok(), Some(Token::Comma)) {
                self.bump();
            }
        }
        self.expect(&Token::RBracket, "]")?;
        Ok(Expr::List(items))
    }

    /// `{ key: val, ... }` (record) or `{ pipeline }` (block), disambiguated by
    /// an `ident :` / `string :` lookahead.
    fn brace(&mut self) -> Result<Expr, ParseError> {
        let is_record = matches!(self.peek_tok(), Some(Token::Ident(_)) | Some(Token::Str(_)))
            && matches!(
                self.toks.get(self.pos + 1).map(|s| &s.tok),
                Some(Token::Colon)
            )
            || matches!(self.peek_tok(), Some(Token::RBrace));
        if is_record {
            let mut fields = Vec::new();
            while !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                let key = match self.bump().map(|s| s.tok) {
                    Some(Token::Ident(k)) => k,
                    Some(Token::Str(k)) => k,
                    other => {
                        return Err(ParseError::Expected {
                            expected: "record key",
                            found: other.unwrap_or(Token::Pipe),
                        })
                    }
                };
                self.expect(&Token::Colon, ":")?;
                let val = self.expr(0)?;
                fields.push((key, val));
                if matches!(self.peek_tok(), Some(Token::Comma)) {
                    self.bump();
                }
            }
            self.expect(&Token::RBrace, "}")?;
            Ok(Expr::Record(fields))
        } else {
            let pipe = self.pipeline()?;
            self.expect(&Token::RBrace, "}")?;
            Ok(Expr::Block(Box::new(pipe)))
        }
    }

    fn expect(&mut self, want: &Token, name: &'static str) -> Result<(), ParseError> {
        match self.bump() {
            Some(s) if &s.tok == want => Ok(()),
            Some(s) => Err(ParseError::Expected {
                expected: name,
                found: s.tok,
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }
}

fn is_literal_word(name: &str) -> bool {
    matches!(name, "true" | "false" | "null")
}

fn as_binop(t: &Token) -> Option<(BinOp, u8)> {
    Some(match t {
        Token::Or => (BinOp::Or, 1),
        Token::And => (BinOp::And, 2),
        Token::EqEq => (BinOp::Eq, 3),
        Token::Ne => (BinOp::Ne, 3),
        Token::Lt => (BinOp::Lt, 4),
        Token::Le => (BinOp::Le, 4),
        Token::Gt => (BinOp::Gt, 4),
        Token::Ge => (BinOp::Ge, 4),
        Token::Plus => (BinOp::Add, 5),
        Token::Minus => (BinOp::Sub, 5),
        Token::Star => (BinOp::Mul, 6),
        Token::Slash => (BinOp::Div, 6),
        _ => return None,
    })
}
