use eyre::{bail, ContextCompat};
use itertools::Itertools;
use lang_core::ast::{
    DefEnum, DefFunction, DefStruct, DefTrait, DefType, EnumTypeVariant, Expr, ExprSelfType,
    FunctionParam, Import, Item, Type, TypeEnum, TypeStruct,
};
use lang_core::id::{Ident, Path};
use lang_core::utils::anybox::AnyBox;
use syn::{Fields, FnArg, ReturnType};

use crate::parser::parse_fn;
use crate::{parser, RawItemMacro, RawUse};

pub fn parse_fn_arg(i: FnArg) -> eyre::Result<FunctionParam> {
    Ok(match i {
        FnArg::Receiver(rev) => FunctionParam {
            name: Ident::new("self"),
            ty: {
                Type::expr(Expr::SelfType(
                    ExprSelfType {
                        reference: rev.reference.is_some(),
                        mutability: rev.mutability.is_some(),
                    }
                    .into(),
                ))
            },
        },

        FnArg::Typed(t) => FunctionParam {
            name: parser::parse_pat(*t.pat)?
                .as_ident()
                .context("No ident")?
                .clone(),
            ty: parser::parse_type_value(*t.ty)?,
        },
    })
}

pub fn parse_return_type(o: ReturnType) -> eyre::Result<Type> {
    Ok(match o {
        ReturnType::Default => Type::unit(),
        ReturnType::Type(_, t) => parser::parse_type_value(*t)?,
    })
}

pub fn parse_use(u: syn::ItemUse) -> eyre::Result<Import, RawUse> {
    let mut segments = vec![];
    let mut tree = u.tree.clone();
    loop {
        match tree {
            syn::UseTree::Path(p) => {
                segments.push(parser::parse_ident(p.ident));
                tree = *p.tree;
            }
            syn::UseTree::Name(name) => {
                segments.push(parser::parse_ident(name.ident));
                break;
            }
            syn::UseTree::Glob(_) => {
                segments.push(Ident::new("*"));
                break;
            }
            _ => return Err(RawUse { raw: u }.into()),
        }
    }
    Ok(Import {
        visibility: parser::parse_vis(u.vis),
        path: Path::new(segments),
    })
}

pub fn parse_item_struct(s: syn::ItemStruct) -> eyre::Result<TypeStruct> {
    Ok(TypeStruct {
        name: parser::parse_ident(s.ident),
        fields: s
            .fields
            .into_iter()
            .enumerate()
            .map(|(i, f)| parser::parse_struct_field(i, f))
            .try_collect()?,
    })
}

fn parse_trait(t: syn::ItemTrait) -> eyre::Result<DefTrait> {
    // TODO: generis params
    let bounds = parser::parse_type_param_bounds(t.supertraits.into_iter().collect())?;
    let vis = parser::parse_vis(t.vis);
    Ok(DefTrait {
        name: parser::parse_ident(t.ident),
        bounds,
        items: t
            .items
            .into_iter()
            .map(|x| parser::parse_trait_item(x))
            .try_collect()?,
        visibility: vis,
    })
}

pub fn parse_item(item: syn::Item) -> eyre::Result<Item> {
    match item {
        syn::Item::Fn(f0) => {
            let visibility = parser::parse_vis(f0.vis.clone());
            let f = parse_fn(f0)?;
            let d = DefFunction {
                name: f.name.clone().unwrap(),
                ty: None,
                value: f,
                visibility,
            };
            Ok(Item::DefFunction(d))
        }
        syn::Item::Impl(im) => Ok(Item::Impl(parser::parse_impl(im)?)),
        syn::Item::Use(u) => Ok(match parse_use(u) {
            Ok(i) => Item::Import(i),
            Err(r) => Item::Any(AnyBox::new(r)),
        }),
        syn::Item::Macro(m) => Ok(Item::any(RawItemMacro { raw: m })),
        syn::Item::Struct(s) => {
            let visibility = parser::parse_vis(s.vis.clone());

            let struct_type = parse_item_struct(s)?;
            Ok(Item::DefStruct(DefStruct {
                name: struct_type.name.clone(),
                value: struct_type,
                visibility,
            }))
        }
        syn::Item::Enum(e) => {
            let visibility = parser::parse_vis(e.vis.clone());
            let ident = parser::parse_ident(e.ident.clone());
            let variants = e
                .variants
                .into_iter()
                .map(|x| {
                    let name = parser::parse_ident(x.ident);
                    let ty = match x.fields {
                        Fields::Named(_) => bail!("Does not support named fields"),
                        Fields::Unnamed(_) => bail!("Does not support unnamed fields"),
                        Fields::Unit => {
                            // be int or string
                            Type::any()
                        }
                    };
                    Ok(EnumTypeVariant { name, value: ty })
                })
                .try_collect()?;
            Ok(Item::DefEnum(DefEnum {
                name: ident.clone(),
                value: TypeEnum {
                    name: ident.clone(),
                    variants,
                },
                visibility,
            }))
        }
        syn::Item::Type(t) => {
            let visibility = parser::parse_vis(t.vis.clone());
            let ty = parser::parse_type_value(*t.ty)?;
            Ok(Item::DefType(DefType {
                name: parser::parse_ident(t.ident),
                value: ty,
                visibility,
            }))
        }
        syn::Item::Mod(m) => Ok(Item::Module(parser::parse_module(m)?)),
        syn::Item::Trait(t) => {
            let trait_ = parse_trait(t)?;
            Ok(Item::DefTrait(trait_))
        }
        _ => bail!("Does not support item yet: {:?}", item),
    }
}
