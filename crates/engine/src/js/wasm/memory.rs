use rquickjs::{Ctx, Function};
use wasmtime::{Memory, MemoryType};

use super::{ensure_store, next_id, WASM_MEMORIES, WASM_STORE};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_wasm_memory_new",
        Function::new(ctx.clone(), |initial: u32, maximum: i64| -> i32 {
            if let Err(_e) = ensure_store() {
                return -1;
            }
            WASM_STORE.with(|s| {
                let mut store = s.borrow_mut();
                let store = store.as_mut().unwrap();
                let mem_type = if maximum >= 0 {
                    MemoryType::new(initial, Some(maximum as u32))
                } else {
                    MemoryType::new(initial, None)
                };
                match Memory::new(&mut *store, mem_type) {
                    Ok(mem) => {
                        let id = next_id();
                        WASM_MEMORIES.with(|m| m.borrow_mut().insert(id, mem));
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
        "__braille_wasm_memory_grow",
        Function::new(ctx.clone(), |id: u32, delta: u32| -> i64 {
            WASM_MEMORIES.with(|m| {
                let memories = m.borrow();
                let mem = match memories.get(&id) {
                    Some(m) => *m,
                    None => return -1,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    match mem.grow(&mut *store, delta as u64) {
                        Ok(old) => old as i64,
                        Err(_) => -1,
                    }
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_memory_size",
        Function::new(ctx.clone(), |id: u32| -> u32 {
            WASM_MEMORIES.with(|m| {
                let memories = m.borrow();
                let mem = match memories.get(&id) {
                    Some(m) => *m,
                    None => return 0,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    mem.size(&*store) as u32
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_memory_read",
        Function::new(ctx.clone(), |id: u32, offset: u32, length: u32| -> Vec<u8> {
            WASM_MEMORIES.with(|m| {
                let memories = m.borrow();
                let mem = match memories.get(&id) {
                    Some(m) => *m,
                    None => return vec![],
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    let data = mem.data(&*store);
                    let start = offset as usize;
                    let end = start + length as usize;
                    if end > data.len() {
                        return vec![];
                    }
                    data[start..end].to_vec()
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_memory_write",
        Function::new(ctx.clone(), |id: u32, offset: u32, data: Vec<u8>| -> bool {
            WASM_MEMORIES.with(|m| {
                let memories = m.borrow();
                let mem = match memories.get(&id) {
                    Some(m) => *m,
                    None => return false,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    let mem_data = mem.data_mut(&mut *store);
                    let start = offset as usize;
                    let end = start + data.len();
                    if end > mem_data.len() {
                        return false;
                    }
                    mem_data[start..end].copy_from_slice(&data);
                    true
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_wasm_memory_byte_length",
        Function::new(ctx.clone(), |id: u32| -> u32 {
            WASM_MEMORIES.with(|m| {
                let memories = m.borrow();
                let mem = match memories.get(&id) {
                    Some(m) => *m,
                    None => return 0,
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    mem.data_size(&*store) as u32
                })
            })
        })
        .unwrap(),
    )
    .unwrap();
}
