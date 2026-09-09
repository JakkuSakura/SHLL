use super::*;

fn test_pkg() -> hir::PackageId {
    hir::PackageId::new("test")
}

fn hid(index: u32) -> hir::HirId {
    hir::HirId::new(hir::OwnerId::root(test_pkg()), index)
}

/// Test-only stand-in for the old `HirTypeChecker::new(program).check()`
/// single-future entry point — spawns one task per top-level item (see
/// `HirTypeChecker::spawn_item_task`) and awaits them all directly, the
/// same way `fp_compiler::driver::type_check_program` does (no
/// driver-specific setup, no comptime requests expected in these
/// tests). `async` all the way through, rather than hand-rolling a
/// poll/tick loop: the caller drives it to completion via
/// `ExecutorHandle::run` (see call sites below), on the same executor
/// the item tasks are spawned on.
async fn typecheck_program(
    package: hir::HirPackage,
    executor: ExecutorHandle,
) -> Result<Rc<RefCell<hir::HirPackage>>> {
    let package = Rc::new(RefCell::new(package));
    let checker = HirTypeChecker::new(Rc::clone(&package), None, None, executor);
    let item_ids: Vec<_> = checker
        .borrow()
        .package()
        .items
        .iter()
        .map(|item| item.def_id.clone())
        .collect();
    let handles: Vec<_> = item_ids
        .into_iter()
        .map(|def_id| HirTypeChecker::spawn_item_task(&checker, def_id))
        .collect();
    for handle in handles {
        handle.await;
    }
    Ok(checker.borrow().finish())
}

/// The core same-package ordering fix: `const A` (checked first, per
/// `program.items`' textual order) references `const B`, declared
/// *later* in the same list. Before `expr_path_ty`'s `Const` arm
/// awaited `B`'s own task on demand, this silently fell back to
/// "constant type was not recorded" instead of resolving `B`'s real
/// type.
#[test]
fn forward_referenced_const_resolves_regardless_of_item_order() {
    let b_def_id = hir::DefId::local(2);
    let a_def_id = hir::DefId::local(1);

    let b_item = hir::Item {
        hir_id: hid(10),
        def_id: b_def_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Const(hir::Const {
            name: "B".into(),
            mutable: false,
            is_host: false,
            ty: hir::TypeExpr {
                hir_id: hid(11),
                kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
                span: fp_core::span::Span::null(),
            },
            body: hir::Body {
                hir_id: hid(12),
                params: Vec::new(),
                value: hir::Expr {
                    hir_id: hid(13),
                    kind: hir::ExprKind::Literal(hir::Lit::Integer(41)),
                    span: fp_core::span::Span::null(),
                },
            },
        }),
        span: fp_core::span::Span::null(),
    };

    let a_item = hir::Item {
        hir_id: hid(20),
        def_id: a_def_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Const(hir::Const {
            name: "A".into(),
            mutable: false,
            is_host: false,
            ty: hir::TypeExpr {
                hir_id: hid(21),
                kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
                span: fp_core::span::Span::null(),
            },
            body: hir::Body {
                hir_id: hid(22),
                params: Vec::new(),
                value: hir::Expr {
                    hir_id: hid(23),
                    kind: hir::ExprKind::Binary(
                        hir::BinOp::Add,
                        Box::new(hir::Expr {
                            hir_id: hid(24),
                            kind: hir::ExprKind::Path(hir::QPath::resolved(hir::Path {
                                span: Default::default(),
                                segments: vec![hir::PathSegment {
                                    ident: "B".into(),
                                    hir_id: Default::default(),
                                    args: None,
                                    infer_args: true,
                                    delegation_child_segment: false,
                                    res: hir::Res::Def(b_def_id.clone()),
                                }],
                                res: hir::Res::Def(b_def_id.clone()),
                            })),
                            span: fp_core::span::Span::null(),
                        }),
                        Box::new(hir::Expr {
                            hir_id: hid(25),
                            kind: hir::ExprKind::Literal(hir::Lit::Integer(1)),
                            span: fp_core::span::Span::null(),
                        }),
                    ),
                    span: fp_core::span::Span::null(),
                },
            },
        }),
        span: fp_core::span::Span::null(),
    };

    let mut program = hir::HirPackage::new(test_pkg());
    // Textual order: A first, B second -- A's own task must await B's
    // on demand rather than assuming it's already been checked.
    program.items.push(a_item.clone());
    program.items.push(b_item.clone());
    program.def_map.insert(a_def_id.clone(), a_item);
    program.def_map.insert(b_def_id.clone(), b_item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        results.borrow().const_type(a_def_id),
        Some(Ty::int(ty::IntTy::I64)),
        "forward-referenced const B's type must resolve, not fall back to error_ty"
    );
    assert_eq!(
        results.borrow().const_type(b_def_id),
        Some(Ty::int(ty::IntTy::I64))
    );
}

#[test]
fn records_literal_type_by_hir_id() {
    let expr = hir::Expr {
        hir_id: hid(7),
        kind: hir::ExprKind::Literal(hir::Lit::Integer(4)),
        span: fp_core::span::Span::null(),
    };
    let mut program = hir::HirPackage::new(test_pkg());
    let item = hir::Item {
        hir_id: hid(1),
        def_id: hir::DefId::local(1),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(expr),
        span: fp_core::span::Span::null(),
    };
    program.items.push(item.clone());
    // Real HIR lowering always populates `def_map` before typing begins
    // (see `ast_to_hir::transform_package`'s last step) — per-item tasks
    // look items up by `DefId` through it (needed so a cross-reference
    // to an item spawned only by `def_id`, not handed the `Item`
    // directly, can still find it), so a hand-built test program needs
    // to mirror that.
    program.def_map.insert(item.def_id.clone(), item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        results.borrow().expr_type(hid(7)),
        Some(Ty::int(ty::IntTy::I64))
    );
}

