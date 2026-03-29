mod console;
mod crypto_subtle;
mod css;
mod dom_stubs;
mod fetch;
mod iframe;
mod intl_js;
mod messaging;
mod timers;
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
    super::intl::register_intl(ctx);
    intl_js::register_intl_js(ctx);
}
