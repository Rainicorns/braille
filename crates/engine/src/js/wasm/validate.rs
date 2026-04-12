use rquickjs::{Ctx, Function};

use super::{ensure_engine, WASM_ENGINE};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_wasm_validate",
        Function::new(ctx.clone(), |data: Vec<u8>| -> bool {
            if ensure_engine().is_err() {
                return false;
            }
            WASM_ENGINE.with(|e| {
                let engine = e.borrow();
                let engine = engine.as_ref().unwrap();
                Module::validate(engine, &data).is_ok()
            })
        })
        .unwrap(),
    )
    .unwrap();
}

use wasmtime::Module;
