//! AST are trees, so Box<T> is fine

use crate::{common_enum, common_struct};
use std::path::PathBuf;

mod expr;
mod item;
pub use expr::*;
pub use item::*;
common_struct! {
    pub struct File {
        pub path: PathBuf,
        pub module: Module,
    }
}

common_enum! {
    /// Tree is any syntax tree element
    pub enum Tree {
        Item(Item),
        Expr(Expr),
        File(File),
    }
}
