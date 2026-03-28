use rquickjs::prelude::Rest;
use rquickjs::{Ctx, Function, Object};

use crate::js::dom_bridge::with_state_mut;

pub(super) fn register_console(ctx: &Ctx<'_>) {
    let console = Object::new(ctx.clone()).unwrap();

    let mk = |prefix: &'static str| {
        Function::new(ctx.clone(), move |args: Rest<rquickjs::Value<'_>>| {
            let parts: Vec<String> = args.0.iter().map(|v| {
                if let Some(s) = v.as_string() {
                    s.to_string().unwrap_or_default()
                } else if v.is_null() {
                    "null".to_string()
                } else if v.is_undefined() {
                    "undefined".to_string()
                } else if let Some(b) = v.as_bool() {
                    b.to_string()
                } else if let Some(n) = v.as_int() {
                    n.to_string()
                } else if let Some(n) = v.as_float() {
                    format!("{n}")
                } else if v.is_error() {
                    // JS Error object — extract message
                    if let Some(obj) = v.as_object().cloned() {
                        let msg = obj.get::<_, String>("message").unwrap_or_default();
                        let name = obj.get::<_, String>("name").unwrap_or_else(|_| "Error".to_string());
                        format!("{name}: {msg}")
                    } else {
                        "Error".to_string()
                    }
                } else {
                    v.get::<String>().unwrap_or_else(|_| "[object]".to_string())
                }
            }).collect();
            let line = if prefix.is_empty() {
                parts.join(" ")
            } else {
                format!("[{}] {}", prefix, parts.join(" "))
            };
            with_state_mut(|s| s.console_buffer.push(line));
        })
        .unwrap()
    };

    console.set("log", mk("")).unwrap();
    console.set("info", mk("info")).unwrap();
    console.set("warn", mk("warn")).unwrap();
    console.set("error", mk("error")).unwrap();
    console.set("debug", mk("debug")).unwrap();
    // Stubs for methods that don't produce output
    let noop = Function::new(ctx.clone(), || {}).unwrap();
    console.set("trace", noop.clone()).unwrap();
    console.set("assert", noop.clone()).unwrap();
    console.set("count", noop.clone()).unwrap();
    console.set("time", noop.clone()).unwrap();
    console.set("timeEnd", noop.clone()).unwrap();
    console.set("group", noop.clone()).unwrap();
    console.set("groupEnd", noop.clone()).unwrap();
    console.set("table", noop).unwrap();

    ctx.globals().set("console", console).unwrap();
}