#[test]
fn numeric_binary_context_reaches_nested_literal_expression() {
    let nested_lhs_id = hid(14);
    let nested_rhs_id = hid(15);
    let expr = hir::Expr {
        hir_id: hid(7),
        kind: hir::ExprKind::Binary(
            hir::BinOp::Ne,
            Box::new(hir::Expr {
                hir_id: hid(8),
                kind: hir::ExprKind::Cast(
                    Box::new(hir::Expr {
                        hir_id: hid(9),
                        kind: hir::ExprKind::Literal(hir::Lit::Integer(0)),
                        span: fp_core::span::Span::null(),
                    }),
                    Box::new(hir::TypeExpr {
                        hir_id: hid(10),
                        kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::U128)),
                        span: fp_core::span::Span::null(),
                    }),
                ),
                span: fp_core::span::Span::null(),
            }),
            Box::new(hir::Expr {
                hir_id: hid(11),
                kind: hir::ExprKind::Binary(
                    hir::BinOp::Shl,
                    Box::new(hir::Expr {
                        hir_id: nested_lhs_id.clone(),
                        kind: hir::ExprKind::Literal(hir::Lit::Integer(1)),
                        span: fp_core::span::Span::null(),
                    }),
                    Box::new(hir::Expr {
                        hir_id: nested_rhs_id.clone(),
                        kind: hir::ExprKind::Literal(hir::Lit::Integer(127)),
                        span: fp_core::span::Span::null(),
                    }),
                ),
                span: fp_core::span::Span::null(),
            }),
        ),
        span: fp_core::span::Span::null(),
    };
    let item = hir::Item {
        hir_id: hid(1),
        def_id: hir::DefId::local(1),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(expr),
        span: fp_core::span::Span::null(),
    };
    let mut program = hir::HirPackage::new(test_pkg());
    program.items.push(item.clone());
    program.def_map.insert(item.def_id.clone(), item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        results.borrow().expr_type(nested_lhs_id),
        Some(Ty::uint(ty::UintTy::U128))
    );
    assert_eq!(
        results.borrow().expr_type(nested_rhs_id),
        Some(Ty::uint(ty::UintTy::U128))
    );
}

#[test]
fn array_index_literal_infers_usize() {
    let index_id = hid(16);
    let expr = hir::Expr {
        hir_id: hid(17),
        kind: hir::ExprKind::Index(
            Box::new(hir::Expr {
                hir_id: hid(18),
                kind: hir::ExprKind::Array(vec![hir::Expr {
                    hir_id: hid(19),
                    kind: hir::ExprKind::Literal(hir::Lit::Integer(1)),
                    span: fp_core::span::Span::null(),
                }]),
                span: fp_core::span::Span::null(),
            }),
            Box::new(hir::Expr {
                hir_id: index_id.clone(),
                kind: hir::ExprKind::Literal(hir::Lit::Integer(0)),
                span: fp_core::span::Span::null(),
            }),
        ),
        span: fp_core::span::Span::null(),
    };
    let item = hir::Item {
        hir_id: hid(20),
        def_id: hir::DefId::local(2),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(expr),
        span: fp_core::span::Span::null(),
    };
    let mut program = hir::HirPackage::new(test_pkg());
    program.items.push(item.clone());
    program.def_map.insert(item.def_id.clone(), item);
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        results.borrow().expr_type(index_id),
        Some(Ty::uint(ty::UintTy::Usize))
    );
}

