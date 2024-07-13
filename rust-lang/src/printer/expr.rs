use crate::printer::RustPrinter;
use eyre::bail;
use itertools::Itertools;
use lang_core::ast::{
    AstExpr, BlockStmt, ExprAssign, ExprBinOp, ExprBlock, ExprIf, ExprIndex, ExprInvoke,
    ExprInvokeTarget, ExprLoop, ExprMatch, ExprParen, ExprRange, ExprRangeLimit, ExprReference,
    ExprSelect, ExprSelectType, StmtLet,
};
use lang_core::ops::BinOpKind;
use proc_macro2::TokenStream;
use quote::quote;
use syn::LitInt;

impl RustPrinter {
    pub fn print_expr_optimized(&self, node: &AstExpr) -> eyre::Result<TokenStream> {
        match node {
            AstExpr::Block(n) => self.print_statement_chunk(&n.stmts),
            AstExpr::Value(v) if v.is_unit() => Ok(quote!()),
            _ => self.print_expr(node),
        }
    }
    pub fn print_expr_id(&self, id: u64) -> eyre::Result<TokenStream> {
        let num = LitInt::new(&id.to_string(), proc_macro2::Span::call_site());
        Ok(quote!(Expr # #num))
    }
    pub fn print_expr(&self, node: &AstExpr) -> eyre::Result<TokenStream> {
        match node {
            AstExpr::Id(id) => self.print_expr_id(*id),
            AstExpr::Locator(loc) => self.print_locator(loc),
            AstExpr::Value(n) => self.print_value(n),
            AstExpr::Invoke(n) => self.print_invoke_expr(n),
            AstExpr::BinOp(op) => self.print_bin_op(op),
            AstExpr::Any(n) => self.print_any(n),
            AstExpr::Match(n) => self.print_match(n),
            AstExpr::If(n) => self.print_if(n),
            AstExpr::Block(n) => self.print_block(n),
            AstExpr::InitStruct(n) => self.print_struct_expr(n),
            AstExpr::Select(n) => self.print_select(n),
            AstExpr::Reference(n) => self.print_ref(n),
            AstExpr::Assign(n) => self.print_assign(n),
            AstExpr::Index(n) => self.print_index(n),
            AstExpr::Closured(n) => self.print_expr(&n.expr.get()),
            AstExpr::Paren(n) => self.print_paren(n),
            AstExpr::Loop(n) => self.print_loop(n),
            AstExpr::Range(n) => self.print_range(n),
            _ => bail!("Unable to serialize {:?}", node),
        }
    }

    pub fn print_bin_op(&self, binop: &ExprBinOp) -> eyre::Result<TokenStream> {
        let lhs = self.print_expr(&binop.lhs.get())?;
        let rhs = self.print_expr(&binop.rhs.get())?;
        let op = self.print_bin_op_kind(&binop.kind);
        Ok(quote!(#lhs #op #rhs))
    }

    pub fn print_invoke_expr(&self, invoke: &ExprInvoke) -> eyre::Result<TokenStream> {
        let fun = self.print_invoke_target(&invoke.target)?;
        let args: Vec<_> = invoke
            .args
            .iter()
            .map(|x| self.print_expr(&x.get()))
            .try_collect()?;
        Ok(quote!(
            #fun(#(#args), *)
        ))
    }

    pub fn print_let(&self, let_: &StmtLet) -> eyre::Result<TokenStream> {
        let pat = self.print_pattern(&let_.pat)?;

        let value = self.print_expr(&let_.value)?;

        Ok(quote!(
            let #pat = #value;
        ))
    }
    pub fn print_assign(&self, assign: &ExprAssign) -> eyre::Result<TokenStream> {
        let target = self.print_expr(&assign.target)?;
        let value = self.print_expr(&assign.value)?;
        Ok(quote!(
            #target = #value;
        ))
    }
    pub fn print_index(&self, index: &ExprIndex) -> eyre::Result<TokenStream> {
        let expr = self.print_expr(&index.expr.get())?;
        let index = self.print_expr(&index.index.get())?;
        Ok(quote!(
            #expr[#index]
        ))
    }
    pub fn print_paren(&self, paren: &ExprParen) -> eyre::Result<TokenStream> {
        let expr = self.print_expr(&paren.expr.get())?;
        Ok(quote!(
            (#expr)
        ))
    }

    pub fn print_loop(&self, loop_: &ExprLoop) -> eyre::Result<TokenStream> {
        let body = self.print_expr_optimized(&loop_.body)?;
        Ok(quote!(
            loop {
                #body
            }
        ))
    }
    pub fn print_statement(&self, stmt: &BlockStmt) -> eyre::Result<TokenStream> {
        match stmt {
            BlockStmt::Item(item) => self.print_item(item),
            BlockStmt::Let(let_) => self.print_let(let_),
            BlockStmt::Expr(expr) => self.print_expr(expr),
            BlockStmt::Any(any) => self.print_any(any),
        }
    }
    pub fn print_statement_chunk(&self, items: &[BlockStmt]) -> eyre::Result<TokenStream> {
        let mut stmts = vec![];
        for item in items {
            let item = self.print_statement(item)?;
            stmts.push(item);
        }
        Ok(quote!(#(#stmts) *))
    }
    pub fn print_block(&self, n: &ExprBlock) -> eyre::Result<TokenStream> {
        let chunk = self.print_statement_chunk(&n.stmts)?;
        Ok(quote!({
            #chunk
        }))
    }
    pub fn print_if(&self, if_: &ExprIf) -> eyre::Result<TokenStream> {
        let cond = self.print_expr(&if_.cond)?;
        let then = self.print_expr_optimized(&if_.then)?;
        let elze = if let Some(elze) = &if_.elze {
            let elze = self.print_expr(elze)?;
            quote!(else #elze)
        } else {
            quote!()
        };
        Ok(quote!(
            if #cond {
                #then
            }
            #elze
        ))
    }

    pub fn print_match(&self, m: &ExprMatch) -> eyre::Result<TokenStream> {
        let mut ts = vec![];
        for (_i, c) in m.cases.iter().enumerate() {
            let node = &c.cond;
            let co = self.print_expr(node)?;
            let node = &c.body;
            let ex = self.print_expr_optimized(node)?;
            ts.push(quote!(
                if #co => { #ex }
            ))
        }
        Ok(quote!(match () {
            () #(#ts)*
            _ => {}
        }))
    }

    pub fn print_invoke(&self, node: &ExprInvoke) -> eyre::Result<TokenStream> {
        let fun = self.print_invoke_target(&node.target)?;
        let args: Vec<_> = node
            .args
            .iter()
            .map(|x| self.print_expr(&x.get()))
            .try_collect()?;
        match &node.target {
            ExprInvokeTarget::Function(_) => {
                return Ok(quote!(
                    #fun(#(#args), *)
                ));
            }
            ExprInvokeTarget::Type(_) => {
                return Ok(quote!(
                    <#fun>::<#(#args), *>
                ));
            }
            ExprInvokeTarget::BinOp(op) => {
                let ret = match op {
                    BinOpKind::Add => quote!(#(#args) + *),
                    BinOpKind::AddTrait => quote!(#(#args) + *),
                    BinOpKind::Sub => quote!(#(#args) - *),
                    BinOpKind::Div => quote!(#(#args) / *),
                    BinOpKind::Mul => {
                        let mut result = vec![];
                        for (i, a) in args.into_iter().enumerate() {
                            if i != 0 {
                                result.push(quote!(*));
                            }
                            result.push(a);
                        }
                        quote!(#(#result)*)
                    }
                    BinOpKind::Mod => quote!(#(#args) % *),
                    BinOpKind::Gt => quote!(#(#args) > *),
                    BinOpKind::Lt => quote!(#(#args) < *),
                    BinOpKind::Ge => quote!(#(#args) >= *),
                    BinOpKind::Le => quote!(#(#args) <= *),
                    BinOpKind::Eq => quote!(#(#args) == *),
                    BinOpKind::Ne => quote!(#(#args) != *),
                    BinOpKind::Or => quote!(#(#args) || *),
                    BinOpKind::And => quote!(#(#args) && *),
                    BinOpKind::BitOr => quote!(#(#args) | *),
                    BinOpKind::BitAnd => quote!(#(#args) & *),
                    BinOpKind::BitXor => quote!(#(#args) ^ *),
                };
                return Ok(ret);
            }

            ExprInvokeTarget::Method(select) => match select.select {
                ExprSelectType::Field => {
                    return Ok(quote!(
                        (#fun)(#(#args), *)
                    ));
                }
                _ => {}
            },
            _ => {}
        }

        let fun_str = fun.to_string();

        // TODO: deprecate it
        let code = match fun_str.as_str() {
            "tuple" => quote!(
                (#(#args), *)
            ),
            _ => quote!(
                #fun(#(#args), *)
            ),
        };
        // if true {
        //     return Ok(quote!((#code)));
        // }
        Ok(code)
    }

    pub fn print_ref(&self, n: &ExprReference) -> eyre::Result<TokenStream> {
        let referee = self.print_expr(&n.referee.get())?;
        if n.mutable == Some(true) {
            Ok(quote!(&mut #referee))
        } else {
            Ok(quote!(&#referee))
        }
    }

    pub fn print_select(&self, select: &ExprSelect) -> eyre::Result<TokenStream> {
        let obj = self.print_expr(&select.obj.get())?;
        let field = self.print_ident(&select.field);
        match select.select {
            ExprSelectType::Const => Ok(quote!(
                #obj::#field
            )),
            _ => Ok(quote!(
                #obj.#field
            )),
        }
    }
    pub fn print_args(&self, node: &Vec<AstExpr>) -> eyre::Result<TokenStream> {
        let args: Vec<_> = node.iter().map(|x| self.print_expr(x)).try_collect()?;
        Ok(quote!((#(#args),*)))
    }
    pub fn print_range(&self, range: &ExprRange) -> eyre::Result<TokenStream> {
        let start = range
            .start
            .as_ref()
            .map(|x| self.print_expr(x))
            .transpose()?;
        let end = range.end.as_ref().map(|x| self.print_expr(x)).transpose()?;
        let dots = match range.limit {
            ExprRangeLimit::Inclusive => quote!(..=),
            ExprRangeLimit::Exclusive => quote!(..),
        };
        Ok(quote!(#start #dots #end))
    }
}