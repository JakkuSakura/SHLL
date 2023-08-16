use crate::context::ScopedContext;
use crate::interpreter::Interpreter;
use crate::passes::OptimizePass;
use crate::tree::*;
use crate::type_system::TypeSystem;
use crate::value::*;
use crate::*;
use common::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct SpecializePass {
    spec_id: AtomicUsize,
    serializer: Rc<dyn Serializer>,
    interpreter: Interpreter,
    type_system: TypeSystem,
}
impl SpecializePass {
    pub fn new(serializer: Rc<dyn Serializer>) -> Self {
        Self {
            spec_id: AtomicUsize::default(),
            interpreter: Interpreter::new(serializer.clone()),
            type_system: TypeSystem::new(serializer.clone()),
            serializer,
        }
    }
    pub fn get_expr_by_ident(&self, ident: Ident, ctx: &ScopedContext) -> Result<Expr> {
        return match ident.as_str() {
            "+" | "-" | "*" | "<" | ">" | "<=" | ">=" | "==" | "!=" => Ok(Expr::Ident(ident)),
            "print" => Ok(Expr::Ident(ident)),
            _ => ctx
                .get_expr(&ident)
                .with_context(|| format!("Could not find {:?} in context", ident)),
        };
    }

    pub fn specialize_expr(&self, expr: Expr, ctx: &ScopedContext) -> Result<Expr> {
        match expr {
            Expr::Any(x) => Ok(Expr::Any(x.clone())),
            Expr::Cond(x) => self.specialize_cond(x, ctx),
            _ => Ok(expr),
        }
    }

    pub fn specialize_import(&self, import: Import, _ctx: &ScopedContext) -> Result<Import> {
        Ok(import)
    }

    pub fn specialize_invoke_details(
        &self,
        invoke: Invoke,
        func: &FunctionValue,
        ctx: &ScopedContext,
    ) -> Result<Invoke> {
        let args: Vec<_> = invoke
            .args
            .iter()
            .map(|x| match x {
                Expr::Ident(v) => ctx
                    .get_expr(v)
                    .with_context(|| format!("Couldn't find {:?} in context", v)),
                Expr::Path(v) => ctx
                    .get_expr(v)
                    .with_context(|| format!("Couldn't find {:?} in context", v)),
                _ => Ok(x.clone()),
            })
            .try_collect()?;
        debug!(
            "Specializing Invoke {} with {} [{}]",
            self.serializer.serialize_function(&func)?,
            self.serializer.serialize_args(&args)?,
            ctx.list_values()
                .into_iter()
                .map(|x| x.to_string())
                .join(", ")
        );
        let name = func.name.as_ref().map(|x| x.name.as_str()).unwrap_or("fun");
        let mut new_params: Vec<FunctionParam> = vec![];
        let mut new_args: Vec<Expr> = vec![];
        for (param, arg) in zip_eq(func.params.iter(), args.iter()) {
            match self.interpreter.interpret_expr(arg, ctx) {
                Err(err) => {
                    warn!("Cannot evaluate arg {} {:?}: {:?}", param.name, arg, err);
                    new_args.push(arg.clone());
                    new_params.push(param.clone());
                }
                Ok(_) => {}
            }
        }
        if !new_params.is_empty() && new_params.len() == func.params.len() {
            warn!(
                "Couldn't specialize Invoke {} with {}",
                self.serializer.serialize_function(&func)?,
                self.serializer.serialize_args(&args)?,
            );
            ctx.print_values(&*self.serializer)?;
            return Ok(invoke);
        }
        let mut bindings = vec![];
        for name in ctx.list_values() {
            let value = ctx.get_value(&name).unwrap();

            if matches!(value, Value::Function(_)) {
                continue;
            }
            let Some(name) = name.try_into_ident() else {
                continue
            };

            let binding = Statement::Let(Let {
                name,
                ty: None,
                value: Expr::value(value),
            });
            bindings.push(binding);
        }

        let new_body = Expr::block(Block::prepend(bindings, *func.body.clone()));
        let new_name = Ident::new(format!(
            "{}_{}",
            name,
            self.spec_id.fetch_add(1, Ordering::Relaxed)
        ));

        let mut ret = func.ret.clone();
        match &ret {
            TypeValue::Expr(expr) => match &**expr {
                TypeExpr::Ident(ident)
                    if func
                        .generics_params
                        .iter()
                        .find(|x| &x.name == ident)
                        .is_some() =>
                {
                    ret = self.type_system.infer_expr(&new_body, &ctx)?.into();
                }
                _ => {}
            },
            _ => {}
        }
        let new_func = FunctionValue {
            name: Some(new_name.clone()),
            params: new_params,
            generics_params: vec![],
            ret,
            body: new_body.into(),
        };
        trace!(
            "Specialized function {} with {} => {}",
            name,
            self.serializer.serialize_args(&args)?,
            self.serializer.serialize_function(&new_func)?
        );

        ctx.root()
            .insert_specialized(new_name.clone().into(), new_func);
        return Ok(Invoke {
            func: Expr::Ident(new_name).into(),
            args: Default::default(),
        });
    }