#[test]
fn array_index_integer_local_preserves_element_type() {
    let index_local_id = hid(26);
    let index_expr_id = hid(27);
    let value_expr_id = hid(28);
    let item = hir::Item {
        hir_id: hid(29),
        def_id: hir::DefId::local(3),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(hir::Expr {
            hir_id: hid(30),
            kind: hir::ExprKind::Block(hir::Block {
                hir_id: hid(31),
                stmts: vec![hir::Stmt {
                    hir_id: hid(32),
                    kind: hir::StmtKind::Local(hir::Local {
                        hir_id: hid(33),
                        pat: hir::Pat {
                            hir_id: hid(34),
                            kind: hir::PatKind::Binding {
                                name: "i".into(),
                                mutable: false,
                            },
                        },
                        ty: Some(hir::TypeExpr {
                            hir_id: hid(35),
                            kind: hir::TypeExprKind::Infer,
                            span: fp_core::span::Span::null(),
                        }),
                        init: Some(hir::Expr {
                            hir_id: index_local_id.clone(),
                            kind: hir::ExprKind::Literal(hir::Lit::Integer(0)),
                            span: fp_core::span::Span::null(),
                        }),
                    }),
                }],
                expr: Some(Box::new(hir::Expr {
                    hir_id: value_expr_id,
                    kind: hir::ExprKind::Index(
                        Box::new(hir::Expr {
                            hir_id: hid(36),
                            kind: hir::ExprKind::Array(vec![hir::Expr {
                                hir_id: hid(37),
                                kind: hir::ExprKind::Literal(hir::Lit::Integer(1)),
                                span: fp_core::span::Span::null(),
                            }]),
                            span: fp_core::span::Span::null(),
                        }),
                        Box::new(hir::Expr {
                            hir_id: index_expr_id,
                            kind: hir::ExprKind::Path(hir::QPath::resolved(hir::Path {
                                span: fp_core::span::Span::null(),
                                segments: vec![hir::PathSegment {
                                    ident: "i".into(),
                                    hir_id: Default::default(),
                                    args: None,
                                    infer_args: true,
                                    delegation_child_segment: false,
                                    res: hir::Res::Local(hid(34)),
                                }],
                                res: hir::Res::Local(hid(34)),
                            })),
                            span: fp_core::span::Span::null(),
                        }),
                    ),
                    span: fp_core::span::Span::null(),
                })),
            }),
            span: fp_core::span::Span::null(),
        }),
        span: fp_core::span::Span::null(),
    };
    let mut package = hir::HirPackage::new(test_pkg());
    package.def_map.insert(item.def_id.clone(), item.clone());
    package.items.push(item);
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let result = executor
        .run(typecheck_program(package, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        result.borrow().expr_type(hid(30)),
        Some(Ty::int(ty::IntTy::I64))
    );
}

#[test]
fn records_binding_pattern_type() {
    let pattern = hir::Pat {
        hir_id: hid(8),
        kind: hir::PatKind::Binding {
            name: "value".into(),
            mutable: false,
        },
    };
    let expr = hir::Expr {
        hir_id: hid(9),
        kind: hir::ExprKind::Let(
            pattern,
            Box::new(hir::TypeExpr {
                hir_id: hid(10),
                kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
                span: fp_core::span::Span::null(),
            }),
            None,
        ),
        span: fp_core::span::Span::null(),
    };
    let mut program = hir::HirPackage::new(test_pkg());
    let item = hir::Item {
        hir_id: hid(1),
        def_id: hir::DefId::local(1),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(expr),
        span: fp_core::span::Span::null(),
    };
    program.items.push(item.clone());
    program.def_map.insert(item.def_id.clone(), item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        results.borrow().pat_type(hid(8)),
        Some(Ty::int(ty::IntTy::I64))
    );
}

fn str_shaped_ty() -> Ty {
    Ty { kind: TyKind::Str }
}

#[test]
fn string_and_byte_slice_use_distinct_method_lookup_buckets() {
    let string_keys =
        type_shapes::ty_shape_keys(&TyKind::Str).expect("str has a method lookup shape");
    let byte_slice_keys =
        type_shapes::ty_shape_keys(&TyKind::Slice(Box::new(Ty::uint(ty::UintTy::U8))))
            .expect("[u8] has a method lookup shape");

    assert_eq!(string_keys, vec!["str"]);
    assert_eq!(byte_slice_keys, vec!["[]"]);
}

#[test]
fn error_types_are_invalid_for_lookup_even_when_nested() {
    let invalid = Ty {
        kind: TyKind::Ref(
            ty::Region::ReStatic,
            Box::new(Ty {
                kind: TyKind::Tuple(vec![Box::new(Ty::error())]),
            }),
            ty::Mutability::Not,
        ),
    };

    assert!(ty_contains_error(&invalid));
    assert!(!ty_contains_error(&Ty {
        kind: TyKind::Param(ty::ParamTy {
            index: 0,
            name: "T".into(),
        }),
    }));
}

#[test]
fn impl_header_obligation_is_keyed_by_impl_def_id() {
    let package_id = test_pkg();
    let shared_self_ty = hir::TypeExpr {
        hir_id: hid(901),
        kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
        span: fp_core::span::Span::null(),
    };
    let first_impl = hir::DefId::new(package_id.clone(), 901);
    let second_impl = hir::DefId::new(package_id.clone(), 902);
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let checker = HirTypeChecker::new(
        Rc::new(RefCell::new(hir::HirPackage::new(package_id))),
        None,
        None,
        executor.clone(),
    );
    executor.run(async move {
        let mut checker = checker.borrow_mut();
        let first = checker
            .checked_impl_self_ty(&first_impl, &shared_self_ty)
            .await
            .expect("first impl header should resolve");
        let second = checker
            .checked_impl_self_ty(&second_impl, &shared_self_ty)
            .await
            .expect("distinct impl header should resolve");
        assert_eq!(first, second);
        assert!(checker.resolving_impl_headers.is_empty());
    });
}

#[test]
fn repeated_generic_call_parameter_refines_self_binding() {
    let package = Rc::new(RefCell::new(hir::HirPackage::new(test_pkg())));
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let checker = HirTypeChecker::new(package, None, None, executor);
    let parameter = ty::ParamTy {
        index: 7,
        name: "T".into(),
    };
    let parameter_ty = Ty {
        kind: TyKind::Param(parameter.clone()),
    };
    let callable = Ty {
        kind: TyKind::FnPtr(ty::PolyFnSig {
            binder: ty::Binder {
                value: ty::FnSig {
                    inputs: vec![
                        Box::new(parameter_ty.clone()),
                        Box::new(parameter_ty.clone()),
                    ],
                    output: Box::new(parameter_ty),
                    c_variadic: false,
                    unsafety: ty::Unsafety::Normal,
                    abi: ty::Abi::Rust,
                },
                bound_vars: Vec::new(),
            },
        }),
    };
    let actual = Ty::float(ty::FloatTy::F32);
    let checker = checker.borrow();
    let (substitutions, output) = checker
        .instantiate_call(&callable, &[actual.clone(), actual.clone()], None)
        .expect("generic call instantiation should succeed")
        .expect("callable signature should instantiate");
    assert_eq!(substitutions.get(&parameter), Some(&actual));
    assert_eq!(output, actual);
}

/// Wraps a bare `hir::TypeExpr` in `let value: <ty>;` (no initializer)
/// the same way `f16_and_f128_type_paths_resolve_as_primitive_floats`
/// does, so `check_type_expr`'s handling of a single `TypeExprKind` can
/// be exercised in isolation via `results.pat_type(hid(8))`.
fn let_with_type(ty_kind: hir::TypeExprKind) -> hir::HirPackage {
    let pattern = hir::Pat {
        hir_id: hid(8),
        kind: hir::PatKind::Binding {
            name: "value".into(),
            mutable: false,
        },
    };
    let expr = hir::Expr {
        hir_id: hid(9),
        kind: hir::ExprKind::Let(
            pattern,
            Box::new(hir::TypeExpr {
                hir_id: hid(10),
                kind: ty_kind,
                span: fp_core::span::Span::null(),
            }),
            None,
        ),
        span: fp_core::span::Span::null(),
    };
    let mut program = hir::HirPackage::new(test_pkg());
    let item = hir::Item {
        hir_id: hid(1),
        def_id: hir::DefId::local(1),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(expr),
        span: fp_core::span::Span::null(),
    };
    program.items.push(item.clone());
    program.def_map.insert(item.def_id.clone(), item);
    program
}

#[test]
fn string_literal_type_resolves_to_str() {
    let program = let_with_type(hir::TypeExprKind::LiteralString("foo".into()));
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(results.borrow().pat_type(hid(8)), Some(str_shaped_ty()));
}

#[test]
fn union_of_string_literal_types_resolves_to_str() {
    let program = let_with_type(hir::TypeExprKind::TypeBinaryOp(hir::TypeBinaryOp {
        kind: fp_core::ast::TypeBinaryOpKind::Union,
        lhs: Box::new(hir::TypeExpr {
            hir_id: hid(11),
            kind: hir::TypeExprKind::LiteralString("a".into()),
            span: fp_core::span::Span::null(),
        }),
        rhs: Box::new(hir::TypeExpr {
            hir_id: hid(12),
            kind: hir::TypeExprKind::LiteralString("b".into()),
            span: fp_core::span::Span::null(),
        }),
    }));
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(results.borrow().pat_type(hid(8)), Some(str_shaped_ty()));
}

/// A union of two *non*-literal types (e.g. two primitives) must keep
/// erroring exactly as it did before literal-union support was added —
/// only a union where every operand is itself a literal-string type (or
/// a nested union of them) is accepted.
#[test]
fn union_of_non_literal_types_still_errors() {
    let program = let_with_type(hir::TypeExprKind::TypeBinaryOp(hir::TypeBinaryOp {
        kind: fp_core::ast::TypeBinaryOpKind::Union,
        lhs: Box::new(hir::TypeExpr {
            hir_id: hid(11),
            kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
            span: fp_core::span::Span::null(),
        }),
        rhs: Box::new(hir::TypeExpr {
            hir_id: hid(12),
            kind: hir::TypeExprKind::Primitive(TypePrimitive::Bool),
            span: fp_core::span::Span::null(),
        }),
    }));
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(results.borrow().pat_type(hid(8)), Some(Ty::error()));
}

/// `f16`/`f128` are real, stabilized Rust primitive float types (same
/// family as `f32`/`f64`), not name-resolution gaps — a bare `f16`/
/// `f128` type path must resolve straight to `Ty::Float`, never fall
/// through to `path_ty`'s "unresolved type path" `error_ty` branch the
/// way an actually-undeclared name would.
#[test]
fn f16_and_f128_type_paths_resolve_as_primitive_floats() {
    // `let value: f16/f128;` with no initializer — `ExprKind::Let`'s
    // declared-type slot (`check_type_expr(target)`) is recorded into
    // `pat_types` verbatim, unlike a `Const`'s slot (which gets
    // overwritten by the body's own inferred type), so this isolates
    // exactly what `path_ty`/`primitive_path_ty` resolve a bare
    // `f16`/`f128` path to.
    fn let_item(
        def_id: hir::DefId,
        hir_id_base: u32,
        pat_name: &str,
        path_name: &str,
    ) -> hir::Item {
        let pattern = hir::Pat {
            hir_id: hid(hir_id_base + 1),
            kind: hir::PatKind::Binding {
                name: pat_name.into(),
                mutable: false,
            },
        };
        let expr = hir::Expr {
            hir_id: hid(hir_id_base + 2),
            kind: hir::ExprKind::Let(
                pattern,
                Box::new(hir::TypeExpr {
                    hir_id: hid(hir_id_base + 3),
                    kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
                        span: Default::default(),
                        segments: vec![hir::PathSegment {
                            ident: path_name.into(),
                            hir_id: Default::default(),
                            args: None,
                            infer_args: true,
                            delegation_child_segment: false,
                            res: hir::Res::Error,
                        }],
                        res: hir::Res::Error,
                    })),
                    span: fp_core::span::Span::null(),
                }),
                None,
            ),
            span: fp_core::span::Span::null(),
        };
        hir::Item {
            hir_id: hid(hir_id_base),
            def_id,
            visibility: hir::Visibility::Private,
            kind: hir::ItemKind::Expr(expr),
            span: fp_core::span::Span::null(),
        }
    }

    let f16_def_id = hir::DefId::local(1);
    let f128_def_id = hir::DefId::local(2);
    let f16_item = let_item(f16_def_id.clone(), 10, "f16_value", "f16");
    let f128_item = let_item(f128_def_id.clone(), 20, "f128_value", "f128");

    let mut program = hir::HirPackage::new(test_pkg());
    program.items.push(f16_item.clone());
    program.items.push(f128_item.clone());
    program.def_map.insert(f16_def_id, f16_item);
    program.def_map.insert(f128_def_id, f128_item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(program, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        results.borrow().pat_type(hid(11)),
        Some(Ty::float(ty::FloatTy::F16)),
        "bare `f16` type path must resolve to the f16 primitive, not an unresolved-path error type"
    );
    assert_eq!(
        results.borrow().pat_type(hid(21)),
        Some(Ty::float(ty::FloatTy::F128)),
        "bare `f128` type path must resolve to the f128 primitive, not an unresolved-path error type"
    );
}

