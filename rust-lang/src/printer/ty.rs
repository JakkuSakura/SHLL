use crate::printer::RustPrinter;
use eyre::bail;
use itertools::Itertools;
use lang_core::ast::{
    AstType, DecimalType, ExprInvoke, StructuralField, TypeInt, TypePrimitive, TypeReference,
    TypeStruct, TypeStructural,
};
use proc_macro2::TokenStream;
use quote::quote;
impl RustPrinter {
    pub fn print_type(&self, v: &AstType) -> eyre::Result<TokenStream> {
        let ty = match v {
            AstType::Function(f) => self.print_func_type(f)?,
            AstType::Primitive(p) => self.print_primitive_type(*p)?,
            AstType::Struct(s) => self.print_struct_type(s)?,
            AstType::Structural(s) => self.print_structural_type(s)?,
            AstType::Expr(e) => self.print_expr(e)?,
            AstType::ImplTraits(t) => self.print_impl_traits(t)?,
            AstType::TypeBounds(t) => self.print_type_bounds(t)?,
            AstType::Unit(_) => quote!(()),
            AstType::Any(_) => quote!(dyn Any),
            AstType::Nothing(_) => quote!(!),
            AstType::Unknown(_) => quote!(_),
            AstType::Reference(r) => self.print_type_ref(r)?,
            AstType::Value(v) => self.print_value(&v.value)?,
            _ => bail!("Not supported {:?}", v),
        };
        Ok(ty)
    }
    fn print_type_ref(&self, reference: &TypeReference) -> eyre::Result<TokenStream> {
        let ty = self.print_type(&reference.ty)?;
        if reference.mutability == Some(true) {
            Ok(quote!(&mut #ty))
        } else {
            Ok(quote!(&#ty))
        }
    }

    pub fn print_primitive_type(&self, ty: TypePrimitive) -> eyre::Result<TokenStream> {
        match ty {
            TypePrimitive::Int(TypeInt::I64) => Ok(quote!(i64)),
            TypePrimitive::Int(TypeInt::U64) => Ok(quote!(u64)),
            TypePrimitive::Int(TypeInt::I32) => Ok(quote!(i32)),
            TypePrimitive::Int(TypeInt::U32) => Ok(quote!(u32)),
            TypePrimitive::Int(TypeInt::I16) => Ok(quote!(i16)),
            TypePrimitive::Int(TypeInt::U16) => Ok(quote!(u16)),
            TypePrimitive::Int(TypeInt::I8) => Ok(quote!(i8)),
            TypePrimitive::Int(TypeInt::U8) => Ok(quote!(u8)),
            TypePrimitive::Decimal(DecimalType::F64) => Ok(quote!(f64)),
            TypePrimitive::Decimal(DecimalType::F32) => Ok(quote!(f32)),
            TypePrimitive::Bool => Ok(quote!(bool)),
            TypePrimitive::String => Ok(quote!(String)),
            TypePrimitive::Char => Ok(quote!(char)),
            TypePrimitive::List => Ok(quote!(Vec)),
            _ => bail!("Not supported {:?}", ty),
        }
    }
    pub fn print_struct_type(&self, s: &TypeStruct) -> eyre::Result<TokenStream> {
        let name = self.print_ident(&s.name);
        let fields: Vec<_> = s
            .fields
            .iter()
            .map(|x| self.print_field(&x))
            .try_collect()?;
        Ok(quote!(struct #name {
            #(#fields), *
        }))
    }

    pub fn print_field(&self, field: &StructuralField) -> eyre::Result<TokenStream> {
        let name = self.print_ident(&field.name);
        let ty = self.print_type(&field.value)?;
        Ok(quote!(pub #name: #ty ))
    }

    pub fn print_structural_type(&self, s: &TypeStructural) -> eyre::Result<TokenStream> {
        let fields: Vec<_> = s
            .fields
            .iter()
            .map(|x| self.print_field(&x))
            .try_collect()?;
        Ok(quote!(
            struct {
                #(#fields), *
            }
        ))
    }

    pub fn print_invoke_type(&self, invoke: &ExprInvoke) -> eyre::Result<TokenStream> {
        let fun = self.print_invoke_target(&invoke.target)?;
        let args: Vec<_> = invoke
            .args
            .iter()
            .map(|x| self.print_expr(&x.get()))
            .try_collect()?;
        Ok(quote!(
            #fun::<#(#args), *>
        ))
    }
}
