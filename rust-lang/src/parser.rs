use crate::{RawExpr, RawExprMacro, RawImplTrait, RawItemMacro, RawType, RawUse};
use common::*;
use common_lang::ast::*;
use quote::ToTokens;
use syn::{
    BinOp, FnArg, GenericParam, Item, ItemFn, Lit, Member, Pat, ReturnType, Stmt, TypeParamBound,
};
fn parse_ident(i: syn::Ident) -> Ident {
    Ident::new(i.to_string())
}
fn parse_type(t: syn::Type) -> Result<Expr> {
    let t = match t {
        syn::Type::BareFn(f) => Types::func(
            f.inputs
                .into_iter()
                .map(|x| x.ty)
                .map(parse_type)
                .try_collect()?,
            parse_return_type(f.output)?,
        )
        .into(),
        syn::Type::Path(p) => {
            let s = p.path.to_token_stream().to_string();

            match s.as_str() {
                "i64" | "i32" | "u64" | "u32" | "f64" | "f32" => Ident::new(s).into(),
                _ if p.path.segments.len() == 1 => {
                    let first_segment = p.path.segments.first().unwrap();
                    match &first_segment.arguments {
                        syn::PathArguments::None => {
                            let ident = parse_ident(first_segment.ident.clone());
                            ident.into()
                        }
                        _ => RawType { raw: p }.into(),
                    }
                }
                _ => bail!("Type not supported: {}", s),
            }
        }
        syn::Type::ImplTrait(im) => RawImplTrait { raw: im }.into(),
        syn::Type::Tuple(t) if t.elems.is_empty() => Types::Unit.into(),
        t => bail!("Type not supported {:?}", t),
    };
    Ok(t)
}

fn parse_input(i: FnArg) -> Result<Param> {
    Ok(match i {
        FnArg::Receiver(rev) => Param {
            name: Ident::new("self"),
            ty: match (rev.reference.is_some(), rev.mutability.is_some()) {
                (true, true) => Ident::new("&mut Self").into(),
                (true, false) => Ident::new("&Self").into(),
                (false, true) => Ident::new("Self").into(),
                (false, false) => Ident::new("mut Self").into(),
            },
        },

        FnArg::Typed(t) => Param {
            name: parse_pat(*t.pat)?,
            ty: parse_type(*t.ty)?,
        },
    })
}

fn parse_pat(p: syn::Pat) -> Result<Ident> {
    Ok(match p {
        Pat::Ident(name) => parse_ident(name.ident),
        Pat::Wild(_) => Ident::new("_"),
        _ => bail!("Pattern not supported {:?}", p),
    })
}

fn parse_return_type(o: ReturnType) -> Result<Expr> {
    Ok(match o {
        ReturnType::Default => LiteralUnit.into(),
        ReturnType::Type(_, t) => parse_type(*t)?,
    })
}
fn parse_type_param_bound(b: TypeParamBound) -> Result<Expr> {
    match b {
        TypeParamBound::Trait(t) => parse_path(t.path),
        _ => bail!("Does not support liftimes {:?}", b),
    }
}

