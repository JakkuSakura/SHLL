use crate::interpreter::InterpreterContext;
use crate::{Block, Call, Def, Expr, FuncDecl, Generics, Ident, Module, Params, PosArgs, Unit};
use common::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Specializer {
    spec_id: AtomicUsize,
}
impl Specializer {
    pub fn new() -> Self {
        Self {
            spec_id: AtomicUsize::default(),
        }
    }

    pub fn specialize_expr(&self, expr: Expr, ctx: &InterpreterContext) -> Result<Expr> {
        debug!("Specializing {:?}", expr);
        if let Some(n) = expr.as_ast::<Block>() {
            return self.specialize_block(n.clone(), ctx).map(|x| x.into());
        }
        if let Some(n) = expr.as_ast::<Module>() {
            return self.specialize_module(n.clone(), ctx).map(|x| x.into());
        }
        if expr.is_literal() {
            return Ok(expr);
        }
        if expr.is_raw() {
            return Ok(expr);
        }
        if let Some(d) = expr.as_ast::<Def>() {
            return self.specialize_def(d.clone(), ctx).map(|x| x.into());
        }
        if let Some(c) = expr.as_ast::<Call>() {
            return self.specialize_call(c.clone(), ctx);
        }
        if let Some(n) = expr.as_ast::<Ident>() {
            return match n.name.as_str() {
                "+" | "-" | "*" => Ok(expr),
                "print" => Ok(expr),
                _ => ctx
                    .get(n)
                    .with_context(|| format!("Could not find {:?} in context", n.name)),
            };
        }
        bail!("Could not specialize {:?}", expr)
        // Ok(expr)
    }
    pub fn specialize_call(&self, node: Call, ctx: &InterpreterContext) -> Result<Expr> {
        let mut fun = self.specialize_expr(node.fun.clone(), ctx)?;
        if let Some(g) = fun.as_ast::<Generics>() {
            fun = g.value.clone();
        }
        let args: Vec<_> = node
            .args
            .args
            .iter()
            .cloned()
            .map(|x| self.specialize_expr(x, ctx))
            .try_collect()?;
        if let Some(f) = fun.as_ast::<FuncDecl>() {
            let name = f.name.as_ref().map(|x| x.name.as_str()).unwrap_or("<fun>");
            debug!("Invoking {} with {:?}", name, args);
            let sub = ctx.child();
            for (i, arg) in args.iter().cloned().enumerate() {
                let param = f
                    .params
                    .params
                    .get(i)
                    .with_context(|| format!("Couldn't find {} parameter of {:?}", i, f))?;
                // TODO: type check here
                sub.insert(param.name.clone(), arg);
            }
            let new_body =
                self.specialize_block(f.body.clone().context("Funtion body is empty")?, &sub)?;
            debug!("Specialied {} with {:?} => {:?}", name, args, new_body);
            let new_name = Ident::new(format!(
                "{}_{}",
                name,
                self.spec_id.fetch_add(1, Ordering::Relaxed)
            ));
            ctx.root().insert_specialized(
                new_name.clone(),
                FuncDecl {
                    name: Some(new_name.clone()),
                    params: Params::default(),
                    ret: f.ret.clone(),
                    body: Some(new_body),
                }
                .into(),
            );
            return Ok(Call {
                fun: new_name.into(),
                args: PosArgs::default(),
            }
            .into());
        }
        if let Some(id) = fun.as_ast::<Ident>() {
            return Ok(Call {
                fun: id.clone().into(),
                args: PosArgs { args },
            }
            .into());
        }
        bail!("Failed to specialize {:?}", node)
    }
    pub fn specialize_module(&self, m: Module, ctx: &InterpreterContext) -> Result<Module> {
        let mut stmts: Vec<_> = m
            .stmts
            .into_iter()
            .map(|x| self.specialize_expr(x, ctx))
            .try_collect()?;
        let specialized: Vec<_> = ctx
            .list_specialized()
            .into_iter()
            .map(|x| {
                ctx.get(&x)
                    .map(|x| {
                        Def {
                            name: x.inner.as_ast::<FuncDecl>().unwrap().name.clone().unwrap(),
                            ty: None,
                            value: x,
                        }
                        .into()
                    })
                    .context("impossible")
            })
            .try_collect()?;
        stmts.extend(specialized);
        Ok(Module {
            stmts: stmts
                .into_iter()
                .filter(|x| x.as_ast::<Unit>().is_none())
                .collect(),
        })
    }
    pub fn specialize_block(&self, b: Block, ctx: &InterpreterContext) -> Result<Block> {
        Ok(Block {
            stmts: b
                .stmts
                .into_iter()
                .map(|x| self.specialize_expr(x, ctx))
                .try_collect()?,
            last_value: b.last_value,
        })
    }
    pub fn specialize_def(&self, d: Def, ctx: &InterpreterContext) -> Result<Expr> {
        let fun;
        if let Some(g) = d.value.as_ast::<Generics>() {
            fun = g.value.clone();
        } else {
            fun = d.value.clone();
        }

        if let Some(f) = fun.as_ast::<FuncDecl>().cloned() {
            match f.name.as_ref().map(|x| x.name.as_str()).unwrap_or("") {
                "main" => {
                    return Ok(Def {
                        name: d.name,
                        ty: None,
                        value: FuncDecl {
                            name: f.name,
                            params: f.params,
                            ret: f.ret,
                            body: Some(self.specialize_block(f.body.context("empty main")?, ctx)?),
                        }
                        .into(),
                    }
                    .into());
                }

                "print" => {
                    ctx.insert(d.name.clone(), d.value.clone());
                    return Ok(d.into());
                }
                _ => ctx.insert(d.name.clone(), d.value.clone()),
            }
        }
        Ok(Unit.into())
    }
}
