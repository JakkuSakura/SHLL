use crate::ast::{get_threadlocal_serializer, AstType, BItem, BValue, Value, ValueUnit};
use crate::common_enum;
use crate::id::{Ident, Locator, Path};
use crate::utils::anybox::{AnyBox, AnyBoxable};
use std::fmt::{Debug, Display, Formatter};

mod closure;
mod stmt;
mod ty;
mod value;

pub use closure::*;
pub use stmt::*;
pub use ty::*;
pub use value::*;

pub type ExprId = u64;
pub type BExpr = Box<AstExpr>;

common_enum! {
    /// Expr is an expression that returns a value, note that a Type is also a Value
    pub enum AstExpr {
        /// An id for the expression node
        Id(ExprId),
        Locator(Locator),
        Value(BValue),
        Block(ExprBlock),
        Match(ExprMatch),
        If(ExprIf),
        Loop(ExprLoop),
        Invoke(ExprInvoke),
        BinOp(ExprBinOp),
        UnOp(ExprUnOp),
        Assign(ExprAssign),
        Select(ExprSelect),
        Index(ExprIndex),
        InitStruct(ExprInitStruct),
        InitStructual(ExprInitStructural),
        Reference(ExprReference),

        /// closured because it's conceptually a closure, not a real one
        Closured(ExprClosure),
        Paren(ExprParen),
        SelfType(ExprSelfType),
        Range(ExprRange),
        /// for items in dynamic languages
        Item(BItem),
        Any(AnyBox),
    }

}

impl AstExpr {
    pub fn get(&self) -> Self {
        self.clone()
    }
    pub fn unit() -> AstExpr {
        AstExpr::Value(Value::Unit(ValueUnit).into())
    }
    pub fn is_unit(&self) -> bool {
        match self {
            AstExpr::Value(value) => value.is_unit(),
            _ => false,
        }
    }
    pub fn value(v: Value) -> AstExpr {
        match v {
            Value::Expr(expr) => *expr,
            Value::Any(any) => AstExpr::Any(any),
            Value::Type(AstType::Expr(expr)) => *expr,
            _ => AstExpr::Value(v.into()),
        }
    }
    pub fn ident(name: Ident) -> AstExpr {
        AstExpr::Locator(Locator::ident(name))
    }
    pub fn path(path: Path) -> AstExpr {
        AstExpr::Locator(Locator::path(path))
    }
    pub fn block(block: ExprBlock) -> AstExpr {
        if block.stmts.len() == 0 {
            return block.ret.map(|x| *x).unwrap_or(AstExpr::unit());
        }

        AstExpr::Block(block)
    }
    pub fn any<T: AnyBoxable>(any: T) -> Self {
        Self::Any(AnyBox::new(any))
    }
}
impl Display for AstExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = get_threadlocal_serializer().serialize_expr(self).unwrap();
        f.write_str(&s)
    }
}
impl From<BExpr> for AstExpr {
    fn from(expr: BExpr) -> Self {
        *expr
    }
}
