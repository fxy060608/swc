use std::{self, collections::HashMap, env::current_dir, fs::read_dir, io, path::PathBuf};

use anyhow::Error;
use relative_path::RelativePath;
use swc_atoms::js_word;
use swc_common::{sync::Lrc, FileName, FilePathMapping, Globals, SourceMap, Span};
use swc_ecma_ast::{
    Bool, Expr, Ident, KeyValueProp, Lit, MemberExpr, MemberProp, MetaPropExpr, MetaPropKind,
    PropName, Str,
};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_loader::NODE_BUILTINS;
use swc_ecma_transforms_base::fixer::fixer;
use swc_ecma_visit::FoldWith;
use testing::NormalizedOutput;

use self::common::*;
use crate::{BundleKind, Bundler, Config, ModuleRecord};

#[path = "common/mod.rs"]
mod common;

#[test]
fn bundle_test() {
    let pattern = "tests/uts/input";
    let base_dir = current_dir().unwrap();
    println!("{:?}", base_dir);
    let entry = RelativePath::new(&pattern).to_path(&base_dir);
    println!("{:?}", entry);
    let entries = read_dir(&entry)
        .unwrap()
        .filter(|e| match e {
            Ok(e) => e
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("entry"),
            _ => false,
        })
        .map(|e| -> Result<_, io::Error> {
            let e = e?;
            Ok((
                e.file_name().to_string_lossy().to_string(),
                FileName::Real(e.path()),
            ))
        })
        .collect::<Result<HashMap<_, _>, _>>()
        .unwrap();
    println!("{:?}", entries);
    let globals = Globals::default();
    let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
    let mut bundler = Bundler::new(
        &globals,
        cm.clone(),
        Loader { cm: cm.clone() },
        NodeResolver,
        Config {
            require: true,
            disable_inliner: false,
            external_modules: NODE_BUILTINS.iter().copied().map(From::from).collect(),
            module: Default::default(),
            ..Default::default()
        },
        Box::new(Hook),
    );

    let modules = bundler
        .bundle(entries)
        .map_err(|err| println!("{:?}", err))
        .unwrap();

    println!("Bundled as {} modules", modules.len());

    for bundled in modules {
        let code = {
            let mut buf = vec![];

            {
                let mut emitter = Emitter {
                    cfg: swc_ecma_codegen::Config {
                        ..Default::default()
                    },
                    cm: cm.clone(),
                    comments: None,
                    wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None)),
                };

                emitter
                    .emit_module(&bundled.module.fold_with(&mut fixer(None)))
                    .unwrap();
            }

            String::from_utf8_lossy(&buf).to_string()
        };

        let name = match bundled.kind {
            BundleKind::Named { name } | BundleKind::Lib { name } => PathBuf::from(name),
            BundleKind::Dynamic => format!("dynamic.{}.js", bundled.id).into(),
        };

        let output_dir = entry.join("../output");

        let output_path = output_dir.join(name.file_name().unwrap());

        println!("Printing {}", output_path.display());
        println!("{}", code);

        let s = NormalizedOutput::from(code.to_string());

        match s.compare_to_file(&output_path) {
            Ok(_) => {}
            Err(err) => {
                println!("Diff: {:?}", err);
            }
        }
    }
}

struct Hook;

impl crate::Hook for Hook {
    fn get_import_meta_props(
        &self,
        span: Span,
        module_record: &ModuleRecord,
    ) -> Result<Vec<KeyValueProp>, Error> {
        let file_name = module_record.file_name.to_string();

        Ok(vec![
            KeyValueProp {
                key: PropName::Ident(Ident::new(js_word!("url"), span)),
                value: Box::new(Expr::Lit(Lit::Str(Str {
                    span,
                    raw: None,
                    value: file_name.into(),
                }))),
            },
            KeyValueProp {
                key: PropName::Ident(Ident::new(js_word!("main"), span)),
                value: Box::new(if module_record.is_entry {
                    Expr::Member(MemberExpr {
                        span,
                        obj: Box::new(Expr::MetaProp(MetaPropExpr {
                            span,
                            kind: MetaPropKind::ImportMeta,
                        })),
                        prop: MemberProp::Ident(Ident::new(js_word!("main"), span)),
                    })
                } else {
                    Expr::Lit(Lit::Bool(Bool { span, value: false }))
                }),
            },
        ])
    }
}
