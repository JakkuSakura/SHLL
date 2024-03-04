use crate::ast::Visibility;
use crate::expr::Expr;
use crate::id::{Ident, Path};
use crate::ty::TypeValue;
use crate::value::{Value, ValueFunction};
use common::*;
use dashmap::DashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct ValueStorage {
    pub value: Option<Value>,
    pub ty: Option<TypeValue>,
    pub is_specialized: bool,
}
pub struct ScopedContext {
    parent: Option<ArcScopedContext>,
    #[allow(dead_code)]
    ident: Ident,
    path: Path,
    storages: DashMap<Ident, ValueStorage>,
    childs: DashMap<Ident, ArcScopedContext>,
    buffer: Mutex<Vec<String>>,
    #[allow(dead_code)]
    visibility: Visibility,
    access_parent_locals: bool,
}

// TODO: rename to ScopedContext
pub type ArcScopedContext = Arc<ScopedContext>;

impl ScopedContext {
    pub fn new() -> Self {
        ScopedContext {
            parent: None,
            ident: Ident::root(),
            path: Path::root(),
            storages: Default::default(),
            childs: Default::default(),
            buffer: Mutex::new(vec![]),
            visibility: Visibility::Public,
            access_parent_locals: false,
        }
    }
    pub fn into_shared(self) -> ArcScopedContext {
        Arc::new(self)
    }

    pub fn child(
        self: &ArcScopedContext,
        name: Ident,
        visibility: Visibility,
        access_parent_locals: bool,
    ) -> ArcScopedContext {
        let child = Arc::new(ScopedContext {
            parent: Some(self.clone()),
            ident: name.clone(),
            path: self.path.with_ident(name.clone()),
            storages: Default::default(),
            childs: Default::default(),
            buffer: Mutex::new(vec![]),
            visibility,
            access_parent_locals,
        });
        self.childs.insert(name, child.clone());
        child
    }

    pub fn insert_type(&self, key: impl Into<Ident>, value: TypeValue) {
        self.storages.entry(key.into()).or_default().ty = Some(value);
    }

