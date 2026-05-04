pub(crate) mod class_hierarchy;
pub(crate) mod collections;
pub(crate) mod console;
pub(crate) mod crypto_subtle;
pub(crate) mod css;
pub(crate) mod dom_helpers;
pub(crate) mod fetch;
pub(crate) mod iframe;
pub(crate) mod intl_js;
pub(crate) mod messaging;
pub(crate) mod native_hooks;
pub(crate) mod shadowrealm;
pub(crate) mod timers;
pub(crate) mod tree_traversal;
pub(crate) mod wasm_api;
pub(crate) mod web_apis;
pub(crate) mod xpath;
pub(crate) mod websocket;
pub(crate) mod worker;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function};

use crate::dom::tree::DomTree;

use super::state::EngineState;

/// Register all global objects and functions in the JS context.
///
/// Loading order matters:
///   1. console, timers — basic runtime utilities
///   2. native_hooks — Rust native functions (alert, confirm, clipboard, etc.)
///   3. web_apis — standalone Web API polyfills (Event, URL, navigator, etc.)
///   4. collections — NodeFilter, NodeList, HTMLCollection, DOMTokenList, Attr
///   5. class_hierarchy — Node, Element, HTMLElement subclasses, CE registry, event handlers
///   6. dom_helpers — shared helpers (__makeHTMLCollection, MO, __isConnected, CE queue, DOMParser)
///   7. tree_traversal — TreeWalker, NodeIterator
///   8. worker, fetch, crypto — async APIs
///   9. dom_bridge — IIFE with prototype wiring + all bridge modules
///  10. dom_helpers::finalize — make interface objects non-enumerable (must run after dom_bridge)
///  11. css, messaging, iframe, etc. — post-bridge utilities
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

    // 1. Basic runtime
    console::register_console(ctx);
    timers::register_timers(ctx);

    // 2. Rust native functions (navigation, dialogs, clipboard, form submit)
    native_hooks::register(ctx);

    // 3. Standalone Web API polyfills (Event classes, URL, navigator, etc.)
    web_apis::register(ctx);

    // 4. Class hierarchy (Node, Element, HTMLElement + subclasses, CE registry, event handlers)
    class_hierarchy::register(ctx);

    // 5. Collection types (NodeList, HTMLCollection, DOMTokenList, Attr — needs Node from class_hierarchy)
    collections::register(ctx);

    // 6. Shared DOM helpers (__makeHTMLCollection, MO, __isConnected, CE queue, DOMParser)
    dom_helpers::register(ctx);

    // 7. Tree traversal (TreeWalker, NodeIterator)
    tree_traversal::register(ctx);

    // 7b. XPath API (XPathResult class + evaluator — uses TreeWalker)
    xpath::register(ctx);

    // 8. Async APIs
    worker::register_worker(ctx);
    fetch::register_fetch(ctx);
    super::crypto::register(ctx);
    crypto_subtle::register_crypto(ctx);
    super::wasm::register(ctx);
    wasm_api::register_wasm(ctx);

    // 9. DOM bridge (IIFE with class hierarchy + all bridge modules)
    super::dom_bridge::install(ctx, Rc::clone(&tree), Rc::clone(&state));

    // 10. Make interface objects non-enumerable (must run after dom_bridge defines Document, Text, etc.)
    dom_helpers::finalize(ctx);

    // 11. Post-bridge utilities
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
