use crate::ast::*;
use crate::common_enum;
use crate::value::*;
use std::hash::Hash;

common_enum! {
    /// Item is an syntax tree node that "declares" a thing without returning a value
    pub enum Item {
        Module(Module),
        Define(Define),
        Declare(Declare),
        Import(Import),
        Impl(Impl),
        Expr(Expr),
        Any(AnyBox),
    }
}

impl Item {
    pub fn any<T: AnyBoxable>(any: T) -> Self {
        Self::Any(AnyBox::new(any))
    }
    pub fn as_expr(&self) -> Option<&Expr> {
        match self {
            Self::Expr(expr) => Some(expr),
            _ => None,
        }
    }
    pub fn is_unit(&self) -> bool {
        if let Some(expr) = self.as_expr() {
            if let Expr::Value(value) = expr {
                if let Value::Unit(_) = value.as_ref() {
                    return true;
                }
            }
        }
        false
    }
    pub fn as_define(&self) -> Option<&Define> {
        match self {
            Self::Define(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_module(&self) -> Option<&Module> {
        match self {
            Self::Module(module) => Some(module),
            _ => None,
        }
    }
    pub fn as_import(&self) -> Option<&Import> {
        match self {
            Self::Import(import) => Some(import),
            _ => None,
        }
    }
    pub fn as_impl(&self) -> Option<&Impl> {
        match self {
            Self::Impl(impl_) => Some(impl_),
            _ => None,
        }
    }
    pub fn get_ident(&self) -> Option<&Ident> {
        match self {
            Self::Define(define) => Some(&define.name),
            Self::Declare(declare) => Some(&declare.name),
            Self::Module(module) => Some(&module.name),
            _ => None,
        }
    }
}

pub type ItemChunk = Vec<Item>;
pub trait ItemChunkExt {
    fn find_item(&self, name: &str) -> Option<&Item>;
    fn list_impl_trait(&self, trait_ty: &str) -> Vec<&Impl>;
    fn list_impl_type(&self, self_ty: &str) -> Vec<&Impl>;
}
impl ItemChunkExt for ItemChunk {
    fn find_item(&self, name: &str) -> Option<&Item> {
        self.iter().find(|item| {
            if let Some(ident) = item.get_ident() {
                ident.as_str() == name
            } else {
                false
            }
        })
    }
    fn list_impl_trait(&self, trait_ty: &str) -> Vec<&Impl> {
        self.iter()
            .filter_map(|item| match item {
                Item::Impl(impl_) => {
                    if let Some(trait_ty_) = &impl_.trait_ty {
                        if trait_ty_.to_string() == trait_ty {
                            Some(impl_)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect()
    }
    fn list_impl_type(&self, self_ty: &str) -> Vec<&Impl> {
        self.iter()
            .filter_map(|item| match item {
                Item::Impl(impl_) if impl_.trait_ty.is_none() => {
                    if let TypeExpr::Locator(Locator::Ident(ident)) = &impl_.self_ty {
                        if ident.as_str() == self_ty {
                            Some(impl_)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect()
    }
}
common_derives! {
    pub struct Module {
        pub name: Ident,
        pub items: ItemChunk,
        pub visibility: Visibility,
    }
}

common_derives! {
    pub struct Block {
        pub stmts: StatementChunk,
    }
}
impl Block {
    pub fn new(stmts: StatementChunk) -> Self {
        Self { stmts }
    }
    pub fn prepend(lhs: StatementChunk, rhs: Expr) -> Self {
        let mut stmts = lhs;
        match rhs {
            Expr::Block(block) => {
                stmts.extend(block.stmts);
            }
            _ => {
                stmts.push(Statement::Expr(rhs));
            }
        }
        Self::new(stmts)
    }
    pub fn make_last_side_effect(&mut self) {
        if let Some(last) = self.stmts.last_mut() {
            last.try_make_stmt();
        }
    }
}

common_derives! {
    #[derive(Copy)]
    pub enum Visibility {
        Public,
        Private,
        Inherited,
    }
}

common_derives! {
    #[derive(Copy)]
    pub enum DefineKind {
        Unknown,
        Function,
        Type,
        Const,
        Trait,
    }
}

common_derives! {
    pub enum DefineValue {
        Function(FunctionValue),
        Type(TypeExpr),
        Const(Expr),
        Trait(Trait),
    }
}
impl DefineValue {
    pub fn as_function(&self) -> Option<&FunctionValue> {
        match self {
            DefineValue::Function(fn_) => Some(fn_),
            _ => None,
        }
    }
    pub fn as_type(&self) -> Option<&TypeExpr> {
        match self {
            DefineValue::Type(ty) => Some(ty),
            _ => None,
        }
    }
    pub fn as_const(&self) -> Option<&Expr> {
        match self {
            DefineValue::Const(expr) => Some(expr),
            _ => None,
        }
    }
}

common_derives! {
    pub struct Define {
        pub name: Ident,
        pub kind: DefineKind,
        pub ty: Option<TypeValue>,
        pub value: DefineValue,
        pub visibility: Visibility,
    }
}

common_derives! {
    pub struct Import {
        pub visibility: Visibility,
        pub path: Path,
    }
}

common_derives! {
    pub struct Impl {
        pub trait_ty: Option<Locator>,
        pub self_ty: TypeExpr,
        pub items: ItemChunk,
    }
}

common_derives! {
    pub struct Trait {
        pub name: Ident,
        pub bounds: TypeBounds,
        pub items: ItemChunk,
    }
}
common_derives! {
    pub enum DeclareKind {
        Const { ty: TypeValue },
        Type { bounds: TypeBounds },
        Function { sig: FunctionSignature },

    }
}
common_derives! {
    pub struct Declare {
        pub name: Ident,
        pub kind: DeclareKind,
    }
}
