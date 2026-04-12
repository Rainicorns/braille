use rquickjs::{Ctx, Function, Value};
use wasmtime::{Extern, ExternType, Func, FuncType, Instance, Val, ValType};

use super::{
    ensure_store, next_id, JS_CALL_BRIDGE, WASM_GLOBALS, WASM_INSTANCES, WASM_MEMORIES,
    WASM_MODULES, WASM_STORE, WASM_TABLES,
};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // __braille_wasm_instantiate(module_id, imports_json) -> instance_id or -1
    // imports_json: JSON string describing resolved imports:
    //   [{ "module": "spectest", "name": "print", "kind": "function", "fn_id": 3 },
    //    { "module": "spectest", "name": "memory", "kind": "memory", "mem_id": 5 }, ...]
    g.set(
        "__braille_wasm_instantiate",
        Function::new(ctx.clone(), |ctx: Ctx<'_>, module_id: u32, imports_json: String| -> i32 {
            let module = WASM_MODULES.with(|wm| wm.borrow().get(&module_id).cloned());
            let module = match module {
                Some(m) => m,
                None => {
                    set_last_error("module not found");
                    return -1;
                }
            };

            if let Err(e) = ensure_store() {
                set_last_error(&e);
                return -1;
            }

            let import_descs: Vec<ImportDesc> = match serde_json::from_str(&imports_json) {
                Ok(v) => v,
                Err(e) => {
                    set_last_error(&format!("invalid imports JSON: {e}"));
                    return -1;
                }
            };

            // Build extern imports in the order the module expects
            let mut externs: Vec<Extern> = Vec::new();

            for import in module.imports() {
                let desc = import_descs.iter().find(|d| d.module == import.module() && d.name == import.name());

                let ext = match import.ty() {
                    ExternType::Func(func_type) => {
                        let fn_id = desc.and_then(|d| d.fn_id);
                        match fn_id {
                            Some(fn_id) => make_import_func(&ctx, func_type, fn_id),
                            None => {
                                set_last_error(&format!(
                                    "missing import: {}.{} (function)",
                                    import.module(),
                                    import.name()
                                ));
                                return -1;
                            }
                        }
                    }
                    ExternType::Memory(_) => {
                        let mem_id = desc.and_then(|d| d.mem_id);
                        match mem_id {
                            Some(mem_id) => {
                                let mem = WASM_MEMORIES.with(|m| m.borrow().get(&mem_id).copied());
                                match mem {
                                    Some(m) => Extern::Memory(m),
                                    None => {
                                        set_last_error(&format!(
                                            "memory not found for import {}.{}",
                                            import.module(),
                                            import.name()
                                        ));
                                        return -1;
                                    }
                                }
                            }
                            None => {
                                set_last_error(&format!(
                                    "missing import: {}.{} (memory)",
                                    import.module(),
                                    import.name()
                                ));
                                return -1;
                            }
                        }
                    }
                    ExternType::Table(_) => {
                        let table_id = desc.and_then(|d| d.table_id);
                        match table_id {
                            Some(table_id) => {
                                let table = WASM_TABLES.with(|t| t.borrow().get(&table_id).copied());
                                match table {
                                    Some(t) => Extern::Table(t),
                                    None => {
                                        set_last_error(&format!(
                                            "table not found for import {}.{}",
                                            import.module(),
                                            import.name()
                                        ));
                                        return -1;
                                    }
                                }
                            }
                            None => {
                                set_last_error(&format!(
                                    "missing import: {}.{} (table)",
                                    import.module(),
                                    import.name()
                                ));
                                return -1;
                            }
                        }
                    }
                    ExternType::Global(_) => {
                        let global_id = desc.and_then(|d| d.global_id);
                        match global_id {
                            Some(global_id) => {
                                let global = WASM_GLOBALS.with(|g| g.borrow().get(&global_id).copied());
                                match global {
                                    Some(g) => Extern::Global(g),
                                    None => {
                                        set_last_error(&format!(
                                            "global not found for import {}.{}",
                                            import.module(),
                                            import.name()
                                        ));
                                        return -1;
                                    }
                                }
                            }
                            None => {
                                set_last_error(&format!(
                                    "missing import: {}.{} (global)",
                                    import.module(),
                                    import.name()
                                ));
                                return -1;
                            }
                        }
                    }
                    ExternType::Tag(_) => {
                        set_last_error(&format!(
                            "tag imports not supported: {}.{}",
                            import.module(),
                            import.name()
                        ));
                        return -1;
                    }
                };

                externs.push(ext);
            }

            // Instantiate
            let result = WASM_STORE.with(|s| {
                let mut store = s.borrow_mut();
                let store = store.as_mut().unwrap();
                // Refuel before instantiation (start function may run)
                let _ = store.set_fuel(1_000_000_000);
                Instance::new(store, &module, &externs)
            });

            match result {
                Ok(instance) => {
                    let id = next_id();
                    // Register any exported memories/tables/globals in their maps
                    register_exports(id, &instance);
                    WASM_INSTANCES.with(|wi| wi.borrow_mut().insert(id, instance));
                    id as i32
                }
                Err(e) => {
                    set_last_error(&format!("{e}"));
                    -1
                }
            }
        })
        .unwrap(),
    )
    .unwrap();

    // __braille_wasm_instance_exports(instance_id) -> JSON
    g.set(
        "__braille_wasm_instance_exports",
        Function::new(ctx.clone(), |instance_id: u32| -> String {
            WASM_INSTANCES.with(|wi| {
                let instances = wi.borrow();
                let instance = match instances.get(&instance_id) {
                    Some(i) => *i,
                    None => return "[]".to_string(),
                };
                WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    let exports: Vec<(String, String)> = instance
                        .exports(&mut *store)
                        .map(|e| {
                            let name = e.name().to_string();
                            let kind = match e.into_extern() {
                                Extern::Func(_) => "function",
                                Extern::Memory(_) => "memory",
                                Extern::Table(_) => "table",
                                Extern::Global(_) => "global",
                                _ => "unknown",
                            };
                            (name, kind.to_string())
                        })
                        .collect();
                    let json: Vec<String> = exports
                        .iter()
                        .map(|(name, kind)| {
                            format!(
                                r#"{{"name":{},"kind":"{}"}}"#,
                                serde_json::to_string(name).unwrap(),
                                kind
                            )
                        })
                        .collect();
                    format!("[{}]", json.join(","))
                })
            })
        })
        .unwrap(),
    )
    .unwrap();

    // __braille_wasm_instance_export_ids(instance_id) -> JSON with IDs for memories/tables/globals
    g.set(
        "__braille_wasm_instance_export_ids",
        Function::new(ctx.clone(), |instance_id: u32| -> String {
            WASM_INSTANCES.with(|wi| {
                let instances = wi.borrow();
                let instance = match instances.get(&instance_id) {
                    Some(i) => *i,
                    None => return "{}".to_string(),
                };
                // Collect exports first, then process (avoids borrow conflicts)
                let export_data: Vec<(String, Extern)> = WASM_STORE.with(|s| {
                    let mut store = s.borrow_mut();
                    let store = store.as_mut().unwrap();
                    instance
                        .exports(&mut *store)
                        .map(|e| (e.name().to_string(), e.into_extern()))
                        .collect()
                });
                let mut entries: Vec<String> = Vec::new();
                for (name, ext) in &export_data {
                    match ext {
                        Extern::Memory(mem) => {
                            let id = find_or_register_memory(*mem);
                            entries.push(format!(
                                r#"{}:{{"kind":"memory","id":{id}}}"#,
                                serde_json::to_string(name).unwrap()
                            ));
                        }
                        Extern::Table(table) => {
                            let id = find_or_register_table(*table);
                            entries.push(format!(
                                r#"{}:{{"kind":"table","id":{id}}}"#,
                                serde_json::to_string(name).unwrap()
                            ));
                        }
                        Extern::Global(global) => {
                            let id = find_or_register_global(*global);
                            entries.push(format!(
                                r#"{}:{{"kind":"global","id":{id}}}"#,
                                serde_json::to_string(name).unwrap()
                            ));
                        }
                        Extern::Func(_) => {
                            entries.push(format!(
                                r#"{}:{{"kind":"function"}}"#,
                                serde_json::to_string(name).unwrap()
                            ));
                        }
                        _ => {}
                    }
                }
                format!("{{{}}}", entries.join(","))
            })
        })
        .unwrap(),
    )
    .unwrap();
}

