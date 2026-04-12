use rquickjs::{Ctx, Function};
use wasmtime::{GlobalType, Mutability, Val, ValType};

use super::{ensure_store, next_id, WASM_GLOBALS, WASM_STORE};

fn parse_val(valtype: &str, value: f64) -> Val {
    match valtype {
        "i32" => Val::I32(value as i32),
        "i64" => Val::I64(value as i64),
        "f32" => Val::F32((value as f32).to_bits()),
        "f64" => Val::F64(value.to_bits()),
        _ => Val::I32(0),
    }
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_wasm_global_new",
        Function::new(ctx.clone(), |valtype: String, mutable: bool, value: f64| -> i32 {
            if ensure_store().is_err() {
                return -1;
            }
            let vt = match valtype.as_str() {
                "i32" => ValType::I32,
                "i64" => ValType::I64,
                "f32" => ValType::F32,
                "f64" => ValType::F64,
                _ => return -1,
            };
            let mutability = if mutable { Mutability::Var } else { Mutability::Const };
            let global_type = GlobalType::new(vt, mutability);
            let init_val = parse_val(&valtype, value);
            WASM_STORE.with(|s| {
                let mut store = s.borrow_mut();
                let store = store.as_mut().unwrap();
                match wasmtime::Global::new(&mut *store, global_type, init_val) {
                    Ok(global) => {
                        let id = next_id();
                        WASM_GLOBALS.with(|gl| gl.borrow_mut().insert(id, global));
                        id as i32
                    }
                    Err(_) => -1,
                }
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_global_get",
        Function::new(ctx.clone(), |id: u32| -> f64 {
            WASM_GLOBALS.with(|gl| {
                let globals = gl.borrow();
                let global = match globals.get(&id) {
                    Some(g) => *g,
                    None => return 0.0,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    match global.get(&mut *store) {
                        Val::I32(v) => v as f64,
                        Val::I64(v) => v as f64,
                        Val::F32(v) => f32::from_bits(v) as f64,
                        Val::F64(v) => f64::from_bits(v),
                        _ => 0.0,
                    }
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_global_set",
        Function::new(ctx.clone(), |id: u32, value: f64| -> bool {
            WASM_GLOBALS.with(|gl| {
                let globals = gl.borrow();
                let global = match globals.get(&id) {
                    Some(g) => *g,
                    None => return false,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    let vt = global.ty(&*store).content().clone();
                    let val = match vt {
                        ValType::I32 => Val::I32(value as i32),
                        ValType::I64 => Val::I64(value as i64),
                        ValType::F32 => Val::F32((value as f32).to_bits()),
                        ValType::F64 => Val::F64(value.to_bits()),
                        _ => return false,
                    };
                    global.set(&mut *store, val).is_ok()
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_global_type",
        Function::new(ctx.clone(), |id: u32| -> String {
            WASM_GLOBALS.with(|gl| {
                let globals = gl.borrow();
                let global = match globals.get(&id) {
                    Some(g) => *g,
                    None => return "{}".to_string(),
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    let gt = global.ty(&*store);
                    let vt = match gt.content() {
                        ValType::I32 => "i32",
                        ValType::I64 => "i64",
                        ValType::F32 => "f32",
                        ValType::F64 => "f64",
                        _ => "unknown",
                    };
                    let mutable = gt.mutability() == Mutability::Var;
                    format!(r#"{{"value":"{vt}","mutable":{mutable}}}"#)
                })
            })
        })
        .unwrap(),
    )
    .unwrap();
}
