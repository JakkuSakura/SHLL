use crate::context::ExecutionContext;
use crate::ops::BinOpKind;
use crate::tree::{Expr, Ident};
use crate::value::*;
use crate::Serializer;
use common::*;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

#[derive(Clone, Serialize, Deserialize)]
pub enum BuiltinFnName {
    BinOpKind(BinOpKind),
    Name(Ident),
}
impl Display for BuiltinFnName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinFnName::BinOpKind(k) => std::fmt::Display::fmt(k, f),
            BuiltinFnName::Name(n) => std::fmt::Display::fmt(n, f),
        }
    }
}
impl Debug for BuiltinFnName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinFnName::BinOpKind(k) => std::fmt::Debug::fmt(k, f),
            BuiltinFnName::Name(n) => std::fmt::Debug::fmt(n, f),
        }
    }
}
#[derive(Clone)]
pub struct BuiltinFn {
    pub name: BuiltinFnName,
    f: Rc<dyn Fn(&[Expr], &ExecutionContext) -> Result<Expr>>,
}
impl BuiltinFn {
    pub fn new(
        name: BinOpKind,
        f: impl Fn(&[Expr], &ExecutionContext) -> Result<Expr> + 'static,
    ) -> Self {
        Self {
            name: BuiltinFnName::BinOpKind(name),
            f: Rc::new(f),
        }
    }
    pub fn new_with_ident(
        name: Ident,
        f: impl Fn(&[Expr], &ExecutionContext) -> Result<Expr> + 'static,
    ) -> Self {
        Self {
            name: BuiltinFnName::Name(name),
            f: Rc::new(f),
        }
    }
    pub fn call(&self, args: &[Expr], ctx: &ExecutionContext) -> Result<Expr> {
        (self.f)(args, ctx)
    }
}

impl Debug for BuiltinFn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltinFn")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
impl Serialize for BuiltinFn {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.name.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for BuiltinFn {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = BuiltinFnName::deserialize(deserializer)?;
        Ok(Self {
            name,
            f: Rc::new(|_, _| unreachable!()),
        })
    }
}

pub fn operate_on_literals(
    name: BinOpKind,
    op_i64: impl Fn(&[i64]) -> i64 + 'static,
    op_f64: impl Fn(&[f64]) -> f64 + 'static,
) -> BuiltinFn {
    BuiltinFn::new(name, move |args, _ctx| {
        let mut args_i64 = vec![];
        let mut args_f64 = vec![];
        for arg in args {
            match arg {
                Expr::Value(Value::Int(x)) => args_i64.push(x.value),
                Expr::Value(Value::Decimal(x)) => args_f64.push(x.value),
                _ => bail!("Does not support argument type {:?}", args),
            }
        }
        if !args_i64.is_empty() && !args_f64.is_empty() {
            bail!("Does not support argument type {:?}", args)
        }
        if !args_i64.is_empty() {
            return Ok(Expr::value(Value::int(op_i64(&args_i64))));
        }
        if !args_f64.is_empty() {
            return Ok(Expr::value(Value::decimal(op_f64(&args_f64))));
        }
        bail!("Does not support argument type {:?}", args)
    })
}
pub fn binary_comparison_on_literals(
    name: BinOpKind,
    op_i64: impl Fn(i64, i64) -> bool + 'static,
    op_f64: impl Fn(f64, f64) -> bool + 'static,
) -> BuiltinFn {
    BuiltinFn::new(name, move |args, _ctx| {
        if args.len() != 2 {
            bail!("Argument expected 2, got: {:?}", args)
        }
        let mut args_i64 = vec![];
        let mut args_f64 = vec![];
        for arg in args {
            match arg {
                Expr::Value(Value::Int(x)) => args_i64.push(x.value),
                Expr::Value(Value::Decimal(x)) => args_f64.push(x.value),
                _ => bail!("Does not support argument type {:?}", args),
            }
        }
        if !args_i64.is_empty() && !args_f64.is_empty() {
            bail!("Does not support argument type {:?}", args)
        }
        if !args_i64.is_empty() {
            return Ok(Expr::value(Value::bool(op_i64(args_i64[0], args_i64[1]))));
        }
        if !args_f64.is_empty() {
            return Ok(Expr::value(Value::bool(op_f64(args_f64[0], args_f64[1]))));
        }

        bail!("Does not support argument type {:?}", args)
    })
}
pub fn builtin_add() -> BuiltinFn {
    operate_on_literals(
        BinOpKind::Add,
        |x| x.into_iter().sum(),
        |x| x.into_iter().sum(),
    )
}
pub fn builtin_sub() -> BuiltinFn {
    operate_on_literals(
        BinOpKind::Sub,
        |x| {
            x.into_iter()
                .enumerate()
                .map(|(i, &x)| if i > 0 { -x } else { x })
                .sum()
        },
        |x| {
            x.into_iter()
                .enumerate()
                .map(|(i, &x)| if i > 0 { -x } else { x })
                .sum()
        },
    )
}

pub fn builtin_mul() -> BuiltinFn {
    operate_on_literals(
        BinOpKind::Mul,
        |x| x.into_iter().product(),
        |x| x.into_iter().product(),
    )
}

pub fn builtin_gt() -> BuiltinFn {
    binary_comparison_on_literals(BinOpKind::Gt, |x, y| x > y, |x, y| x > y)
}

pub fn builtin_ge() -> BuiltinFn {
    binary_comparison_on_literals(BinOpKind::Ge, |x, y| x >= y, |x, y| x >= y)
}
pub fn builtin_lt() -> BuiltinFn {
    binary_comparison_on_literals(BinOpKind::Lt, |x, y| x < y, |x, y| x < y)
}
pub fn builtin_le() -> BuiltinFn {
    binary_comparison_on_literals(BinOpKind::Le, |x, y| x <= y, |x, y| x <= y)
}
pub fn builtin_eq() -> BuiltinFn {
    binary_comparison_on_literals(BinOpKind::Eq, |x, y| x == y, |x, y| x == y)
}
pub fn builtin_ne() -> BuiltinFn {
    binary_comparison_on_literals(BinOpKind::Ne, |x, y| x != y, |x, y| x != y)
}

pub fn builtin_print(se: Rc<dyn Serializer>) -> BuiltinFn {
    BuiltinFn::new_with_ident("print".into(), move |args, ctx| {
        let formatted: Vec<_> = args
            .into_iter()
            .map(|x| se.serialize_expr(x))
            .try_collect()?;
        ctx.root().print_str(formatted.join(" "));
        Ok(Expr::unit())
    })
}
