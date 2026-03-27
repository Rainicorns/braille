use boa_engine::{native_function::NativeFunction, JsValue};

use crate::js::realm_state;
use crate::js::realm_state::TimerEntry;

pub(super) fn make_set_timer(is_interval: bool) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let callback = args.first().cloned().unwrap_or(JsValue::undefined());
            let delay_ms = args
                .get(1)
                .map(|v| v.to_u32(ctx).unwrap_or(0))
                .unwrap_or(0);
            let ts = realm_state::timer_state(ctx);
            let mut state = ts.borrow_mut();
            let id = state.next_id;
            state.next_id += 1;
            let registered_at = state.current_time_ms;
            state.entries.insert(
                id,
                TimerEntry {
                    id,
                    callback,
                    delay_ms,
                    is_interval,
                    registered_at,
                },
            );
            Ok(JsValue::from(id))
        })
    }
}

pub(super) fn make_clear_timer() -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if let Some(id_val) = args.first() {
                let id = id_val.to_u32(ctx)?;
                let ts = realm_state::timer_state(ctx);
                ts.borrow_mut().entries.remove(&id);
            }
            Ok(JsValue::undefined())
        })
    }
}