fn make_import_func(_ctx: &Ctx<'_>, func_type: FuncType, fn_id: u32) -> Extern {
    let param_types: Vec<ValType> = func_type.params().collect();
    let result_types: Vec<ValType> = func_type.results().collect();

    WASM_STORE.with(|s| {
        let mut store = s.borrow_mut();
        let store = store.as_mut().unwrap();
        let func = Func::new(&mut *store, func_type, move |_caller, params, results| {
            JS_CALL_BRIDGE.with(|bridge| {
                let ctx_ptr = bridge.get();
                let ctx_ptr = match ctx_ptr {
                    Some(p) => p,
                    None => {
                        return Err(wasmtime::Error::msg("JS call bridge not set"));
                    }
                };
                // SAFETY: single-threaded, Ctx valid for entire wasmtime call duration.
                // The pointer is set in call.rs right before the wasmtime call and cleared
                // immediately after. No other thread can access it.
                let ctx: &Ctx<'_> = unsafe { &*(ctx_ptr as *const Ctx<'_>) };

                // Convert wasmtime params to JS values
                let js_args: Vec<String> = params
                    .iter()
                    .zip(param_types.iter())
                    .map(|(val, _vt)| match val {
                        Val::I32(v) => format!("{v}"),
                        Val::I64(v) => format!("{v}n"),
                        Val::F32(v) => {
                            let f = f32::from_bits(*v);
                            if f.is_nan() { "NaN".to_string() }
                            else if f.is_infinite() { if f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() } }
                            else { format!("{f}") }
                        }
                        Val::F64(v) => {
                            let f = f64::from_bits(*v);
                            if f.is_nan() { "NaN".to_string() }
                            else if f.is_infinite() { if f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() } }
                            else { format!("{f}") }
                        }
                        _ => "null".to_string(),
                    })
                    .collect();

                let call_code = format!(
                    "__braille_wasm_import_fns[{fn_id}]({})",
                    js_args.join(",")
                );

                let result: Result<Value<'_>, _> = ctx.eval(call_code);
                match result {
                    Ok(val) => {
                        for (i, rt) in result_types.iter().enumerate() {
                            if i < results.len() {
                                results[i] = js_value_to_wasm_val(&val, rt);
                            }
                        }
                        Ok(())
                    }
                    Err(e) => {
                        Err(wasmtime::Error::msg(format!("JS import call failed: {e:?}")))
                    }
                }
            })
        });
        Extern::Func(func)
    })
}

