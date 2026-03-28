mod console;
mod css;
mod dom_stubs;
mod fetch;
mod intl_js;
mod timers;
mod worker;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::Ctx;

use crate::dom::tree::DomTree;

use super::state::EngineState;

/// Register all global objects and functions in the JS context.
pub fn register_all(ctx: &Ctx<'_>, tree: Rc<RefCell<DomTree>>, state: Rc<RefCell<EngineState>>) {
    console::register_console(ctx);
    timers::register_timers(ctx);
    dom_stubs::register_dom_stubs(ctx);
    worker::register_worker(ctx);
    fetch::register_fetch(ctx);
    super::crypto::register(ctx);
    super::dom_bridge::install(ctx, Rc::clone(&tree), Rc::clone(&state));
    css::register_css_object(ctx);
    super::intl::register_intl(ctx);
    intl_js::register_intl_js(ctx);
}