#[test]
fn associated_const_lookup_is_not_filtered_by_expected_value_type() {
    // Rustc resolves an associated item from the receiver and item name
    // first. A conflicting surrounding expectation is diagnosed at the use
    // site; it must not make an existing `f128::INFINITY` declaration look
    // unresolved.
    let package_id = test_pkg();
    let impl_id = hir::DefId::new(package_id.clone(), 30);
    let const_id = hir::DefId::new(package_id.clone(), 31);
    let f128_path = |hir_id: u32| hir::TypeExpr {
        hir_id: hid(hir_id),
        kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: Default::default(),
            segments: vec![hir::PathSegment {
                ident: "f128".into(),
                hir_id: Default::default(),
                args: None,
                infer_args: false,
                delegation_child_segment: false,
                res: hir::Res::Error,
            }],
            res: hir::Res::Builtin(hir::BuiltinSelfType::Primitive("f128".into())),
        })),
        span: fp_core::span::Span::null(),
    };
    let constant = hir::ImplItem {
        def_id: const_id,
        hir_id: hid(32),
        name: "INFINITY".into(),
        kind: hir::ImplItemKind::AssocConst(hir::Const {
            name: "INFINITY".into(),
            ty: f128_path(33),
            body: hir::Body {
                hir_id: hid(34),
                params: Vec::new(),
                value: hir::Expr {
                    hir_id: hid(35),
                    kind: hir::ExprKind::Literal(hir::Lit::Float(0.0)),
                    span: fp_core::span::Span::null(),
                },
            },
            mutable: false,
            is_host: false,
        }),
    };
    let implementation = hir::Item {
        hir_id: hid(36),
        def_id: impl_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Impl(hir::Impl {
            generics: hir::Generics::default(),
            trait_ty: None,
            self_ty: f128_path(37),
            items: vec![constant],
        }),
        span: fp_core::span::Span::null(),
    };
    let mut package = hir::HirPackage::new(package_id);
    package.items.push(implementation.clone());
    package.def_map.insert(impl_id, implementation);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let checker = HirTypeChecker::new(Rc::new(RefCell::new(package)), None, None, executor.clone());
    let result = executor.run(async move {
        let mut checker = checker.borrow_mut();
        checker.expected_expr_type = Some(Ty::bool());
        checker
            .method_declared_signature_at(&Ty::float(ty::FloatTy::F128), &"INFINITY".into())
            .await
            .expect("associated constant lookup should not fail")
    });
    assert_eq!(result, Some(Ty::float(ty::FloatTy::F128)));
}

