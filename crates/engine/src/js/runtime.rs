use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::{Context, Function, Module, Runtime};

use crate::dom::tree::DomTree;
use crate::dom::NodeId;

use super::module_loader::{self, SharedModuleRegistry, BrailleResolver, BrailleLoader};
use super::state::EngineState;

thread_local! {
    static PENDING_REJECTIONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The JS runtime wrapper. Owns a QuickJS Runtime + Context and provides
/// a high-level API that hides JS engine details from the rest of the engine.
pub struct JsRuntime {
    runtime: Runtime,
    context: Context,
    tree: Rc<RefCell<DomTree>>,
    pub(crate) state: Rc<RefCell<EngineState>>,
    module_registry: SharedModuleRegistry,
}

impl JsRuntime {
    /// Creates a new JS runtime wired to the given DomTree.
    pub fn new(tree: Rc<RefCell<DomTree>>) -> Self {
        let runtime = Runtime::new().expect("failed to create QuickJS runtime");
        runtime.set_memory_limit(256 * 1024 * 1024);
        runtime.set_max_stack_size(64 * 1024 * 1024);

        // Track unhandled promise rejections
        runtime.set_host_promise_rejection_tracker(Some(Box::new(
            |_ctx: rquickjs::Ctx<'_>, _promise: rquickjs::Value<'_>, reason: rquickjs::Value<'_>, is_handled: bool| {
                if !is_handled {
                    let reason_str = js_value_to_string(&reason);
                    PENDING_REJECTIONS.with(|pr| {
                        pr.borrow_mut().push(reason_str);
                    });
                }
            },
        )));

        // Set up custom module loader with shared registry
        let registry = module_loader::new_registry();
        runtime.set_loader(
            BrailleResolver { registry: Rc::clone(&registry) },
            BrailleLoader { registry: Rc::clone(&registry) },
        );

        let context = Context::full(&runtime).expect("failed to create QuickJS context");
        let state = Rc::new(RefCell::new(EngineState::new()));

        // Set thread-locals before register_all so closures can find them
        super::dom_bridge::set_tree(Rc::clone(&tree));
        super::dom_bridge::set_state(Rc::clone(&state));

        let rt = Self {
            runtime,
            context,
            tree: Rc::clone(&tree),
            state: Rc::clone(&state),
            module_registry: registry,
        };

        // Register all globals
        rt.context.with(|ctx| {
            super::globals::register_all(&ctx, Rc::clone(&tree), Rc::clone(&state));
        });

        rt
    }

    /// Rebind the runtime to a new page without re-registering globals.
    /// Swaps the tree and state in thread-locals, clears JS-side caches,
    /// and resets the module registry. ~0.5ms vs ~8.5ms for full init.
    pub fn rebind_for_new_page(&mut self, new_tree: Rc<RefCell<DomTree>>, new_state: Rc<RefCell<EngineState>>) {
        // Swap tree in thread-local
        self.tree = Rc::clone(&new_tree);
        super::dom_bridge::set_tree(new_tree);

        // Swap state in thread-local
        self.state = Rc::clone(&new_state);
        super::dom_bridge::set_state(new_state);

        // Clear module registry
        module_loader::clear_registry(&self.module_registry);

        // Clear JS-side wrapper cache + DOM state
        let _ = self.eval(r#"
            if (typeof __braille_reset_dom_cache === 'function') __braille_reset_dom_cache();
            if (typeof _cookieJar !== 'undefined') {
                for (var k in _cookieJar) delete _cookieJar[k];
            }
            if (typeof localStorage !== 'undefined') localStorage.clear();
            if (typeof sessionStorage !== 'undefined') sessionStorage.clear();
            if (typeof __braille_pending_navigation !== 'undefined') __braille_pending_navigation = null;
            if (typeof __braille_fetch_resolvers !== 'undefined') __braille_fetch_resolvers = {};
            if (typeof __braille_fetch_rejecters !== 'undefined') __braille_fetch_rejecters = {};
            if (typeof __braille_next_resolver_id !== 'undefined') __braille_next_resolver_id = 1;
            if (typeof __braille_worker_scripts !== 'undefined') __braille_worker_scripts = {};
            if (typeof __braille_pending_rejections !== 'undefined') __braille_pending_rejections = [];
        "#);

        self.flush_jobs();
    }

    /// Evaluate a JS source string. Errors are returned as strings.
    pub fn eval(&mut self, code: &str) -> Result<(), String> {
        self.context.with(|ctx| {
            ctx.eval::<(), _>(code)
                .map_err(|e| format_js_error(&ctx, e))
        })?;
        self.flush_jobs();
        Ok(())
    }

    /// Evaluate JS and return the result as a string.
    pub fn eval_to_string(&mut self, code: &str) -> Result<String, String> {
        let result = self.context.with(|ctx| {
            let val: rquickjs::Value = ctx.eval(code).map_err(|e| format_js_error(&ctx, e))?;
            Ok(js_value_to_string(&val))
        });
        self.flush_jobs();
        result
    }

    /// Evaluate an ES module source string.
    pub fn eval_module(&mut self, code: &str, specifier: Option<&str>) -> Result<(), String> {
        let spec = specifier.unwrap_or("__inline_module__");
        self.context.with(|ctx| {
            let _module = Module::evaluate(ctx.clone(), spec, code)
                .map_err(|e| format_js_error(&ctx, e))?;
            Ok::<_, String>(())
        })?;
        self.flush_jobs();
        Ok(())
    }

    /// Register a module in the loader without evaluating it.
    /// Adds to the shared registry so the custom resolver/loader can find it.
    pub fn register_module(&mut self, specifier: &str, code: &str) -> Result<(), String> {
        self.module_registry.borrow_mut().modules.insert(
            specifier.to_string(),
            code.to_string(),
        );
        Ok(())
    }

    /// Add an import map entry: bare specifier -> resolved specifier.
    pub fn add_import_map_entry(&mut self, bare: &str, resolved: &str) {
        self.module_registry.borrow_mut().import_map.insert(
            bare.to_string(),
            resolved.to_string(),
        );
    }

    /// Returns a reference to the shared DomTree.
    pub fn tree(&self) -> &Rc<RefCell<DomTree>> {
        &self.tree
    }

    /// Returns a clone of the console output buffer.
    pub fn console_output(&self) -> Vec<String> {
        self.state.borrow().console_buffer.clone()
    }

    /// Clears the console output buffer.
    pub fn clear_console(&self) {
        self.state.borrow_mut().console_buffer.clear();
    }

    /// Deliver pending MutationObserver records to their callbacks.
    pub fn notify_mutation_observers(&mut self) {
        // TODO: implement when MutationObserver is ported
    }

    /// Run microtask queue (Promises).
    pub fn run_jobs(&mut self) -> bool {
        self.flush_jobs();
        true
    }

    /// Returns true if there are pending MutationObserver records.
    pub fn has_pending_mutation_observers(&self) -> bool {
        // TODO: implement when MutationObserver is ported
        false
    }

    /// Fire all timers whose deadline has passed. Returns true if any fired.
    /// Timers are fired in registration order (ascending ID) to match browser
    /// semantics: setTimeout(f, 0) calls registered earlier fire first.
    pub fn fire_ready_timers(&mut self) -> bool {
        let mut ready: Vec<(u32, bool)> = {
            let state = self.state.borrow();
            let current_time = state.timer_current_time_ms;
            state
                .timer_entries
                .values()
                .filter(|e| e.registered_at + e.delay_ms <= current_time)
                .map(|e| (e.id, e.is_interval))
                .collect()
        };

        if ready.is_empty() {
            return false;
        }

        // Fire in registration order (lower ID = registered first)
        ready.sort_by_key(|(id, _)| *id);

        for (id, is_interval) in ready {
            let callback_code = {
                let mut state = self.state.borrow_mut();
                if let Some(entry) = state.timer_entries.get(&id) {
                    let code = entry.callback_code.clone();
                    let current_time = state.timer_current_time_ms;
                    if is_interval {
                        let e = state.timer_entries.get_mut(&id).unwrap();
                        e.registered_at = current_time;
                    } else {
                        state.timer_entries.remove(&id);
                    }
                    Some(code)
                } else {
                    None
                }
            };

            if let Some(code) = callback_code {
                let _ = self.eval(&code);
            }
        }

        true
    }

    /// Advance the virtual timer clock to the next pending deadline.
    pub fn advance_timers_to_next_deadline(&mut self) -> bool {
        let mut state = self.state.borrow_mut();
        let current = state.timer_current_time_ms;

        let next_deadline = state
            .timer_entries
            .values()
            .map(|e| e.registered_at + e.delay_ms)
            .filter(|&deadline| deadline > current)
            .min();

        if let Some(deadline) = next_deadline {
            let capped = deadline.min(current + 10000);
            state.timer_current_time_ms = capped;
            true
        } else {
            false
        }
    }

    /// Returns true if there are any pending timer entries.
    pub fn has_pending_timers(&self) -> bool {
        !self.state.borrow().timer_entries.is_empty()
    }

    /// Returns the current virtual time in milliseconds.
    pub fn current_time_ms(&self) -> u64 {
        self.state.borrow().timer_current_time_ms
    }

    /// Set the virtual timer clock to a specific time.
    /// Used by settle_inner to snap virtual time forward when wall clock is ahead.
    pub fn set_timer_current_time(&self, time_ms: u64) {
        self.state.borrow_mut().timer_current_time_ms = time_ms;
    }

    /// Returns the earliest deadline across all pending timers,
    /// without actually advancing the clock.
    pub fn next_timer_deadline(&self) -> Option<u64> {
        let state = self.state.borrow();
        state
            .timer_entries
            .values()
            .map(|e| e.registered_at + e.delay_ms)
            .filter(|&d| d > state.timer_current_time_ms)
            .min()
    }

    // -- Abstraction methods --

    /// Dispatch a click event on a DOM element via its JS `.click()` method.
    pub fn click_element(&mut self, node_id: NodeId, _tree: Rc<RefCell<DomTree>>) {
        self.context.with(|ctx| {
            let global = ctx.globals();
            // Call __braille_click(nodeId) which dispatches through JS
            if let Ok(click_fn) = global.get::<_, Function>("__braille_click") {
                let _ = click_fn.call::<_, ()>((node_id as u32,));
            }
        });
    }

    /// Take the pending navigation URL if JS set location.href.
    pub fn take_pending_navigation(&self) -> Option<String> {
        self.state.borrow_mut().pending_navigation.take()
    }

    /// Returns true if there are pending fetch requests.
    pub fn has_pending_fetches(&self) -> bool {
        !self.state.borrow().pending_fetches.is_empty()
    }

    /// Returns all pending fetch requests as serializable DTOs.
    pub fn pending_fetches(&self) -> Vec<braille_wire::FetchRequest> {
        self.state
            .borrow()
            .pending_fetches
            .iter()
            .map(|pf| braille_wire::FetchRequest {
                id: pf.id,
                url: pf.url.clone(),
                method: pf.method.clone(),
                headers: pf.headers.clone(),
                body: pf.body.clone(),
            })
            .collect()
    }

    /// Resolve a pending fetch with a response.
    pub fn resolve_fetch(&mut self, id: u64, response: &braille_wire::FetchResponseData) {
        let entry = {
            let mut state = self.state.borrow_mut();
            let pos = state.pending_fetches.iter().position(|pf| pf.id == id);
            pos.map(|i| state.pending_fetches.remove(i))
        };

        if let Some(pf) = entry {
            self.context.with(|ctx| {
                let global = ctx.globals();
                if let Ok(resolve_fn) = global.get::<_, Function>("__braille_resolve_fetch") {
                    let response_json = serde_json::to_string(response).unwrap_or_default();
                    let _ = resolve_fn.call::<_, ()>((pf.resolve_id, response_json));
                }
            });
            self.flush_jobs();
        }
    }

    /// Reject a pending fetch with an error message.
    pub fn reject_fetch(&mut self, id: u64, error: &str) {
        let entry = {
            let mut state = self.state.borrow_mut();
            let pos = state.pending_fetches.iter().position(|pf| pf.id == id);
            pos.map(|i| state.pending_fetches.remove(i))
        };

        if let Some(pf) = entry {
            self.context.with(|ctx| {
                let global = ctx.globals();
                if let Ok(reject_fn) = global.get::<_, Function>("__braille_reject_fetch") {
                    let _ = reject_fn.call::<_, ()>((pf.reject_id, error.to_string()));
                }
            });
            self.flush_jobs();
        }
    }

    /// Set the location URL. Suppresses the navigate signal so the engine
    /// doesn't treat its own URL sync as a JS-initiated navigation.
    pub fn set_url(&self, url: &str) {
        self.state.borrow_mut().location_url = url.to_string();
        self.context.with(|ctx| {
            let _ = ctx.eval::<(), _>(format!(
                "if(typeof location !== 'undefined') {{ location.__suppress_nav = true; location.href = {:?}; location.__suppress_nav = false; }}",
                url
            ));
        });
    }

    /// Store pre-fetched iframe HTML content.
    pub fn populate_iframe_content(&self, iframes: &HashMap<String, String>) {
        let mut state = self.state.borrow_mut();
        for (url, content) in iframes {
            state
                .iframe_src_content
                .insert(url.clone(), content.clone());
        }
    }

    /// Fire `window.onload` handler.
    pub fn fire_window_load(&mut self) {
        self.context.with(|ctx| {
            let _ = ctx.eval::<(), _>(
                "if(typeof window !== 'undefined' && typeof window.onload === 'function') { window.onload(new Event('load')); }",
            );
        });
    }

    /// Process iframe loads.
    pub fn process_iframe_loads(&mut self, _tree: &Rc<RefCell<DomTree>>) {
        // TODO: implement iframe onload when iframe support is ported
    }

    /// Fire input and change events on an element (after handle_type sets the value).
    pub fn fire_input_events(&mut self, node_id: NodeId) {
        self.context.with(|ctx| {
            let code = format!(
                r#"(function() {{
                    var el = __braille_get_element_wrapper({nid});
                    if (!el) return;

                    if (el._valueTracker) {{
                        el._valueTracker.setValue('');
                    }}

                    var elId = el.getAttribute('id');

                    // Each event dispatch is isolated: errors in framework handlers
                    // must not prevent subsequent events from firing.
                    function safeDispatch(target, event) {{
                        try {{ target.dispatchEvent(event); }} catch(e) {{
                            if (!globalThis.__braille_dispatch_errors) globalThis.__braille_dispatch_errors = [];
                            globalThis.__braille_dispatch_errors.push(event.type + ': ' + (e.message || e));
                        }}
                    }}

                    // Focus lifecycle
                    try {{ el.focus(); }} catch(e) {{}}
                    safeDispatch(el, new FocusEvent('focusin', {{bubbles: true}}));
                    safeDispatch(el, new FocusEvent('focus', {{bubbles: false}}));

                    // Input/change events (for non-React frameworks and generic listeners)
                    safeDispatch(el, new Event('input', {{bubbles: true}}));
                    safeDispatch(el, new Event('change', {{bubbles: true}}));

                    // React 18 controlled input fallback: React's event delegation
                    // may reject our dispatched events (e.g. getNearestMountedFiber
                    // returns null, or containerInfo identity check fails). Directly
                    // invoke the React onChange handler from __reactProps$ as a
                    // fallback to ensure controlled input state updates.
                    var reactPropsKey = null;
                    var keys = Object.keys(el);
                    for (var i = 0; i < keys.length; i++) {{
                        if (keys[i].indexOf('__reactProps$') === 0) {{
                            reactPropsKey = keys[i];
                            break;
                        }}
                    }}
                    if (reactPropsKey) {{
                        var props = el[reactPropsKey];
                        if (props && typeof props.onChange === 'function') {{
                            try {{
                                props.onChange({{
                                    target: el,
                                    currentTarget: el,
                                    type: 'change',
                                    bubbles: true,
                                    preventDefault: function() {{}},
                                    stopPropagation: function() {{}},
                                    persist: function() {{}},
                                    nativeEvent: new Event('change', {{bubbles: true}})
                                }});
                            }} catch(e) {{
                                if (!globalThis.__braille_dispatch_errors) globalThis.__braille_dispatch_errors = [];
                                globalThis.__braille_dispatch_errors.push('react_onChange: ' + (e.message || e));
                            }}
                        }}
                    }}

                    // Blur lifecycle — re-resolve element in case React re-rendered
                    var blurEl = el;
                    if (elId) {{
                        var fresh = document.getElementById(elId);
                        if (fresh) blurEl = fresh;
                    }}
                    try {{ blurEl.blur(); }} catch(e) {{}}
                    safeDispatch(blurEl, new FocusEvent('focusout', {{bubbles: true}}));
                    safeDispatch(blurEl, new FocusEvent('blur', {{bubbles: false}}));
                }})()"#,
                nid = node_id
            );
            let _ = ctx.eval::<(), _>(code.as_str());
        });
        self.flush_jobs();
        // Drain 0ms timers (React scheduler uses MessageChannel shimmed to setTimeout(fn, 0))
        if self.fire_ready_timers() {
            self.flush_jobs();
        }
    }

    /// Synthesize MutationObserver records for parser-inserted nodes.
    pub fn synthesize_parser_mutations(&mut self, _tree: &Rc<RefCell<DomTree>>, _watermark: usize) {
        // TODO: implement when MutationObserver is ported
    }

    // -- Internal helpers --

    fn flush_jobs(&self) {
        while self.runtime.is_job_pending() {
            let _ = self.runtime.execute_pending_job();
        }
        // Drain any pending unhandled promise rejections
        let rejections: Vec<String> = PENDING_REJECTIONS.with(|pr| pr.borrow_mut().drain(..).collect());
        if !rejections.is_empty() {
            // Push reasons to JS array and invoke the drain function
            self.context.with(|ctx| {
                for reason in &rejections {
                    let escaped = reason.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n").replace('\r', "\\r");
                    let _ = ctx.eval::<(), _>(format!("__braille_pending_rejections.push('{escaped}')"));
                }
                let _ = ctx.eval::<(), _>("if(typeof __braille_drain_rejections==='function')__braille_drain_rejections()");
            });
            // Flush any jobs that the rejection handlers may have queued
            while self.runtime.is_job_pending() {
                let _ = self.runtime.execute_pending_job();
            }
        }
    }
}

