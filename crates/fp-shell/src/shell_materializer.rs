use fp_core::ast::{
    BlockStmt, Expr, ExprBlock, ExprInvoke, ExprInvokeTarget, ExprKind, File, FunctionSignature,
    Item, ItemKind,
};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct ShellMaterializer {
    sigs: RefCell<Option<HashMap<String, FunctionSignature>>>,
}

impl ShellMaterializer {
    pub fn new(_inventory: Option<File>) -> Self {
        Self {
            sigs: RefCell::new(None),
        }
    }

    pub fn prepare_file(&self, file: &mut File) {
        *self.sigs.borrow_mut() = Some(scan_all_signatures(file));
        if let Some(sigs) = self.sigs.borrow().as_ref() {
            for item in &mut file.items {
                inject_with_contexts_in_item(item, sigs);
            }
        }

        let mut new_items = Vec::new();
        let mut i = 0;
        while i < file.items.len() {
            match file.items[i].kind() {
                ItemKind::DefFunction(f) if f.name.as_str() == "main" => {
                    push_main_body_from_block(&f.body, &mut new_items);
                }
                ItemKind::DefConst(c) if c.name.as_str() == "main" => {
                    if let ExprKind::Block(block) = c.value.kind() {
                        push_main_body_from_block(block, &mut new_items);
                    }
                }
                _ => new_items.push(file.items[i].clone()),
            }
            i += 1;
        }
        file.items = new_items;
    }
}
fn inject_with_contexts_in_item(item: &mut Item, sigs: &HashMap<String, FunctionSignature>) {
    match item.kind_mut() {
        ItemKind::DefFunction(function) => {
            inject_with_contexts_in_block(&mut function.body, sigs, None);
        }
        ItemKind::DefConst(def) => inject_with_contexts(def.value.as_mut(), sigs, None),
        ItemKind::Expr(expr) => inject_with_contexts(expr, sigs, None),
        ItemKind::Module(module) => {
            for child in &mut module.items {
                inject_with_contexts_in_item(child, sigs);
            }
        }
        _ => {}
    }
}