    pub fn insert_value(&self, key: impl Into<Ident>, value: Value) {
        self.storages.entry(key.into()).or_default().value = Some(value);
    }
    pub fn insert_function(&self, key: impl Into<Ident>, value: ValueFunction) {
        self.insert_value(key.into(), Value::Function(value));
    }
    pub fn insert_expr(&self, key: impl Into<Ident>, value: Expr) {
        self.insert_value(key, Value::expr(value));
    }
    pub fn list_values(&self) -> Vec<Path> {
        let mut values = if let Some(parent) = &self.parent {
            parent.list_values()
        } else {
            vec![]
        };
        values.extend(
            self.storages
                .iter()
                .map(|x| x.key().clone())
                .sorted()
                .map(|x| self.path.with_ident(x)),
        );
        values
    }
    pub fn print_values(&self) -> Result<()> {
        if let Some(parent) = &self.parent {
            parent.print_values()?;
        }
        self.print_local_values()
    }
    pub fn print_local_values(&self) -> Result<()> {
        debug!("Values in {}", self.path);
        for key in self.storages.iter() {
            let (k, v) = key.pair();
            let value = v.value.as_ref().unwrap_or(&Value::UNDEFINED);
            let ty = v.ty.as_ref().unwrap_or(&TypeValue::ANY);
            debug!("{}: val:{} ty:{}", k, value, ty)
        }
        Ok(())
    }
    pub fn insert_specialized(&self, key: Ident, value: ValueFunction) {
        self.insert_function(key.clone(), value);
        self.storages.get_mut(&key).unwrap().is_specialized = true;
    }
    pub fn get_function(self: &ArcScopedContext, key: impl Into<Path>) -> Option<ValueFunction> {
        let value = self.get_value(key)?;
        match value {
            Value::Function(func) => Some(func),
            _ => None,
        }
    }
    pub fn get_module_recursive(
        self: &ArcScopedContext,
        key: impl Into<Path>,
    ) -> Option<ArcScopedContext> {
        let key = key.into();
        let mut this = self.clone();
        if key.segments.is_empty() {
            return Some(this);
        }
        for seg in &key.segments {
            if seg.is_root() {
                this = this.root().clone();
                continue;
            }
            let v = this.childs.get(seg)?.clone();
            this = v;
        }

        Some(this)
    }
    pub fn get_storage(
        self: &ArcScopedContext,
        key: impl Into<Path>,
        access_local: bool,
    ) -> Option<ValueStorage> {
        let key = key.into();
        debug!(
            "get_storage in {} {} access_local={}",
            self.path, key, access_local
        );
        self.print_local_values().unwrap();
        if key.segments.is_empty() {
            return None;
        }
        if key.segments.len() == 1 {
            // TODO: when calling function, use context of its own
            // if access_local {
            let value = self.storages.get(&key.segments[0]);
            if let Some(value) = value {
                return Some(value.value().clone());
            }
            // }
            return self
                .parent
                .as_ref()?
                .get_storage(key, self.access_parent_locals);
        }

        let (paths, key) = key.segments.split_at(key.segments.len() - 1);
        let this = self.get_module_recursive(Path::new(paths.to_owned()))?;
        let value = this.storages.get(&key[0])?.value().clone();
        Some(value)
    }
    pub fn get_value(self: &ArcScopedContext, key: impl Into<Path>) -> Option<Value> {
        let storage = self.get_storage(key, true)?;
        storage.value
    }
    pub fn get_expr(self: &ArcScopedContext, key: impl Into<Path>) -> Option<Expr> {
        self.get_value(key).map(Expr::value)
    }
    pub fn get_type(self: &ArcScopedContext, key: impl Into<Path>) -> Option<TypeValue> {
        let storage = self.get_storage(key, true)?;
        storage.ty
    }
    pub fn root(self: &ArcScopedContext) -> ArcScopedContext {
        self.parent
            .as_ref()
            .map(|x| x.root())
            .unwrap_or_else(|| self.clone())
    }
    pub fn print_str(&self, s: String) {
        self.buffer.lock().unwrap().push(s);
    }
    pub fn take_outputs(&self) -> Vec<String> {
        std::mem::replace(&mut self.buffer.lock().unwrap(), vec![])
    }
    pub fn list_specialized(&self) -> Vec<Ident> {
        self.storages
            .iter()
            .filter(|x| x.is_specialized)
            .map(|x| x.key().clone())
            .collect()
    }
    // TODO: integrate it to optimizers
    pub fn try_get_value_from_expr(self: &ArcScopedContext, expr: &Expr) -> Option<Value> {
        // info!("try_get_value_from_expr {}", expr);
        let ret = match expr {
            Expr::Locator(ident) => self.get_value(ident),
            Expr::Value(value) => Some(*value.clone()),
            _ => None,
        };
        info!(
            "try_get_value_from_expr {} => {}",
            expr,
            ret.as_ref().map(|x| x.to_string()).unwrap_or_default()
        );
        ret
    }
    pub fn get_value_recursive(self: &ArcScopedContext, key: impl Into<Path>) -> Option<Value> {
        let key = key.into();
        info!("get_value_recursive {}", key);
        let expr = self.get_expr(&key)?;
        info!("get_value_recursive {} => {:?}", key, expr);
        match expr {
            Expr::Locator(ident) => self.get_value_recursive(ident),
            _ => Some(Value::expr(expr)),
        }
    }
}

#[derive(Clone)]
pub struct LazyValue<Expr> {
    pub ctx: ArcScopedContext,
    pub expr: Expr,
}

impl<Expr: PartialEq> PartialEq for LazyValue<Expr> {
    fn eq(&self, other: &Self) -> bool {
        (self.ctx.as_ref() as *const _ == other.ctx.as_ref() as *const _)
            && self.expr.eq(&other.expr)
    }
}
impl<Expr: Eq> Eq for LazyValue<Expr> {}
impl<Expr: Debug> Debug for LazyValue<Expr> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "LazyValue({:?})", self.expr)
    }
}
impl Serialize for LazyValue<Expr> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:?}", self))
    }
}
impl<'de> Deserialize<'de> for LazyValue<Expr> {
    fn deserialize<D: serde::Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        unreachable!()
    }
}
