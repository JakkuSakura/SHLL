pub mod rustfmt;

use barebone::{Block, Ident, *};
use common::{Result, *};
use proc_macro2::TokenStream;
use quote::*;
use syn::*;

pub struct RustSerde;
impl RustSerde {
    fn serialize_ident(&self, i: &Ident) -> syn::Ident {
        format_ident!("{}", i.name)
    }
    fn serialize_block(&self, n: &Block) -> Result<TokenStream> {
        let stmts: Vec<_> = n
            .stmts
            .iter()
            .map(|x| self.serialize_quote(x))
            .try_collect()?;
        let q = if n.last_value {
            quote!({
                #(#stmts);*
            })
        } else {
            quote!({
                #(#stmts;)*
            })
        };
        return Ok(q);
    }
    fn serialize_fun(&self, n: &Fun) -> Result<TokenStream> {
        let name = format_ident!("{}", n.name.as_ref().unwrap().name);
        let ret = self.serialize_quote(&n.ret)?;
        let param_names: Vec<_> = n
            .params
            .params
            .iter()
            .map(|x| self.serialize_ident(&x.name))
            .collect();
        let param_types: Vec<_> = n
            .params
            .params
            .iter()
            .map(|x| self.serialize_quote(&x.ty))
            .try_collect()?;
        let stmts = self.serialize_block(n.body.as_ref().unwrap())?;
        let q = quote!(
            fn #name(#(#param_names: #param_types), *) -> #ret
                #stmts

        );
        return Ok(q);
    }
    fn serialize_apply(&self, node: &Apply) -> Result<TokenStream> {
        let fun = self.serialize_quote(&node.fun)?;
        let args: Vec<_> = node
            .args
            .args
            .iter()
            .map(|x| self.serialize_quote(x))
            .try_collect()?;
        Ok(quote!(
            (#fun)(#(#args), *)
        ))
    }
    fn serialize_literal_int(&self, n: &LiteralInt) -> TokenStream {
        let n = n.value;
        quote!(
            #n
        )
    }
    fn serialize_quote(&self, node: &AstNode) -> Result<TokenStream> {
        if let Some(n) = node.as_ast::<Block>() {
            return self.serialize_block(n);
        }
        if let Some(m) = node.as_ast::<Module>() {
            let stmts: Vec<_> = m
                .stmts
                .iter()
                .map(|x| self.serialize_quote(x))
                .try_collect()?;
            return Ok(quote!(
                #(#stmts)*
            ));
        }
        if let Some(n) = node.as_ast::<Fun>() {
            return self.serialize_fun(n);
        }
        if let Some(n) = node.as_ast::<Ident>() {
            return Ok(self.serialize_ident(n).to_token_stream());
        }

        if let Some(_n) = node.as_ast::<Unit>() {
            return Ok(quote!(()));
        }

        if let Some(n) = node.as_ast::<Apply>() {
            return self.serialize_apply(n);
        }
        if let Some(n) = node.as_ast::<LiteralInt>() {
            return Ok(self.serialize_literal_int(n));
        }
        bail!("Unable to serialize {:?}", node)
    }
}
impl Serializer for RustSerde {
    fn serialize(&self, node: &AstNode) -> Result<String> {
        self.serialize_quote(node).map(|x| x.to_string())
    }
}
impl Deserializer for RustSerde {
    fn deserialize(&self, code: &str) -> Result<AstNode> {
        let code: syn::File = parse_str(code)?;
        parse_file(code)
    }
}
fn parse_type(t: syn::Type) -> Result<AstNode> {
    let t = match t {
        Type::Array(_) => {
            todo!()
        }
        // Type::BareFn(_) => {}
        // Type::Group(_) => {}
        // Type::ImplTrait(_) => {}
        // Type::Infer(_) => {}
        // Type::Macro(_) => {}
        // Type::Never(_) => {}
        // Type::Paren(_) => {}
        // Type::Path(_) => {}
        // Type::Ptr(_) => {}
        // Type::Reference(_) => {}
        // Type::Slice(_) => {}
        // Type::TraitObject(_) => {}
        // Type::Tuple(_) => {}
        // Type::Verbatim(_) => {}
        _ => todo!(),
    };
    Ok(t)
}
fn parse_input(i: FnArg) -> Result<Param> {
    Ok(match i {
        FnArg::Receiver(_) => {
            todo!()
        }
        FnArg::Typed(t) => Param {
            name: parse_pat(*t.pat)?,
            ty: parse_type(*t.ty)?,
        },
    })
}
fn parse_pat(p: syn::Pat) -> Result<Ident> {
    Ok(match p {
        Pat::Ident(name) => Ident {
            name: name.ident.to_string(),
        },
        _ => todo!(),
    })
}
fn parse_fn(f: ItemFn) -> Result<Fun> {
    Ok(Fun {
        name: Some(Ident {
            name: f.sig.ident.to_string(),
        }),
        params: Params {
            params: f
                .sig
                .inputs
                .into_iter()
                .map(|x| parse_input(x))
                .try_collect()?,
        },
        ret: match f.sig.output {
            ReturnType::Default => Unit.into(),
            ReturnType::Type(_, t) => parse_type(*t)?,
        },
        body: Some(parse_block(*f.block)?),
    })
}
fn parse_call(call: syn::ExprCall) -> Result<Apply> {
    Ok(Apply {
        fun: parse_expr(*call.func)?,
        args: PosArgs {
            args: call.args.into_iter().map(parse_expr).try_collect()?,
        },
    })
}
fn parse_expr(expr: syn::Expr) -> Result<AstNode> {
    Ok(match expr {
        // Expr::Array(_) => {}
        // Expr::Assign(_) => {}
        // Expr::AssignOp(_) => {}
        // Expr::Async(_) => {}
        // Expr::Await(_) => {}
        // Expr::Binary(_) => {}
        // Expr::Block(_) => {}
        // Expr::Box(_) => {}
        // Expr::Break(_) => {}
        Expr::Call(c) => parse_call(c)?.into(),
        // Expr::Cast(_) => {}
        // Expr::Closure(_) => {}
        // Expr::Continue(_) => {}
        // Expr::Field(_) => {}
        // Expr::ForLoop(_) => {}
        // Expr::Group(_) => {}
        // Expr::If(_) => {}
        // Expr::Index(_) => {}
        // Expr::Let(_) => {}
        Expr::Lit(l) => match l.lit {
            Lit::Int(i) => LiteralInt::new(i.base10_parse()?).into(),
            _ => todo!(),
        },
        // Expr::Loop(_) => {}
        // Expr::Macro(_) => {}
        // Expr::Match(_) => {}
        // Expr::MethodCall(_) => {}
        // Expr::Paren(_) => {}
        Expr::Path(p) => Ident::new(p.path.segments.first().unwrap().ident.to_string()).into(),
        // Expr::Range(_) => {}
        // Expr::Reference(_) => {}
        // Expr::Repeat(_) => {}
        // Expr::Return(_) => {}
        // Expr::Struct(_) => {}
        // Expr::Try(_) => {}
        // Expr::TryBlock(_) => {}
        // Expr::Tuple(_) => {}
        // Expr::Type(_) => {}
        // Expr::Unary(_) => {}
        // Expr::Unsafe(_) => {}
        // Expr::Verbatim(_) => {}
        // Expr::While(_) => {}
        // Expr::Yield(_) => {}
        x => todo!("{:?}", x.to_token_stream()),
    })
}
fn parse_stmt(stmt: syn::Stmt) -> Result<AstNode> {
    Ok(match stmt {
        Stmt::Local(l) => Def {
            name: parse_pat(l.pat)?,
            ty: None,
            value: parse_expr(*l.init.context("No value")?.1)?,
        }
        .into(),
        Stmt::Item(_) => {
            todo!()
        }
        Stmt::Expr(e) => parse_expr(e)?,
        Stmt::Semi(e, _) => parse_expr(e)?,
    })
}
fn parse_block(block: syn::Block) -> Result<Block> {
    let last_value = block
        .stmts
        .last()
        .map(|x| match x {
            Stmt::Semi(..) => false,
            _ => true,
        })
        .unwrap_or_default();
    Ok(Block {
        stmts: block.stmts.into_iter().map(parse_stmt).try_collect()?,
        last_value,
    })
}
fn parse_item(item: syn::Item) -> Result<AstNode> {
    match item {
        Item::Fn(f) => parse_fn(f).map(|x| x.into()),
        _ => todo!(),
    }
}
fn parse_file(file: syn::File) -> Result<AstNode> {
    Ok(Module {
        stmts: file.items.into_iter().map(parse_item).try_collect()?,
    }
    .into())
}