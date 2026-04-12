use rquickjs::{Ctx, Function};
use wasmtime::ExternType;

use super::WASM_MODULES;

fn valtype_str(vt: &wasmtime::ValType) -> &'static str {
    match vt {
        wasmtime::ValType::I32 => "i32",
        wasmtime::ValType::I64 => "i64",
        wasmtime::ValType::F32 => "f32",
        wasmtime::ValType::F64 => "f64",
        wasmtime::ValType::V128 => "v128",
        wasmtime::ValType::Ref(_) => "externref",
    }
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // __braille_wasm_module_exports(module_id) -> JSON string
    g.set(
        "__braille_wasm_module_exports",
        Function::new(ctx.clone(), |module_id: u32| -> String {
            WASM_MODULES.with(|wm| {
                let modules = wm.borrow();
                let module = match modules.get(&module_id) {
                    Some(m) => m,
                    None => return "[]".to_string(),
                };
                let exports: Vec<String> = module
                    .exports()
                    .map(|e| {
                        let kind = match e.ty() {
                            ExternType::Func(_) => "function",
                            ExternType::Table(_) => "table",
                            ExternType::Memory(_) => "memory",
                            ExternType::Global(_) => "global",
                            ExternType::Tag(_) => "tag",
                        };
                        format!(r#"{{"name":{},"kind":"{}"}}"#, serde_json::to_string(e.name()).unwrap(), kind)
                    })
                    .collect();
                format!("[{}]", exports.join(","))
            })
        })
        .unwrap(),
    )
    .unwrap();

    // __braille_wasm_module_imports(module_id) -> JSON string
    g.set(
        "__braille_wasm_module_imports",
        Function::new(ctx.clone(), |module_id: u32| -> String {
            WASM_MODULES.with(|wm| {
                let modules = wm.borrow();
                let module = match modules.get(&module_id) {
                    Some(m) => m,
                    None => return "[]".to_string(),
                };
                let imports: Vec<String> = module
                    .imports()
                    .map(|i| {
                        let kind = match i.ty() {
                            ExternType::Func(ft) => {
                                let params: Vec<&str> = ft.params().map(|vt| valtype_str(&vt)).collect();
                                let results: Vec<&str> = ft.results().map(|vt| valtype_str(&vt)).collect();
                                format!(r#""function","params":[{}],"results":[{}]"#,
                                    params.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(","),
                                    results.iter().map(|r| format!("\"{r}\"")).collect::<Vec<_>>().join(","))
                            }
                            ExternType::Table(_) => "\"table\"".to_string(),
                            ExternType::Memory(_) => "\"memory\"".to_string(),
                            ExternType::Global(gt) => {
                                let vt = valtype_str(gt.content());
                                let mutable = gt.mutability() == wasmtime::Mutability::Var;
                                format!(r#""global","valtype":"{vt}","mutable":{mutable}"#)
                            }
                            ExternType::Tag(_) => "\"tag\"".to_string(),
                        };
                        format!(
                            r#"{{"module":{},"name":{},"kind":{kind}}}"#,
                            serde_json::to_string(i.module()).unwrap(),
                            serde_json::to_string(i.name()).unwrap()
                        )
                    })
                    .collect();
                format!("[{}]", imports.join(","))
            })
        })
        .unwrap(),
    )
    .unwrap();
}
