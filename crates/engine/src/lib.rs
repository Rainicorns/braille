pub mod a11y;
pub mod commands;
pub mod css;
pub mod dom;
pub mod html;
pub mod js;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Engine-level code should not reference Boa types directly.
// All JS engine operations go through JsRuntime methods.

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;
use crate::js::JsRuntime;
use braille_wire::SnapMode;

/// Represents a script to be executed — either inline or external (needing fetch).
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptDescriptor {
    /// Classic script text content, ready to execute. Second field is the script element's NodeId.
    Inline(String, Option<NodeId>),
    /// A classic script src URL that needs to be fetched by the host. Second field is the script element's NodeId.
    External(String, Option<NodeId>),
    /// ES module inline script (`<script type="module">...</script>`).
    InlineModule(String),
    /// ES module external script (`<script type="module" src="...">`).
    ExternalModule(String),
    /// Import map JSON text (`<script type="importmap">...</script>`).
    ImportMap(String),
}

impl ScriptDescriptor {
    /// Returns true if this is a module script (inline or external).
    pub fn is_module(&self) -> bool {
        matches!(self, Self::InlineModule(_) | Self::ExternalModule(_))
    }

    /// Returns the external URL if this is an External or ExternalModule descriptor.
    pub fn external_url(&self) -> Option<&str> {
        match self {
            Self::External(url, _) | Self::ExternalModule(url) => Some(url),
            _ => None,
        }
    }
}

/// Per HTML spec, a script type is JavaScript only if it matches one of these
/// (case-insensitive, after trimming). Empty string also means JS.
fn is_javascript_type(type_value: &str) -> bool {
    let t = type_value.trim();
    if t.is_empty() {
        return true;
    }
    matches!(
        t.to_ascii_lowercase().as_str(),
        "text/javascript"
            | "application/javascript"
            | "text/ecmascript"
            | "application/ecmascript"
            | "text/jscript"
            | "text/livescript"
    )
}

