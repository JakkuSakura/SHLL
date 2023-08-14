use serde::*;
use std::fmt::{Debug, Display, Formatter};
pub mod builtins;

use crate::tree::Ident;
pub use builtins::*;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
    LogicalOr,
    LogicalAnd,
    BitOr,
    BitAnd,
    BitXor,
    Any(Ident),
}
impl Display for BinOpKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOpKind::Add => write!(f, "+"),
            BinOpKind::Sub => write!(f, "-"),
            BinOpKind::Mul => write!(f, "*"),
            BinOpKind::Div => write!(f, "/"),
            BinOpKind::Mod => write!(f, "%"),
            BinOpKind::Gt => write!(f, ">"),
            BinOpKind::Lt => write!(f, "<"),
            BinOpKind::Ge => write!(f, ">="),
            BinOpKind::Le => write!(f, "<="),
            BinOpKind::Eq => write!(f, "=="),
            BinOpKind::Ne => write!(f, "!="),
            BinOpKind::LogicalOr => write!(f, "||"),
            BinOpKind::LogicalAnd => write!(f, "&&"),
            BinOpKind::BitOr => write!(f, "|"),
            BinOpKind::BitAnd => write!(f, "&"),
            BinOpKind::BitXor => write!(f, "^"),
            BinOpKind::Any(i) => write!(f, "{}", i),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinOp<T> {
    Add(AddOp<T>),
    Sub(SubOp<T>),
    Mul(MulOp<T>),
    Div(DivOp<T>),
    Mod(ModOp<T>),
    Gt(GtOp<T>),
    Lt(LtOp<T>),
    Gte(GteOp<T>),
    Lte(LteOp<T>),
    Eq(EqOp<T>),
    Ne(NeOp<T>),
    Any(AnyBinOp<T>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MulOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GteOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LteOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeOp<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnyBinOp<T> {
    pub kind: BinOpKind,
    pub lhs: Box<T>,
    pub rhs: Box<T>,
}