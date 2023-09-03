pub mod ast;
pub mod context;
pub mod interpreter;
pub mod ops;
pub mod optimizer;
pub mod passes;
pub mod type_system;
pub mod value;
use ast::*;
use common::*;

use crate::ast::Tree;

use crate::value::{FunctionValue, TypeValue, Value};
use std::rc::Rc;
#[macro_export]
macro_rules! common_derives {
    (no_debug $($t:tt)*) => {
        #[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
        $($t)*
    };
    ($($t:tt)*) => {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
        $($t)*
    };
}

pub trait Serializer {
    fn serialize_tree(&self, node: &Tree) -> Result<String>;
    fn serialize_expr(&self, node: &Expr) -> Result<String>;
    fn serialize_args(&self, nodes: &[Expr]) -> Result<String> {
        let mut s = String::new();
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&self.serialize_expr(node)?);
        }
        Ok(s)
    }
    fn serialize_invoke(&self, node: &Invoke) -> Result<String>;
    fn serialize_item(&self, node: &Item) -> Result<String>;
    fn serialize_block(&self, node: &Block) -> Result<String>;
    fn serialize_module(&self, node: &Module) -> Result<String>;
    fn serialize_value(&self, node: &Value) -> Result<String>;
    fn serialize_values(&self, nodes: &[Value]) -> Result<String> {
        let mut s = String::new();
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&self.serialize_value(node)?);
        }
        Ok(s)
    }
    fn serialize_type(&self, node: &TypeValue) -> Result<String>;
    fn serialize_stmt(&self, node: &Statement) -> Result<String>;
    fn serialize_function(&self, node: &FunctionValue) -> Result<String>;
}

pub trait Deserializer {
    fn deserialize(&self, code: &str) -> Result<Tree>;
}
