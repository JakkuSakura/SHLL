use common::{bail, Itertools, Result};
use common_lang::ast::Ident;
use common_lang::value::{EnumTypeVariant, TypeEnum, TypeValue};
use common_lang::Serializer;
use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::rc::Rc;
use swc_ecma_ast::{Script, TsEnumDecl, TsEnumMemberId};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_quote::swc_common::sync::Lrc;
use swc_ecma_quote::swc_common::{SourceMap, DUMMY_SP};
#[derive(Clone)]
struct SharedWriter {
    wr: Rc<RefCell<Cursor<Vec<u8>>>>,
}
impl SharedWriter {
    pub fn new() -> Self {
        Self {
            wr: Rc::new(RefCell::new(Cursor::new(Vec::new()))),
        }
    }
    pub fn take_string(&self) -> String {
        let vec = std::mem::take(self.wr.borrow_mut().get_mut());
        String::from_utf8(vec).unwrap()
    }
}
impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.wr.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.wr.borrow_mut().flush()
    }
}
pub struct TsPrinter {
    emitter: RefCell<Emitter<'static, JsWriter<'static, SharedWriter>, SourceMap>>,
    writer: SharedWriter,
}
impl TsPrinter {
    pub fn new() -> Self {
        let writer = SharedWriter::new();
        let source_map = Lrc::new(SourceMap::default());
        Self {
            emitter: RefCell::new(Emitter {
                cfg: Default::default(),
                cm: source_map.clone(),
                comments: None,
                wr: JsWriter::new(source_map, "\n", writer.clone(), None),
            }),
            writer,
        }
    }
    pub fn to_ident(&self, name: &Ident) -> swc_ecma_ast::Ident {
        swc_ecma_ast::Ident::new(name.name.as_str().into(), DUMMY_SP)
    }

    pub fn to_enum_member(&self, name: &EnumTypeVariant) -> Result<swc_ecma_ast::TsEnumMember> {
        Ok(swc_ecma_ast::TsEnumMember {
            span: Default::default(),
            id: TsEnumMemberId::Ident(self.to_ident(&name.name)),
            // TODO: deal with init
            init: None,
        })
    }
    pub fn to_enum(&self, decl: &TypeEnum) -> Result<TsEnumDecl> {
        Ok(TsEnumDecl {
            span: DUMMY_SP,
            declare: true,
            id: self.to_ident(&decl.name),
            is_const: true,
            members: decl
                .variants
                .iter()
                .map(|x| self.to_enum_member(x))
                .try_collect()?,
        })
    }
    pub fn print_script(&self, script: &Script) -> Result<String> {
        self.emitter.borrow_mut().emit_script(script)?;
        Ok(self.writer.take_string())
    }
}
impl Serializer for TsPrinter {
    fn serialize_type(&self, node: &TypeValue) -> Result<String> {
        match node {
            TypeValue::Enum(decl) => {
                let decl = self.to_enum(decl)?;

                self.print_script(&Script {
                    span: Default::default(),
                    body: vec![swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::TsEnum(
                        decl.into(),
                    ))],
                    shebang: None,
                })
            }
            _ => bail!("Not implemented"),
        }
    }
}
