pub mod a11y;
pub mod browser_events;
pub mod commands;
pub mod cookies;
pub mod css;
pub mod dom;
mod fetch;
pub mod html;
pub mod js;
pub mod layout;
mod loading;
mod meta_refresh;
pub mod navigation;
pub mod permissions;
mod scripts;
pub mod transcript;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Engine-level code should not reference QuickJS types directly.
// All JS engine operations go through JsRuntime methods.

use crate::dom::tree::DomTree;
use crate::dom::NodeId;
use crate::js::JsRuntime;
use crate::permissions::{PermissionState, Permissions};
use braille_wire::{BrowserEvent, SnapMode};

// Re-export types that moved to sub-modules so the public API doesn't change.
pub use crate::meta_refresh::{check_refresh_header, MetaRefresh};
pub use crate::navigation::{FetchProvider, MockFetcher};
pub use crate::scripts::ScriptDescriptor;

/// Pre-fetched resources for external scripts, iframe content, and stylesheets.
#[derive(Default)]
pub struct FetchedResources {
    /// Maps script src URL -> fetched JavaScript content.
    pub scripts: HashMap<String, String>,
    /// Maps iframe src URL -> fetched HTML content.
    pub iframes: HashMap<String, String>,
    /// Maps link href URL -> fetched CSS content.
    pub css: HashMap<String, String>,
}

// Note: derive(Default) used instead of manual impl

impl FetchedResources {
    /// Create a FetchedResources with only scripts (no iframes).
    pub fn scripts_only(scripts: HashMap<String, String>) -> Self {
        Self {
            scripts,
            iframes: HashMap::new(),
            css: HashMap::new(),
        }
    }
}

/// Controls whether the JS runtime is reused across page loads.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Reuse the JS runtime across page loads (default). Rebinds tree/state
    /// without re-registering globals. ~18x faster on subsequent loads.
    #[default]
    Fast,
    /// Create a fresh JS runtime for every page load.
    Clean,
}

