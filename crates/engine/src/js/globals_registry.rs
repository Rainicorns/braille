//! GlobalsRegistry: phase-based JS globals registration with plugin hooks.
//!
//! The default registration order is:
//!   Phase 10: console, timers (basic runtime)
//!   Phase 20: native_hooks (Rust native functions)
//!   Phase 30: web_apis (Event, URL, navigator, etc.)
//!   Phase 40: class_hierarchy (Node, Element, HTMLElement)
//!   Phase 50: collections (NodeList, HTMLCollection, DOMTokenList)
//!   Phase 60: dom_helpers (shared helpers)
//!   Phase 70: tree_traversal, xpath
//!   Phase 80: async APIs (Worker, fetch, crypto, WASM)
//!   Phase 90: dom_bridge (IIFE with prototype wiring)
//!   Phase 95: finalize (non-enumerable interface objects)
//!   Phase 100: post-bridge utilities (CSS, messaging, iframe, etc.)
//!
//! Plugins can register at any phase (including fractional-ish via intermediate values
//! like 25, 85, etc.) to inject globals before or after specific built-in phases.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::Ctx;

use crate::dom::tree::DomTree;
use super::state::EngineState;

/// Type alias for registration functions.
type RegisterFn = Box<dyn Fn(&Ctx<'_>, Rc<RefCell<DomTree>>, Rc<RefCell<EngineState>>)>;

/// A registration entry: a named function that runs at a specific phase.
struct RegistrationEntry {
    phase: u32,
    name: &'static str,
    register_fn: RegisterFn,
}

/// Registry of JS globals to register, ordered by phase.
/// Plugins add entries; `execute_all` runs them in phase order.
pub struct GlobalsRegistry {
    entries: Vec<RegistrationEntry>,
}

impl Default for GlobalsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalsRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Register a globals provider at a specific phase.
    /// Lower phases execute first. Entries within the same phase execute in insertion order.
    pub fn register(
        &mut self,
        phase: u32,
        name: &'static str,
        f: impl Fn(&Ctx<'_>, Rc<RefCell<DomTree>>, Rc<RefCell<EngineState>>) + 'static,
    ) {
        self.entries.push(RegistrationEntry {
            phase,
            name,
            register_fn: Box::new(f),
        });
    }

    /// Execute all registered providers in phase order.
    pub fn execute_all(
        &self,
        ctx: &Ctx<'_>,
        tree: Rc<RefCell<DomTree>>,
        state: Rc<RefCell<EngineState>>,
    ) {
        let mut indices: Vec<usize> = (0..self.entries.len()).collect();
        indices.sort_by_key(|&i| self.entries[i].phase);

        for i in indices {
            let entry = &self.entries[i];
            (entry.register_fn)(ctx, Rc::clone(&tree), Rc::clone(&state));
        }
    }

    /// List registered entries (for debugging/introspection).
    pub fn entries(&self) -> Vec<(u32, &'static str)> {
        let mut result: Vec<(u32, &'static str)> = self.entries.iter()
            .map(|e| (e.phase, e.name))
            .collect();
        result.sort_by_key(|(phase, _)| *phase);
        result
    }
}

/// Phase constants for built-in registrations.
pub mod phases {
    pub const RUNTIME_BASICS: u32 = 10;
    pub const NATIVE_HOOKS: u32 = 20;
    pub const WEB_APIS: u32 = 30;
    pub const CLASS_HIERARCHY: u32 = 40;
    pub const COLLECTIONS: u32 = 50;
    pub const DOM_HELPERS: u32 = 60;
    pub const TREE_TRAVERSAL: u32 = 70;
    pub const ASYNC_APIS: u32 = 80;
    pub const DOM_BRIDGE: u32 = 90;
    pub const FINALIZE: u32 = 95;
    pub const POST_BRIDGE: u32 = 100;
}

/// Build the default registry with all built-in globals.
pub fn default_registry() -> GlobalsRegistry {
    let mut reg = GlobalsRegistry::new();

    reg.register(phases::RUNTIME_BASICS, "console", |ctx, _, _| {
        super::globals::console::register_console(ctx);
    });
    reg.register(phases::RUNTIME_BASICS, "timers", |ctx, _, _| {
        super::globals::timers::register_timers(ctx);
    });
    reg.register(phases::RUNTIME_BASICS, "test_progress", |ctx, _, _| {
        use rquickjs::Function;
        ctx.globals()
            .set(
                "__braille_test_progress",
                Function::new(ctx.clone(), |passed: bool| {
                    use std::io::Write;
                    if passed { print!("."); } else { print!("F"); }
                    std::io::stdout().flush().unwrap();
                }).unwrap(),
            ).unwrap();
    });

    reg.register(phases::NATIVE_HOOKS, "native_hooks", |ctx, _, _| {
        super::globals::native_hooks::register(ctx);
    });

    reg.register(phases::WEB_APIS, "web_apis", |ctx, _, _| {
        super::globals::web_apis::register(ctx);
    });

    reg.register(phases::CLASS_HIERARCHY, "class_hierarchy", |ctx, _, _| {
        super::globals::class_hierarchy::register(ctx);
    });

    reg.register(phases::COLLECTIONS, "collections", |ctx, _, _| {
        super::globals::collections::register(ctx);
    });

    reg.register(phases::DOM_HELPERS, "dom_helpers", |ctx, _, _| {
        super::globals::dom_helpers::register(ctx);
    });

    reg.register(phases::TREE_TRAVERSAL, "tree_traversal", |ctx, _, _| {
        super::globals::tree_traversal::register(ctx);
    });
    reg.register(phases::TREE_TRAVERSAL, "xpath", |ctx, _, _| {
        super::globals::xpath::register(ctx);
    });

    reg.register(phases::ASYNC_APIS, "worker", |ctx, _, _| {
        super::globals::worker::register_worker(ctx);
    });
    reg.register(phases::ASYNC_APIS, "fetch", |ctx, _, _| {
        super::globals::fetch::register_fetch(ctx);
    });
    reg.register(phases::ASYNC_APIS, "crypto", |ctx, _, _| {
        super::crypto::register(ctx);
    });
    reg.register(phases::ASYNC_APIS, "crypto_subtle", |ctx, _, _| {
        super::globals::crypto_subtle::register_crypto(ctx);
    });
    reg.register(phases::ASYNC_APIS, "wasm_runtime", |ctx, _, _| {
        super::wasm::register(ctx);
    });
    reg.register(phases::ASYNC_APIS, "wasm_api", |ctx, _, _| {
        super::globals::wasm_api::register_wasm(ctx);
    });

    reg.register(phases::DOM_BRIDGE, "dom_bridge", |ctx, tree, state| {
        super::dom_bridge::install(ctx, tree, state);
    });

    reg.register(phases::FINALIZE, "finalize", |ctx, _, _| {
        super::globals::dom_helpers::finalize(ctx);
    });

    reg.register(phases::POST_BRIDGE, "css", |ctx, _, _| {
        super::globals::css::register_css_object(ctx);
    });
    reg.register(phases::POST_BRIDGE, "messaging", |ctx, _, _| {
        super::globals::messaging::register_messaging(ctx);
    });
    reg.register(phases::POST_BRIDGE, "iframe", |ctx, _, _| {
        super::globals::iframe::register_iframe(ctx);
    });
    reg.register(phases::POST_BRIDGE, "shadowrealm", |ctx, _, _| {
        super::globals::shadowrealm::register_shadowrealm(ctx);
    });
    reg.register(phases::POST_BRIDGE, "intl", |ctx, _, _| {
        super::intl::register_intl(ctx);
    });
    reg.register(phases::POST_BRIDGE, "intl_js", |ctx, _, _| {
        super::globals::intl_js::register_intl_js(ctx);
    });
    reg.register(phases::POST_BRIDGE, "websocket", |ctx, _, _| {
        use rquickjs::Function;
        ctx.globals()
            .set("__braille_ws_connect", Function::new(ctx.clone(), |id: u32, url: String, protocols: String| {
                super::dom_bridge::with_state_mut(|s| {
                    s.pending_ws_connects.push(super::state::PendingWsConnect { id, url, protocols });
                });
            }).unwrap()).unwrap();
        ctx.globals()
            .set("__braille_ws_send", Function::new(ctx.clone(), |id: u32, data: String| {
                super::dom_bridge::with_state_mut(|s| {
                    s.pending_ws_sends.push(super::state::PendingWsSend { id, data });
                });
            }).unwrap()).unwrap();
        ctx.globals()
            .set("__braille_ws_close", Function::new(ctx.clone(), |id: u32, code: u16, reason: String| {
                super::dom_bridge::with_state_mut(|s| {
                    s.pending_ws_closes.push(super::state::PendingWsClose { id, code, reason });
                });
            }).unwrap()).unwrap();
        ctx.eval::<(), _>(super::globals::websocket::websocket_js()).unwrap_or(());
    });

    reg
}