#[test]
fn generic_associated_const_path_resolves_before_def_id_lookup() {
    // rustc resolves `T::IS_ZST` from T's parameter environment. Its HIR
    // carries `Res::Generic` for T, which has no nominal DefId suitable for
    // the ordinary value-path lookup below it.
    let package_id = test_pkg();
    let trait_id = hir::DefId::new(package_id.clone(), 40);
    let parameter_id = hir::DefId::new(package_id.clone(), 41);
    let bool_ty = |hir_id| hir::TypeExpr {
        hir_id: hid(hir_id),
        kind: hir::TypeExprKind::Primitive(TypePrimitive::Bool),
        span: fp_core::span::Span::null(),
    };
    let trait_path = hir::TypeExpr {
        hir_id: hid(42),
        kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: fp_core::span::Span::null(),
            segments: vec![hir::PathSegment::with_hir_id(
                "Layout",
                hid(43),
                None,
                hir::Res::Def(trait_id.clone()),
                true,
            )],
            res: hir::Res::Def(trait_id.clone()),
        })),
        span: fp_core::span::Span::null(),
    };
    let trait_item = hir::Item {
        hir_id: hid(44),
        def_id: trait_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Trait(hir::Trait {
            generics: hir::Generics::default(),
            items: vec![hir::TraitItem {
                def_id: hir::DefId::new(package_id.clone(), 42),
                hir_id: hid(45),
                name: "IS_ZST".into(),
                kind: hir::TraitItemKind::AssocConst(hir::TraitAssocConst {
                    name: "IS_ZST".into(),
                    ty: bool_ty(46),
                    body: None,
                }),
            }],
            supertraits: Vec::new(),
        }),
        span: fp_core::span::Span::null(),
    };
    let generics = hir::Generics {
        params: vec![hir::GenericParam {
            hir_id: hid(47),
            def_id: parameter_id.clone(),
            name: "T".into(),
            span: fp_core::span::Span::null(),
            pure_wrt_drop: false,
            kind: hir::GenericParamKind::Type {
                default: None,
                synthetic: false,
            },
            colon_span: None,
            source: hir::GenericParamSource::Generics,
            bounds: vec![trait_path],
            explicit_bindings: Vec::new(),
            projection_bounds: Vec::new(),
        }],
        where_clause: None,
        span: fp_core::span::Span::null(),
    };
    let qpath = hir::QPath::resolved(hir::Path {
        span: fp_core::span::Span::null(),
        segments: vec![
            hir::PathSegment::with_hir_id(
                "T",
                hid(48),
                None,
                hir::Res::Generic(parameter_id.clone()),
                true,
            ),
            hir::PathSegment::with_hir_id("IS_ZST", hid(49), None, hir::Res::Error, true),
        ],
        res: hir::Res::Generic(parameter_id),
    });
    let mut package = hir::HirPackage::new(package_id);
    package.items.push(trait_item.clone());
    package.def_map.insert(trait_id, trait_item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let checker = HirTypeChecker::new(Rc::new(RefCell::new(package)), None, None, executor.clone());
    let result = executor.run(async move {
        let mut scope = checker.borrow().with_generics(&generics);
        scope.expr_path_ty(&qpath).await
    });
    assert_eq!(result.expect("generic associated const lookup"), Ty::bool());
}

#[test]
fn generic_associated_const_uses_universal_blanket_impl_default() {
    // `SizedTypeProperties` in core has this shape: a default constant on
    // the trait plus `impl<T> SizedTypeProperties for T {}`. Rustc permits
    // `T::IS_ZST` without an explicit bound because the blanket impl applies
    // to every type parameter.
    let package_id = test_pkg();
    let trait_id = hir::DefId::new(package_id.clone(), 50);
    let impl_id = hir::DefId::new(package_id.clone(), 51);
    let use_parameter_id = hir::DefId::new(package_id.clone(), 52);
    let impl_parameter_id = hir::DefId::new(package_id.clone(), 53);
    let bool_ty = |hir_id| hir::TypeExpr {
        hir_id: hid(hir_id),
        kind: hir::TypeExprKind::Primitive(TypePrimitive::Bool),
        span: fp_core::span::Span::null(),
    };
    let parameter_path = |name: &str, hir_id, def_id: hir::DefId| hir::TypeExpr {
        hir_id: hid(hir_id),
        kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: fp_core::span::Span::null(),
            segments: vec![hir::PathSegment::with_hir_id(
                name,
                hid(hir_id + 1),
                None,
                hir::Res::Generic(def_id.clone()),
                true,
            )],
            res: hir::Res::Generic(def_id),
        })),
        span: fp_core::span::Span::null(),
    };
    let trait_path = hir::TypeExpr {
        hir_id: hid(54),
        kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: fp_core::span::Span::null(),
            segments: vec![hir::PathSegment::with_hir_id(
                "Properties",
                hid(55),
                None,
                hir::Res::Def(trait_id.clone()),
                true,
            )],
            res: hir::Res::Def(trait_id.clone()),
        })),
        span: fp_core::span::Span::null(),
    };
    let trait_item = hir::Item {
        hir_id: hid(56),
        def_id: trait_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Trait(hir::Trait {
            generics: hir::Generics::default(),
            items: vec![hir::TraitItem {
                def_id: hir::DefId::new(package_id.clone(), 54),
                hir_id: hid(57),
                name: "IS_ZST".into(),
                kind: hir::TraitItemKind::AssocConst(hir::TraitAssocConst {
                    name: "IS_ZST".into(),
                    ty: bool_ty(58),
                    body: Some(hir::Body {
                        hir_id: hid(59),
                        params: Vec::new(),
                        value: hir::Expr {
                            hir_id: hid(60),
                            kind: hir::ExprKind::Literal(hir::Lit::Bool(false)),
                            span: fp_core::span::Span::null(),
                        },
                    }),
                }),
            }],
            supertraits: Vec::new(),
        }),
        span: fp_core::span::Span::null(),
    };
    let impl_generics = hir::Generics {
        params: vec![hir::GenericParam {
            hir_id: hid(61),
            def_id: impl_parameter_id.clone(),
            name: "U".into(),
            span: fp_core::span::Span::null(),
            pure_wrt_drop: false,
            kind: hir::GenericParamKind::Type {
                default: None,
                synthetic: false,
            },
            colon_span: None,
            source: hir::GenericParamSource::Generics,
            bounds: Vec::new(),
            explicit_bindings: Vec::new(),
            projection_bounds: Vec::new(),
        }],
        where_clause: None,
        span: fp_core::span::Span::null(),
    };
    let implementation = hir::Item {
        hir_id: hid(62),
        def_id: impl_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Impl(hir::Impl {
            generics: impl_generics,
            trait_ty: Some(trait_path),
            self_ty: parameter_path("U", 63, impl_parameter_id),
            items: Vec::new(),
        }),
        span: fp_core::span::Span::null(),
    };
    let use_generics = hir::Generics {
        params: vec![hir::GenericParam {
            hir_id: hid(65),
            def_id: use_parameter_id.clone(),
            name: "T".into(),
            span: fp_core::span::Span::null(),
            pure_wrt_drop: false,
            kind: hir::GenericParamKind::Type {
                default: None,
                synthetic: false,
            },
            colon_span: None,
            source: hir::GenericParamSource::Generics,
            bounds: Vec::new(),
            explicit_bindings: Vec::new(),
            projection_bounds: Vec::new(),
        }],
        where_clause: None,
        span: fp_core::span::Span::null(),
    };
    let qpath = hir::QPath::resolved(hir::Path {
        span: fp_core::span::Span::null(),
        segments: vec![
            hir::PathSegment::with_hir_id(
                "T",
                hid(66),
                None,
                hir::Res::Generic(use_parameter_id.clone()),
                true,
            ),
            hir::PathSegment::with_hir_id("IS_ZST", hid(67), None, hir::Res::Error, true),
        ],
        res: hir::Res::Generic(use_parameter_id),
    });
    let mut package = hir::HirPackage::new(package_id);
    for item in [trait_item, implementation] {
        package.def_map.insert(item.def_id.clone(), item.clone());
        package.items.push(item);
    }

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let checker = HirTypeChecker::new(Rc::new(RefCell::new(package)), None, None, executor.clone());
    let result = executor.run(async move {
        let mut scope = checker.borrow().with_generics(&use_generics);
        scope.expr_path_ty(&qpath).await
    });
    assert_eq!(result.expect("blanket associated const lookup"), Ty::bool());
}