    pub fn specialize_invoking(
        &self,
        invoke: Invoke,
        func: &FunctionValue,
        ctx: &ScopedContext,
    ) -> Result<Invoke> {
        match &*invoke.func {
            Expr::Ident(ident) if ident.as_str() == "print" => {
                return Ok(invoke);
            }
            _ => {}
        }

        self.specialize_invoke_details(invoke, func, ctx)
    }
    pub fn specialize_module(&self, mut module: Module, ctx: &ScopedContext) -> Result<Module> {
        debug!(
            "Specializing module {}",
            self.serializer.serialize_module(&module)?
        );

        for specialized_name in ctx.list_specialized().into_iter().sorted() {
            let func = ctx.get_function(specialized_name).unwrap();
            let define = Define {
                name: func.name.clone().expect("No specialized name"),
                kind: DefKind::Function,
                ty: Some(TypeValue::Function(
                    self.type_system.infer_function(&func, ctx)?,
                )),
                value: DefValue::Function(func),
                visibility: Visibility::Public,
            };
            module.items.push(Item::Def(define));
        }

        Ok(module)
    }
    pub fn specialize_item(&self, item: Item, ctx: &ScopedContext) -> Result<Option<Item>> {
        match item {
            Item::Def(x) => {
                if let Some(x) = self.specialize_def(x, ctx)? {
                    Ok(Some(Item::Def(x)))
                } else {
                    Ok(None)
                }
            }
            Item::Module(x) => self.specialize_module(x, ctx).map(Item::Module).map(Some),
            _ => Ok(Some(item)),
        }
    }

    pub fn specialize_cond(&self, b: Cond, ctx: &ScopedContext) -> Result<Expr> {
        for case in &b.cases {
            let interpreted =
                Interpreter::new(self.serializer.clone()).interpret_expr(&case.cond, ctx)?;
            match interpreted {
                Value::Bool(b) => {
                    if b.value {
                        return self.specialize_expr(case.body.clone(), ctx);
                    } else {
                        continue;
                    }
                }
                _ => break,
            }
        }
        Ok(Expr::Cond(Cond {
            cases: b
                .cases
                .into_iter()
                .map(|case| {
                    Ok::<_, Error>(CondCase {
                        cond: self.specialize_expr(case.cond, ctx)?,
                        body: self.specialize_expr(case.body, ctx)?,
                    })
                })
                .try_collect()?,
            if_style: b.if_style,
        }))
    }

    pub fn specialize_def(&self, def: Define, _ctx: &ScopedContext) -> Result<Option<Define>> {
        match &def.value {
            DefValue::Function(f) if f.params.is_empty() && f.generics_params.is_empty() => {
                Ok(Some(def))
            }
            DefValue::Function(_) => match def.name.as_str() {
                "print" => Ok(Some(def)),
                "add" => Ok(Some(def)),
                _ => Ok(None),
            },

            _ => Ok(Some(def)),
        }
    }
}
impl OptimizePass for SpecializePass {
    fn name(&self) -> &str {
        "specialize"
    }

    fn optimize_item_post(&self, item: Item, ctx: &ScopedContext) -> Result<Option<Item>> {
        self.specialize_item(item, ctx)
    }
    fn optimize_invoke_post(
        &self,
        invoke: Invoke,
        func: &FunctionValue,
        ctx: &ScopedContext,
    ) -> Result<Invoke> {
        self.specialize_invoking(invoke, &func, ctx)
    }
    // fn optimize_expr(&self, expr: Expr, ctx: &ExecutionContext) -> Result<Expr> {
    //     self.specialize_expr(expr, ctx)
    // }
}