/// Pre-fetched resources for external scripts and iframe content.
#[derive(Default)]
pub struct FetchedResources {
    /// Maps script src URL → fetched JavaScript content.
    pub scripts: HashMap<String, String>,
    /// Maps iframe src URL → fetched HTML content.
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
    pending_url: Option<String>,
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
        }
    }

    pub fn load_html(&mut self, html: &str) {
        // 1. Parse HTML into a DomTree
        let tree = crate::html::parse_html(html);
        self.tree = tree;

        // 2. Create a new JsRuntime bound to this tree
        let mut runtime = JsRuntime::new(Rc::clone(&self.tree));

        // 3. Walk the tree to find all <script> elements in document order,
        //    collect their text content, and execute each one.
        let scripts = self.collect_scripts();
        for script_content in scripts {
            if !script_content.trim().is_empty() {
                // Execute the script; let errors propagate as panics (fail fast)
                runtime.eval(&script_content).unwrap();
                runtime.notify_mutation_observers();
            }
        }

        // 4. Store the runtime
        self.runtime = Some(runtime);

        // 5. Reset focus when loading new page
        self.focused_element = None;

        // 6. Compute CSS styles after script execution
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
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

    /// Walk the DomTree recursively from the document root, collecting the text
    /// content of each `<script>` element in document order.
    fn collect_scripts(&self) -> Vec<String> {
        let tree = self.tree.borrow();
        let mut scripts = Vec::new();
        Self::walk_for_scripts(&tree, tree.document(), &mut scripts);
        scripts
    }

    fn walk_for_scripts(tree: &DomTree, root: NodeId, scripts: &mut Vec<String>) {
        let mut stack = vec![root];
        while let Some(node_id) = stack.pop() {
            let node = tree.get_node(node_id);
            if let NodeData::Element {
                tag_name, attributes, ..
            } = &node.data
            {
                if tag_name.eq_ignore_ascii_case("script") {
                    // Check type attribute — skip non-JS scripts (data blocks)
                    let type_attr = attributes.iter().find(|a| a.local_name == "type").map(|a| a.value.as_str());
                    if let Some(t) = type_attr {
                        if !is_javascript_type(t) && !t.trim().eq_ignore_ascii_case("module") {
                            continue; // data block (importmap, ld+json, etc.) — skip
                        }
                    }
                    let text = tree.get_text_content(node_id);
                    scripts.push(text);
                    continue; // don't descend into script children
                }
            }
            for &child_id in node.children.iter().rev() {
                stack.push(child_id);
            }
        }
    }

    /// Walk the DomTree recursively from the document root, collecting script descriptors
    /// for each `<script>` element in document order.
    /// Per HTML spec: if a script has a `src` attribute, the src wins (inline text is ignored).
    fn collect_script_descriptors(&self) -> Vec<ScriptDescriptor> {
        let tree = self.tree.borrow();
        let mut descriptors = Vec::new();
        Self::walk_for_script_descriptors(&tree, tree.document(), &mut descriptors);
        descriptors
    }

    fn walk_for_script_descriptors(tree: &DomTree, root: NodeId, descriptors: &mut Vec<ScriptDescriptor>) {
        let mut stack = vec![root];
        while let Some(node_id) = stack.pop() {
            let node = tree.get_node(node_id);
            if let NodeData::Element {
                tag_name, attributes, ..
            } = &node.data
            {
                if tag_name.eq_ignore_ascii_case("script") {
                    // Skip nomodule scripts — they are fallback for browsers that don't support modules
                    let has_nomodule = attributes.iter().any(|a| a.local_name == "nomodule");
                    if has_nomodule {
                        continue;
                    }

                    // 4-way type decision per HTML spec
                    let type_attr = attributes
                        .iter()
                        .find(|a| a.local_name == "type")
                        .map(|a| a.value.as_str());

                    let type_trimmed = type_attr.map(|t| t.trim().to_ascii_lowercase());

                    if let Some(ref t) = type_trimmed {
                        if t == "module" {
                            // Module script
                            let src = attributes.iter().find(|a| a.local_name == "src").map(|a| a.value.clone());
                            if let Some(url) = src {
                                descriptors.push(ScriptDescriptor::ExternalModule(url));
                            } else {
                                descriptors.push(ScriptDescriptor::InlineModule(tree.get_text_content(node_id)));
                            }
                        } else if t == "importmap" {
                            // Import map — store JSON text
                            descriptors.push(ScriptDescriptor::ImportMap(tree.get_text_content(node_id)));
                        } else if is_javascript_type(t) {
                            // Explicit JS MIME type — treat as classic script
                            let src = attributes.iter().find(|a| a.local_name == "src").map(|a| a.value.clone());
                            if let Some(url) = src {
                                descriptors.push(ScriptDescriptor::External(url, Some(node_id)));
                            } else {
                                descriptors.push(ScriptDescriptor::Inline(tree.get_text_content(node_id), Some(node_id)));
                            }
                        }
                        // else: data block (ld+json, etc.) — skip entirely
                    } else {
                        // No type attribute — classic script
                        let src = attributes.iter().find(|a| a.local_name == "src").map(|a| a.value.clone());
                        if let Some(url) = src {
                            descriptors.push(ScriptDescriptor::External(url, Some(node_id)));
                        } else {
                            descriptors.push(ScriptDescriptor::Inline(tree.get_text_content(node_id), Some(node_id)));
                        }
                    }
                    continue; // don't descend into script children
                }
            }
            for &child_id in node.children.iter().rev() {
                stack.push(child_id);
            }
        }
    }

    /// Parse HTML and identify all scripts (inline and external) in document order.
    /// Does NOT execute anything. Returns list of script descriptors.
    /// Stores the parsed tree in self.tree.
    pub fn parse_and_collect_scripts(&mut self, html: &str) -> Vec<ScriptDescriptor> {
        let tree = crate::html::parse_html(html);
        self.tree = tree;
        self.collect_script_descriptors()
    }

    /// Execute scripts with externally-fetched content.
    /// `descriptors` is the list returned by parse_and_collect_scripts.
    /// `fetched` contains pre-fetched scripts and iframe content.
    /// Executes all scripts in document order, substituting external content.
    /// Skips external scripts whose URL is not found in `fetched.scripts`.
    pub fn execute_scripts(&mut self, descriptors: &[ScriptDescriptor], fetched: &FetchedResources) {
        let mut runtime = JsRuntime::new(Rc::clone(&self.tree));

        // Apply pending URL so scripts see correct location from the start
        if let Some(url) = &self.pending_url {
            runtime.set_url(url);
        }

        // Populate iframe_src_content in RealmState with pre-fetched iframe content
        Self::populate_iframe_src_content(&fetched.iframes, &runtime);

        // Pre-register external modules so import statements can resolve them
        for descriptor in descriptors {
            if let ScriptDescriptor::ExternalModule(url) = descriptor {
                if let Some(script_content) = fetched.scripts.get(url) {
                    if !script_content.trim().is_empty() {
                        // Best-effort: if parsing fails, we'll get an error when the module is imported
                        let _ = runtime.register_module(url, script_content);
                    }
                }
            }
        }

        for descriptor in descriptors {
            match descriptor {
                ScriptDescriptor::Inline(text, nid) => {
                    if !text.trim().is_empty() {
                        if let Some(nid) = nid {
                            let _ = runtime.eval(&format!("document.currentScript = __braille_get_element_wrapper({nid})"));
                        }
                        runtime.eval(text).unwrap();
                        let _ = runtime.eval("document.currentScript = null");
                        runtime.notify_mutation_observers();
                    }
                }
                ScriptDescriptor::External(url, nid) => {
                    if let Some(script_content) = fetched.scripts.get(url) {
                        if !script_content.trim().is_empty() {
                            if let Some(nid) = nid {
                                let _ = runtime.eval(&format!("document.currentScript = __braille_get_element_wrapper({nid})"));
                            }
                            runtime.eval(script_content).unwrap();
                            let _ = runtime.eval("document.currentScript = null");
                            runtime.notify_mutation_observers();
                        }
                    }
                }
                ScriptDescriptor::InlineModule(text) => {
                    if !text.trim().is_empty() {
                        runtime.eval_module(text, None).unwrap();
                        runtime.notify_mutation_observers();
                    }
                }
                ScriptDescriptor::ExternalModule(url) => {
                    if let Some(script_content) = fetched.scripts.get(url) {
                        if !script_content.trim().is_empty() {
                            runtime.eval_module(script_content, Some(url)).unwrap();
                            runtime.notify_mutation_observers();
                        }
                    }
                }
                ScriptDescriptor::ImportMap(json) => {
                    Self::process_import_map(&mut runtime, json, fetched);
                }
            }
        }

        // Fire DOMContentLoaded on document (defer scripts have all run)
        let _ = runtime.eval("document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));");
        runtime.run_jobs();

        // Fire onload handlers for iframes with pre-fetched content
        Self::process_iframe_loads(&mut runtime, &self.tree);

        // Fire window.onload handler
        Self::fire_window_load(&mut runtime);

        self.runtime = Some(runtime);
        self.focused_element = None;

        // Compute CSS styles after script execution
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
    }

    /// Convenience: load HTML with external script and iframe support.
    /// Combines parse_and_collect_scripts + execute_scripts.
    /// For pages with only inline scripts, pass `FetchedResources::default()`.
    pub fn load_html_with_resources(&mut self, html: &str, fetched: &FetchedResources) {
        let descriptors = self.parse_and_collect_scripts(html);
        self.execute_scripts(&descriptors, fetched);
    }

    /// Convenience wrapper around [`load_html_with_resources`](Self::load_html_with_resources)
    /// for the common case of scripts-only (no iframes).
    /// `fetched` maps src URLs to their fetched JavaScript content.
    /// For pages with only inline scripts, pass an empty HashMap.
    pub fn load_html_with_scripts(&mut self, html: &str, fetched: &HashMap<String, String>) {
        self.load_html_with_resources(html, &FetchedResources::scripts_only(fetched.clone()));
    }

    /// Execute scripts without panicking on JS errors.
    /// Returns Ok(()) if all scripts executed, or Err with the first error message.
    /// Unlike execute_scripts, this continues past errors in individual scripts.
    pub fn execute_scripts_lossy(
        &mut self,
        descriptors: &[ScriptDescriptor],
        fetched: &FetchedResources,
    ) -> Vec<String> {
        let mut runtime = JsRuntime::new(Rc::clone(&self.tree));
        let mut errors = Vec::new();

        // Apply pending URL so scripts see correct location from the start
        if let Some(url) = &self.pending_url {
            runtime.set_url(url);
        }

        // Populate iframe_src_content in RealmState with pre-fetched iframe content
        Self::populate_iframe_src_content(&fetched.iframes, &runtime);

        // Pre-register external modules so import statements can resolve them
        for descriptor in descriptors {
            if let ScriptDescriptor::ExternalModule(url) = descriptor {
                if let Some(script_content) = fetched.scripts.get(url) {
                    if !script_content.trim().is_empty() {
                        let _ = runtime.register_module(url, script_content);
                    }
                }
            }
        }

        for descriptor in descriptors {
            if let ScriptDescriptor::ImportMap(json) = descriptor {
                Self::process_import_map(&mut runtime, json, fetched);
                continue;
            }
            let is_module = descriptor.is_module();
            let script_nid = match descriptor {
                ScriptDescriptor::Inline(_, nid) | ScriptDescriptor::External(_, nid) => *nid,
                _ => None,
            };
            let code = match descriptor {
                ScriptDescriptor::Inline(text, _) | ScriptDescriptor::InlineModule(text) => {
                    if text.trim().is_empty() {
                        continue;
                    }
                    text.clone()
                }
                ScriptDescriptor::External(url, _) | ScriptDescriptor::ExternalModule(url) => {
                    match fetched.scripts.get(url) {
                        Some(content) if !content.trim().is_empty() => content.clone(),
                        _ => continue,
                    }
                }
                ScriptDescriptor::ImportMap(_) => unreachable!(),
            };
            // Set document.currentScript for classic scripts
            if !is_module {
                if let Some(nid) = script_nid {
                    let _ = runtime.eval(&format!("document.currentScript = __braille_get_element_wrapper({nid})"));
                }
            }
            let result = if is_module {
                let specifier = match descriptor {
                    ScriptDescriptor::ExternalModule(url) => Some(url.as_str()),
                    _ => None,
                };
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    runtime.eval_module(&code, specifier)
                }))
            } else {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.eval(&code)))
            };
            // Clear document.currentScript
            if !is_module {
                let _ = runtime.eval("document.currentScript = null");
            }
            match result {
                Ok(Ok(_)) => {
                    runtime.notify_mutation_observers();
                }
                Ok(Err(e)) => {
                    runtime.notify_mutation_observers();
                    errors.push(format!("{:?}", e));
                }
                Err(panic_err) => {
                    let msg = if let Some(s) = panic_err.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic_err.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "unknown panic".to_string()
                    };
                    errors.push(format!("PANIC: {}", msg));
                }
            }
        }

        // Fire DOMContentLoaded on document (defer scripts have all run)
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.eval("document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));")
        }));
        runtime.run_jobs();

        // Fire onload handlers for iframes with pre-fetched content
        Self::process_iframe_loads(&mut runtime, &self.tree);

        // Fire window.onload handler
        Self::fire_window_load(&mut runtime);

        self.runtime = Some(runtime);
        self.focused_element = None;
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
        errors
    }

    /// Convenience: load HTML with external scripts and iframe content, tolerating JS errors.
    /// Returns any JS errors that occurred during script execution.
    pub fn load_html_with_resources_lossy(&mut self, html: &str, fetched: &FetchedResources) -> Vec<String> {
        let descriptors = self.parse_and_collect_scripts(html);
        self.execute_scripts_lossy(&descriptors, fetched)
    }

    /// Convenience wrapper around [`load_html_with_resources_lossy`](Self::load_html_with_resources_lossy)
    /// for the common case of scripts-only (no iframes).
    /// Returns any JS errors that occurred during script execution.
    pub fn load_html_with_scripts_lossy(&mut self, html: &str, fetched: &HashMap<String, String>) -> Vec<String> {
        self.load_html_with_resources_lossy(html, &FetchedResources::scripts_only(fetched.clone()))
    }

    /// Parse an import map JSON and register bare specifier → URL mappings.
    /// For each entry in `imports`, if the URL has been pre-fetched, register it as a module.
    fn process_import_map(runtime: &mut JsRuntime, json: &str, fetched: &FetchedResources) {
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json) else {
            return;
        };
        let Some(imports) = parsed.get("imports").and_then(|v| v.as_object()) else {
            return;
        };
        for (specifier, url_value) in imports {
            let Some(url) = url_value.as_str() else {
                continue;
            };
            // Register the import map entry: bare specifier -> URL
            runtime.add_import_map_entry(specifier, url);
            // If the URL content has been fetched, register it as a module under the URL
            if let Some(content) = fetched.scripts.get(url) {
                if !content.trim().is_empty() {
                    let _ = runtime.register_module(url, content);
                    // Also register under the bare specifier for direct lookup
                    let _ = runtime.register_module(specifier, content);
                }
            }
        }
    }

    /// Returns all URLs referenced in import maps found in the parsed document.
    /// Call after `parse_and_collect_scripts` so the CLI can fetch these URLs.
    pub fn import_map_urls(descriptors: &[ScriptDescriptor]) -> Vec<String> {
        let mut urls = Vec::new();
        for desc in descriptors {
            if let ScriptDescriptor::ImportMap(json) = desc {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json) {
                    if let Some(imports) = parsed.get("imports").and_then(|v| v.as_object()) {
                        for url_value in imports.values() {
                            if let Some(url) = url_value.as_str() {
                                urls.push(url.to_string());
                            }
                        }
                    }
                }
            }
        }
        urls
    }

    /// Incrementally parse HTML, executing scripts as the parser encounters them.
    /// MutationObserver records are synthesized for parser-inserted nodes between
    /// script executions. Returns any JS errors encountered.
    pub fn load_html_incremental_with_resources_lossy(
        &mut self,
        html: &str,
        fetched: &FetchedResources,
    ) -> Vec<String> {
        use crate::html::parser::{split_html_at_scripts, IncrementalParser};
        // MutationObserver synthesis is handled by JsRuntime

        let mut errors = Vec::new();

        // 1. Create incremental parser (creates tree internally)
        let mut inc = IncrementalParser::new();
        let tree = Rc::clone(inc.tree());

        // 2. Create JsRuntime bound to the shared tree
        let mut runtime = JsRuntime::new(Rc::clone(&tree));

        // 3. Populate iframe src content
        Self::populate_iframe_src_content(&fetched.iframes, &runtime);

        // 4. Split HTML at </script> boundaries
        let chunks = split_html_at_scripts(html);

        // 5. Feed chunks, executing scripts between them
        for chunk in &chunks {
            let watermark = tree.borrow().node_count();

            // Feed this chunk to the parser
            inc.process(chunk);

            // Find new script nodes (ID >= watermark, tag == "script")
            let new_scripts: Vec<(NodeId, Option<String>, String)> = {
                let t = tree.borrow();
                let count = t.node_count();
                let mut scripts = Vec::new();
                for nid in watermark..count {
                    let node = t.get_node(nid);
                    if let NodeData::Element {
                        tag_name, attributes, ..
                    } = &node.data
                    {
                        if tag_name.eq_ignore_ascii_case("script") {
                            let src = attributes
                                .iter()
                                .find(|a| a.local_name == "src")
                                .map(|a| a.value.clone());
                            let inline_text = t.get_text_content(nid);
                            scripts.push((nid, src, inline_text));
                        }
                    }
                }
                scripts
            };

            // Synthesize MO records for ALL new nodes (including scripts and their text children)
            runtime.synthesize_parser_mutations(&tree, watermark);

            // Execute each new script
            for (_script_id, src, inline_text) in &new_scripts {
                let code = if let Some(url) = src {
                    match fetched.scripts.get(url) {
                        Some(content) if !content.trim().is_empty() => content.clone(),
                        _ => continue,
                    }
                } else if !inline_text.trim().is_empty() {
                    inline_text.clone()
                } else {
                    continue
                };

                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    runtime.eval(&code)
                })) {
                    Ok(Ok(_)) => {
                        runtime.notify_mutation_observers();
                    }
                    Ok(Err(e)) => {
                        runtime.notify_mutation_observers();
                        errors.push(format!("{:?}", e));
                    }
                    Err(panic_err) => {
                        let msg = if let Some(s) = panic_err.downcast_ref::<String>() {
                            s.clone()
                        } else if let Some(s) = panic_err.downcast_ref::<&str>() {
                            s.to_string()
                        } else {
                            "unknown panic".to_string()
                        };
                        errors.push(format!("PANIC: {}", msg));
                    }
                }
            }
        }

        // 6. Finish parsing
        let _ = inc.finish();

        // 7. Post-processing
        Self::process_iframe_loads(&mut runtime, &tree);
        Self::fire_window_load(&mut runtime);

        self.tree = tree;
        self.runtime = Some(runtime);
        self.focused_element = None;
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
        errors
    }

    /// Evaluate a JavaScript expression and return the result as a string.
    /// Panics if no runtime is loaded (call load_html or execute_scripts first).
    pub fn eval_js(&mut self, code: &str) -> Result<String, String> {
        let runtime = self.runtime.as_mut().expect("eval_js: no runtime loaded");
        runtime.eval_to_string(code)
    }

    /// Fire `window.onload` handler after all scripts and iframe loads have completed.
    fn fire_window_load(runtime: &mut JsRuntime) {
        runtime.fire_window_load();
    }

    /// Store pre-fetched iframe HTML content in the realm state.
    fn populate_iframe_src_content(iframes: &HashMap<String, String>, runtime: &JsRuntime) {
        runtime.populate_iframe_content(iframes);
    }

    /// After scripts have executed, walk the DOM for `<iframe>` elements with a `src`
    /// attribute. For each one whose content was pre-fetched, ensure the content doc
    /// is populated and fire any `onload` handler.
    fn process_iframe_loads(runtime: &mut JsRuntime, tree: &Rc<RefCell<DomTree>>) {
        runtime.process_iframe_loads(tree);
    }

    // collect_iframes moved to JsRuntime::collect_iframes_impl

    // -- Fetch API public methods --

    /// Returns true if there are pending fetch requests that need to be serviced.
    pub fn has_pending_fetches(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            runtime.has_pending_fetches()
        } else {
            false
        }
    }

    /// Returns true if there are pending timers.
    pub fn has_pending_timers(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            runtime.has_pending_timers()
        } else {
            false
        }
    }

    /// Returns all pending fetch requests as serializable DTOs.
    pub fn pending_fetches(&self) -> Vec<braille_wire::FetchRequest> {
        if let Some(runtime) = &self.runtime {
            runtime.pending_fetches()
        } else {
            Vec::new()
        }
    }

    /// Resolve a pending fetch with a response.
    pub fn resolve_fetch(&mut self, id: u64, response: &braille_wire::FetchResponseData) {
        let runtime = self.runtime.as_mut().expect("resolve_fetch: no runtime loaded");
        runtime.resolve_fetch(id, response);
    }

    /// Reject a pending fetch with an error message.
    pub fn reject_fetch(&mut self, id: u64, error: &str) {
        let runtime = self.runtime.as_mut().expect("reject_fetch: no runtime loaded");
        runtime.reject_fetch(id, error);
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

    /// Check the DOM for `<meta http-equiv="refresh">` tags and return the
    /// parsed redirect information if one is found.
    ///
    /// The `content` attribute format is either:
    /// - `SECONDS; url=URL` — redirect to URL after SECONDS
    /// - `SECONDS` — refresh the current page after SECONDS
    ///
    /// If a URL is present and relative, it is resolved against `base_url`.
    /// If `base_url` is None, relative URLs are returned as-is.
    pub fn check_meta_refresh(&self, base_url: Option<&str>) -> Option<MetaRefresh> {
        let tree = self.tree.borrow();
        let metas = tree.get_elements_by_tag_name("meta");
        for meta_id in metas {
            let node = tree.get_node(meta_id);
            if let NodeData::Element { attributes, .. } = &node.data {
                let is_refresh = attributes.iter().any(|a| {
                    a.local_name.eq_ignore_ascii_case("http-equiv")
                        && a.value.eq_ignore_ascii_case("refresh")
                });
                if !is_refresh {
                    continue;
                }
                let content = attributes
                    .iter()
                    .find(|a| a.local_name.eq_ignore_ascii_case("content"))
                    .map(|a| a.value.as_str());
                if let Some(content) = content {
                    return Some(parse_meta_refresh_content(content, base_url));
                }
            }
        }
        None
    }
}

