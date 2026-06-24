//! The expression evaluator: evaluates a NovaLang [`Expr`] against an input
//! [`Value`] (the current `$in` / `$it`). Pipelines/subexpressions are delegated
//! to the [`Host`] (implemented by `nova-engine`), keeping this crate free of
//! the pipeline driver while staying unit-testable on its own.

use crate::error::CmdError;
use nova_lang::{BinOp, Expr, Literal, Pipeline, UnaryOp};
use nova_value::{Record, Value};

/// Implemented by the engine: runs a sub-pipeline / block with a given input.
pub trait Host {
    fn run_pipeline(&mut self, pipeline: &Pipeline, input: Value) -> Result<Value, CmdError>;
}

/// Evaluate an expression. A `Bareword` resolves to a field of `input` when
/// `input` has it, otherwise to its literal string — this is what makes
/// `where size > 0` (row field) and `sort-by name` (column name) both work.
pub fn eval_expr(expr: &Expr, input: &Value, host: &mut dyn Host) -> Result<Value, CmdError> {
    match expr {
        Expr::Literal(l) => Ok(literal(l)),
        Expr::Bareword(name) => Ok(input
            .get(name)
            .unwrap_or_else(|| Value::String(name.clone()))),
        Expr::Var(segments) => Ok(resolve_var(segments, input)),
        Expr::Unary(op, e) => unary(*op, eval_expr(e, input, host)?),
        Expr::Binary(l, op, r) => {
            let lhs = eval_expr(l, input, host)?;
            let rhs = eval_expr(r, input, host)?;
            binary(lhs, *op, rhs)
        }
        Expr::Subexpr(p) | Expr::Block(p) => host.run_pipeline(p, input.clone()),
        Expr::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(eval_expr(it, input, host)?);
            }
            Ok(Value::List(out))
        }
        Expr::Record(fields) => {
            let mut r = Record::new();
            for (k, v) in fields {
                r.push(k.clone(), eval_expr(v, input, host)?);
            }
            Ok(Value::Record(r))
        }
        Expr::Command(c) => Err(CmdError::msg(format!(
            "command `{}` cannot appear in an expression",
            c.name
        ))),
    }
}

fn literal(l: &Literal) -> Value {
    match l {
        Literal::Int(i) => Value::Int(*i),
        Literal::Float(f) => Value::Float(*f),
        Literal::Str(s) => Value::String(s.clone()),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Null => Value::Null,
        Literal::Filesize(b) => Value::Filesize(*b),
        Literal::Duration(d) => Value::Duration(*d),
    }
}

fn resolve_var(segments: &[String], input: &Value) -> Value {
    let mut cur = match segments.first().map(String::as_str) {
        Some("in") | Some("it") => input.clone(),
        _ => return Value::Null, // user variables land here in a later phase
    };
    for seg in &segments[1..] {
        cur = cur.get(seg).unwrap_or(Value::Null);
    }
    cur
}

fn unary(op: UnaryOp, v: Value) -> Result<Value, CmdError> {
    Ok(match op {
        UnaryOp::Not => Value::Bool(!v.is_truthy()),
        UnaryOp::Neg => match v {
            Value::Int(i) => Value::Int(-i),
            Value::Float(f) => Value::Float(-f),
            other => {
                return Err(CmdError::Type {
                    ctx: "negation",
                    expected: "number",
                    got: other.type_name(),
                })
            }
        },
    })
}

fn binary(lhs: Value, op: BinOp, rhs: Value) -> Result<Value, CmdError> {
    use std::cmp::Ordering;
    let cmp = || lhs.compare(&rhs);
    Ok(match op {
        BinOp::Eq => Value::Bool(lhs == rhs),
        BinOp::Ne => Value::Bool(lhs != rhs),
        BinOp::Lt => Value::Bool(matches!(cmp(), Some(Ordering::Less))),
        BinOp::Le => Value::Bool(matches!(cmp(), Some(Ordering::Less | Ordering::Equal))),
        BinOp::Gt => Value::Bool(matches!(cmp(), Some(Ordering::Greater))),
        BinOp::Ge => Value::Bool(matches!(cmp(), Some(Ordering::Greater | Ordering::Equal))),
        BinOp::And => Value::Bool(lhs.is_truthy() && rhs.is_truthy()),
        BinOp::Or => Value::Bool(lhs.is_truthy() || rhs.is_truthy()),
        BinOp::Add => arith(lhs, rhs, |a, b| a + b, i64::checked_add)?,
        BinOp::Sub => arith(lhs, rhs, |a, b| a - b, i64::checked_sub)?,
        BinOp::Mul => arith(lhs, rhs, |a, b| a * b, i64::checked_mul)?,
        BinOp::Div => match (lhs.as_number(), rhs.as_number()) {
            (Some(a), Some(b)) if b != 0.0 => Value::Float(a / b),
            _ => return Err(CmdError::msg("division by zero or non-numeric operands")),
        },
    })
}

fn arith(
    lhs: Value,
    rhs: Value,
    f: fn(f64, f64) -> f64,
    fi: fn(i64, i64) -> Option<i64>,
) -> Result<Value, CmdError> {
    if let (Value::String(a), Value::String(b)) = (&lhs, &rhs) {
        return Ok(Value::String(format!("{a}{b}"))); // string concat via `+`
    }
    match (lhs.as_int(), rhs.as_int(), &lhs, &rhs) {
        (Some(a), Some(b), Value::Int(_), Value::Int(_)) => Ok(Value::Int(fi(a, b).unwrap_or(0))),
        _ => match (lhs.as_number(), rhs.as_number()) {
            (Some(a), Some(b)) => Ok(Value::Float(f(a, b))),
            _ => Err(CmdError::Type {
                ctx: "arithmetic",
                expected: "number",
                got: lhs.type_name(),
            }),
        },
    }
}