fn js_value_to_string(val: &rquickjs::Value<'_>) -> String {
    if val.is_null() {
        "null".to_string()
    } else if val.is_undefined() {
        "undefined".to_string()
    } else if let Some(b) = val.as_bool() {
        if b { "true".to_string() } else { "false".to_string() }
    } else if let Some(n) = val.as_int() {
        format!("{n}")
    } else if let Some(n) = val.as_float() {
        if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
            format!("{}", n as i64)
        } else {
            format!("{n}")
        }
    } else if let Some(s) = val.as_string() {
        s.to_string().unwrap_or_else(|_| String::new())
    } else {
        // Object — try toString()
        val.get::<String>().unwrap_or_else(|_| "[object Object]".to_string())
    }
}

fn format_js_error(ctx: &rquickjs::Ctx<'_>, err: rquickjs::Error) -> String {
    match err {
        rquickjs::Error::Exception => {
            let exc = ctx.catch();
            if let Some(exc) = exc.as_exception() {
                let msg = exc.message().unwrap_or_default();
                let stack = exc.stack().unwrap_or_default();
                if stack.is_empty() {
                    msg
                } else {
                    format!("{msg}\n{stack}")
                }
            } else {
                format!("{exc:?}")
            }
        }
        other => format!("{other:?}"),
    }
}