#[test]
fn lifetime_arguments_do_not_shift_nominal_type_arguments() {
    let package_id = test_pkg();
    let wrapper_id = hir::DefId::new(package_id.clone(), 2);
    let wrapper_param_id = hir::DefId::new(package_id.clone(), 3);
    let value_id = hir::DefId::new(package_id.clone(), 1);
    let wrapper_generics = hir::Generics {
        params: vec![hir::GenericParam {
            hir_id: hid(30),
            def_id: wrapper_param_id,
            name: "T".into(),
            span: fp_core::span::Span::null(),
            pure_wrt_drop: false,
            kind: hir::GenericParamKind::Type {
                default: None,
                synthetic: false,
            },
            colon_span: None,
            source: hir::GenericParamSource::Generics,
            bounds: Vec::new(),
            explicit_bindings: Vec::new(),
            projection_bounds: Vec::new(),
        }],
        where_clause: None,
        span: fp_core::span::Span::null(),
    };
    let wrapper = hir::Item {
        hir_id: hid(2),
        def_id: wrapper_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Struct(hir::Struct {
            name: "Wrapper".into(),
            fields: Vec::new(),
            generics: wrapper_generics,
            repr: fp_core::ast::ReprOptions::default(),
        }),
        span: fp_core::span::Span::null(),
    };
    let wrapper_path = hir::TypeExpr {
        hir_id: hid(11),
        kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: Default::default(),
            segments: vec![hir::PathSegment {
                ident: "Wrapper".into(),
                hir_id: Default::default(),
                args: Some(hir::GenericArgs {
                    args: vec![
                        hir::GenericArg::Lifetime("'a".into()),
                        hir::GenericArg::Type(Box::new(hir::TypeExpr {
                            hir_id: hid(12),
                            kind: hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I32)),
                            span: fp_core::span::Span::null(),
                        })),
                    ],
                    constraints: Vec::new(),
                    parenthesized: hir::GenericArgsParentheses::No,
                    span_ext: fp_core::span::Span::null(),
                }),
                infer_args: false,
                delegation_child_segment: false,
                res: hir::Res::Def(wrapper_id.clone()),
            }],
            res: hir::Res::Def(wrapper_id.clone()),
        })),
        span: fp_core::span::Span::null(),
    };
    let item = hir::Item {
        hir_id: hid(1),
        def_id: value_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Expr(hir::Expr {
            hir_id: hid(13),
            kind: hir::ExprKind::Let(
                hir::Pat {
                    hir_id: hid(14),
                    kind: hir::PatKind::Binding {
                        name: "value".into(),
                        mutable: false,
                    },
                },
                Box::new(wrapper_path),
                None,
            ),
            span: fp_core::span::Span::null(),
        }),
        span: fp_core::span::Span::null(),
    };
    let mut package = hir::HirPackage::new(package_id);
    package.items.extend([wrapper.clone(), item.clone()]);
    package.def_map.insert(wrapper_id, wrapper);
    package.def_map.insert(value_id, item);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let results = executor
        .run(typecheck_program(package, executor.clone()))
        .expect("HIR type check");
    let actual = results.borrow().pat_type(hid(14));
    let Some(Ty {
        kind: TyKind::Adt(_, args),
    }) = actual.as_ref()
    else {
        panic!("expected Wrapper<'a, i32> to resolve as an ADT, got {actual:?}");
    };
    assert_eq!(
        args.len(),
        1,
        "erased lifetimes must not occupy ADT arg slots"
    );
    assert_eq!(args[0], ty::GenericArg::Type(Ty::int(ty::IntTy::I32)));
}

