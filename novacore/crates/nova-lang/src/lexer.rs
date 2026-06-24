//! NovaLang lexer: source text → tokens (with spans).

use crate::ast::Span;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LexError {
    #[error("unterminated string starting at byte {0}")]
    UnterminatedString(usize),
    #[error("unexpected character {1:?} at byte {0}")]
    Unexpected(usize, char),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Pipe,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Assign, // =
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    Int(i64),
    Float(f64),
    Filesize(u64),
    Duration(i64),
    Str(String),
    Ident(String),
    LongFlag(String),
    ShortFlag(String),
    Var(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub tok: Token,
    pub span: Span,
}

pub fn lex(src: &str) -> Result<Vec<Spanned>, LexError> {
    Lexer {
        src: src.as_bytes(),
        pos: 0,
    }
    .run()
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn run(mut self) -> Result<Vec<Spanned>, LexError> {
        let mut out = Vec::new();
        while let Some(t) = self.next_token()? {
            out.push(t);
        }
        Ok(out)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }
    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    fn emit(&self, tok: Token, start: usize) -> Spanned {
        Spanned {
            tok,
            span: Span::new(start, self.pos),
        }
    }

    fn next_token(&mut self) -> Result<Option<Spanned>, LexError> {
        self.skip_trivia();
        let start = self.pos;
        let Some(c) = self.peek() else {
            return Ok(None);
        };

        // Multi/single char operators and punctuation.
        macro_rules! two {
            ($b:expr, $t:expr) => {{
                if self.peek2() == Some($b) {
                    self.pos += 2;
                    return Ok(Some(self.emit($t, start)));
                }
            }};
        }
        match c {
            b'|' => {
                if self.peek2() == Some(b'|') {
                    self.pos += 2;
                    return Ok(Some(self.emit(Token::Or, start)));
                }
                self.pos += 1;
                Ok(Some(self.emit(Token::Pipe, start)))
            }
            b'&' => {
                two!(b'&', Token::And);
                Err(LexError::Unexpected(start, '&'))
            }
            b'(' => self.single(Token::LParen, start),
            b')' => self.single(Token::RParen, start),
            b'{' => self.single(Token::LBrace, start),
            b'}' => self.single(Token::RBrace, start),
            b'[' => self.single(Token::LBracket, start),
            b']' => self.single(Token::RBracket, start),
            b',' => self.single(Token::Comma, start),
            b':' => self.single(Token::Colon, start),
            b'+' => self.single(Token::Plus, start),
            b'*' => self.single(Token::Star, start),
            b'/' => self.single(Token::Slash, start),
            b'=' => {
                two!(b'=', Token::EqEq);
                self.single(Token::Assign, start)
            }
            b'!' => {
                two!(b'=', Token::Ne);
                self.single(Token::Bang, start)
            }
            b'<' => {
                two!(b'=', Token::Le);
                self.single(Token::Lt, start)
            }
            b'>' => {
                two!(b'=', Token::Ge);
                self.single(Token::Gt, start)
            }
            b'"' | b'\'' => Ok(Some(self.string(c, start)?)),
            b'$' => Ok(Some(self.variable(start))),
            b'-' => Ok(Some(self.dash(start)?)),
            c if c.is_ascii_digit() => Ok(Some(self.number(start))),
            c if is_word_start(c) => Ok(Some(self.bareword(start))),
            other => Err(LexError::Unexpected(start, other as char)),
        }
    }

    fn single(&mut self, tok: Token, start: usize) -> Result<Option<Spanned>, LexError> {
        self.pos += 1;
        Ok(Some(self.emit(tok, start)))
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') => self.pos += 1,
                Some(b'#') => {
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self, quote: u8, start: usize) -> Result<Spanned, LexError> {
        self.pos += 1; // opening quote
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == quote {
                self.pos += 1;
                return Ok(self.emit(Token::Str(s), start));
            }
            if c == b'\\' && quote == b'"' {
                self.pos += 1;
                match self.peek() {
                    Some(b'n') => s.push('\n'),
                    Some(b't') => s.push('\t'),
                    Some(b'\\') => s.push('\\'),
                    Some(b'"') => s.push('"'),
                    Some(other) => s.push(other as char),
                    None => break,
                }
                self.pos += 1;
            } else {
                s.push(c as char);
                self.pos += 1;
            }
        }
        Err(LexError::UnterminatedString(start))
    }

    fn variable(&mut self, start: usize) -> Spanned {
        self.pos += 1; // $
        let mut segments = vec![self.take_segment()];
        while self.peek() == Some(b'.') {
            self.pos += 1;
            segments.push(self.take_segment());
        }
        self.emit(Token::Var(segments), start)
    }

    /// A dotless identifier segment (variable name / field name).
    fn take_segment(&mut self) -> String {
        let s = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.src[s..self.pos]).into_owned()
    }

    fn dash(&mut self, start: usize) -> Result<Spanned, LexError> {
        // `--name` long flag, `-abc` short flags, `-3`/`-1.5` negative number,
        // bare `-` minus operator.
        match self.peek2() {
            Some(b'-') => {
                self.pos += 2;
                let name = self.take_word();
                Ok(self.emit(Token::LongFlag(name), start))
            }
            Some(c) if c.is_ascii_alphabetic() => {
                self.pos += 1;
                let name = self.take_word();
                Ok(self.emit(Token::ShortFlag(name), start))
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                self.pos += 1; // consume '-', number() reads digits; negate after
                let n = self.number(start);
                Ok(negate(n))
            }
            _ => {
                self.pos += 1;
                Ok(self.emit(Token::Minus, start))
            }
        }
    }

    fn number(&mut self, start: usize) -> Spanned {
        let num_start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') && self.peek2().is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.pos += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let num_text = std::str::from_utf8(&self.src[num_start..self.pos]).unwrap_or("0");
        let value: f64 = num_text.parse().unwrap_or(0.0);

        // Optional unit suffix → typed literal.
        let unit_start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        let unit = std::str::from_utf8(&self.src[unit_start..self.pos]).unwrap_or("");
        if !unit.is_empty() {
            if let Some(bytes) = filesize_mult(unit) {
                return self.emit(Token::Filesize((value * bytes) as u64), start);
            }
            if let Some(nanos) = duration_mult(unit) {
                return self.emit(Token::Duration((value * nanos) as i64), start);
            }
            // Unknown suffix: roll back so it lexes as a separate bareword.
            self.pos = unit_start;
        }
        if is_float {
            self.emit(Token::Float(value), start)
        } else {
            self.emit(Token::Int(value as i64), start)
        }
    }

    fn bareword(&mut self, start: usize) -> Spanned {
        let word = self.take_word();
        self.emit(Token::Ident(word), start)
    }

    /// Consume a run of word characters.
    fn take_word(&mut self) -> String {
        let s = self.pos;
        while self.peek().is_some_and(is_word_char) {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.src[s..self.pos]).into_owned()
    }
}

fn negate(s: Spanned) -> Spanned {
    let tok = match s.tok {
        Token::Int(i) => Token::Int(-i),
        Token::Float(f) => Token::Float(-f),
        Token::Filesize(b) => Token::Int(-(b as i64)),
        Token::Duration(d) => Token::Duration(-d),
        other => other,
    };
    Spanned { tok, span: s.span }
}

fn is_word_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || matches!(c, b'_' | b'.' | b'/' | b'~' | b'*' | b'?' | b'@' | b'%')
}

fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            b'_' | b'.' | b'/' | b'~' | b'*' | b'?' | b'@' | b'%' | b'-' | b'+'
        )
}

fn filesize_mult(unit: &str) -> Option<f64> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1024f64.powi(4),
        "pb" => 1024f64.powi(5),
        _ => return None,
    })
}

fn duration_mult(unit: &str) -> Option<f64> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "ns" => 1.0,
        "us" => 1_000.0,
        "ms" => 1_000_000.0,
        "s" => 1_000_000_000.0,
        "m" => 60.0 * 1e9,
        "h" => 3600.0 * 1e9,
        "d" => 86_400.0 * 1e9,
        _ => return None,
    })
}