fn js_value_to_wasm_val(val: &Value<'_>, vt: &ValType) -> Val {
    match vt {
        ValType::I32 => {
            let n = val.as_int().unwrap_or_else(|| val.as_float().map(|f| f as i32).unwrap_or(0));
            Val::I32(n)
        }
        ValType::I64 => {
            // Try BigInt first, fall back to number
            let n = val.as_int().map(|i| i as i64).unwrap_or_else(|| {
                val.as_float().map(|f| f as i64).unwrap_or(0)
            });
            Val::I64(n)
        }
        ValType::F32 => {
            let f = val.as_float().unwrap_or_else(|| val.as_int().map(|i| i as f64).unwrap_or(0.0));
            Val::F32((f as f32).to_bits())
        }
        ValType::F64 => {
            let f = val.as_float().unwrap_or_else(|| val.as_int().map(|i| i as f64).unwrap_or(0.0));
            Val::F64(f.to_bits())
        }
        _ => Val::I32(0),
    }
}

fn register_exports(_instance_id: u32, instance: &Instance) {
    let externs: Vec<Extern> = WASM_STORE.with(|s| {
        let mut store = s.borrow_mut();
        let store = store.as_mut().unwrap();
        instance.exports(&mut *store).map(|e| e.into_extern()).collect()
    });
    for ext in externs {
        match ext {
            Extern::Memory(mem) => { find_or_register_memory(mem); }
            Extern::Table(table) => { find_or_register_table(table); }
            Extern::Global(global) => { find_or_register_global(global); }
            _ => {}
        }
    }
}

fn find_or_register_memory(mem: wasmtime::Memory) -> u32 {
    let needle = format!("{mem:?}");
    WASM_MEMORIES.with(|m| {
        let memories = m.borrow();
        for (&id, existing) in memories.iter() {
            if format!("{existing:?}") == needle {
                return id;
            }
        }
        drop(memories);
        let id = next_id();
        m.borrow_mut().insert(id, mem);
        id
    })
}

fn find_or_register_table(table: wasmtime::Table) -> u32 {
    let needle = format!("{table:?}");
    WASM_TABLES.with(|t| {
        let tables = t.borrow();
        for (&id, existing) in tables.iter() {
            if format!("{existing:?}") == needle {
                return id;
            }
        }
        drop(tables);
        let id = next_id();
        t.borrow_mut().insert(id, table);
        id
    })
}

fn find_or_register_global(global: wasmtime::Global) -> u32 {
    let needle = format!("{global:?}");
    WASM_GLOBALS.with(|g| {
        let globals = g.borrow();
        for (&id, existing) in globals.iter() {
            if format!("{existing:?}") == needle {
                return id;
            }
        }
        drop(globals);
        let id = next_id();
        g.borrow_mut().insert(id, global);
        id
    })
}

fn set_last_error(msg: &str) {
    super::compile::set_last_error_pub(msg);
}

#[derive(serde::Deserialize)]
struct ImportDesc {
    module: String,
    name: String,
    #[serde(default)]
    fn_id: Option<u32>,
    #[serde(default)]
    mem_id: Option<u32>,
    #[serde(default)]
    table_id: Option<u32>,
    #[serde(default)]
    global_id: Option<u32>,
}
