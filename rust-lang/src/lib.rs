pub mod parser;
pub mod printer;
pub mod rustfmt;

use barebone::{Expr, *};
use common::Result;

use std::fmt::Debug;
use syn::*;
#[derive(Debug)]
pub struct RawMacro {
    raw: syn::ExprMacro,
}
impl Ast for RawMacro {
    fn is_raw(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct RawUse {
    raw: syn::ItemUse,
}
impl Ast for RawUse {
    fn is_raw(&self) -> bool {
        true
    }
}
pub struct RustSerde;
impl Serializer for RustSerde {
    fn serialize(&self, node: &Expr) -> Result<String> {
        self.serialize_expr(node).map(|x| x.to_string())
    }
}
impl Deserializer for RustSerde {
    fn deserialize(&self, code: &str) -> Result<Expr> {
        let code: syn::File = parse_str(code)?;
        crate::parser::parse_file(code)
    }
}