#[test]
fn typed_command_helper_local_preserves_method_def_identity() {
    let package_id = test_pkg();
    let command_id = hir::DefId::new(package_id.clone(), 1);
    let helper_id = hir::DefId::new(package_id.clone(), 2);
    let impl_id = hir::DefId::new(package_id.clone(), 3);
    let output_id = hir::DefId::new(package_id.clone(), 4);
    let new_id = hir::DefId::new(package_id.clone(), 6);
    let caller_id = hir::DefId::new(package_id.clone(), 5);
    let output_call_hir_id = hid(50);
    let command_path = || hir::TypeExpr {
        hir_id: hid(60),
        kind: hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: Default::default(),
            segments: vec![hir::PathSegment {
                ident: "Command".into(),
                hir_id: Default::default(),
                args: None,
                infer_args: true,
                delegation_child_segment: false,
                res: hir::Res::Def(command_id.clone()),
            }],
            res: hir::Res::Def(command_id.clone()),
        })),
        span: fp_core::span::Span::null(),
    };
    let unit_ty = || hir::TypeExpr {
        hir_id: hid(61),
        kind: hir::TypeExprKind::Tuple(Vec::new()),
        span: fp_core::span::Span::null(),
    };
    let function = |name: &str, output: hir::TypeExpr, body: hir::Block| hir::Function {
        sig: hir::FunctionSig {
            name: name.into(),
            inputs: Vec::new(),
            output,
            generics: hir::Generics::default(),
            abi: ty::Abi::Rust,
        },
        body: Some(body),
        is_const: false,
        is_extern: false,
        is_async: false,
        attrs: Vec::new(),
    };
    let command = hir::Item {
        hir_id: hid(1),
        def_id: command_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Struct(hir::Struct {
            name: "Command".into(),
            fields: Vec::new(),
            generics: hir::Generics::default(),
            repr: fp_core::ast::ReprOptions::default(),
        }),
        span: fp_core::span::Span::null(),
    };
    let helper = hir::Item {
        hir_id: hid(2),
        def_id: helper_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Function(function(
            "helper",
            command_path(),
            hir::Block {
                hir_id: hid(20),
                stmts: Vec::new(),
                expr: Some(Box::new(hir::Expr {
                    hir_id: hid(21),
                    kind: hir::ExprKind::Struct(
                        hir::QPath::resolved(hir::Path {
                            span: Default::default(),
                            segments: vec![hir::PathSegment {
                                ident: "Command".into(),
                                hir_id: Default::default(),
                                args: None,
                                infer_args: true,
                                delegation_child_segment: false,
                                res: hir::Res::Def(command_id.clone()),
                            }],
                            res: hir::Res::Def(command_id.clone()),
                        }),
                        Vec::new(),
                    ),
                    span: fp_core::span::Span::null(),
                })),
            },
        )),
        span: fp_core::span::Span::null(),
    };
    let impl_item = hir::Item {
        hir_id: hid(3),
        def_id: impl_id,
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Impl(hir::Impl {
            generics: hir::Generics::default(),
            trait_ty: None,
            self_ty: command_path(),
            items: vec![
                hir::ImplItem {
                    def_id: output_id.clone(),
                    hir_id: hid(30),
                    name: "output".into(),
                    kind: hir::ImplItemKind::Method({
                        let mut method = function(
                            "output",
                            unit_ty(),
                            hir::Block {
                                hir_id: hid(31),
                                stmts: Vec::new(),
                                expr: Some(Box::new(hir::Expr {
                                    hir_id: hid(32),
                                    kind: hir::ExprKind::Tuple(Vec::new()),
                                    span: fp_core::span::Span::null(),
                                })),
                            },
                        );
                        method.sig.inputs.push(hir::Param {
                            hir_id: hid(33),
                            pat: hir::Pat {
                                hir_id: hid(34),
                                kind: hir::PatKind::Binding {
                                    name: "self".into(),
                                    mutable: true,
                                },
                            },
                            ty: hir::TypeExpr {
                                hir_id: hid(35),
                                kind: hir::TypeExprKind::Ref(Box::new(command_path())),
                                span: fp_core::span::Span::null(),
                            },
                            is_context: false,
                            as_tuple: false,
                            as_dict: false,
                            default: None,
                        });
                        method
                    }),
                },
                hir::ImplItem {
                    def_id: new_id.clone(),
                    hir_id: hid(36),
                    name: "new".into(),
                    kind: hir::ImplItemKind::Method(function(
                        "new",
                        command_path(),
                        hir::Block {
                            hir_id: hid(37),
                            stmts: Vec::new(),
                            expr: Some(Box::new(hir::Expr {
                                hir_id: hid(38),
                                kind: hir::ExprKind::Struct(
                                    hir::QPath::resolved(hir::Path {
                                        span: Default::default(),
                                        segments: vec![hir::PathSegment {
                                            ident: "Command".into(),
                                            hir_id: Default::default(),
                                            args: None,
                                            infer_args: true,
                                            delegation_child_segment: false,
                                            res: hir::Res::Def(command_id.clone()),
                                        }],
                                        res: hir::Res::Def(command_id.clone()),
                                    }),
                                    Vec::new(),
                                ),
                                span: fp_core::span::Span::null(),
                            })),
                        },
                    )),
                },
            ],
        }),
        span: fp_core::span::Span::null(),
    };
    let caller = hir::Item {
        hir_id: hid(4),
        def_id: caller_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Function(function(
            "caller",
            unit_ty(),
            hir::Block {
                hir_id: hid(40),
                stmts: vec![
                    hir::Stmt {
                        hir_id: hid(41),
                        kind: hir::StmtKind::Local(hir::Local {
                            hir_id: hid(42),
                            pat: hir::Pat {
                                hir_id: hid(43),
                                kind: hir::PatKind::Binding {
                                    name: "cmd".into(),
                                    mutable: true,
                                },
                            },
                            ty: Some(command_path()),
                            init: Some(hir::Expr {
                                hir_id: hid(44),
                                kind: hir::ExprKind::Call(
                                    Box::new(hir::Expr {
                                        hir_id: hid(45),
                                        kind: hir::ExprKind::Path(hir::QPath::resolved(
                                            hir::Path {
                                                span: Default::default(),
                                                segments: vec![hir::PathSegment {
                                                    ident: "helper".into(),
                                                    hir_id: Default::default(),
                                                    args: None,
                                                    infer_args: true,
                                                    delegation_child_segment: false,
                                                    res: hir::Res::Def(helper_id.clone()),
                                                }],
                                                res: hir::Res::Def(helper_id),
                                            },
                                        )),
                                        span: fp_core::span::Span::null(),
                                    }),
                                    Vec::new(),
                                ),
                                span: fp_core::span::Span::null(),
                            }),
                        }),
                    },
                    hir::Stmt {
                        hir_id: hid(46),
                        kind: hir::StmtKind::Local(hir::Local {
                            hir_id: hid(47),
                            pat: hir::Pat {
                                hir_id: hid(48),
                                kind: hir::PatKind::Binding {
                                    name: "created".into(),
                                    mutable: false,
                                },
                            },
                            ty: None,
                            init: Some(hir::Expr {
                                hir_id: hid(49),
                                kind: hir::ExprKind::Call(
                                    Box::new(hir::Expr {
                                        hir_id: hid(52),
                                        kind: hir::ExprKind::Path(hir::QPath::resolved(
                                            hir::Path {
                                                span: Default::default(),
                                                segments: vec![
                                                    hir::PathSegment {
                                                        ident: "Command".into(),
                                                        hir_id: Default::default(),
                                                        args: None,
                                                        infer_args: true,
                                                        delegation_child_segment: false,
                                                        res: hir::Res::Def(command_id.clone()),
                                                    },
                                                    hir::PathSegment {
                                                        ident: "new".into(),
                                                        hir_id: Default::default(),
                                                        args: None,
                                                        infer_args: true,
                                                        delegation_child_segment: false,
                                                        res: hir::Res::Error,
                                                    },
                                                ],
                                                // Type-relative resolution is deliberately the type;
                                                // typeck must record `new_id` on the enclosing call.
                                                res: hir::Res::Def(command_id.clone()),
                                            },
                                        )),
                                        span: fp_core::span::Span::null(),
                                    }),
                                    Vec::new(),
                                ),
                                span: fp_core::span::Span::null(),
                            }),
                        }),
                    },
                ],
                expr: Some(Box::new(hir::Expr {
                    hir_id: output_call_hir_id.clone(),
                    kind: hir::ExprKind::MethodCall(
                        Box::new(hir::Expr {
                            hir_id: hid(51),
                            kind: hir::ExprKind::Path(hir::QPath::resolved(hir::Path {
                                span: Default::default(),
                                segments: vec![hir::PathSegment {
                                    ident: "cmd".into(),
                                    hir_id: Default::default(),
                                    args: None,
                                    infer_args: true,
                                    delegation_child_segment: false,
                                    res: hir::Res::Local(hid(43)),
                                }],
                                res: hir::Res::Local(hid(43)),
                            })),
                            span: fp_core::span::Span::null(),
                        }),
                        "output".into(),
                        Some(hir::GenericArgs {
                            args: vec![hir::GenericArg::Lifetime("'a".into())],
                            constraints: Vec::new(),
                            parenthesized: hir::GenericArgsParentheses::No,
                            span_ext: fp_core::span::Span::null(),
                        }),
                        Vec::new(),
                    ),
                    span: fp_core::span::Span::null(),
                })),
            },
        )),
        span: fp_core::span::Span::null(),
    };

    let mut package = hir::HirPackage::new(package_id);
    for item in [command, helper, impl_item, caller] {
        package.def_map.insert(item.def_id.clone(), item.clone());
        package.items.push(item);
    }
    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let result = executor
        .run(typecheck_program(package, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        result.borrow().method_resolution(output_call_hir_id),
        Some(output_id),
        "an erased lifetime argument must not prevent method resolution"
    );
    assert_eq!(
        result.borrow().method_resolution(hid(49)),
        Some(new_id),
        "a type-relative associated call must retain its selected impl member DefId"
    );
}

