use rquickjs::{Ctx, Function};
use wasmtime::{Ref, Table, TableType, RefType};

use super::{ensure_store, next_id, WASM_STORE, WASM_TABLES};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_wasm_table_new",
        Function::new(ctx.clone(), |element: String, initial: u32, maximum: i64| -> i32 {
            if ensure_store().is_err() {
                return -1;
            }
            let ref_type = match element.as_str() {
                "anyfunc" | "funcref" => RefType::FUNCREF,
                "externref" => RefType::EXTERNREF,
                _ => return -1,
            };
            let max = if maximum >= 0 { Some(maximum as u32) } else { None };
            let table_type = TableType::new(ref_type, initial, max);
            let init_val = match element.as_str() {
                "anyfunc" | "funcref" => Ref::Func(None),
                _ => Ref::Extern(None),
            };
            WASM_STORE.with(|s| {
                let mut store = s.borrow_mut();
                let store = store.as_mut().unwrap();
                match Table::new(&mut *store, table_type, init_val) {
                    Ok(table) => {
                        let id = next_id();
                        WASM_TABLES.with(|t| t.borrow_mut().insert(id, table));
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
        "__braille_wasm_table_size",
        Function::new(ctx.clone(), |id: u32| -> u64 {
            WASM_TABLES.with(|t| {
                let tables = t.borrow();
                let table = match tables.get(&id) {
                    Some(t) => *t,
                    None => return 0,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    table.size(&*store)
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_table_grow",
        Function::new(ctx.clone(), |id: u32, delta: u32, element: String| -> i64 {
            WASM_TABLES.with(|t| {
                let tables = t.borrow();
                let table = match tables.get(&id) {
                    Some(t) => *t,
                    None => return -1,
                };
                let init_val = match element.as_str() {
                    "anyfunc" | "funcref" => Ref::Func(None),
                    _ => Ref::Extern(None),
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    match table.grow(&mut *store, delta as u64, init_val) {
                        Ok(old) => old as i64,
                        Err(_) => -1,
                    }
                })
            })
        })
        .unwrap(),
    )
    .unwrap();
}
