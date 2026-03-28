pub mod a11y;
pub mod commands;
pub mod cookies;
pub mod css;
pub mod dom;
mod fetch;
pub mod html;
pub mod js;
mod loading;
mod meta_refresh;
pub mod navigation;
mod scripts;
pub mod transcript;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Engine-level code should not reference Boa types directly.
// All JS engine operations go through JsRuntime methods.

use crate::dom::tree::DomTree;
use crate::dom::NodeId;
use crate::js::JsRuntime;
use braille_wire::SnapMode;

// Re-export types that moved to sub-modules so the public API doesn't change.
pub use crate::meta_refresh::{check_refresh_header, MetaRefresh};
pub use crate::navigation::{FetchProvider, MockFetcher};
pub use crate::scripts::ScriptDescriptor;

/// Pre-fetched resources for external scripts and iframe content.
#[derive(Default)]
pub struct FetchedResources {
    /// Maps script src URL -> fetched JavaScript content.
    pub scripts: HashMap<String, String>,
    /// Maps iframe src URL -> fetched HTML content.
    pub iframes: HashMap<String, String>,
}

// Note: derive(Default) used instead of manual impl

impl FetchedResources {
    /// Create a FetchedResources with only scripts (no iframes).
    pub fn scripts_only(scripts: HashMap<String, String>) -> Self {
        Self {
            scripts,
            iframes: HashMap::new(),
        }
    }
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

    fn settle_inner(&mut self, time_budget_ms: u64) {
        if let Some(runtime) = self.runtime.as_mut() {
            let starting_time = runtime.current_time_ms();

            for _ in 0..100 {
                // 1. Flush microtask queue (Promises from event handlers)
                runtime.run_jobs();

                // 2. Deliver pending MO records
                let had_mo = runtime.has_pending_mutation_observers();
                if had_mo {
                    runtime.notify_mutation_observers();
                    runtime.run_jobs();
                }

                // 3. Fire ready timers (delay <= current virtual time)
                let fired_timers = runtime.fire_ready_timers();

                if fired_timers {
                    // Timer callbacks may have queued MO/microtasks — loop again
                    continue;
                }

                // 4. No MO and no ready timers at current time — try advancing clock
                if !had_mo && !fired_timers {
                    if time_budget_ms > 0 && runtime.has_pending_timers() && !runtime.has_pending_fetches() {
                        // Only advance if the next deadline is within our time budget
                        if let Some(next) = runtime.next_timer_deadline() {
                            if next <= starting_time + time_budget_ms
                                && runtime.advance_timers_to_next_deadline()
                            {
                                continue;
                            }
                        }
                    }
                    // Truly quiescent (or waiting on fetches, or next timer is beyond budget)
                    break;
                }
            }
        }

        // Recompute CSS styles after all JS has settled
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
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

        let result = match mode {
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
            SnapMode::Markdown => "[Markdown mode not yet implemented]".to_string(),
        };

        // Drop the immutable borrow before restoring
        drop(tree);

        // Restore original attribute values
        self.restore_patched_values(patched);

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
                        let original = tree.get_attribute(nid, "value");
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

    /// Evaluate a JavaScript expression and return the result as a string.
    /// Panics if no runtime is loaded (call load_html or execute_scripts first).
    pub fn eval_js(&mut self, code: &str) -> Result<String, String> {
        let runtime = self.runtime.as_mut().expect("eval_js: no runtime loaded");
        runtime.eval_to_string(code)
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

    /// Set the location URL in the JS runtime (e.g., after navigation).
    /// If no runtime exists yet, the URL is stored and applied when the runtime
    /// is created (so scripts see the correct location from the start).
    pub fn set_url(&mut self, url: &str) {
        if let Some(runtime) = &self.runtime {
            runtime.set_url(url);
        }
        self.pending_url = Some(url.to_string());
    }
}

// The #[cfg(test)] module and all tests are preserved below via include.
// This avoids copying 1350+ lines of test code during the refactor.
// The tests use `super::*` which picks up all re-exports.

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
