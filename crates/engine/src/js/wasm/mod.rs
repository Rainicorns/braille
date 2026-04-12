mod validate;
mod compile;
mod module_info;
mod instantiate;
mod call;
mod memory;
mod table;
mod global;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use rquickjs::Ctx;
use wasmtime::{Engine, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, Table, Global};

struct WasmHost {
    limiter: StoreLimits,
}

impl WasmHost {
    fn new() -> Self {
        Self {
            limiter: StoreLimitsBuilder::new()
                .memory_size(256 * 1024 * 1024) // 256 MB max
                .table_elements(10_000)
                .instances(100)
                .tables(100)
                .memories(100)
                .build(),
        }
    }
}

thread_local! {
    static WASM_ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    static WASM_STORE: RefCell<Option<Store<WasmHost>>> = const { RefCell::new(None) };
    static WASM_MODULES: RefCell<HashMap<u32, Module>> = RefCell::new(HashMap::new());
    static WASM_INSTANCES: RefCell<HashMap<u32, Instance>> = RefCell::new(HashMap::new());
    static WASM_MEMORIES: RefCell<HashMap<u32, Memory>> = RefCell::new(HashMap::new());
    static WASM_TABLES: RefCell<HashMap<u32, Table>> = RefCell::new(HashMap::new());
    static WASM_GLOBALS: RefCell<HashMap<u32, Global>> = RefCell::new(HashMap::new());
    static MODULE_CACHE: RefCell<HashMap<u64, Module>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
    /// Raw pointer to the rquickjs Ctx, set during wasmtime calls so import
    /// function closures can call back into JS. Sound because: single-threaded,
    /// Ctx valid for entire wasmtime call duration, set/cleared in same scope.
    static JS_CALL_BRIDGE: Cell<Option<*const ()>> = const { Cell::new(None) };
}

fn next_id() -> u32 {
    NEXT_ID.with(|c| {
        let id = c.get();
        c.set(id + 1);
        id
    })
}

fn ensure_engine() -> Result<(), String> {
    WASM_ENGINE.with(|e| {
        if e.borrow().is_none() {
            let mut config = wasmtime::Config::new();
            config.consume_fuel(true);
            config.wasm_multi_value(true);
            config.wasm_bulk_memory(true);
            config.wasm_simd(true);
            let engine = Engine::new(&config).map_err(|e| format!("{e}"))?;
            *e.borrow_mut() = Some(engine);
        }
        Ok(())
    })
}

fn ensure_store() -> Result<(), String> {
    ensure_engine()?;
    WASM_STORE.with(|s| {
        if s.borrow().is_none() {
            WASM_ENGINE.with(|e| {
                let engine = e.borrow();
                let engine = engine.as_ref().unwrap();
                let mut store = Store::new(engine, WasmHost::new());
                store.limiter(|host| &mut host.limiter);
                store.set_fuel(u64::MAX).map_err(|e| format!("{e}"))?;
                *s.borrow_mut() = Some(store);
                Ok(())
            })
        } else {
            Ok(())
        }
    })
}

/// Reset all WASM state (called on page navigation).
/// Engine and module cache persist across pages.
pub fn reset() {
    WASM_STORE.with(|s| { s.borrow_mut().take(); });
    WASM_INSTANCES.with(|m| m.borrow_mut().clear());
    WASM_MEMORIES.with(|m| m.borrow_mut().clear());
    WASM_TABLES.with(|m| m.borrow_mut().clear());
    WASM_GLOBALS.with(|m| m.borrow_mut().clear());
    WASM_MODULES.with(|m| m.borrow_mut().clear());
    NEXT_ID.with(|c| c.set(1));
}

pub fn register(ctx: &Ctx<'_>) {
    validate::register(ctx);
    compile::register(ctx);
    module_info::register(ctx);
    memory::register(ctx);
    table::register(ctx);
    global::register(ctx);
    instantiate::register(ctx);
    call::register(ctx);
}
