use common::*;

use lang_core::context::SharedScopedContext;
use lang_optimize::pass::{load_optimizers, FoldOptimizer};
use proc_macro::TokenStream;
use rust_lang::parser::RustParser;
use rust_lang::printer::RustPrinter;

use lang_core::ast::{AstExpr, AstFile, AstModule};
use std::sync::Arc;

trait Optimizee {
    fn optimize(
        self,
        optimizer: Vec<FoldOptimizer>,
        ctx: &SharedScopedContext,
    ) -> Result<TokenStream>;
}
impl Optimizee for AstExpr {
    fn optimize(
        mut self,
        optimizer: Vec<FoldOptimizer>,
        ctx: &SharedScopedContext,
    ) -> Result<TokenStream> {
        for opt in optimizer {
            self = opt.optimize_expr(self, ctx)?;
        }

        let node = RustPrinter::new().print_expr(&self)?;
        Ok(node.into())
    }
}
impl Optimizee for AstModule {
    fn optimize(
        mut self,
        optimizer: Vec<FoldOptimizer>,
        ctx: &SharedScopedContext,
    ) -> Result<TokenStream> {
        for opt in optimizer {
            self = opt.optimize_module(self, ctx, true)?;
        }

        let node = RustPrinter::new().print_module(&self)?;
        Ok(node.into())
    }
}
impl Optimizee for AstFile {
    fn optimize(
        mut self,
        optimizer: Vec<FoldOptimizer>,
        ctx: &SharedScopedContext,
    ) -> Result<TokenStream> {
        for opt in optimizer {
            self = opt.optimize_file(self, ctx)?;
        }
        let node = RustPrinter::new().print_file(&self)?;
        Ok(node.into())
    }
}
fn specialize_inner(code: impl Optimizee) -> Result<TokenStream> {
    let ctx = SharedScopedContext::new();
    let formatter = RustPrinter::new();
    let optimizer = load_optimizers(Arc::new(formatter));
    let node = code.optimize(optimizer, &ctx)?;

    Ok(node.into())
}
#[proc_macro]
pub fn specialize(input: TokenStream) -> TokenStream {
    let input: syn::File = syn::parse(input.into()).unwrap();
    let input = RustParser::new().parse_file("".into(), input).unwrap();
    specialize_inner(input).unwrap().into()
}

#[proc_macro_attribute]
pub fn specialize_module(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let input: syn::ItemMod = syn::parse(input.into()).unwrap();
    let input = RustParser::new().parse_module(input).unwrap();

    specialize_inner(input).unwrap().into()
}
