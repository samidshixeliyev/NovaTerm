//! `nova-cmd` — NovaCore's command engine: the [`Command`] trait, the
//! expression evaluator, the [`Registry`], and the built-in commands. Commands
//! consume and produce structured [`nova_value::Value`]s.

#![forbid(unsafe_code)]

pub mod builtins;
pub mod command;
pub mod error;
pub mod eval;
pub mod registry;

pub use builtins::register_builtins;
pub use command::{Call, Command, EvalCtx, Signature};
pub use error::CmdError;
pub use eval::{eval_expr, Host};
pub use registry::Registry;

#[cfg(test)]
mod tests {
    use super::*;
    use nova_lang::{parse_str, Command as AstCommand, Expr};
    use nova_value::{Record, Value};

    struct NullHost;
    impl Host for NullHost {
        fn run_pipeline(&mut self, _p: &nova_lang::Pipeline, _i: Value) -> Result<Value, CmdError> {
            Err(CmdError::msg("sub-pipelines require the engine host"))
        }
    }

    fn cmd_of(src: &str) -> AstCommand {
        let mut p = parse_str(src).unwrap();
        match p.stages.remove(0) {
            Expr::Command(c) => c,
            other => panic!("not a command: {other:?}"),
        }
    }

    fn rec(name: &str, size: u64) -> Value {
        let mut r = Record::new();
        r.push("name", Value::from(name));
        r.push("size", Value::Filesize(size));
        Value::Record(r)
    }

    fn run(src: &str, input: Value) -> Value {
        let mut reg = Registry::new();
        register_builtins(&mut reg);
        let c = cmd_of(src);
        let cmd = reg
            .get(&c.name)
            .unwrap_or_else(|| panic!("no command {}", c.name));
        let mut host = NullHost;
        let mut ctx = EvalCtx {
            cwd: std::env::temp_dir(),
            host: &mut host,
        };
        let call = Call::new(c.name.clone(), &c.args);
        cmd.run(&mut ctx, input, &call).unwrap()
    }

    fn sample() -> Value {
        Value::List(vec![rec("b", 50), rec("a", 200), rec("c", 10)])
    }

    #[test]
    fn registry_has_builtins() {
        let mut reg = Registry::new();
        register_builtins(&mut reg);
        for c in [
            "ls", "where", "sort-by", "first", "get", "echo", "pwd", "lines", "length",
        ] {
            assert!(reg.contains(c), "missing builtin {c}");
        }
    }

    #[test]
    fn where_filters_by_field() {
        let out = run("where size > 100", sample());
        let rows = match out {
            Value::List(r) => r,
            other => panic!("{other:?}"),
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name"), Some(Value::from("a")));
    }

    #[test]
    fn sort_by_field() {
        let out = run("sort-by name", sample());
        let Value::List(rows) = out else { panic!() };
        let names: Vec<Value> = rows.iter().map(|r| r.get("name").unwrap()).collect();
        assert_eq!(
            names,
            vec![Value::from("a"), Value::from("b"), Value::from("c")]
        );
    }

    #[test]
    fn first_n() {
        let out = run("first 2", sample());
        let Value::List(rows) = out else { panic!() };
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn get_column() {
        let out = run("get size", sample());
        assert_eq!(
            out,
            Value::List(vec![
                Value::Filesize(50),
                Value::Filesize(200),
                Value::Filesize(10)
            ])
        );
    }

    #[test]
    fn echo_and_length() {
        assert_eq!(run("echo hello", Value::Null), Value::from("hello"));
        assert_eq!(run("length", sample()), Value::Int(3));
    }
}