fn parse_path(p: syn::Path) -> Result<Expr> {
    let first = p.segments.first().unwrap();
    ensure!(
        first.arguments.is_none(),
        "Path argument not supported {:?}",
        p
    );
    let mut ret: Expr = parse_ident(first.ident.clone()).into();
    for s in p.segments.iter().skip(1) {
        ret = Select {
            obj: ret,
            field: parse_ident(s.ident.clone()),
            select: SelectType::Const,
        }
        .into();
        ensure!(s.arguments.is_none(), "Path argument not supported {:?}", p);
    }
    Ok(ret)
}
fn parse_fn(f: ItemFn) -> Result<(Ident, Expr)> {
    let name = parse_ident(f.sig.ident);
    let ff = FuncDecl {
        name: Some(name.clone()),
        params: Params {
            params: f
                .sig
                .inputs
                .into_iter()
                .map(|x| parse_input(x))
                .try_collect()?,
        },
        ret: parse_return_type(f.sig.output)?,
        body: Some(parse_block(*f.block)?),
    };
    if f.sig.generics.params.is_empty() {
        Ok((name, ff.into()))
    } else {
        let params = f
            .sig
            .generics
            .params
            .into_iter()
            .map(|x| match x {
                GenericParam::Type(t) => Ok(Param {
                    name: parse_ident(t.ident),
                    // TODO: support multiple type bounds
                    ty: parse_type_param_bound(t.bounds.first().cloned().unwrap())?,
                }),
                _ => bail!("Does not generic param {:?}", x),
            })
            .try_collect()?;
        Ok((
            name,
            Generics {
                params: Params { params },
                value: ff.into(),
            }
            .into(),
        ))
    }
}

fn parse_call(call: syn::ExprCall) -> Result<Call> {
    let fun = parse_expr(*call.func)?;
    let args: Vec<_> = call.args.into_iter().map(parse_expr).try_collect()?;

    Ok(Call { fun, args })
}
fn parse_method_call(call: syn::ExprMethodCall) -> Result<Call> {
    Ok(Call {
        fun: Select {
            obj: parse_expr(*call.receiver)?,
            field: parse_ident(call.method),
            select: SelectType::Method,
        }
        .into(),
        args: call.args.into_iter().map(parse_expr).try_collect()?,
    })
}

fn parse_if(i: syn::ExprIf) -> Result<Cond> {
    let mut cases = vec![CondCase {
        cond: parse_expr(*i.cond)?,
        body: parse_block(i.then_branch)?.into(),
    }];
    if let Some((_, else_body)) = i.else_branch {
        'else_check: {
            let body = parse_expr(*else_body)?;
            if let Some(m) = body.as_ast::<Cond>() {
                if m.if_style {
                    cases.extend(m.cases.clone());
                    break 'else_check;
                }
            }
            cases.push(CondCase {
                cond: LiteralBool { value: true }.into(),
                body,
            });
        };
    }

    Ok(Cond {
        cases,
        if_style: true,
    })
}
fn parse_binary(b: syn::ExprBinary) -> Result<Call> {
    let l = parse_expr(*b.left)?;
    let r = parse_expr(*b.right)?;
    let (op, flatten) = match b.op {
        BinOp::Add(_) => (Ident::new("+"), true),
        BinOp::Mul(_) => (Ident::new("*"), true),
        BinOp::Sub(_) => (Ident::new("-"), false),
        BinOp::Gt(_) => (Ident::new(">"), false),
        BinOp::Ge(_) => (Ident::new(">="), false),
        BinOp::Le(_) => (Ident::new("<="), false),
        BinOp::Lt(_) => (Ident::new("<"), false),
        BinOp::Eq(_) => (Ident::new("=="), false),
        BinOp::Ne(_) => (Ident::new("!="), false),
        BinOp::BitOr(_) => (Ident::new("|"), true),
        _ => bail!("Op not supported {:?}", b.op),
    };
    if flatten {
        if let Some(first_arg) = l.as_ast::<Call>() {
            if Some(&op) == first_arg.fun.as_ast::<Ident>() {
                let mut first_arg = first_arg.clone();
                first_arg.args.push(r);
                return Ok(first_arg);
            }
        }
    }
    Ok(Call {
        fun: op.into(),
        args: vec![l, r],
    })
}
fn parse_tuple(t: syn::ExprTuple) -> Result<Call> {
    let args = t.elems.into_iter().map(parse_expr).try_collect()?;
    Ok(Call {
        fun: Ident::new("tuple").into(),
        args,
    })
}
fn parse_member(mem: Member) -> Result<Ident> {
    Ok(match mem {
        syn::Member::Named(n) => parse_ident(n),
        syn::Member::Unnamed(_) => bail!("Does not support unnmaed field yet {:?}", mem),
    })
}
fn parse_field_value(fv: syn::FieldValue) -> Result<FieldValue> {
    Ok(FieldValue {
        name: parse_member(fv.member)?,
        value: parse_expr(fv.expr)?,
    })
}
pub fn parse_struct_expr(s: syn::ExprStruct) -> Result<BuildStruct> {
    Ok(BuildStruct {
        name: parse_path(s.path)?.into(),
        fields: s
            .fields
            .into_iter()
            .map(|x| parse_field_value(x))
            .try_collect()?,
    })
}
pub fn parse_expr(expr: syn::Expr) -> Result<Expr> {
    Ok(match expr {
        syn::Expr::Binary(b) => parse_binary(b)?.into(),
        syn::Expr::Block(b) if b.label.is_none() => parse_block(b.block)?.into(),

        syn::Expr::Call(c) => parse_call(c)?.into(),
        syn::Expr::If(i) => parse_if(i)?.into(),

        syn::Expr::Lit(l) => match l.lit {
            Lit::Int(i) => LiteralInt::new(i.base10_parse()?).into(),
            Lit::Float(i) => LiteralDecimal::new(i.base10_parse()?).into(),
            Lit::Str(s) => LiteralString::new_ref(s.value()).into(),
            _ => bail!("Lit not supported: {:?}", l.lit.to_token_stream()),
        },
        syn::Expr::Macro(m) => RawExprMacro { raw: m }.into(),
        syn::Expr::MethodCall(c) => parse_method_call(c)?.into(),
        syn::Expr::Path(p) => parse_path(p.path)?.into(),
        syn::Expr::Reference(r) => parse_ref(r)?.into(),
        syn::Expr::Tuple(t) => parse_tuple(t)?.into(),
        syn::Expr::Struct(s) => parse_struct_expr(s)?.into(),

        raw => {
            warn!("RawExpr {:?}", raw);
            RawExpr { raw }.into()
        } // x => bail!("Expr not supported: {:?}", x),
    })
}

