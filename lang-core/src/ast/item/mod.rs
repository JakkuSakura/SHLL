use std::fmt::Formatter;
use std::hash::Hash;

use crate::ast::*;
use crate::id::{Ident, Locator};
use crate::utils::anybox::{AnyBox, AnyBoxable};
use crate::{common_enum, common_struct};

mod decl;
mod def;
mod import;

pub use decl::*;
pub use def::*;
pub use import::*;

pub type BItem = Box<AstItem>;

common_enum! {
    /// Item is syntax node that "declares" a thing without returning a value
    ///
    /// It usually happens at compile time
    ///
    /// For run timm declarations, like in Python, `class Foo: pass`, it is not an item,
    /// rather an Expr
    pub enum AstItem {
        Module(AstModule),
        DefStruct(ItemDefStruct),
        DefStructural(ItemDefStructural),
        DefEnum(ItemDefEnum),
        DefType(ItemDefType),
        DefConst(ItemDefConst),
        DefStatic(ItemDefStatic),
        DefFunction(ItemDefFunction),
        DefTrait(ItemDefTrait),
        DeclType(ItemDeclType),
        DeclConst(ItemDeclConst),
        DeclStatic(ItemDeclStatic),
        DeclFunction(ItemDeclFunction),
        Import(ItemImport),
        Impl(ItemImpl),
        /// not for direct construction, but for interpretation and optimization
        Expr(AstExpr),
        Any(AnyBox),
    }
}

impl std::fmt::Display for AstItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let serializer = get_threadlocal_serializer();
        write!(f, "{}", serializer.serialize_item(self).unwrap())
    }
}

impl AstItem {
    pub fn any<T: AnyBoxable>(any: T) -> Self {
        Self::Any(AnyBox::new(any))
    }
    pub fn as_expr(&self) -> Option<&AstExpr> {
        match self {
            Self::Expr(expr) => Some(expr),
            _ => None,
        }
    }
    pub fn unit() -> Self {
        Self::Expr(AstExpr::Value(AstValue::unit().into()))
    }
    pub fn is_unit(&self) -> bool {
        if let Some(expr) = self.as_expr() {
            if let AstExpr::Value(value) = expr {
                if let AstValue::Unit(_) = &**value {
                    return true;
                }
            }
        }
        false
    }
    pub fn as_function(&self) -> Option<&ItemDefFunction> {
        match self {
            Self::DefFunction(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_struct(&self) -> Option<&ItemDefStruct> {
        match self {
            Self::DefStruct(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_enum(&self) -> Option<&ItemDefEnum> {
        match self {
            Self::DefEnum(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_type(&self) -> Option<&ItemDefType> {
        match self {
            Self::DefType(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_const(&self) -> Option<&ItemDefConst> {
        match self {
            Self::DefConst(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_trait(&self) -> Option<&ItemDefTrait> {
        match self {
            Self::DefTrait(define) => Some(define),
            _ => None,
        }
    }
    pub fn as_module(&self) -> Option<&AstModule> {
        match self {
            Self::Module(module) => Some(module),
            _ => None,
        }
    }
    pub fn as_import(&self) -> Option<&ItemImport> {
        match self {
            Self::Import(import) => Some(import),
            _ => None,
        }
    }
    pub fn as_impl(&self) -> Option<&ItemImpl> {
        match self {
            Self::Impl(impl_) => Some(impl_),
            _ => None,
        }
    }
    pub fn get_ident(&self) -> Option<&Ident> {
        match self {
            Self::DefFunction(define) => Some(&define.name),
            Self::DefStruct(define) => Some(&define.name),
            Self::DefEnum(define) => Some(&define.name),
            Self::DefType(define) => Some(&define.name),
            Self::DefConst(define) => Some(&define.name),
            Self::DeclType(declare) => Some(&declare.name),
            Self::DeclConst(declare) => Some(&declare.name),
            Self::DeclFunction(declare) => Some(&declare.name),
            Self::Module(module) => Some(&module.name),
            _ => None,
        }
    }
}

pub type ItemChunk = Vec<AstItem>;
pub trait ItemChunkExt {
    fn find_item(&self, name: &str) -> Option<&AstItem>;
    fn list_impl_trait(&self, trait_ty: &str) -> Vec<&ItemImpl>;
    fn list_impl_type(&self, self_ty: &str) -> Vec<&ItemImpl>;
}
impl ItemChunkExt for ItemChunk {
    fn find_item(&self, name: &str) -> Option<&AstItem> {
        self.iter().find(|item| {
            if let Some(ident) = item.get_ident() {
                ident.as_str() == name
            } else {
                false
            }
        })
    }
    fn list_impl_trait(&self, trait_ty: &str) -> Vec<&ItemImpl> {
        self.iter()
            .filter_map(|item| match item {
                AstItem::Impl(impl_) => {
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
    fn list_impl_type(&self, self_ty: &str) -> Vec<&ItemImpl> {
        self.iter()
            .filter_map(|item| match item {
                AstItem::Impl(impl_) if impl_.trait_ty.is_none() => {
                    if let AstExpr::Locator(Locator::Ident(ident)) = &impl_.self_ty {
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
common_struct! {
    pub struct AstModule {
        pub name: Ident,
        pub items: ItemChunk,
        pub visibility: Visibility,
    }
}

common_enum! {
    /// Visibility is a label to an item
    /// The exact semantic is dependent on the language and context
    #[derive(Copy)]
    pub enum Visibility {
        Public,
        Private,
        Inherited,
    }
}

common_struct! {
    pub struct ItemImpl {
        pub trait_ty: Option<Locator>,
        pub self_ty: AstExpr,
        pub items: ItemChunk,
    }
}

impl ItemImpl {
    pub fn new_ident(self_ty: Ident, items: ItemChunk) -> Self {
        Self {
            trait_ty: None,
            self_ty: AstExpr::ident(self_ty).into(),
            items,
        }
    }
    pub fn new(trait_ty: Option<Locator>, self_ty: AstExpr, items: ItemChunk) -> Self {
        Self {
            trait_ty,
            self_ty,
            items,
        }
    }
}
