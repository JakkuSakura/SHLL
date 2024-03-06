use crate::ast::Visibility;
use crate::expr::Expr;
use crate::id::{Ident, Path};
use crate::value::{TypeValue, Value, ValueFunction};
use common::*;
use dashmap::DashMap;
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Default)]
pub struct ValueStorage {
    value: Option<Value>,
    ty: Option<Value>,
    closure: Option<Arc<ScopedContext>>,
}
pub struct ScopedContext {
    parent: Option<Weak<Self>>,
    #[allow(dead_code)]
    ident: Ident,
    path: Path,
    storages: DashMap<Ident, ValueStorage>,
    childs: DashMap<Ident, Arc<Self>>,
    buffer: Mutex<Vec<String>>,
    #[allow(dead_code)]
    visibility: Visibility,
    access_parent_locals: bool,
}

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

    pub fn insert_value(&self, key: impl Into<Ident>, value: Value) {
        self.storages.entry(key.into()).or_default().value = Some(value);
    }

    pub fn insert_expr(&self, key: impl Into<Ident>, value: Expr) {
        self.insert_value(key, Value::expr(value));
    }

    pub fn print_local_values(&self) -> Result<()> {
        debug!("Values in {}", self.path);
        for key in self.storages.iter() {
            let (k, v) = key.pair();
            let value = v.value.as_ref().unwrap_or(&Value::UNDEFINED);
            let ty = v.ty.as_ref().unwrap_or(&Value::UNDEFINED);
            debug!("{}: val:{} ty:{}", k, value, ty)
        }
        Ok(())
    }

    pub fn print_str(&self, s: String) {
        self.buffer.lock().unwrap().push(s);
    }
    pub fn take_outputs(&self) -> Vec<String> {
        std::mem::replace(&mut self.buffer.lock().unwrap(), vec![])
    }
}

#[derive(Clone)]
pub struct SharedScopedContext(Arc<ScopedContext>);
impl Deref for SharedScopedContext {
    type Target = ScopedContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl SharedScopedContext {
    pub fn new() -> Self {
        Self(Arc::new(ScopedContext::new()))
    }
    pub fn child(&self, name: Ident, visibility: Visibility, access_parent_locals: bool) -> Self {
        let child = Self(Arc::new(ScopedContext {
            parent: Some(Arc::downgrade(&self.0)),
            ident: name.clone(),
            path: self.path.with_ident(name.clone()),
            storages: Default::default(),
            childs: Default::default(),
            buffer: Mutex::new(vec![]),
            visibility,
            access_parent_locals,
        }));
        self.childs.insert(name, child.0.clone());
        child
    }

    pub fn get_function(&self, key: impl Into<Path>) -> Option<(ValueFunction, Self)> {
        let value = self.get_storage(key, true)?;
        match value.value? {
            Value::Function(func) => Some((func.clone(), Self(value.closure.clone().unwrap()))),
            _ => None,
        }
    }
    pub fn get_module_recursive(
        self: &SharedScopedContext,
        key: impl Into<Path>,
    ) -> Option<SharedScopedContext> {
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
            this = Self(v);
        }

        Some(this)
    }
    pub fn get_storage(&self, key: impl Into<Path>, access_local: bool) -> Option<ValueStorage> {
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
                .get_parent()?
                .get_storage(key, self.access_parent_locals);
        }

        let (paths, key) = key.segments.split_at(key.segments.len() - 1);
        let this = self.get_module_recursive(Path::new(paths.to_owned()))?;
        let value = this.storages.get(&key[0])?.value().clone();
        Some(value)
    }
    pub fn get_value(&self, key: impl Into<Path>) -> Option<Value> {
        let storage = self.get_storage(key, true)?;
        storage.value
    }
    pub fn insert_value_with_ctx(&self, key: impl Into<Ident>, value: Value) {
        let mut store = self.storages.entry(key.into()).or_default();
        store.value = Some(value);
        store.closure = Some(self.clone().0);
    }
    pub fn get_expr(&self, key: impl Into<Path>) -> Option<Expr> {
        self.get_value(key).map(Expr::value)
    }
    pub fn get_expr_with_ctx(&self, key: impl Into<Path>) -> Option<Expr> {
        self.get_value(key).map(Expr::value)
    }
    pub fn get_type(&self, key: impl Into<Path>) -> Option<TypeValue> {
        let storage = self.get_storage(key, true)?;
        let ty = storage.ty?;
        match ty {
            Value::Type(ty) => Some(ty),
            _ => None,
        }
    }
    pub fn root(&self) -> Self {
        self.get_parent()
            .map(|x| x.root())
            .unwrap_or_else(|| self.clone())
    }
    // TODO: integrate it to optimizers
    pub fn try_get_value_from_expr(&self, expr: &Expr) -> Option<Value> {
        // info!("try_get_value_from_expr {}", expr);
        let ret = match expr {
            Expr::Locator(ident) => self.get_value(ident.to_path()),
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
    pub fn get_value_recursive(&self, key: impl Into<Path>) -> Option<Value> {
        let key = key.into();
        info!("get_value_recursive {}", key);
        let expr = self.get_expr(&key)?;
        info!("get_value_recursive {} => {:?}", key, expr);
        match expr {
            Expr::Locator(ident) => self.get_value_recursive(ident.to_path()),
            _ => Some(Value::expr(expr)),
        }
    }
    pub fn get_parent(&self) -> Option<Self> {
        match &self.parent {
            Some(parent) => match parent.upgrade() {
                Some(parent) => Some(Self(parent)),
                None => {
                    panic!("Context parent is dropped")
                }
            },
            _ => None,
        }
    }

    pub fn print_values(&self) -> Result<()> {
        if let Some(parent) = self.get_parent() {
            parent.print_values()?;
        }
        self.print_local_values()
    }
    pub fn list_values(&self) -> Vec<Path> {
        let mut values = if let Some(parent) = self.get_parent() {
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
}
impl PartialEq for SharedScopedContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for SharedScopedContext {}
