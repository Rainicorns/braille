use rquickjs::{Ctx, Function};

use crate::js::dom_bridge::with_state_mut;

pub(super) fn register_timers(ctx: &Ctx<'_>) {
    // setTimeout/setInterval: JS wrapper stores callback functions, Rust tracks timing
    {
        let register_timer = Function::new(ctx.clone(), move |delay: rquickjs::Value<'_>, is_interval: bool| -> u32 {
            let delay_ms = delay.as_float().or_else(|| delay.as_int().map(|i| i as f64)).unwrap_or(0.0).max(0.0) as u64;
            with_state_mut(|st| {
                let id = st.next_timer_id;
                st.next_timer_id += 1;
                let current_time = st.timer_current_time_ms;
                st.timer_entries.insert(id, crate::js::state::TimerEntry {
                    id,
                    callback_code: format!("__braille_fire_timer({id})"),
                    delay_ms,
                    registered_at: current_time,
                    is_interval,
                });
                id
            })
        }).unwrap();
        ctx.globals().set("__braille_register_timer", register_timer).unwrap();

        let clear_timer = Function::new(ctx.clone(), move |id: rquickjs::Value<'_>| {
            if let Some(n) = id.as_int() {
                with_state_mut(|st| { st.timer_entries.remove(&(n as u32)); });
            }
        }).unwrap();
        ctx.globals().set("__braille_clear_timer", clear_timer).unwrap();

        ctx.eval::<(), _>(r#"
            (function() {
                var _cbs = {};
                globalThis.setTimeout = function(cb, delay) {
                    var id = __braille_register_timer(delay || 0, false);
                    if (typeof cb === 'function') _cbs[id] = cb;
                    else _cbs[id] = function() { eval(cb); };
                    return id;
                };
                globalThis.setInterval = function(cb, delay) {
                    var id = __braille_register_timer(delay || 0, true);
                    if (typeof cb === 'function') _cbs[id] = cb;
                    else _cbs[id] = function() { eval(cb); };
                    return id;
                };
                globalThis.clearTimeout = function(id) { delete _cbs[id]; __braille_clear_timer(id); };
                globalThis.clearInterval = function(id) { delete _cbs[id]; __braille_clear_timer(id); };
                globalThis.__braille_timer_errors = [];
                globalThis.__braille_fire_timer = function(id) {
                    if (_cbs[id]) {
                        try { _cbs[id](); }
                        catch(e) { __braille_timer_errors.push('timer ' + id + ': ' + (e instanceof Error ? e.message + '\n' + (e.stack || '') : String(e))); }
                    }
                };
            })();
        "#).unwrap();
    }

}
