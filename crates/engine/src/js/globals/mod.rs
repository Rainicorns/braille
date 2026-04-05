mod console;
mod crypto_subtle;
mod css;
mod dom_stubs;
mod fetch;
mod iframe;
mod intl_js;
mod messaging;
mod shadowrealm;
mod timers;
mod websocket;
mod worker;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function};

use crate::dom::tree::DomTree;

use super::state::EngineState;

/// Register all global objects and functions in the JS context.
pub fn register_all(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>) {
    // Test progress: prints . or F to stderr per subtest, flushed immediately
    ctx.globals()
        .set(
            "__braille_test_progress",
            Function::new(ctx.clone(), |passed: bool| {
                use std::io::Write;
                if passed {
                    print!(".");
                } else {
                    print!("F");
                }
                std::io::stdout().flush().unwrap();
            })
            .unwrap(),
        )
        .unwrap();

    console::register_console(ctx);
    timers::register_timers(ctx);
    dom_stubs::register_dom_stubs(ctx);
    worker::register_worker(ctx);
    fetch::register_fetch(ctx);
    super::crypto::register(ctx);
    crypto_subtle::register_crypto(ctx);
    super::dom_bridge::install(ctx, Rc::clone(&tree), Rc::clone(&state));
    css::register_css_object(ctx);
    messaging::register_messaging(ctx);
    iframe::register_iframe(ctx);
    shadowrealm::register_shadowrealm(ctx);
    super::intl::register_intl(ctx);
    intl_js::register_intl_js(ctx);

    // WebSocket native hooks (must be registered before the JS class)
    ctx.globals()
        .set(
            "__braille_ws_connect",
            Function::new(ctx.clone(), |id: u32, url: String, protocols: String| {
                super::dom_bridge::with_state_mut(|s| {
                    s.pending_ws_connects.push(super::state::PendingWsConnect { id, url, protocols });
                });
            })
            .unwrap(),
        )
        .unwrap();
    ctx.globals()
        .set(
            "__braille_ws_send",
            Function::new(ctx.clone(), |id: u32, data: String| {
                super::dom_bridge::with_state_mut(|s| {
                    s.pending_ws_sends.push(super::state::PendingWsSend { id, data });
                });
            })
            .unwrap(),
        )
        .unwrap();
    ctx.globals()
        .set(
            "__braille_ws_close",
            Function::new(ctx.clone(), |id: u32, code: u16, reason: String| {
                super::dom_bridge::with_state_mut(|s| {
                    s.pending_ws_closes.push(super::state::PendingWsClose { id, code, reason });
                });
            })
            .unwrap(),
        )
        .unwrap();

    // WebSocket JS class
    ctx.eval::<(), _>(websocket::websocket_js()).unwrap_or(());
}
