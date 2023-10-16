use crate::ast::Ident;
use crate::context::ArcScopedContext;
use crate::value::TypeValue;
use common::*;

pub struct Injector {}
impl Injector {
    pub fn new() -> Self {
        Self {}
    }
    pub fn pick(&self, arg_ty: &TypeValue, ctx: &ArcScopedContext) -> Option<Ident> {
        let mut candidates = vec![];
        for ident in ctx.list_values() {
            let Some(value_type) = ctx.get_type(&ident) else {
                continue;
            };
            // if arg_ty is a solid type and value_type is a solid type
            if &value_type == arg_ty {
                candidates.push(ident.last().clone());
                continue;
            }
            // info!("Comparing {} and {}", arg_ty, value_type);
            // if arg_ty is a shared reference and value_type is a mut reference or solid type
            // or if arg_ty is a mut reference and value_type is a solid type
            // it also counts
            if let TypeValue::Reference(r) = arg_ty {
                if r.mutability.unwrap_or_default() == false {
                    if let TypeValue::Reference(r2) = &value_type {
                        if r.ty == r2.ty
                            && r.lifetime == r2.lifetime
                            && r2.mutability.unwrap_or_default() == true
                        {
                            candidates.push(ident.last().clone());
                            continue;
                        }
                    }
                }
                if *r.ty == value_type {
                    candidates.push(ident.last().clone());
                    continue;
                }
            }
        }
        if candidates.len() > 1 {
            warn!("Ambiguous type: {}", arg_ty);
        }
        candidates.into_iter().next()
    }
    pub fn pick_args(&self, args: &[TypeValue], ctx: &ArcScopedContext) -> Result<Vec<Ident>> {
        args.iter()
            .map(|arg| {
                self.pick(arg, ctx)
                    .with_context(|| format!("Cannot pickup type: {}", arg))
            })
            .collect()
    }
}