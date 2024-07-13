use std::sync::Arc;

use common::*;
use lang_core::ast::*;
use lang_core::ast::{AstItem, ItemDefFunction, ItemImpl, Visibility};
use lang_core::ast::{AstType, FunctionParam, FunctionSignature, TypePrimitive, ValueFunction};
use lang_core::id::Locator;
use lang_core::register_threadlocal_serializer;
use pretty_assertions::assert_eq;
use rust_lang::{shll_parse_expr, shll_parse_item, RustSerde};

#[test]
fn test_parse_fn() -> Result<()> {
    register_threadlocal_serializer(Arc::new(RustSerde::new()));
    let code = shll_parse_item! {
        fn foo(a: i64) -> i64 {
            a + 1
        }
    };
    let block = shll_parse_expr! {
        a + 1
    };
    assert_eq!(
        code,
        AstItem::DefFunction(ItemDefFunction {
            name: "foo".into(),
            ty: None,
            value: ValueFunction {
                sig: FunctionSignature {
                    name: Some("foo".into()),
                    params: vec![FunctionParam {
                        name: "a".into(),
                        ty: AstType::Primitive(TypePrimitive::i64())
                    }],
                    generics_params: vec![],
                    ret: AstType::Primitive(TypePrimitive::i64())
                },
                body: block.into(),
            },
            visibility: Visibility::Private,
        })
    );
    Ok(())
}
#[test]
fn test_parse_impl_for() -> Result<()> {
    register_threadlocal_serializer(Arc::new(RustSerde::new()));

    let code = shll_parse_item! {
        impl Foo for Bar {
            fn foo(a: i64) -> i64 {
                a + 1
            }
        }
    };
    assert_eq!(
        code,
        AstItem::Impl(ItemImpl {
            trait_ty: Some(Locator::Ident("Foo".into())),
            self_ty: AstExpr::ident("Bar".into()),
            items: vec![shll_parse_item! {
                fn foo(a: i64) -> i64 {
                    a + 1
                }
            }],
        })
    );
    Ok(())
}
