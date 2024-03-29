use common::{bail, ensure, ContextCompat, Error, Result};
use itertools::Itertools;

use lang_core::ast::{
    DecimalType, ExprInvokeTarget, FieldTypeValue, ImplTraits, Type, TypeBounds, TypeFunction,
    TypeInt, TypePrimitive, TypeStruct, TypeStructural, TypeType, Value, ValueFunction,
};
use lang_core::ast::{Expr, Visibility};
use lang_core::context::SharedScopedContext;
use lang_core::ctx::{Context, TypeSystem};
use lang_core::id::{Ident, Locator};
use lang_core::utils::conv::TryConv;

use crate::pass::{FoldOptimizer, InterpreterPass};

impl InterpreterPass {
    pub fn type_check_value(&self, lit: &Value, ty: &Type) -> Result<()> {
        match lit {
            Value::Int(_) => {
                ensure!(
                    matches!(ty, Type::Primitive(TypePrimitive::Int(_))),
                    "Expected i64, got {:?}",
                    lit
                )
            }
            Value::Bool(_) => {
                ensure!(
                    matches!(ty, Type::Primitive(TypePrimitive::Bool)),
                    "Expected bool, got {:?}",
                    lit
                )
            }
            Value::Decimal(_) => {
                ensure!(
                    matches!(ty, Type::Primitive(TypePrimitive::Decimal(_))),
                    "Expected f64, got {:?}",
                    lit
                )
            }
            Value::Char(_) => {
                ensure!(
                    matches!(ty, Type::Primitive(TypePrimitive::Char)),
                    "Expected char, got {:?}",
                    lit
                )
            }
            Value::String(_) => {
                ensure!(
                    matches!(ty, Type::Primitive(TypePrimitive::String)),
                    "Expected string, got {:?}",
                    lit
                )
            }
            Value::List(_) => {
                ensure!(
                    matches!(ty, Type::Primitive(TypePrimitive::List)),
                    "Expected list, got {:?}",
                    lit
                )
            }
            Value::Unit(_) => {
                ensure!(matches!(ty, Type::Unit(_)), "Expected unit, got {:?}", lit)
            }
            Value::Type(_) => {
                ensure!(matches!(ty, Type::Type(_)), "Expected type, got {:?}", lit)
            }
            _ => {}
        }
        Ok(())
    }
    pub fn type_check_expr_against_value(
        &self,
        expr: &Expr,
        type_value: &Type,
        ctx: &SharedScopedContext,
    ) -> Result<()> {
        match expr {
            Expr::Locator(n) => {
                let expr = ctx
                    .get_expr(n.to_path())
                    .with_context(|| format!("Could not find {:?} in context", n))?;
                return self.type_check_expr_against_value(&expr, type_value, ctx);
            }

            Expr::Value(v) => return self.type_check_value(v, type_value),
            _ => {}
        }
        Ok(())
    }

    pub fn evaluate_type_value(&self, ty: &Type, ctx: &SharedScopedContext) -> Result<Type> {
        match ty {
            Type::Expr(expr) => {
                let value = self.interpret_expr(expr, ctx)?;
                let ty = value.try_conv()?;
                return Ok(ty);
            }
            Type::Struct(n) => {
                let fields = n
                    .fields
                    .iter()
                    .map(|x| {
                        let value = self.evaluate_type_value(&x.value, ctx)?;
                        Ok::<_, Error>(FieldTypeValue {
                            name: x.name.clone(),
                            value,
                        })
                    })
                    .try_collect()?;
                return Ok(Type::Struct(TypeStruct {
                    name: n.name.clone(),
                    fields,
                }));
            }
            Type::Structural(n) => {
                let fields = n
                    .fields
                    .iter()
                    .map(|x| {
                        let value = self.evaluate_type_value(&x.value, ctx)?;
                        Ok::<_, Error>(FieldTypeValue {
                            name: x.name.clone(),
                            value,
                        })
                    })
                    .try_collect()?;
                return Ok(Type::Structural(TypeStructural { fields }));
            }
            Type::Function(f) => {
                let sub = ctx.child(Ident::new("__func__"), Visibility::Private, false);
                for g in &f.generics_params {
                    let constrain = self.evaluate_type_bounds(&g.bounds, &sub)?;
                    sub.insert_value_with_ctx(g.name.clone(), constrain.into());
                }
                let params = f
                    .params
                    .iter()
                    .map(|x| self.evaluate_type_value(x, &sub))
                    .try_collect()?;
                let ret = self.evaluate_type_value(&f.ret, &sub)?;
                return Ok(Type::Function(
                    TypeFunction {
                        params,
                        generics_params: f.generics_params.clone(),
                        ret,
                    }
                    .into(),
                ));
            }
            Type::TypeBounds(b) => return self.evaluate_type_bounds(b, ctx),
            Type::ImplTraits(t) => return self.evaluate_impl_traits(t, ctx),
            _ => Ok(ty.clone()),
        }
    }
    pub fn evaluate_impl_traits(
        &self,
        traits: &ImplTraits,
        ctx: &SharedScopedContext,
    ) -> Result<Type> {
        let traits = self.evaluate_type_bounds(&traits.bounds, ctx)?;
        match traits {
            Type::TypeBounds(bounds) => Ok(Type::ImplTraits(ImplTraits { bounds })),
            _ => Ok(traits),
        }
    }

    pub fn evaluate_type_bounds(
        &self,
        bounds: &TypeBounds,
        ctx: &SharedScopedContext,
    ) -> Result<Type> {
        let bounds: Vec<_> = bounds
            .bounds
            .iter()
            .map(|x| self.interpret_expr(x, ctx))
            .try_collect()?;
        if bounds.is_empty() {
            return Ok(Type::any());
        }
        if bounds.len() == 1 {
            return bounds.first().unwrap().clone().try_conv();
        }

        bail!("failed to evaluate type bounds: {:?}", bounds)
        // Ok(TypeValue::TypeBounds(TypeBounds { bounds }))
    }

