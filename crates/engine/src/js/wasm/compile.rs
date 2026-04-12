use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rquickjs::{Ctx, Function};
use wasmtime::Module;

use super::{ensure_engine, next_id, MODULE_CACHE, WASM_ENGINE, WASM_MODULES};

thread_local! {
    static LAST_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub fn set_last_error_pub(msg: &str) {
    LAST_ERROR.with(|le| *le.borrow_mut() = msg.to_string());
}

fn content_hash(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // __braille_wasm_compile(bytes: Vec<u8>) -> i32
    // Returns module_id on success, -1 on error (error in __braille_wasm_last_error)
    g.set(
        "__braille_wasm_compile",
        Function::new(ctx.clone(), |data: Vec<u8>| -> i32 {
            if let Err(e) = ensure_engine() {
                LAST_ERROR.with(|le| *le.borrow_mut() = e);
                return -1;
            }

            let hash = content_hash(&data);

            // Check module cache first
            let cached = MODULE_CACHE.with(|mc| mc.borrow().get(&hash).cloned());

            let module = if let Some(m) = cached {
                m
            } else {
                let result = WASM_ENGINE.with(|e| {
                    let engine = e.borrow();
                    let engine = engine.as_ref().unwrap();
                    Module::new(engine, &data)
                });
                match result {
                    Ok(m) => {
                        MODULE_CACHE.with(|mc| mc.borrow_mut().insert(hash, m.clone()));
                        m
                    }
                    Err(e) => {
                        LAST_ERROR.with(|le| *le.borrow_mut() = format!("{e}"));
                        return -1;
                    }
                }
            };

            let id = next_id();
            WASM_MODULES.with(|wm| wm.borrow_mut().insert(id, module));
            id as i32
        })
        .unwrap(),
    )
    .unwrap();

    // __braille_wasm_last_error() -> String
    g.set(
        "__braille_wasm_last_error",
        Function::new(ctx.clone(), || -> String {
            LAST_ERROR.with(|le| le.borrow().clone())
        })
        .unwrap(),
    )
    .unwrap();
}