/// Parsed result of a `<meta http-equiv="refresh">` tag.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaRefresh {
    /// Delay in seconds before the redirect/refresh.
    pub delay_seconds: u32,
    /// The target URL, or None if the page should refresh itself.
    pub url: Option<String>,
}

/// Parse the `content` attribute value of a meta refresh tag.
///
/// Handles formats like:
/// - `"5"` — refresh same page after 5 seconds
/// - `"2; url=/path"` — redirect to /path after 2 seconds
/// - `"0;url=https://example.com"` — immediate redirect (space around ; is optional)
/// - `"2; URL=/path"` — case-insensitive "url=" prefix
fn parse_meta_refresh_content(content: &str, base_url: Option<&str>) -> MetaRefresh {
    let content = content.trim();

    // Split on ';' or ',' (both are valid separators per the spec)
    let (delay_str, rest) = match content.find(|c| c == ';' || c == ',') {
        Some(pos) => (&content[..pos], Some(content[pos + 1..].trim())),
        None => (content, None),
    };

    let delay_seconds = delay_str.trim().parse::<u32>().unwrap_or(0);

    let url = rest.and_then(|rest| {
        // Strip optional "url=" prefix (case-insensitive)
        let rest_lower = rest.to_ascii_lowercase();
        let url_str = if rest_lower.starts_with("url=") {
            rest[4..].trim()
        } else if rest_lower.starts_with("url =") {
            rest[5..].trim()
        } else {
            // No url= prefix but there's content after the semicolon — treat as URL anyway
            rest
        };

        // Strip surrounding quotes if present
        let url_str = url_str
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .or_else(|| url_str.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
            .unwrap_or(url_str);

        if url_str.is_empty() {
            return None;
        }

        // Resolve relative URLs against base_url
        if let Some(base) = base_url {
            if let Ok(base_parsed) = url::Url::parse(base) {
                if let Ok(resolved) = base_parsed.join(url_str) {
                    return Some(resolved.to_string());
                }
            }
        }

        Some(url_str.to_string())
    });

    MetaRefresh {
        delay_seconds,
        url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn test_end_to_end() {
        let html = r#"
        <html><body>
          <h1>Hello</h1>
          <div id="app"></div>
          <script>
            let el = document.createElement("p");
            el.textContent = "Created by JavaScript";
            document.getElementById("app").appendChild(el);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(
            snapshot.contains("heading"),
            "snapshot should contain heading: {}",
            snapshot
        );
        assert!(
            snapshot.contains("Hello"),
            "snapshot should contain Hello: {}",
            snapshot
        );
        assert!(
            snapshot.contains("paragraph"),
            "snapshot should contain paragraph: {}",
            snapshot
        );
        assert!(
            snapshot.contains("Created by JavaScript"),
            "snapshot should contain JS-created text: {}",
            snapshot
        );
    }

    #[test]
    fn test_multiple_scripts() {
        let html = r#"
        <html><body>
          <div id="container"></div>
          <script>
            let p1 = document.createElement("p");
            p1.textContent = "First";
            document.getElementById("container").appendChild(p1);
          </script>
          <script>
            let p2 = document.createElement("p");
            p2.textContent = "Second";
            document.getElementById("container").appendChild(p2);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(
            snapshot.contains("First"),
            "snapshot should contain First: {}",
            snapshot
        );
        assert!(
            snapshot.contains("Second"),
            "snapshot should contain Second: {}",
            snapshot
        );
    }

    #[test]
    fn test_resolve_ref_after_snapshot() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
          <button>Click me</button>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot.contains("@e1"), "snapshot should contain @e1: {}", snapshot);
        assert!(snapshot.contains("@e2"), "snapshot should contain @e2: {}", snapshot);

        // Verify we can resolve the refs
        let ref1 = engine.resolve_ref("@e1");
        let ref2 = engine.resolve_ref("@e2");

        assert!(ref1.is_some(), "should resolve @e1");
        assert!(ref2.is_some(), "should resolve @e2");
        assert_ne!(ref1, ref2, "refs should point to different nodes");
    }

    #[test]
    fn test_resolve_ref_returns_none_for_invalid_ref() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        assert_eq!(engine.resolve_ref("@e999"), None, "invalid ref should return None");
        assert_eq!(engine.resolve_ref("invalid"), None, "malformed ref should return None");
        assert_eq!(engine.resolve_ref(""), None, "empty ref should return None");
    }

    #[test]
    fn test_resolve_ref_before_snapshot_returns_none() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Try to resolve before calling snapshot
        assert_eq!(engine.resolve_ref("@e1"), None, "should return None before snapshot");
    }

    #[test]
    fn test_ref_map_with_no_interactive_elements() {
        let html = r#"
        <html><body>
          <h1>Title</h1>
          <p>Paragraph</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        assert_eq!(
            engine.resolve_ref("@e1"),
            None,
            "should return None when no interactive elements"
        );
    }

    #[test]
    fn test_ref_map_updates_on_new_snapshot() {
        // Test that ref_map is updated when snapshot is called again after DOM modification
        let html = r#"
        <html><body>
          <div id="container">
            <a href="/first">First</a>
          </div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot1 = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot1.contains("@e1"), "first snapshot should contain @e1");
        let ref1_first = engine.resolve_ref("@e1");
        assert!(ref1_first.is_some(), "should resolve @e1 after first snapshot");

        // Now load new HTML with different interactive elements
        let html2 = r#"
        <html><body>
          <button>Button 1</button>
          <button>Button 2</button>
          <input type="text">
        </body></html>"#;

        engine.load_html(html2);
        let snapshot2 = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot2.contains("@e1"), "second snapshot should contain @e1");
        assert!(snapshot2.contains("@e2"), "second snapshot should contain @e2");
        assert!(snapshot2.contains("@e3"), "second snapshot should contain @e3");

        let ref1_second = engine.resolve_ref("@e1");
        let ref2_second = engine.resolve_ref("@e2");
        let ref3_second = engine.resolve_ref("@e3");

        assert!(ref1_second.is_some(), "should resolve @e1 after second snapshot");
        assert!(ref2_second.is_some(), "should resolve @e2 after second snapshot");
        assert!(ref3_second.is_some(), "should resolve @e3 after second snapshot");

        // The node IDs should be different since we loaded new HTML
        // (The tree was replaced, so old refs don't apply)
    }

    // ---- ScriptDescriptor / external script tests ----

    #[test]
    fn test_parse_and_collect_scripts_identifies_inline() {
        let html = r#"
        <html><body>
          <script>console.log("hello")</script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 1);
        match &descriptors[0] {
            ScriptDescriptor::Inline(text, _) => {
                assert!(text.contains("console.log"), "inline script text: {}", text);
            }
            other => panic!("expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_and_collect_scripts_identifies_external() {
        let html = r#"
        <html><body>
          <script src="https://example.com/app.js"></script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 1);
        match &descriptors[0] {
            ScriptDescriptor::External(url, _) => {
                assert_eq!(url, "https://example.com/app.js");
            }
            other => panic!("expected External, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_and_collect_scripts_mixed_document_order() {
        let html = r#"
        <html><body>
          <script>var x = 1;</script>
          <script src="https://cdn.example.com/lib.js"></script>
          <script>var y = 2;</script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 3, "should find 3 scripts");

        match &descriptors[0] {
            ScriptDescriptor::Inline(text, _) => assert!(text.contains("var x = 1")),
            _ => panic!("first script should be Inline"),
        }
        match &descriptors[1] {
            ScriptDescriptor::External(url, _) => assert_eq!(url, "https://cdn.example.com/lib.js"),
            _ => panic!("second script should be External"),
        }
        match &descriptors[2] {
            ScriptDescriptor::Inline(text, _) => assert!(text.contains("var y = 2")),
            _ => panic!("third script should be Inline"),
        }
    }

    #[test]
    fn test_execute_scripts_runs_inline() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script>
            let el = document.createElement("p");
            el.textContent = "inline works";
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);
        let fetched = HashMap::new();
        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("inline works"),
            "inline script should execute: {}",
            snapshot
        );
    }

    #[test]
    fn test_execute_scripts_runs_external_from_fetched() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/app.js"></script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/app.js".to_string(),
            concat!(
                "let el = document.createElement(\"p\");",
                "el.textContent = \"external works\";",
                "document.getElementById(\"target\").appendChild(el);"
            )
            .to_string(),
        );

        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("external works"),
            "external script should execute: {}",
            snapshot
        );
    }

    #[test]
    fn test_execute_scripts_skips_missing_external() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/missing.js"></script>
          <script>
            let el = document.createElement("p");
            el.textContent = "after missing";
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);
        let fetched = HashMap::new();
        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("after missing"),
            "inline script after missing external should run: {}",
            snapshot
        );
    }

    #[test]
    fn test_load_html_with_scripts_end_to_end() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/lib.js"></script>
          <script>
            let el = document.createElement("p");
            el.textContent = "value is " + globalValue;
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/lib.js".to_string(),
            "var globalValue = 42;".to_string(),
        );

        let mut engine = Engine::new();
        engine.load_html_with_scripts(html, &fetched);
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("value is 42"),
            "external script should set global used by inline: {}",
            snapshot
        );
    }

    #[test]
    fn test_mixed_inline_and_external_execute_in_order() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script>var order = [];</script>
          <script src="https://example.com/a.js"></script>
          <script>order.push("inline2");</script>
          <script src="https://example.com/b.js"></script>
          <script>
            let el = document.createElement("p");
            el.textContent = order.join(",");
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/a.js".to_string(),
            "order.push(\"extA\");".to_string(),
        );
        fetched.insert(
            "https://example.com/b.js".to_string(),
            "order.push(\"extB\");".to_string(),
        );

        let mut engine = Engine::new();
        engine.load_html_with_scripts(html, &fetched);
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("extA,inline2,extB"),
            "scripts should execute in document order: {}",
            snapshot
        );
    }

    #[test]
    fn test_script_with_src_and_text_src_wins() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/real.js">
            let bad = document.createElement("p");
            bad.textContent = "INLINE SHOULD NOT RUN";
            document.getElementById("target").appendChild(bad);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 1);
        match &descriptors[0] {
            ScriptDescriptor::External(url, _) => {
                assert_eq!(url, "https://example.com/real.js");
            }
            other => panic!("should be External when src is present, got {:?}", other),
        }

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/real.js".to_string(),
            concat!(
                "let el = document.createElement(\"p\");",
                "el.textContent = \"EXTERNAL RAN\";",
                "document.getElementById(\"target\").appendChild(el);"
            )
            .to_string(),
        );

        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("EXTERNAL RAN"),
            "external content should run: {}",
            snapshot
        );
        assert!(
            !snapshot.contains("INLINE SHOULD NOT RUN"),
            "inline text should be ignored when src present: {}",
            snapshot
        );
    }

    // ---- C-3C: compute_all_styles integration tests ----

    #[test]
    fn test_load_html_computes_styles() {
        let html = r##"
        <html><body>
          <style>p { color: red; }</style>
          <p>Hello</p>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);

        let tree = engine.tree.borrow();
        // Find the <p> element
        let p_id = tree.get_elements_by_tag_name("p")[0];
        let p_node = tree.get_node(p_id);

        // After load_html, computed_style should be populated
        assert!(
            p_node.computed_style.is_some(),
            "computed_style should be set after load_html"
        );
        let style = p_node.computed_style.as_ref().unwrap();
        assert!(style.contains_key("color"), "should have color property");
    }

    #[test]
    fn test_display_none_reflected_in_snapshot() {
        let html = r##"
        <html><body>
          <style>.hidden { display: none; }</style>
          <p>Visible</p>
          <p class="hidden">Hidden</p>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot.contains("Visible"), "visible text should appear: {}", snapshot);
        assert!(
            !snapshot.contains("Hidden"),
            "display:none text should not appear: {}",
            snapshot
        );
    }

    #[test]
    fn test_visibility_hidden_hides_text_in_snapshot() {
        let html = r##"
        <html><body>
          <style>.ghost { visibility: hidden; }</style>
          <p>Visible</p>
          <p class="ghost">Ghost</p>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot.contains("Visible"), "visible text should appear: {}", snapshot);
        assert!(
            !snapshot.contains("Ghost"),
            "visibility:hidden text should not appear: {}",
            snapshot
        );
        // But the paragraph structure should still be there
        let lines: Vec<&str> = snapshot.lines().collect();
        assert!(
            lines.len() >= 2,
            "should have multiple lines including hidden paragraph structure"
        );
    }

    #[test]
    fn test_script_added_element_gets_computed_styles() {
        let html = r##"
        <html><body>
          <style>p { color: blue; }</style>
          <div id="target"></div>
          <script>
            var p = document.createElement("p");
            p.textContent = "Dynamic";
            document.getElementById("target").appendChild(p);
          </script>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);

        let tree = engine.tree.borrow();
        let ps = tree.get_elements_by_tag_name("p");
        assert!(!ps.is_empty(), "should have a <p> element from script");
        let p_node = tree.get_node(ps[0]);
        assert!(
            p_node.computed_style.is_some(),
            "script-created element should get computed styles"
        );
    }

    #[test]
    fn test_load_html_with_scripts_computes_styles() {
        let html = r##"
        <html><body>
          <style>h1 { color: green; }</style>
          <h1>Title</h1>
          <script src="app.js"></script>
        </body></html>"##;

        let mut fetched = HashMap::new();
        fetched.insert("app.js".to_string(), "// no-op".to_string());

        let mut engine = Engine::new();
        engine.load_html_with_scripts(html, &fetched);

        let tree = engine.tree.borrow();
        let h1_id = tree.get_elements_by_tag_name("h1")[0];
        let h1_node = tree.get_node(h1_id);
        assert!(
            h1_node.computed_style.is_some(),
            "load_html_with_scripts should compute styles"
        );
    }

    #[test]
    fn test_request_submit_validates_before_submitting() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="email" type="email" required value="" />
            <button type="submit">Submit</button>
          </form>
          <script>
            var submitted = false;
            var form = document.getElementById('myform');
            form.addEventListener('submit', function(e) {
              submitted = true;
              e.preventDefault();
            });
            form.requestSubmit();
            window.__submitted = submitted;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Validation should fail (required email is empty), so submit event should NOT fire
        let result = engine.eval_js("window.__submitted").unwrap();
        assert_eq!(result, "false", "requestSubmit should not fire submit when validation fails");
    }

    #[test]
    fn test_request_submit_fires_submit_when_valid() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="email" type="email" value="test@example.com" />
            <button type="submit">Submit</button>
          </form>
          <script>
            var submitted = false;
            var form = document.getElementById('myform');
            form.addEventListener('submit', function(e) {
              submitted = true;
              e.preventDefault();
            });
            form.requestSubmit();
            window.__submitted = submitted;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Validation passes, submit event should fire
        let result = engine.eval_js("window.__submitted").unwrap();
        assert_eq!(result, "true", "requestSubmit should fire submit when form is valid");
    }

    #[test]
    fn test_request_submit_respects_prevent_default() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="name" value="hello" />
          </form>
          <script>
            var submitFired = false;
            var preventDefaultCalled = false;
            var form = document.getElementById('myform');
            form.addEventListener('submit', function(e) {
              submitFired = true;
              preventDefaultCalled = true;
              e.preventDefault();
            });
            form.requestSubmit();
            window.__submitFired = submitFired;
            window.__preventDefaultCalled = preventDefaultCalled;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let fired = engine.eval_js("window.__submitFired").unwrap();
        assert_eq!(fired, "true", "submit event should fire");
        let prevented = engine.eval_js("window.__preventDefaultCalled").unwrap();
        assert_eq!(prevented, "true", "preventDefault should have been called");
    }

    #[test]
    fn test_request_submit_with_submitter() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="name" value="hello" />
            <button id="btn" type="submit">Go</button>
          </form>
          <script>
            var capturedSubmitter = null;
            var form = document.getElementById('myform');
            var btn = document.getElementById('btn');
            form.addEventListener('submit', function(e) {
              capturedSubmitter = e.submitter;
              e.preventDefault();
            });
            form.requestSubmit(btn);
            window.__submitterTag = capturedSubmitter ? capturedSubmitter.tagName : 'none';
            window.__submitterId = capturedSubmitter ? capturedSubmitter.id : 'none';
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let tag = engine.eval_js("window.__submitterTag").unwrap();
        assert_eq!(tag, "BUTTON", "submitter should be the button element");
        let id = engine.eval_js("window.__submitterId").unwrap();
        assert_eq!(id, "btn", "submitter should have id=btn");
    }

    #[test]
    fn test_request_submit_fires_invalid_on_failed_validation() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input id="inp" name="email" type="email" required value="" />
          </form>
          <script>
            var invalidFired = false;
            var inp = document.getElementById('inp');
            inp.addEventListener('invalid', function(e) {
              invalidFired = true;
            });
            var form = document.getElementById('myform');
            form.requestSubmit();
            window.__invalidFired = invalidFired;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.eval_js("window.__invalidFired").unwrap();
        assert_eq!(result, "true", "invalid event should fire on failed validation");
    }

    #[test]
    fn test_step_mismatch_number_input() {
        let html = r#"<html><body>
            <input id="a" type="number" step="3" min="0" value="5">
            <input id="b" type="number" step="3" min="0" value="6">
            <input id="c" type="number" value="1.5">
            <input id="d" type="number" step="any" value="3.14159">
            <input id="e" type="number" step="0.1" min="0" value="0.3">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // 5 is not divisible by step=3 from min=0 -> stepMismatch=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('a').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":true"),
            "5 is not a multiple of step 3 from min 0: {}",
            result
        );
        assert!(
            result.contains("\"valid\":false"),
            "should be invalid: {}",
            result
        );

        // 6 IS divisible by step=3 from min=0 -> stepMismatch=false
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('b').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":false"),
            "6 is a multiple of step 3 from min 0: {}",
            result
        );

        // default step=1 for number, 1.5 is not a whole number -> stepMismatch=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('c').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":true"),
            "1.5 is not a multiple of default step 1: {}",
            result
        );

        // step="any" means no step mismatch
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('d').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":false"),
            "step=any should never have stepMismatch: {}",
            result
        );

        // 0.3 with step=0.1 from min=0 -> should be valid
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('e').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":false"),
            "0.3 is a multiple of step 0.1 from min 0: {}",
            result
        );
    }

    #[test]
    fn test_bad_input_number() {
        let html = r#"<html><body>
            <input id="a" type="number" value="abc">
            <input id="b" type="number" value="42">
            <input id="c" type="number" value="">
            <input id="d" type="date" value="not-a-date">
            <input id="e" type="date" value="2024-01-15">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // "abc" is not a valid number -> badInput=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('a').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":true"),
            "abc is not a valid number: {}",
            result
        );
        assert!(
            result.contains("\"valid\":false"),
            "should be invalid: {}",
            result
        );

        // "42" is a valid number -> badInput=false
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('b').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":false"),
            "42 is a valid number: {}",
            result
        );

        // empty value -> badInput=false (no input to be bad)
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('c').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":false"),
            "empty value should not be badInput: {}",
            result
        );

        // "not-a-date" is not a valid date -> badInput=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('d').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":true"),
            "not-a-date is not a valid date: {}",
            result
        );

        // "2024-01-15" is a valid date -> badInput=false
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('e').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":false"),
            "2024-01-15 is a valid date: {}",
            result
        );
    }

    #[test]
    fn test_check_validity_fires_invalid_event() {
        let html = r#"<html><body>
            <input id="inp" type="text" required value="">
            <script>
                window.__invalidFired = false;
                window.__invalidBubbled = false;
                window.__invalidCancelable = null;
                var inp = document.getElementById('inp');
                inp.addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                    window.__invalidCancelable = e.cancelable;
                });
                document.body.addEventListener('invalid', function(e) {
                    window.__invalidBubbled = true;
                });
                window.__checkResult = inp.checkValidity();
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // checkValidity should return false for required empty field
        let result = engine.eval_js("String(window.__checkResult)").unwrap();
        assert_eq!(result, "false", "checkValidity should return false");

        // invalid event should have fired
        let result = engine.eval_js("String(window.__invalidFired)").unwrap();
        assert_eq!(result, "true", "invalid event should fire on checkValidity");

        // invalid event should NOT bubble
        let result = engine.eval_js("String(window.__invalidBubbled)").unwrap();
        assert_eq!(result, "false", "invalid event should not bubble");

        // invalid event should be cancelable
        let result = engine.eval_js("String(window.__invalidCancelable)").unwrap();
        assert_eq!(result, "true", "invalid event should be cancelable");
    }

    #[test]
    fn test_check_validity_no_event_when_valid() {
        let html = r#"<html><body>
            <input id="inp" type="text" value="hello">
            <script>
                window.__invalidFired = false;
                var inp = document.getElementById('inp');
                inp.addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                });
                window.__checkResult = inp.checkValidity();
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.eval_js("String(window.__checkResult)").unwrap();
        assert_eq!(result, "true", "checkValidity should return true");

        let result = engine.eval_js("String(window.__invalidFired)").unwrap();
        assert_eq!(result, "false", "invalid event should not fire when valid");
    }

    #[test]
    fn test_report_validity_fires_invalid_event() {
        let html = r#"<html><body>
            <input id="inp" type="number" required value="">
            <script>
                window.__invalidFired = false;
                var inp = document.getElementById('inp');
                inp.addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                });
                window.__reportResult = inp.reportValidity();
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.eval_js("String(window.__reportResult)").unwrap();
        assert_eq!(result, "false", "reportValidity should return false");

        let result = engine.eval_js("String(window.__invalidFired)").unwrap();
        assert_eq!(result, "true", "invalid event should fire on reportValidity");
    }

    #[test]
    fn test_validation_message_step_mismatch_and_bad_input() {
        let html = r#"<html><body>
            <input id="step" type="number" step="5" min="0" value="3">
            <input id="bad" type="number" value="xyz">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine
            .eval_js("document.getElementById('step').validationMessage")
            .unwrap();
        assert!(
            result.contains("step"),
            "validationMessage should mention step: {}",
            result
        );

        let result = engine
            .eval_js("document.getElementById('bad').validationMessage")
            .unwrap();
        assert!(
            result.contains("valid value"),
            "validationMessage should mention valid value: {}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // textarea property tests
    // -----------------------------------------------------------------------

    fn eval_js_via_runtime(html: &str, js: &str) -> String {
        let mut engine = Engine::new();
        engine.load_html(html);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval_to_string(js).unwrap()
    }

    // -- textarea.defaultValue --

    #[test]
    fn textarea_default_value_returns_text_content() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t">hello world</textarea></body></html>"#,
            r#"document.getElementById("t").defaultValue"#,
        );
        assert_eq!(s, "hello world");
    }

    #[test]
    fn textarea_default_value_setter_replaces_text_content() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t">old</textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").defaultValue = "new text""#).unwrap();
        let s = runtime.eval_to_string(r#"document.getElementById("t").defaultValue"#).unwrap();
        assert_eq!(s, "new text");
    }

    #[test]
    fn input_default_value_reflects_value_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><input id="i" value="initial" /></body></html>"#,
            r#"document.getElementById("i").defaultValue"#,
        );
        assert_eq!(s, "initial");
    }

    // -- textarea.maxLength --

    #[test]
    fn textarea_maxlength_defaults_to_minus_one() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").maxLength)"#,
        );
        assert_eq!(s, "-1");
    }

    #[test]
    fn textarea_maxlength_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" maxlength="10"></textarea></body></html>"#,
            r#"String(document.getElementById("t").maxLength)"#,
        );
        assert_eq!(s, "10");
    }

    #[test]
    fn textarea_maxlength_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").maxLength = 5"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").maxLength)"#).unwrap();
        assert_eq!(s, "5");
    }

    #[test]
    fn textarea_value_truncated_by_maxlength() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t" maxlength="5"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").value = "hello world""#).unwrap();
        let s = runtime.eval_to_string(r#"document.getElementById("t").value"#).unwrap();
        assert_eq!(s, "hello");
    }

    // -- textarea.minLength --

    #[test]
    fn textarea_minlength_defaults_to_minus_one() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").minLength)"#,
        );
        assert_eq!(s, "-1");
    }

    #[test]
    fn textarea_minlength_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").minLength = 3"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").minLength)"#).unwrap();
        assert_eq!(s, "3");
    }

    #[test]
    fn textarea_validity_too_short() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t" minlength="5"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").value = "hi""#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").validity.tooShort)"#).unwrap();
        assert_eq!(s, "true");
    }

    // Old cheating test removed — honest version at bottom of file uses handle_type()

    // -- textarea.cols --

    #[test]
    fn textarea_cols_defaults_to_20() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").cols)"#,
        );
        assert_eq!(s, "20");
    }

    #[test]
    fn textarea_cols_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" cols="40"></textarea></body></html>"#,
            r#"String(document.getElementById("t").cols)"#,
        );
        assert_eq!(s, "40");
    }

    #[test]
    fn textarea_cols_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").cols = 60"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").cols)"#).unwrap();
        assert_eq!(s, "60");
    }

    // -- textarea.rows --

    #[test]
    fn textarea_rows_defaults_to_2() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").rows)"#,
        );
        assert_eq!(s, "2");
    }

    #[test]
    fn textarea_rows_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" rows="10"></textarea></body></html>"#,
            r#"String(document.getElementById("t").rows)"#,
        );
        assert_eq!(s, "10");
    }

    #[test]
    fn textarea_rows_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").rows = 8"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").rows)"#).unwrap();
        assert_eq!(s, "8");
    }

    // -- textarea.wrap --

    #[test]
    fn textarea_wrap_defaults_to_soft() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"document.getElementById("t").wrap"#,
        );
        assert_eq!(s, "soft");
    }

    #[test]
    fn textarea_wrap_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" wrap="hard"></textarea></body></html>"#,
            r#"document.getElementById("t").wrap"#,
        );
        assert_eq!(s, "hard");
    }

    #[test]
    fn textarea_wrap_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").wrap = "hard""#).unwrap();
        let s = runtime.eval_to_string(r#"document.getElementById("t").wrap"#).unwrap();
        assert_eq!(s, "hard");
    }

    // -- textarea.textLength --

    #[test]
    fn textarea_text_length_returns_value_length() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").value = "hello""#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").textLength)"#).unwrap();
        assert_eq!(s, "5");
    }

    #[test]
    fn textarea_text_length_zero_when_empty() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").textLength)"#,
        );
        assert_eq!(s, "0");
    }

    // ---- meta refresh tests ----

    #[test]
    fn test_meta_refresh_with_url() {
        let html = r#"
        <html><head>
          <meta http-equiv="refresh" content="2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=abc&amp;id=123&amp;redir=%2F">
        </head><body><p>Redirecting...</p></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let refresh = engine.check_meta_refresh(None);

        assert!(refresh.is_some(), "should detect meta refresh");
        let refresh = refresh.unwrap();
        assert_eq!(refresh.delay_seconds, 2);
        assert!(refresh.url.is_some(), "should have a URL");
        assert!(
            refresh.url.as_ref().unwrap().contains("pass-challenge"),
            "URL should contain path: {:?}",
            refresh.url
        );
    }

    #[test]
    fn test_meta_refresh_relative_url_resolution() {
        let html = r#"
        <html><head>
          <meta http-equiv="refresh" content="0; url=/login">
        </head><body></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let refresh = engine.check_meta_refresh(Some("https://example.com/page"));

        assert!(refresh.is_some());
        let refresh = refresh.unwrap();
        assert_eq!(refresh.delay_seconds, 0);
        assert_eq!(refresh.url.as_deref(), Some("https://example.com/login"));
    }

    #[test]
    fn test_meta_refresh_no_url() {
        let html = r#"
        <html><head>
          <meta http-equiv="refresh" content="5">
        </head><body></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let refresh = engine.check_meta_refresh(None);

        assert!(refresh.is_some());
        let refresh = refresh.unwrap();
        assert_eq!(refresh.delay_seconds, 5);
        assert!(refresh.url.is_none(), "should have no URL for plain refresh");
    }

    #[test]
    fn test_meta_refresh_missing_returns_none() {
        let html = r#"
        <html><head>
          <meta charset="utf-8">
        </head><body><p>No refresh here</p></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let refresh = engine.check_meta_refresh(None);

        assert!(refresh.is_none(), "should return None when no meta refresh");
    }

    #[test]
    fn test_meta_refresh_case_insensitive() {
        let html = r#"
        <html><head>
          <meta HTTP-EQUIV="Refresh" content="3; URL=/destination">
        </head><body></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let refresh = engine.check_meta_refresh(Some("https://example.com/"));

        assert!(refresh.is_some());
        let refresh = refresh.unwrap();
        assert_eq!(refresh.delay_seconds, 3);
        assert_eq!(
            refresh.url.as_deref(),
            Some("https://example.com/destination")
        );
    }

    #[test]
    fn test_meta_refresh_absolute_url() {
        let html = r#"
        <html><head>
          <meta http-equiv="refresh" content="0; url=https://other.com/page">
        </head><body></body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let refresh = engine.check_meta_refresh(Some("https://example.com/"));

        assert!(refresh.is_some());
        let refresh = refresh.unwrap();
        assert_eq!(refresh.delay_seconds, 0);
        assert_eq!(
            refresh.url.as_deref(),
            Some("https://other.com/page")
        );
    }

    #[test]
    fn textarea_validity_too_long() {
        let html = r#"
        <html><body>
          <textarea id="t" maxlength="3"></textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Use the public .value setter via handle_type (not __props._value directly)
        engine.handle_type("#t", "hello").unwrap();

        let too_long = engine.eval_js(
            "document.getElementById('t').validity.tooLong"
        ).unwrap();
        assert_eq!(
            too_long, "true",
            "textarea with maxlength=3 and value='hello' should have validity.tooLong=true, got: {}",
            too_long
        );

        let valid = engine.eval_js(
            "document.getElementById('t').validity.valid"
        ).unwrap();
        assert_eq!(
            valid, "false",
            "textarea with tooLong should not be valid, got: {}",
            valid
        );
    }
}
