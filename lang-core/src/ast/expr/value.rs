use crate::ast::FieldValue;
use crate::ast::{BExpr, Expr};
use crate::id::Ident;
use crate::ops::{BinOpKind, UnOpKind};
use crate::{common_enum, common_struct, get_threadlocal_serializer};
use std::fmt::{Display, Formatter};
use std::hash::Hash;

common_struct! {
    pub struct ExprInvoke {
        pub func: BExpr,
        pub args: Vec<BExpr>,
    }
}
impl Display for ExprInvoke {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = get_threadlocal_serializer().serialize_invoke(self).unwrap();

        f.write_str(&s)
    }
}
common_enum! {
    pub enum SelectType {
        Unknown,
        Field,
        Method,
        Function,
        Const,
    }

}

common_struct! {
    pub struct ExprSelect {
        pub obj: BExpr,
        pub field: Ident,
        pub select: SelectType,
    }
}

common_struct! {
    pub struct ExprReference {
        pub referee: BExpr,
        pub mutable: Option<bool>,
    }
}

common_struct! {
    pub struct ExprMatch {
        pub cases: Vec<MatchCase>,
    }
}

common_struct! {
    pub struct ExprIf {
        pub cases: Vec<MatchCase>,
    }
}
common_struct! {
    pub struct ExprLoop {
        pub content: BExpr,
    }
}

common_struct! {
    pub struct MatchCase {
        pub cond: Expr,
        pub body: Expr,
    }
}

common_enum! {
    pub enum ControlFlow {
        Continue,
        #[from(ignore)]
        Break(Option<Expr>),
        #[from(ignore)]
        Return(Option<Expr>),
        Into,
        #[from(ignore)]
        IntoAndBreak(Option<Expr>),
    }
}
common_struct! {
    pub struct ExprInitStruct {
        pub name: BExpr, // TypeExpr
        pub fields: Vec<FieldValue>,
    }
}
common_struct! {
    pub struct ExprInitStructural {
        pub fields: Vec<FieldValue>,
    }
}

common_struct! {
    pub struct ExprBinOp {
        pub kind: BinOpKind,
        pub lhs: BExpr,
        pub rhs: BExpr,
    }
}
common_struct! {
    pub struct ExprUnOp {
        pub op: UnOpKind,
        pub val: BExpr,

    }
}
