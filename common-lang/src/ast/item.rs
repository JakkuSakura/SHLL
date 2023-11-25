use crate::ast::*;
use crate::common_enum;
use crate::value::*;
use std::hash::Hash;

common_enum! {
    /// Item is an syntax tree node that "declares" a thing without returning a value
    pub enum Item {
        Module(Module),
        DefStruct(DefStruct),
        DefEnum(DefEnum),
        DefType(DefType),
        DefConst(DefConst),
        DefFunction(DefFunction),
        DefTrait(DefTrait),
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
            Self::DefFunction(define) => Some(&define.name),
            Self::DefStruct(define) => Some(&define.name),
            Self::DefEnum(define) => Some(&define.name),
            Self::DefType(define) => Some(&define.name),
            Self::DefConst(define) => Some(&define.name),
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
    pub struct DefStruct {
        pub name: Ident,
        pub value: TypeStruct,
        pub visibility: Visibility,
    }
}

common_derives! {
    pub struct DefEnum {
        pub name: Ident,
        pub value: TypeEnum,
        pub visibility: Visibility,
    }
}
common_derives! {
    pub struct DefType {
        pub name: Ident,
        pub value: TypeValue,
        pub visibility: Visibility,
    }
}
common_derives! {
    pub struct DefConst {
        pub name: Ident,
        pub ty: Option<TypeValue>,
        pub value: Value,
        pub visibility: Visibility,
    }
}
common_derives! {
    pub struct DefFunction {
        pub name: Ident,
        pub ty: Option<TypeFunction>,
        pub value: ValueFunction,
        pub visibility: Visibility,
    }
}
common_derives! {
    pub struct DefTrait {
        pub name: Ident,
        pub bounds: TypeBounds,
        pub items: ItemChunk,
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
