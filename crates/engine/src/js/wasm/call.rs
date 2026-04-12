use rquickjs::{Ctx, Function, IntoJs};
use wasmtime::{Extern, Func, FuncType, Val, ValType};

use super::{JS_CALL_BRIDGE, WASM_INSTANCES, WASM_STORE};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_wasm_call",
        Function::new(ctx.clone(), wasm_call).unwrap(),
    )
    .unwrap();
}

fn wasm_call(ctx: Ctx<'_>, instance_id: u32, export_name: String, args_json: String) -> rquickjs::Result<rquickjs::Value<'_>> {
    let instance = WASM_INSTANCES.with(|wi| wi.borrow().get(&instance_id).copied());
    let instance = match instance {
        Some(i) => i,
        None => {
            return ctx.eval("throw new Error('instance not found')");
        }
    };

    let func_and_type: Option<(Func, FuncType)> = WASM_STORE.with(|s| {
        let mut store = s.borrow_mut();
        let store = store.as_mut().unwrap();
        instance
            .get_export(&mut *store, &export_name)
            .and_then(|e| match e {
                Extern::Func(f) => {
                    let ft = f.ty(&*store);
                    Some((f, ft))
                }
                _ => None,
            })
    });

    let (func, func_type) = match func_and_type {
        Some(pair) => pair,
        None => {
            let msg = format!("export '{}' not found or not a function", export_name);
            let code = format!("throw new Error({})", serde_json::to_string(&msg).unwrap());
            return ctx.eval(code);
        }
    };

    let args: Vec<ArgDesc> = serde_json::from_str(&args_json).unwrap_or_default();

    let param_types: Vec<ValType> = func_type.params().collect();
    let mut wasm_args: Vec<Val> = Vec::with_capacity(args.len());
    for (i, pt) in param_types.iter().enumerate() {
        let arg = args.get(i);
        let val = match pt {
            ValType::I32 => Val::I32(arg.map(|a| a.value as i32).unwrap_or(0)),
            ValType::I64 => Val::I64(arg.map(|a| a.value as i64).unwrap_or(0)),
            ValType::F32 => {
                let f = arg.map(|a| a.value).unwrap_or(0.0);
                Val::F32((f as f32).to_bits())
            }
            ValType::F64 => {
                let f = arg.map(|a| a.value).unwrap_or(0.0);
                Val::F64(f.to_bits())
            }
            _ => Val::I32(0),
        };
        wasm_args.push(val);
    }

    let result_count = func_type.results().len();
    let mut results = vec![Val::I32(0); result_count];

    let ctx_ptr = &ctx as *const Ctx<'_> as *const ();
    JS_CALL_BRIDGE.with(|bridge| bridge.set(Some(ctx_ptr)));

    let call_result = WASM_STORE.with(|s| {
        let mut store = s.borrow_mut();
        let store = store.as_mut().unwrap();
        let _ = store.set_fuel(1_000_000_000);
        func.call(&mut *store, &wasm_args, &mut results)
    });

    JS_CALL_BRIDGE.with(|bridge| bridge.set(None));

    match call_result {
        Ok(()) => {
            if results.is_empty() {
                Ok(rquickjs::Value::new_undefined(ctx.clone()))
            } else if results.len() == 1 {
                wasm_val_to_js(&ctx, &results[0])
            } else {
                let arr = rquickjs::Array::new(ctx.clone())?;
                for (i, val) in results.iter().enumerate() {
                    let js_val = wasm_val_to_js(&ctx, val)?;
                    arr.set(i, js_val)?;
                }
                arr.into_js(&ctx)
            }
        }
        Err(e) => {
            let err_str = format!("{e}");
            let code = format!(
                "throw new WebAssembly.RuntimeError({})",
                serde_json::to_string(&err_str).unwrap()
            );
            ctx.eval(code)
        }
    }
}

fn wasm_val_to_js<'js>(ctx: &Ctx<'js>, val: &Val) -> rquickjs::Result<rquickjs::Value<'js>> {
    match val {
        Val::I32(v) => Ok(rquickjs::Value::new_int(ctx.clone(), *v)),
        Val::I64(v) => {
            let code = format!("{}n", *v);
            ctx.eval(code)
        }
        Val::F32(bits) => {
            let f = f32::from_bits(*bits) as f64;
            Ok(rquickjs::Value::new_float(ctx.clone(), f))
        }
        Val::F64(bits) => {
            let f = f64::from_bits(*bits);
            Ok(rquickjs::Value::new_float(ctx.clone(), f))
        }
        _ => Ok(rquickjs::Value::new_null(ctx.clone())),
    }
}

#[derive(serde::Deserialize, Default)]
struct ArgDesc {
    #[serde(default)]
    value: f64,
}