fn inject_with_contexts(
    expr: &mut Expr,
    sigs: &HashMap<String, FunctionSignature>,
    context: Option<&Expr>,
) {
    match expr.kind_mut() {
        ExprKind::With(with) => {
            inject_with_contexts(with.body.as_mut(), sigs, Some(with.context.as_ref()));
            inject_with_contexts(with.context.as_mut(), sigs, context);
        }
        ExprKind::Block(block) => {
            inject_with_contexts_in_block(block, sigs, context);
        }
        ExprKind::If(branch) => {
            inject_with_contexts(branch.cond.as_mut(), sigs, context);
            inject_with_contexts(branch.then.as_mut(), sigs, context);
            if let Some(elze) = branch.elze.as_mut() {
                inject_with_contexts(elze, sigs, context);
            }
        }
        ExprKind::Loop(loop_expr) => {
            inject_with_contexts(loop_expr.body.as_mut(), sigs, context);
        }
        ExprKind::While(while_expr) => {
            inject_with_contexts(while_expr.cond.as_mut(), sigs, context);
            inject_with_contexts(while_expr.body.as_mut(), sigs, context);
        }
        ExprKind::For(for_expr) => {
            inject_with_contexts(for_expr.iter.as_mut(), sigs, context);
            inject_with_contexts(for_expr.body.as_mut(), sigs, context);
        }
        ExprKind::Match(match_expr) => {
            if let Some(scrutinee) = match_expr.scrutinee.as_mut() {
                inject_with_contexts(scrutinee, sigs, context);
            }
            for case in &mut match_expr.cases {
                inject_with_contexts(case.cond.as_mut(), sigs, context);
                if let Some(guard) = case.guard.as_mut() {
                    inject_with_contexts(guard, sigs, context);
                }
                inject_with_contexts(case.body.as_mut(), sigs, context);
            }
        }
        ExprKind::Invoke(invoke) => {
            if let Some(context) = context {
                inject_context_arg(invoke, context, sigs);
            }
            for arg in &mut invoke.args {
                inject_with_contexts(arg, sigs, context);
            }
            for kwarg in &mut invoke.kwargs {
                inject_with_contexts(&mut kwarg.value, sigs, context);
            }
        }
        ExprKind::Assign(assign) => {
            inject_with_contexts(assign.target.as_mut(), sigs, context);
            inject_with_contexts(assign.value.as_mut(), sigs, context);
        }
        ExprKind::FieldAccess(select) => inject_with_contexts(select.obj.as_mut(), sigs, context),
        ExprKind::Index(index) => {
            inject_with_contexts(index.obj.as_mut(), sigs, context);
            inject_with_contexts(index.index.as_mut(), sigs, context);
        }
        ExprKind::Struct(struct_expr) => {
            inject_with_contexts(struct_expr.name.as_mut(), sigs, context);
            for field in &mut struct_expr.fields {
                if let Some(value) = field.value.as_mut() {
                    inject_with_contexts(value, sigs, context);
                }
            }
            if let Some(update) = struct_expr.update.as_mut() {
                inject_with_contexts(update, sigs, context);
            }
        }
        ExprKind::Structural(struct_expr) => {
            for field in &mut struct_expr.fields {
                if let Some(value) = field.value.as_mut() {
                    inject_with_contexts(value, sigs, context);
                }
            }
        }
        ExprKind::Cast(cast) => inject_with_contexts(cast.expr.as_mut(), sigs, context),
        ExprKind::Reference(reference) => {
            inject_with_contexts(reference.referee.as_mut(), sigs, context)
        }
        ExprKind::Dereference(deref) => inject_with_contexts(deref.referee.as_mut(), sigs, context),
        ExprKind::Tuple(tuple) => {
            for value in &mut tuple.values {
                inject_with_contexts(value, sigs, context);
            }
        }
        ExprKind::Return(return_expr) => {
            if let Some(value) = return_expr.value.as_mut() {
                inject_with_contexts(value, sigs, context);
            }
        }
        ExprKind::Break(break_expr) => {
            if let Some(value) = break_expr.value.as_mut() {
                inject_with_contexts(value, sigs, context);
            }
        }
        ExprKind::Try(try_expr) => {
            inject_with_contexts(try_expr.expr.as_mut(), sigs, context);
            for catch in &mut try_expr.catches {
                inject_with_contexts(catch.body.as_mut(), sigs, context);
            }
            if let Some(elze) = try_expr.elze.as_mut() {
                inject_with_contexts(elze, sigs, context);
            }
            if let Some(finally) = try_expr.finally.as_mut() {
                inject_with_contexts(finally, sigs, context);
            }
        }
        ExprKind::Async(async_expr) => {
            inject_with_contexts(async_expr.expr.as_mut(), sigs, context)
        }
        ExprKind::Let(let_expr) => inject_with_contexts(let_expr.expr.as_mut(), sigs, context),
        ExprKind::Closure(closure) => inject_with_contexts(closure.body.as_mut(), sigs, context),
        ExprKind::Array(array) => {
            for value in &mut array.values {
                inject_with_contexts(value, sigs, context);
            }
        }
        ExprKind::ArrayRepeat(repeat) => {
            inject_with_contexts(repeat.elem.as_mut(), sigs, context);
            inject_with_contexts(repeat.len.as_mut(), sigs, context);
        }
        ExprKind::ConstBlock(const_block) => {
            inject_with_contexts(const_block.expr.as_mut(), sigs, context);
        }
        ExprKind::Paren(paren) => inject_with_contexts(paren.expr.as_mut(), sigs, context),
        ExprKind::BinOp(binop) => {
            inject_with_contexts(binop.lhs.as_mut(), sigs, context);
            inject_with_contexts(binop.rhs.as_mut(), sigs, context);
        }
        ExprKind::UnOp(unop) => inject_with_contexts(unop.val.as_mut(), sigs, context),
        ExprKind::Range(range) => {
            if let Some(start) = range.start.as_mut() {
                inject_with_contexts(start, sigs, context);
            }
            if let Some(end) = range.end.as_mut() {
                inject_with_contexts(end, sigs, context);
            }
            if let Some(step) = range.step.as_mut() {
                inject_with_contexts(step, sigs, context);
            }
        }
        ExprKind::Splat(splat) => inject_with_contexts(splat.iter.as_mut(), sigs, context),
        ExprKind::SplatDict(splat) => inject_with_contexts(splat.dict.as_mut(), sigs, context),
        _ => {}
    }
}