fn parse_stmt(stmt: syn::Stmt) -> Result<Expr> {
    Ok(match stmt {
        Stmt::Local(l) => Def {
            name: parse_pat(l.pat)?,
            kind: DefKind::Variable,
            ty: None,
            value: parse_expr(*l.init.context("No value")?.expr)?,
            visibility: Visibility::Public,
        }
        .into(),
        Stmt::Item(tm) => parse_item(tm)?,
        Stmt::Expr(e, _) => parse_expr(e)?,
        Stmt::Macro(m) => bail!("Macro not supported: {:?}", m),
    })
}

fn parse_block(block: syn::Block) -> Result<Block> {
    info!("Parsing block {:?}", block);
    let last_value = block
        .stmts
        .last()
        .map(|x| match x {
            Stmt::Expr(_, s) => s.is_none(),
            _ => false,
        })
        .unwrap_or_default();
    Ok(Block {
        stmts: block.stmts.into_iter().map(parse_stmt).try_collect()?,
        last_value,
    })
}
fn parse_vis(v: syn::Visibility) -> Visibility {
    match v {
        syn::Visibility::Public(_) => Visibility::Public,
        syn::Visibility::Restricted(_) => Visibility::Public,
        syn::Visibility::Inherited => Visibility::Private,
    }
}
fn parse_impl_item(item: syn::ImplItem) -> Result<Def> {
    match item {
        syn::ImplItem::Fn(m) => {
            // TODO: defaultness
            let (name, expr) = parse_fn(ItemFn {
                attrs: m.attrs,
                vis: m.vis.clone(),
                sig: m.sig,
                block: Box::new(m.block),
            })?;
            Ok(Def {
                name,
                kind: DefKind::Function,
                ty: None,
                value: expr,
                visibility: parse_vis(m.vis),
            })
        }
        syn::ImplItem::Type(t) => Ok(Def {
            name: parse_ident(t.ident),
            kind: DefKind::Type,
            ty: None,
            value: parse_type(t.ty)?,
            visibility: parse_vis(t.vis),
        }),
        _ => bail!("Does not support impl item {:?}", item),
    }
}
fn parse_impl(im: syn::ItemImpl) -> Result<Impl> {
    Ok(Impl {
        name: parse_type(*im.self_ty)?,
        defs: im.items.into_iter().map(parse_impl_item).try_collect()?,
    })
}
fn parse_struct_field(i: usize, f: syn::Field) -> Result<Field> {
    Ok(Field {
        name: f
            .ident
            .map(parse_ident)
            .unwrap_or(Ident::new(format!("{}", i))),
        ty: parse_type(f.ty)?,
    })
}
fn parse_use(u: syn::ItemUse) -> Result<Expr> {
    let mut segments = vec![];
    let mut tree = u.tree.clone();
    loop {
        match tree {
            syn::UseTree::Path(p) => {
                segments.push(parse_ident(p.ident));
                tree = *p.tree;
            }
            syn::UseTree::Name(name) => {
                segments.push(parse_ident(name.ident));
                break;
            }
            syn::UseTree::Glob(_) => {
                segments.push(Ident::new("*"));
                break;
            }
            _ => return Ok(RawUse { raw: u }.into()),
        }
    }
    Ok(Import {
        visibility: parse_vis(u.vis),
        segments,
    }
    .into())
}
pub fn parse_struct(s: syn::ItemStruct) -> Result<Struct> {
    Ok(Struct {
        name: parse_ident(s.ident),
        fields: s
            .fields
            .into_iter()
            .enumerate()
            .map(|(i, f)| parse_struct_field(i, f))
            .try_collect()?,
    })
}
fn parse_item(item: syn::Item) -> Result<Expr> {
    match item {
        Item::Fn(f0) => {
            let visibility = parse_vis(f0.vis.clone());
            let (name, f) = parse_fn(f0)?;
            let d = Def {
                name,
                kind: DefKind::Function,
                ty: None,
                value: f.into(),
                visibility,
            };
            Ok(d.into())
        }
        Item::Impl(im) => Ok(parse_impl(im)?.into()),
        Item::Use(u) => parse_use(u),
        Item::Macro(m) => Ok(RawItemMacro { raw: m }.into()),
        Item::Struct(s) => Ok(parse_struct(s)?.into()),
        _ => bail!("Does not support item {:?} yet", item),
    }
}
fn parse_ref(item: syn::ExprReference) -> Result<Reference> {
    Ok(Reference {
        referee: parse_expr(*item.expr)?,
        mutable: Some(item.mutability.is_some()),
    })
}

pub fn parse_file(file: syn::File) -> Result<Module> {
    Ok(Module {
        name: Ident::new("__file__"),
        items: file
            .items
            .into_iter()
            .map(parse_item)
            .filter(|x| {
                x.as_ref()
                    .map(|x| x.as_ast::<LiteralUnit>().is_none())
                    .unwrap_or(true)
            })
            .try_collect()?,
    })
}
pub fn parse_module(file: syn::ItemMod) -> Result<Expr> {
    Ok(Module {
        name: parse_ident(file.ident),
        items: file
            .content
            .unwrap()
            .1
            .into_iter()
            .map(parse_item)
            .filter(|x| {
                x.as_ref()
                    .map(|x| x.as_ast::<LiteralUnit>().is_none())
                    .unwrap_or(true)
            })
            .try_collect()?,
    }
    .into())
}
pub struct RustParser;

impl RustParser {
    pub fn parse_expr(&self, code: syn::Expr) -> Result<Expr> {
        parse_expr(code)
    }
    pub fn parse_file(&self, code: syn::File) -> Result<Module> {
        parse_file(code)
    }
    pub fn parse_module(&self, code: syn::ItemMod) -> Result<Expr> {
        parse_module(code)
    }
}