/// The core browser engine. Parses HTML, executes JavaScript, and produces
/// accessibility-tree snapshots for LLM agents to read and interact with.
///
/// # Loading HTML
///
/// | Method | Use when |
/// |--------|----------|
/// | [`load_html`](Self::load_html) | Inline scripts only, panics on JS errors |
/// | [`load_html_with_scripts`](Self::load_html_with_scripts) | External `<script src>` files, panics on JS errors |
/// | [`load_html_with_resources`](Self::load_html_with_resources) | External scripts + iframe content, panics on JS errors |
/// | [`load_html_with_scripts_lossy`](Self::load_html_with_scripts_lossy) | External scripts, collects JS errors |
/// | [`load_html_with_resources_lossy`](Self::load_html_with_resources_lossy) | External scripts + iframes, collects JS errors |
/// | [`load_html_incremental_with_resources_lossy`](Self::load_html_incremental_with_resources_lossy) | MutationObserver tests needing parser-interleaved script execution |
/// | [`parse_and_collect_scripts`](Self::parse_and_collect_scripts) + [`execute_scripts`](Self::execute_scripts) | Two-phase: parse first, fetch externals, then execute |
///
/// # Interaction
///
/// After loading, call [`snapshot`](Self::snapshot) to get a text representation,
/// then use [`handle_click`](Self::handle_click), [`handle_type`](Self::handle_type),
/// [`handle_select`](Self::handle_select), or [`handle_focus`](Self::handle_focus)
/// with element refs (e.g. `@e1`) from the snapshot.
pub struct Engine {
    pub(crate) tree: Rc<RefCell<DomTree>>,
    pub(crate) runtime: Option<JsRuntime>,
    // DESIGN NOTE: ref_map is only populated after a snapshot() call with Accessibility mode.
    // If resolve_ref is called before snapshot, it will return None for all refs.
    // This is intentional - refs are tied to a specific accessibility tree snapshot.
    pub(crate) ref_map: HashMap<String, NodeId>,
    pub(crate) focused_element: Option<NodeId>,
    /// URL to set on the JS runtime when it is next created (before scripts run).
    pub(crate) pending_url: Option<String>,
    /// HTTP-level cookie jar for Set-Cookie ↔ document.cookie sync.
    pub(crate) http_cookie_jar: Vec<cookies::StoredCookie>,
    /// Whether cookies need syncing to JS on next runtime creation.
    pub(crate) cookies_pending_js_sync: bool,
    /// Controls whether JS runtime is reused across page loads.
    pub runtime_mode: RuntimeMode,
    /// When true, snapshot() appends a [Hidden Content] section listing display:none
    /// and visibility:hidden text. Intended for CLI/agent use. Default is false.
    pub include_hidden_content: bool,
    /// Tracks elements that have already fired their initial scroll-snap scrollend.
    /// Reset when an element goes back to display:none so it fires again on re-show.
    /// Permission states (persists across page loads within session).
    pub permissions: Permissions,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            tree: Rc::new(RefCell::new(DomTree::new())),
            runtime: None,
            ref_map: HashMap::new(),
            focused_element: None,
            pending_url: None,
            http_cookie_jar: Vec::new(),
            cookies_pending_js_sync: false,
            runtime_mode: RuntimeMode::default(),
            include_hidden_content: false,
            permissions: Permissions::default(),
        }
    }

    /// Flush microtasks, MutationObserver records, and recompute CSS styles.
    /// Advances virtual time up to 1 second to fire short timers (setTimeout(0),
    /// RAF, debounces) while leaving long-interval polling frozen.
    pub fn settle(&mut self) {
        self.settle_inner(1000);
    }

    /// Like settle(), but does NOT advance virtual time. Only processes
    /// microtasks, mutation observers, and timers that are already due.
    /// Use this during fetch interleaving to avoid firing interval timers
    /// repeatedly (e.g., version polling).
    pub fn settle_no_advance(&mut self) {
        self.settle_inner(0);
    }

    /// Drain all pending JS work (microtasks, MO records, timers) without
    /// touching CSS or observers. Returns true if any work was done.
    pub fn drain_js_work(&mut self) -> bool {
        let runtime = self.runtime.as_mut().unwrap();
        let mut did_work = false;

        for _ in 0..500 {
            // 1. Flush microtask queue
            runtime.run_jobs();

            // 2. Deliver pending MO records
            let had_mo = runtime.has_pending_mutation_observers();
            if had_mo {
                runtime.notify_mutation_observers();
                runtime.run_jobs();
                did_work = true;
                continue;
            }

            // 3. Fire ready timers
            if runtime.fire_ready_timers() {
                did_work = true;
                continue;
            }

            break;
        }
        did_work
    }

    fn settle_inner(&mut self, time_budget_ms: u64) {
        if self.runtime.is_none() {
            crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
            return;
        }

        let starting_time = self.runtime.as_mut().unwrap().current_time_ms();
        let wall_start = std::time::Instant::now();

        // Outer loop: drain JS → render pass → repeat if render caused new JS work
        for _ in 0..20 {
            // Phase 1: Drain all JS work (microtasks, MO, timers) — no CSS
            self.drain_js_work();

            // Phase 2: Try advancing the clock to fire more timers
            let advanced = {
                let runtime = self.runtime.as_mut().unwrap();
                let mut clock_advanced = false;
                if time_budget_ms > 0 && runtime.has_pending_timers() && !runtime.has_pending_fetches() {
                    if let Some(next) = runtime.next_timer_deadline() {
                        if next <= starting_time + time_budget_ms {
                            let virtual_target = next - starting_time;
                            let wall_elapsed = wall_start.elapsed().as_millis() as u64;

                            if wall_elapsed < virtual_target {
                                std::thread::sleep(std::time::Duration::from_millis(
                                    virtual_target - wall_elapsed,
                                ));
                            } else if wall_elapsed > virtual_target {
                                let wall_based_time = starting_time + wall_elapsed;
                                let capped = wall_based_time.min(starting_time + time_budget_ms);
                                runtime.set_timer_current_time(capped);
                            }

                            clock_advanced = runtime.advance_timers_to_next_deadline();
                        }
                    }
                }
                clock_advanced
            };

            if advanced {
                // New timers fired — drain their JS work before render pass
                self.drain_js_work();
            }

            // Phase 3: Render pass — CSS, observers, focus (once per settle cycle)
            // Focus/hover state is already pushed to tree by JS via __n_setFocusedNode
            // and __n_setHoveredNode — no eval polling needed.
            crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());

            let mut needs_relayout = self.validate_focus_after_styles();

            // ResizeObserver — direct function call, no eval
            if self.runtime.as_mut().unwrap().call_global_fn_bool("__ro_check") {
                needs_relayout = true;
            }

            // IntersectionObserver — direct function call, no eval
            if self.runtime.as_mut().unwrap().call_global_fn_bool("__io_check") {
                needs_relayout = true;
            }

            if self.fire_scroll_snap_events() {
                needs_relayout = true;
            }

            if !needs_relayout && !advanced {
                // Truly quiescent — no more work
                break;
            }

            // Observers or focus fired new JS work — loop back to drain it,
            // but skip the expensive render pass until JS is quiescent again
        }
    }

    /// After style recomputation, check if the focused element ended up in an
    /// unfocusable context (inert subtree or display:none subtree). If so, clear
    /// focus and fire blur/focusout events. Returns true if focus was invalidated
    /// (caller should re-loop to process resulting microtasks).
    ///
    /// Implements the HTML spec's "focus fixup rule". Checks are done in Rust
    /// (cheap ancestor walks); JS is only used for event dispatch.
    fn validate_focus_after_styles(&mut self) -> bool {
        // Use tree.focused_node as the single source of truth (already synced
        // from both Rust-side focused_element and JS-side __focusCtx.el).
        let focused = match self.tree.borrow().focused_node {
            Some(id) => id,
            None => return false,
        };

        let tree = self.tree.borrow();
        let should_blur =
            tree.is_in_inert_subtree(focused) || tree.is_in_display_none_subtree(focused);
        drop(tree);

        if !should_blur {
            return false;
        }

        // Clear focus on all three trackers
        self.focused_element = None;
        self.tree.borrow_mut().focused_node = None;

        if let Some(runtime) = self.runtime.as_mut() {
            let js = format!(
                r#"(function(){{
                    var el = __braille_get_element_wrapper({});
                    __focusCtx.el = null;
                    if (!el) return;
                    el.dispatchEvent(new FocusEvent('focusout', {{bubbles:true, relatedTarget:null}}));
                    el.dispatchEvent(new FocusEvent('blur', {{bubbles:false, relatedTarget:null}}));
                }})()"#,
                focused
            );
            let _ = runtime.eval(&js);
            runtime.run_jobs();
        }
        true
    }

    /// Check for mandatory scroll-snap containers that are newly visible and
    /// fire `scrollend` on them. Returns true if any events were fired.
    fn fire_scroll_snap_events(&mut self) -> bool {
        let tree = self.tree.borrow();
        let mut targets = Vec::new();

        for nid in 0..tree.node_count() {
            let node = tree.get_node(nid);
            if let Some(cs) = &node.computed_style {
                let snap_type = cs.get("scroll-snap-type").map(|s| s.as_str()).unwrap_or("none");
                let display = cs.get("display").map(|s| s.as_str()).unwrap_or("inline");

                if snap_type.contains("mandatory") && display != "none" {
                    targets.push(nid);
                }
            }
        }
        drop(tree);

        if targets.is_empty() {
            return false;
        }

        let mut did_snap = false;
        if let Some(runtime) = self.runtime.as_mut() {
            for nid in targets {
                // Snap if the current scroll position is NOT at a snap point,
                // OR if this is a newly-visible mandatory container that hasn't
                // had its initial snap yet (fire scrollend even at position 0).
                let code = format!(
                    r#"(function(){{
                        var el = __braille_get_element_wrapper({});
                        if (!el) return '';
                        var snapped = __computeSnapOffset(el, el.scrollLeft, el.scrollTop);
                        var needsSnap = (snapped.x !== el.scrollLeft || snapped.y !== el.scrollTop);
                        var firstInit = !el.__snap_initialized;
                        if (needsSnap) {{
                            el.__snap_initialized = true;
                            el.scrollTo({{ left: snapped.x, top: snapped.y }});
                            return '1';
                        }} else if (firstInit) {{
                            el.__snap_initialized = true;
                            el.dispatchEvent(new Event('scrollend', {{bubbles: false}}));
                            return '1';
                        }}
                        return '';
                    }})()"#,
                    nid
                );
                if let Ok(result) = runtime.eval_to_string(&code) {
                    if result == "1" {
                        runtime.run_jobs();
                        did_snap = true;
                    }
                }
            }
        }

        did_snap
    }

    /// Deliver a WebSocket event to a JS WebSocket instance.
    /// `event_type` is one of: "open", "message", "error", "close"
    pub fn ws_deliver_event(&mut self, id: u32, event_type: &str, data: &str) {
        if let Some(runtime) = self.runtime.as_mut() {
            let code = format!(
                "if (typeof __braille_ws_deliver === 'function') __braille_ws_deliver({}, '{}', {})",
                id,
                event_type,
                serde_json::to_string(data).unwrap_or_else(|_| "\"\"".to_string())
            );
            runtime.eval_or_log(&code);
        }
    }

    /// Deliver a dynamically-fetched module source to the engine.
    pub fn deliver_module(&mut self, specifier: &str, source: &str) {
        if let Some(runtime) = self.runtime.as_mut() {
            let _ = runtime.register_module(specifier, source);
        }
    }

    pub fn snapshot(&mut self, mode: SnapMode) -> String {
        use crate::a11y::serialize;

        // Temporarily sync JS-side dirty property values into the DOM tree so the
        // serializer can see them. We record which nodes were patched so we can
        // restore the original attribute values afterward to maintain spec correctness
        // (the value attribute should not be changed by .value property assignment).
        let patched = self.sync_dirty_values_to_tree();

        // Always do a full ref assignment first so @eN is stable across views
        let tree = self.tree.borrow();
        let (ref_map, reverse) = serialize::assign_refs(&tree);
        self.ref_map = ref_map;

        let append_webmcp = matches!(mode, SnapMode::Compact | SnapMode::Accessibility);

        let mut result = match mode {
            SnapMode::Compact => {
                let (output, ref_map) = serialize::serialize_compact(&tree, self.focused_element);
                self.ref_map = ref_map;
                output
            }
            SnapMode::Accessibility => {
                let (output, ref_map) = serialize::serialize_a11y(&tree, self.focused_element);
                self.ref_map = ref_map;
                output
            }
            SnapMode::Interactive => serialize::serialize_interactive(&tree, &reverse, self.focused_element),
            SnapMode::Links => serialize::serialize_links(&tree, &reverse),
            SnapMode::Forms => serialize::serialize_forms(&tree, &reverse),
            SnapMode::Headings => serialize::serialize_headings(&tree),
            SnapMode::Text => serialize::serialize_text(&tree),
            SnapMode::Selector(ref selector) => serialize::serialize_selector(&tree, selector, &reverse),
            SnapMode::Region(ref target) => {
                let target_id = crate::dom::find::resolve_selector(&tree, &self.ref_map, target);
                match target_id {
                    Some(id) => serialize::serialize_region(&tree, id, &reverse, self.focused_element),
                    None => format!("error: target not found: {}", target),
                }
            }
            SnapMode::Dom => "[DOM mode not yet implemented]".to_string(),
            SnapMode::Markdown => serialize::serialize_markdown(&tree),
        };

        if append_webmcp {
            let webmcp = serialize::collect_webmcp_section(&tree);
            if !webmcp.is_empty() {
                result.push_str(&webmcp);
            }
        }

        // Append hidden content section so agents know what's hidden on the page.
        // Only when include_hidden_content is enabled (CLI/agent use).
        if self.include_hidden_content {
            let hidden = serialize::collect_hidden_content(&tree);
            if !hidden.is_empty() {
                result.push_str(&hidden);
            }
        }

        // Drop the immutable borrow before restoring
        drop(tree);

        // Restore original attribute values
        self.restore_patched_values(patched);

        // Append browser events footer if any are pending
        let event_count = self.browser_event_count();
        if event_count > 0 {
            result.push_str(&format!(
                "\n[Browser Events: {} pending. Use 'events' to view.]",
                event_count
            ));
        }

        result
    }

    /// Temporarily sync JS-side dirty property values into the DOM tree's value attributes.
    /// Returns a list of (NodeId, Option<original_value>) so we can restore them.
    fn sync_dirty_values_to_tree(&mut self) -> Vec<(NodeId, Option<String>)> {
        let mut patched = Vec::new();
        if let Some(runtime) = self.runtime.as_mut() {
            if let Ok(json) = runtime.eval_to_string("__braille_collect_dirty_values()") {
                if let Ok(pairs) = serde_json::from_str::<Vec<(usize, String)>>(&json) {
                    let mut tree = self.tree.borrow_mut();
                    for (nid, val) in pairs {
                        let original = tree.get_attribute(nid, "value").map(|v| v.to_string());
                        patched.push((nid, original));
                        tree.set_attribute(nid, "value", &val);
                    }
                }
            }
        }
        patched
    }

    /// Restore original attribute values after snapshot.
    fn restore_patched_values(&mut self, patched: Vec<(NodeId, Option<String>)>) {
        let mut tree = self.tree.borrow_mut();
        for (nid, original) in patched {
            match original {
                Some(val) => tree.set_attribute(nid, "value", &val),
                None => { tree.remove_attribute(nid, "value"); },
            }
        }
    }

    /// Resolve an element reference string (e.g., "@e1") to its NodeId.
    /// Returns None if the ref is not found or if snapshot() has not been called yet.
    ///
    /// DESIGN NOTE: This method only works after calling snapshot() with Accessibility mode.
    /// The ref_map is tied to the most recent accessibility snapshot.
    pub fn resolve_ref(&self, ref_str: &str) -> Option<NodeId> {
        self.ref_map.get(ref_str).copied()
    }

    /// Returns true if a JS runtime is loaded (i.e., a page has been loaded).
    pub fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    /// Get the page title from the DOM tree (first `<title>` element's text content).
    /// Returns None if no title element exists or its text is empty.
    pub fn get_title(&self) -> Option<String> {
        let tree = self.tree.borrow();
        let title_id = tree.find_element_by_tag("title")?;
        let text = tree.get_text_content(title_id);
        if text.is_empty() { None } else { Some(text) }
    }

    /// Evaluate a JavaScript expression and return the result as a string.
    /// Panics if no runtime is loaded (call load_html or execute_scripts first).
    pub fn eval_js(&mut self, code: &str) -> Result<String, String> {
        let runtime = self.runtime.as_mut().expect("eval_js: no runtime loaded");
        runtime.eval_to_string(code)
    }

    /// Evaluate JS, logging any errors to the console buffer instead of returning them.
    /// Errors appear via `drain_console()` as `[error] ...` entries.
    pub fn eval_js_or_log(&mut self, code: &str) {
        if let Err(e) = self.eval_js(code) {
            crate::js::dom_bridge::with_state_mut(|s| {
                s.console_buffer.push(format!("[error] {e}"));
            });
        }
    }

    /// Returns all console output (log, warn, error, etc.) since last drain.
    pub fn console_output(&self) -> Vec<String> {
        if let Some(runtime) = &self.runtime {
            runtime.console_output()
        } else {
            Vec::new()
        }
    }

    /// Returns and clears all console output since last drain.
    pub fn drain_console(&self) -> Vec<String> {
        if let Some(runtime) = &self.runtime {
            let output = runtime.console_output();
            runtime.clear_console();
            output
        } else {
            Vec::new()
        }
    }

    /// Fire a keyboard event on an element identified by selector.
    /// Exposed for testing; the main entry point is `handle_send_keys`.
    pub fn fire_keyboard_event_on(
        &mut self,
        selector: &str,
        event_type: &str,
        key: &str,
        code: &str,
    ) -> Result<(), String> {
        let node_id = {
            let tree = self.tree.borrow();
            crate::dom::find::resolve_selector(&tree, &self.ref_map, selector)
                .ok_or_else(|| format!("element not found: {}", selector))?
        };
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.fire_keyboard_event(node_id, event_type, key, code);
        }
        Ok(())
    }

    /// Move input cursor using a key name, on an element identified by selector.
    /// Exposed for testing; normally called internally by `handle_send_keys`.
    pub fn move_input_cursor_on(
        &mut self,
        selector: &str,
        key: &str,
    ) -> Result<(), String> {
        let node_id = {
            let tree = self.tree.borrow();
            crate::dom::find::resolve_selector(&tree, &self.ref_map, selector)
                .ok_or_else(|| format!("element not found: {}", selector))?
        };
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.move_input_cursor(node_id, key);
        }
        Ok(())
    }

    /// Get the engine's virtual time in milliseconds since epoch.
    pub fn virtual_time_ms(&self) -> u64 {
        match &self.runtime {
            Some(r) => r.current_time_ms(),
            None => 0,
        }
    }

    /// Set the engine's virtual time in milliseconds since epoch.
    pub fn set_virtual_time_ms(&mut self, time_ms: u64) {
        if let Some(r) = &self.runtime {
            r.set_timer_current_time(time_ms);
        }
    }

    /// Set the location URL in the JS runtime (e.g., after navigation).
    /// If no runtime exists yet, the URL is stored and applied when the runtime
    /// is created (so scripts see the correct location from the start).
    pub fn set_url(&mut self, url: &str) {
        if let Some(runtime) = &self.runtime {
            runtime.set_url(url);
        }
        self.pending_url = Some(url.to_string());
    }

    // --- Browser Events API ---

    /// Take all pending browser events, clearing the queue.
    pub fn take_browser_events(&mut self) -> Vec<BrowserEvent> {
        if let Some(runtime) = &self.runtime {
            runtime.state.borrow_mut().browser_events.drain()
        } else {
            Vec::new()
        }
    }

    /// Number of pending browser events.
    pub fn browser_event_count(&self) -> usize {
        if let Some(runtime) = &self.runtime {
            runtime.state.borrow().browser_events.len()
        } else {
            0
        }
    }

    /// Respond to a blocking browser event (alert/confirm/prompt).
    /// Clears the blocking state so JS can resume on next settle().
    pub fn respond_to_event(&mut self, id: u64, value: String) {
        if let Some(runtime) = &self.runtime {
            let mut state = runtime.state.borrow_mut();
            if state.blocking_event_id == Some(id) {
                state.blocking_event_response = Some(value);
                state.blocking_event_id = None;
            }
        }
    }

    /// Dismiss an info-only browser event by ID.
    pub fn dismiss_event(&mut self, id: u64) {
        if let Some(runtime) = &self.runtime {
            runtime.state.borrow_mut().browser_events.remove(id);
        }
    }

    /// Set a permission by name.
    pub fn set_permission(&mut self, name: &str, state: PermissionState) -> bool {
        self.permissions.set(name, state)
    }

    /// Check if JS execution is blocked waiting for an event response.
    pub fn is_blocked_on_event(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            runtime.state.borrow().blocking_event_id.is_some()
        } else {
            false
        }
    }
}

// The #[cfg(test)] module and all tests are preserved below via include.
// This avoids copying 1350+ lines of test code during the refactor.
// The tests use `super::*` which picks up all re-exports.

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