#[test]
fn comptime_request_returns_resolver_value_directly() {
    let resolver: ComptimeResolver =
        Rc::new(|_program, _request| Box::pin(async { Ok(fp_core::ast::Value::unit()) }));
    let package = Rc::new(RefCell::new(hir::HirPackage::new(test_pkg())));
    let checker = HirTypeChecker::new(
        package,
        None,
        Some(resolver),
        fp_core::executor::CompilerExecutor::new().handle(),
    );
    let request = ComptimeRequest {
        package_id: test_pkg(),
        def_id: hir::DefId::new(test_pkg(), 0),
    };
    let mut future = Box::pin(async move { checker.borrow().request_comptime(request).await });
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let value = match future.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(result) => result.expect("comptime value"),
        std::task::Poll::Pending => {
            panic!("resolver-backed comptime request should resolve immediately")
        }
    };
    assert!(value.is_unit());
}

#[test]
fn generic_type_alias_is_checked_and_substituted() {
    let alias_id = hir::DefId::new(test_pkg(), 1);
    let parameter_id = hir::DefId::new(test_pkg(), 2);
    let function_id = hir::DefId::new(test_pkg(), 3);
    let parameter = hir::GenericParam {
        hir_id: hid(1),
        def_id: parameter_id.clone(),
        name: "T".into(),
        span: fp_core::span::Span::null(),
        pure_wrt_drop: false,
        kind: hir::GenericParamKind::Type {
            default: None,
            synthetic: false,
        },
        colon_span: None,
        source: hir::GenericParamSource::Generics,
        bounds: Vec::new(),
        explicit_bindings: Vec::new(),
        projection_bounds: Vec::new(),
    };
    let alias_target = hir::TypeExpr::new(
        hid(2),
        hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: fp_core::span::Span::null(),
            res: hir::Res::Generic(parameter_id.clone()),
            segments: vec![hir::PathSegment::with_hir_id(
                "T",
                hid(3),
                None,
                hir::Res::Generic(parameter_id.clone()),
                true,
            )],
        })),
        fp_core::span::Span::null(),
    );
    let alias = hir::Item {
        hir_id: hid(4),
        def_id: alias_id.clone(),
        visibility: hir::Visibility::Public,
        kind: hir::ItemKind::TypeAlias(hir::TypeAlias {
            name: "Alias".into(),
            generics: hir::Generics {
                params: vec![parameter],
                where_clause: None,
                span: fp_core::span::Span::null(),
            },
            target: alias_target,
        }),
        span: fp_core::span::Span::null(),
    };
    let alias_args = hir::GenericArgs {
        args: vec![hir::GenericArg::Type(Box::new(hir::TypeExpr::new(
            hid(5),
            hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
            fp_core::span::Span::null(),
        )))],
        constraints: Vec::new(),
        parenthesized: hir::GenericArgsParentheses::No,
        span_ext: fp_core::span::Span::null(),
    };
    let function_input = hir::TypeExpr::new(
        hid(6),
        hir::TypeExprKind::Path(hir::QPath::resolved(hir::Path {
            span: fp_core::span::Span::null(),
            res: hir::Res::Def(alias_id.clone()),
            segments: vec![hir::PathSegment::with_hir_id(
                "Alias",
                hid(7),
                Some(alias_args),
                hir::Res::Def(alias_id.clone()),
                false,
            )],
        })),
        fp_core::span::Span::null(),
    );
    let output = hir::TypeExpr::new(
        hid(8),
        hir::TypeExprKind::Primitive(TypePrimitive::Int(TypeInt::I64)),
        fp_core::span::Span::null(),
    );
    let function = hir::Item {
        hir_id: hid(9),
        def_id: function_id.clone(),
        visibility: hir::Visibility::Private,
        kind: hir::ItemKind::Function(hir::Function {
            sig: hir::FunctionSig {
                name: "read".into(),
                inputs: vec![hir::Param {
                    hir_id: hid(10),
                    pat: hir::Pat {
                        hir_id: hid(11),
                        kind: hir::PatKind::Binding {
                            name: "value".into(),
                            mutable: false,
                        },
                    },
                    ty: function_input,
                    is_context: false,
                    as_tuple: false,
                    as_dict: false,
                    default: None,
                }],
                output,
                generics: hir::Generics {
                    params: Vec::new(),
                    where_clause: None,
                    span: fp_core::span::Span::null(),
                },
                abi: hir::Abi::Rust,
            },
            body: None,
            is_const: false,
            is_extern: false,
            is_async: false,
            attrs: Vec::new(),
        }),
        span: fp_core::span::Span::null(),
    };
    let mut package = hir::HirPackage::new(test_pkg());
    package.items.extend([alias.clone(), function.clone()]);
    package.def_map.insert(alias_id, alias);
    package.def_map.insert(function_id, function);

    let executor = fp_core::executor::CompilerExecutor::new().handle();
    let result = executor
        .run(typecheck_program(package, executor.clone()))
        .expect("HIR type check");
    assert_eq!(
        result.borrow().type_expr_type(hid(6)),
        Some(Ty::int(ty::IntTy::I64))
    );
}
