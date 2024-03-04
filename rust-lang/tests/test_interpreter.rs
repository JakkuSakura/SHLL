use common::assert_eq;
use common::*;
use common_lang::context::ScopedContext;
use common_lang::expr::*;
use common_lang::interpreter::Interpreter;
use common_lang::register_threadlocal_serializer;
use common_lang::value::Value;
use rust_lang::{shll_parse_expr, shll_parse_value, RustSerde};
use std::rc::Rc;

fn interpret_shll_expr(expr: Expr) -> Result<Value> {
    let interpreter = Interpreter::new(Rc::new(RustSerde::new()));
    let ctx = ScopedContext::new().into_shared();
    interpreter.interpret_expr(expr, &ctx)
}

#[test]
fn test_eval_arithmetics() -> Result<()> {
    register_threadlocal_serializer(Rc::new(RustSerde::new()));

    let code = shll_parse_expr! {
        1 + 2 * 3
    };
    let value = interpret_shll_expr(code)?;
    let expected = shll_parse_value!(7);
    assert_eq!(value, expected);
    Ok(())
}
#[test]
fn test_eval_function_call() -> Result<()> {
    register_threadlocal_serializer(Rc::new(RustSerde::new()));

    let code = shll_parse_expr! {
        {
            fn foo(a: i64, b: i64) -> i64 {
                a + b
            }
            foo(1, 2)
        }
    };
    let value = interpret_shll_expr(code)?;
    let expected = shll_parse_value!(3);
    assert_eq!(value, expected);
    Ok(())
}

#[test]
fn test_eval_function_call_with_main() -> Result<()> {
    setup_logs(LogLevel::Debug)?;
    register_threadlocal_serializer(Rc::new(RustSerde::new()));

    let code = shll_parse_expr! {
        {
            fn foo(a: i64, b: i64) -> i64 {
                a + b
            }
            fn main() -> i64 {
                foo(1, 2)
            }
            main()
        }
    };
    let value = interpret_shll_expr(code)?;
    let expected = shll_parse_value!(3);
    assert_eq!(value, expected);
    Ok(())
}