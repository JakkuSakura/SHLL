use crate::ast::{AnyBox, AnyBoxable, Block, Expr, Ident, Item, Pattern};
use crate::common_derives;
use std::hash::Hash;

common_derives! {
    pub enum Statement {
        Item(Box<Item>),
        Let(StatementLet),
        Assign(StatementAssign),
        SideEffect(SideEffect),
        ForEach(StatementForEach),
        While(StatementWhile),
        Loop(StatementLoop),
        Expr(Expr),
        Any(AnyBox),
    }
}

impl Statement {
    pub fn any<T: AnyBoxable>(any: T) -> Self {
        Self::Any(AnyBox::new(any))
    }
    pub fn item(item: Item) -> Self {
        Self::Item(Box::new(item))
    }
    pub fn stmt_expr(expr: Expr) -> Self {
        Self::SideEffect(SideEffect { expr })
    }
    pub fn maybe_stmt_expr(expr: Expr, is_stmt: bool) -> Self {
        if is_stmt {
            Self::stmt_expr(expr)
        } else {
            Self::Expr(expr)
        }
    }
    pub fn try_make_stmt(&mut self) {
        if let Self::Expr(expr) = self {
            *self = Self::stmt_expr(expr.clone());
        }
    }
    pub fn try_make_expr(&mut self) {
        if let Self::SideEffect(expr) = self {
            *self = Self::Expr(expr.expr.clone());
        }
    }
    pub fn is_unit(&self) -> bool {
        match self {
            Self::Expr(expr) => expr.is_unit(),
            Self::SideEffect(expr) => expr.expr.is_unit(),
            _ => false,
        }
    }
}

common_derives! {
    pub struct SideEffect {
        pub expr: Expr,
    }

}
impl SideEffect {
    pub fn new(expr: Expr) -> Self {
        match expr {
            Expr::Block(block) => {
                let mut block = block;
                block.make_last_side_effect();
                Self {
                    expr: Expr::Block(block),
                }
            }
            _ => Self { expr },
        }
    }
}
common_derives! {
    pub struct StatementLet {
        pub pat: Pattern,
        pub value: Expr,
    }
}

common_derives! {
    pub struct StatementAssign {
        pub target: Expr,
        pub value: Expr,
    }
}

common_derives! {
    pub struct StatementForEach {
        pub variable: Ident,
        pub iterable: Expr,
        pub body: Block,
    }
}

common_derives! {
    pub struct StatementWhile {
        pub cond: Expr,
        pub body: Block,
    }
}
common_derives! {
    pub struct StatementLoop {
        pub body: Block,
    }
}