fn inject_with_contexts_in_block(
    block: &mut ExprBlock,
    sigs: &HashMap<String, FunctionSignature>,
    context: Option<&Expr>,
) {
    for statement in &mut block.stmts {
        match statement {
            BlockStmt::Expr(statement) => {
                inject_with_contexts(statement.expr.as_mut(), sigs, context)
            }
            BlockStmt::Let(statement) => {
                if let Some(init) = statement.init.as_mut() {
                    inject_with_contexts(init, sigs, context);
                }
            }
            BlockStmt::Defer(statement) => {
                inject_with_contexts(statement.expr.as_mut(), sigs, context)
            }
            BlockStmt::Item(item) => inject_with_contexts_in_item(item, sigs),
            BlockStmt::Noop => {}
        }
    }
}

fn inject_context_arg(
    invoke: &mut ExprInvoke,
    context: &Expr,
    sigs: &HashMap<String, FunctionSignature>,
) {
    let name = invoke_target_name(&invoke.target).unwrap_or_default();
    let index = if name.starts_with("__fp_")
        && [
            "_shell_local_",
            "_shell_ssh_",
            "_shell_docker_",
            "_shell_kubectl_",
            "_shell_winrm_",
            "_shell_chroot_",
        ]
        .iter()
        .any(|suffix| name.contains(suffix))
    {
        1
    } else {
        let signature = sigs.get(&name).or_else(|| {
            name.rsplit_once("::")
                .and_then(|(_, function)| sigs.get(function))
        });
        let Some(signature) = signature else {
            return;
        };
        let Some(index) = signature.params.iter().position(|param| param.is_context) else {
            return;
        };
        index
    };
    if invoke
        .kwargs
        .iter()
        .any(|kwarg| kwarg.name == "hosts" || kwarg.name == "target")
        || invoke.args.len() > index
    {
        return;
    }
    invoke.args.insert(index, context.clone());
}

// ── helpers ──

fn push_main_body_from_block(block: &ExprBlock, out: &mut Vec<Item>) {
    for stmt in &block.stmts {
        if let BlockStmt::Expr(expr) = stmt {
            out.push(Item::from(ItemKind::Expr(expr.expr.as_ref().clone())));
        }
    }
}

fn invoke_target_name(target: &ExprInvokeTarget) -> Option<String> {
    match target {
        ExprInvokeTarget::Function(name) => name
            .to_path()
            .segments
            .iter()
            .map(|s| s.as_str().to_string())
            .collect::<Vec<_>>()
            .join("::")
            .into(),
        ExprInvokeTarget::Method(select) => {
            let obj = invoke_target_name(&ExprInvokeTarget::Expr(select.obj.clone()))?;
            Some(format!("{}::{}", obj, select.field))
        }
        ExprInvokeTarget::Expr(expr) => match expr.kind() {
            ExprKind::Name(name) => Some(
                name.to_path()
                    .segments
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
            _ => None,
        },
        _ => None,
    }
}

fn scan_all_signatures(file: &File) -> HashMap<String, FunctionSignature> {
    let mut sigs = HashMap::new();
    scan_sigs(&file.items, &[], &mut sigs);
    sigs
}

fn scan_sigs(items: &[Item], path: &[String], out: &mut HashMap<String, FunctionSignature>) {
    for item in items {
        match item.kind() {
            ItemKind::DefFunction(f) => {
                let name = if path.is_empty() {
                    f.name.as_str().to_string()
                } else {
                    format!("{}::{}", path.join("::"), f.name.as_str())
                };
                out.insert(name, f.sig.clone());
            }
            ItemKind::DeclFunction(f) => {
                let name = if path.is_empty() {
                    f.name.as_str().to_string()
                } else {
                    format!("{}::{}", path.join("::"), f.name.as_str())
                };
                out.insert(name, f.sig.clone());
            }
            ItemKind::Module(m) => {
                let mut child = path.to_vec();
                child.push(m.name.as_str().to_string());
                scan_sigs(&m.items, &child, out);
            }
            _ => {}
        }
    }
}

pub fn flatten_keep_externs(items: Vec<Item>) -> Vec<Item> {
    let mut out = Vec::new();
    for item in items {
        match item.kind() {
            ItemKind::Module(m) => out.extend(flatten_keep_externs(m.items.clone())),
            ItemKind::DeclFunction(d) => out.push(Item::from(ItemKind::DeclFunction(d.clone()))),
            ItemKind::Expr(e) => out.push(Item::from(ItemKind::Expr(e.clone()))),
            _ => {}
        }
    }
    out
}