    pub fn type_check_expr(&self, expr: &Expr, ty: &Expr, ctx: &SharedScopedContext) -> Result<()> {
        let tv = self.interpret_expr(ty, ctx)?.try_conv()?;

        self.type_check_expr_against_value(expr, &tv, ctx)
    }

    pub fn infer_type_call(
        &self,
        callee: &Expr,
        params: &[Expr],
        ctx: &SharedScopedContext,
    ) -> Result<Type> {
        match callee {
            Expr::Locator(Locator::Ident(ident)) => match ident.as_str() {
                "+" | "-" | "*" => {
                    return self.infer_expr(params.first().context("No param")?, ctx)
                }
                "print" => return Ok(Type::unit()),
                _ => {}
            },
            _ => {}
        }

        let callee = self.infer_expr(callee, ctx)?;
        match callee {
            Type::Function(f) => return Ok(f.ret),
            _ => {}
        }

        bail!("Could not infer type call {:?}", callee)
    }
    pub fn infer_ident(&self, ident: &Ident, ctx: &SharedScopedContext) -> Result<Type> {
        match ident.as_str() {
            "print" => {
                return Ok(Type::Function(
                    TypeFunction {
                        params: vec![],
                        generics_params: vec![],
                        ret: Type::unit(),
                    }
                    .into(),
                ))
            }
            _ => {}
        }
        let expr = ctx
            .get_expr(ident)
            .with_context(|| format!("Could not find {:?} in context", ident))?;
        self.infer_expr(&expr, ctx)
    }
    pub fn infer_locator(&self, locator: &Locator, ctx: &SharedScopedContext) -> Result<Type> {
        if let Some(ty) = ctx.get_type(locator.to_path()) {
            return Ok(ty);
        }
        match locator {
            Locator::Ident(ident) => self.infer_ident(ident, ctx),
            _ => bail!("Could not infer locator {:?}", locator),
        }
    }
    pub fn infer_expr_invoke_target(
        &self,
        target: &ExprInvokeTarget,
        ctx: &SharedScopedContext,
    ) -> Result<Type> {
        match target {
            ExprInvokeTarget::Function(ident) => self.infer_locator(ident, ctx),

            _ => bail!("Could not infer invoke target {:?}", target),
        }
    }
    pub fn infer_expr(&self, expr: &Expr, ctx: &SharedScopedContext) -> Result<Type> {
        let ret = match expr {
            Expr::Locator(n) => self.infer_locator(n, ctx)?,
            Expr::Value(l) => match l.as_ref() {
                Value::Int(_) => Type::Primitive(TypePrimitive::Int(TypeInt::I64)),
                Value::Decimal(_) => Type::Primitive(TypePrimitive::Decimal(DecimalType::F64)),
                Value::Unit(_) => Type::unit(),
                Value::Bool(_) => Type::Primitive(TypePrimitive::Bool),
                Value::String(_) => Type::Primitive(TypePrimitive::String),
                Value::Type(_) => Type::Type(TypeType {}),
                Value::Char(_) => Type::Primitive(TypePrimitive::Char),
                Value::List(_) => Type::Primitive(TypePrimitive::List),
                _ => bail!("Could not infer type of {:?}", l),
            },
            Expr::Invoke(invoke) => {
                let function = self.infer_expr_invoke_target(&invoke.target, ctx)?;
                match function {
                    Type::Function(f) => f.ret,
                    _ => bail!("Expected function, got {:?}", function),
                }
            }
            Expr::BinOp(op) => {
                if op.kind.is_ret_bool() {
                    return Ok(Type::Primitive(TypePrimitive::Bool));
                }
                let lhs = self.infer_expr(&op.lhs, ctx)?;
                let rhs = self.infer_expr(&op.rhs, ctx)?;
                ensure!(
                    lhs == rhs,
                    "Expected same types, got {:?} and {:?}",
                    lhs,
                    rhs
                );
                lhs
            }
            _ => bail!("Could not infer type of {:?}", expr),
        };
        Ok(ret)
    }

    pub fn infer_function(
        &self,
        func: &ValueFunction,
        _ctx: &SharedScopedContext,
    ) -> Result<TypeFunction> {
        let mut params = vec![];
        for p in &func.params {
            params.push(p.ty.clone());
        }
        let ret = func.ret.clone();
        Ok(TypeFunction {
            params,
            generics_params: func.generics_params.clone(),
            ret,
        })
    }
}
impl TypeSystem for InterpreterPass {
    fn get_ty_from_expr(&self, ctx: &Context, expr: &Expr) -> Result<Type> {
        let fold = FoldOptimizer::new(self.serializer.clone(), Box::new(self.clone()));

        let expr = fold.optimize_expr(expr.clone(), &ctx.values)?;
        match expr {
            Expr::Value(v) => match v.into() {
                Value::Type(t) => return Ok(t),
                v => bail!("Expected type, got {:?}", v),
            },
            _ => bail!("Expected type, got {:?}", expr),
        }
    }
    fn get_ty_from_value(&self, ctx: &Context, value: &Value) -> Result<Type> {
        let fold = FoldOptimizer::new(self.serializer.clone(), Box::new(self.clone()));

        let value = fold.optimize_expr(Expr::Value(value.clone().into()), &ctx.values)?;

        match value {
            Expr::Value(v) => match v.into() {
                Value::Type(t) => return Ok(t),
                v => bail!("Expected type, got {:?}", v),
            },
            _ => bail!("Expected type, got {:?}", value),
        }
    }
}
