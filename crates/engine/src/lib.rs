pub mod a11y;
pub mod commands;
pub mod css;
pub mod dom;
pub mod html;
pub mod js;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{JsObject, JsValue};

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;
use crate::js::JsRuntime;
use braille_wire::SnapMode;

/// Represents a script to be executed — either inline or external (needing fetch).
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptDescriptor {
    /// Script text content, ready to execute.
    Inline(String),
    /// A src URL that needs to be fetched by the host.
    External(String),
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

pub struct Engine {
    pub(crate) tree: Rc<RefCell<DomTree>>,
    pub(crate) runtime: Option<JsRuntime>,
    // DESIGN NOTE: ref_map is only populated after a snapshot() call with Accessibility mode.
    // If resolve_ref is called before snapshot, it will return None for all refs.
    // This is intentional - refs are tied to a specific accessibility tree snapshot.
    pub(crate) ref_map: HashMap<String, NodeId>,
    pub(crate) focused_element: Option<NodeId>,
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

    pub fn snapshot(&mut self, mode: SnapMode) -> String {
        match mode {
            SnapMode::Accessibility => {
                let (output, ref_map) = crate::a11y::serialize_a11y(&self.tree.borrow(), self.focused_element);
                self.ref_map = ref_map;
                output
            }
            SnapMode::Dom => "[DOM mode not yet implemented]".to_string(),
            SnapMode::Markdown => "[Markdown mode not yet implemented]".to_string(),
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

    fn walk_for_scripts(tree: &DomTree, node_id: NodeId, scripts: &mut Vec<String>) {
        let node = tree.get_node(node_id);
        if let NodeData::Element { tag_name, .. } = &node.data {
            if tag_name.eq_ignore_ascii_case("script") {
                // Extract text content from this script element
                let text = tree.get_text_content(node_id);
                scripts.push(text);
                // Don't recurse into script children (we already got the text)
                return;
            }
        }
        // Recurse into children
        let children: Vec<NodeId> = node.children.clone();
        for child_id in children {
            Self::walk_for_scripts(tree, child_id, scripts);
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

    fn walk_for_script_descriptors(tree: &DomTree, node_id: NodeId, descriptors: &mut Vec<ScriptDescriptor>) {
        let node = tree.get_node(node_id);
        if let NodeData::Element {
            tag_name, attributes, ..
        } = &node.data
        {
            if tag_name.eq_ignore_ascii_case("script") {
                // Per HTML spec: if src attribute exists, it's an external script
                // (inline text content is ignored when src is present)
                let src = attributes
                    .iter()
                    .find(|a| a.local_name == "src")
                    .map(|a| a.value.clone());
                if let Some(url) = src {
                    descriptors.push(ScriptDescriptor::External(url));
                } else {
                    let text = tree.get_text_content(node_id);
                    descriptors.push(ScriptDescriptor::Inline(text));
                }
                // Don't recurse into script children
                return;
            }
        }
        let children: Vec<NodeId> = node.children.clone();
        for child_id in children {
            Self::walk_for_script_descriptors(tree, child_id, descriptors);
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

        // Populate iframe_src_content in RealmState with pre-fetched iframe content
        Self::populate_iframe_src_content(&fetched.iframes, &runtime.context);

        for descriptor in descriptors {
            match descriptor {
                ScriptDescriptor::Inline(text) => {
                    if !text.trim().is_empty() {
                        runtime.eval(text).unwrap();
                        runtime.notify_mutation_observers();
                    }
                }
                ScriptDescriptor::External(url) => {
                    if let Some(script_content) = fetched.scripts.get(url) {
                        if !script_content.trim().is_empty() {
                            runtime.eval(script_content).unwrap();
                            runtime.notify_mutation_observers();
                        }
                    }
                    // Skip if URL not found in fetched map
                }
            }
        }

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

    /// Convenience: load HTML with external script support (scripts only, no iframes).
    /// Combines parse_and_collect_scripts + execute_scripts.
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

        // Populate iframe_src_content in RealmState with pre-fetched iframe content
        Self::populate_iframe_src_content(&fetched.iframes, &runtime.context);

        for descriptor in descriptors {
            let code = match descriptor {
                ScriptDescriptor::Inline(text) => {
                    if text.trim().is_empty() {
                        continue;
                    }
                    text.clone()
                }
                ScriptDescriptor::External(url) => match fetched.scripts.get(url) {
                    Some(content) if !content.trim().is_empty() => content.clone(),
                    _ => continue,
                },
            };
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.eval(&code))) {
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

    /// Convenience: load HTML with external scripts, tolerating JS errors (scripts only, no iframes).
    /// Returns any JS errors that occurred during script execution.
    pub fn load_html_with_scripts_lossy(&mut self, html: &str, fetched: &HashMap<String, String>) -> Vec<String> {
        self.load_html_with_resources_lossy(html, &FetchedResources::scripts_only(fetched.clone()))
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
        use crate::js::bindings::mutation_observer::synthesize_parser_mutations;

        let mut errors = Vec::new();

        // 1. Create incremental parser (creates tree internally)
        let mut inc = IncrementalParser::new();
        let tree = Rc::clone(inc.tree());

        // 2. Create JsRuntime bound to the shared tree
        let mut runtime = JsRuntime::new(Rc::clone(&tree));

        // 3. Populate iframe src content
        Self::populate_iframe_src_content(&fetched.iframes, &runtime.context);

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
            synthesize_parser_mutations(&mut runtime.context, &tree, watermark);

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
        match runtime.eval(code) {
            Ok(val) => {
                if val.is_null() {
                    Ok("null".to_string())
                } else if val.is_undefined() {
                    Ok("undefined".to_string())
                } else {
                    match val.to_string(&mut runtime.context) {
                        Ok(s) => Ok(s.to_std_string_escaped()),
                        Err(_) => Ok("undefined".to_string()),
                    }
                }
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    /// Fire `window.onload` handler after all scripts and iframe loads have completed.
    fn fire_window_load(runtime: &mut JsRuntime) {
        use crate::js::bindings::on_event::get_on_event_handler;
        use crate::js::bindings::window::WINDOW_LISTENER_ID;

        let handler = get_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, "load", &runtime.context);
        if let Some(handler_fn) = handler {
            if handler_fn.is_callable() {
                let window_val = crate::js::realm_state::window_object(&runtime.context)
                    .map(JsValue::from)
                    .unwrap_or(JsValue::undefined());

                // Create a simple Event-like object with type "load"
                let event_obj = boa_engine::object::ObjectInitializer::new(&mut runtime.context).build();
                let _ = event_obj.set(
                    boa_engine::js_string!("type"),
                    JsValue::from(boa_engine::js_string!("load")),
                    false,
                    &mut runtime.context,
                );

                let _ = handler_fn.call(&window_val, &[JsValue::from(event_obj)], &mut runtime.context);
                runtime.notify_mutation_observers();
            }
        }
    }

    /// Store pre-fetched iframe HTML content in the RealmState iframe_src_content.
    fn populate_iframe_src_content(iframes: &HashMap<String, String>, ctx: &boa_engine::Context) {
        let src_content = crate::js::realm_state::iframe_src_content(ctx);
        let mut m = src_content.borrow_mut();
        for (url, content) in iframes {
            m.insert(url.clone(), content.clone());
        }
    }

    /// After scripts have executed, walk the DOM for `<iframe>` elements with a `src`
    /// attribute. For each one whose content was pre-fetched, ensure the content doc
    /// is populated and fire any `onload` handler (attribute or JS-property).
    fn process_iframe_loads(runtime: &mut JsRuntime, tree: &Rc<RefCell<DomTree>>) {
        // Collect iframe nodes that have a `src` attribute
        // Tuple: (node_id, src, onload_attr_code, onload_js_handler)
        let iframes: Vec<(NodeId, String, Option<String>, Option<JsObject>)> = {
            let t = tree.borrow();
            let mut result = Vec::new();
            Self::collect_iframes(&t, t.document(), tree, &mut result, &runtime.context);
            result
        };

        if iframes.is_empty() {
            return;
        }

        // Check which iframes have pre-fetched content (strip fragment before lookup)
        let has_content: Vec<bool> = {
            let src_content = crate::js::realm_state::iframe_src_content(&runtime.context);
            let map = src_content.borrow();
            iframes
                .iter()
                .map(|(_, src, _, _)| {
                    let src_no_fragment = src.split('#').next().unwrap_or(src);
                    map.contains_key(src_no_fragment)
                })
                .collect()
        };

        let tree_ptr = Rc::as_ptr(tree) as usize;

        for (i, (node_id, _src, onload_attr, onload_js)) in iframes.iter().enumerate() {
            if !has_content[i] {
                continue;
            }

            // Ensure the content document is populated (this will use IFRAME_SRC_CONTENT)
            let _ = crate::js::bindings::element::ensure_iframe_content_doc(tree_ptr, *node_id, &mut runtime.context);

            // Get the iframe's JS object for `this` and event.target
            let iframe_js =
                crate::js::bindings::element::get_or_create_js_element(*node_id, tree.clone(), &mut runtime.context)
                    .ok();

            // Create a simple event object with `target` property
            let event_obj = if let Some(ref ijs) = iframe_js {
                let evt = boa_engine::object::ObjectInitializer::new(&mut runtime.context).build();
                let _ = evt.set(
                    boa_engine::js_string!("target"),
                    JsValue::from(ijs.clone()),
                    false,
                    &mut runtime.context,
                );
                JsValue::from(evt)
            } else {
                JsValue::undefined()
            };

            // Fire onload attribute handler if present
            if let Some(handler_code) = onload_attr {
                let func_code = format!("(function(e) {{ {} }})", handler_code);
                if let Ok(func_val) = runtime.eval(&func_code) {
                    if let Ok(func_obj) =
                        func_val.as_object().ok_or(()).and_then(
                            |o| {
                                if o.is_callable() {
                                    Ok(o.clone())
                                } else {
                                    Err(())
                                }
                            },
                        )
                    {
                        if let Some(ref ijs) = iframe_js {
                            let _ = func_obj.call(
                                &JsValue::from(ijs.clone()),
                                std::slice::from_ref(&event_obj),
                                &mut runtime.context,
                            );
                        }
                    }
                }
                runtime.notify_mutation_observers();
            }

            // Fire JS-property onload handler if present
            if let Some(handler_obj) = onload_js {
                if handler_obj.is_callable() {
                    if let Some(ref ijs) = iframe_js {
                        let _ = handler_obj.call(
                            &JsValue::from(ijs.clone()),
                            std::slice::from_ref(&event_obj),
                            &mut runtime.context,
                        );
                    }
                }
                runtime.notify_mutation_observers();
            }
        }
    }

    /// Recursively collect `<iframe>` elements with their src and onload attributes/handlers.
    /// Tuple: (node_id, src, onload_attr_code, onload_js_handler)
    fn collect_iframes(
        tree: &DomTree,
        node_id: NodeId,
        tree_rc: &Rc<RefCell<DomTree>>,
        result: &mut Vec<(NodeId, String, Option<String>, Option<JsObject>)>,
        ctx: &boa_engine::Context,
    ) {
        let node = tree.get_node(node_id);
        if let NodeData::Element {
            tag_name, attributes, ..
        } = &node.data
        {
            if tag_name.eq_ignore_ascii_case("iframe") {
                if let Some(src_attr) = attributes.iter().find(|a| a.local_name == "src") {
                    let src = src_attr.value.clone();
                    let onload_attr = attributes
                        .iter()
                        .find(|a| a.local_name == "onload")
                        .map(|a| a.value.clone());
                    // Check for JS-property onload handler (via unified on_event system)
                    let tree_ptr = Rc::as_ptr(tree_rc) as usize;
                    let onload_js = crate::js::bindings::on_event::get_on_event_handler(tree_ptr, node_id, "load", ctx);
                    result.push((node_id, src, onload_attr, onload_js));
                }
            }
        }
        let children: Vec<NodeId> = node.children.clone();
        for child_id in children {
            Self::collect_iframes(tree, child_id, tree_rc, result, ctx);
        }
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
            ScriptDescriptor::Inline(text) => {
                assert!(text.contains("console.log"), "inline script text: {}", text);
            }
            ScriptDescriptor::External(_) => panic!("expected Inline, got External"),
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
            ScriptDescriptor::External(url) => {
                assert_eq!(url, "https://example.com/app.js");
            }
            ScriptDescriptor::Inline(_) => panic!("expected External, got Inline"),
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
            ScriptDescriptor::Inline(text) => assert!(text.contains("var x = 1")),
            _ => panic!("first script should be Inline"),
        }
        match &descriptors[1] {
            ScriptDescriptor::External(url) => assert_eq!(url, "https://cdn.example.com/lib.js"),
            _ => panic!("second script should be External"),
        }
        match &descriptors[2] {
            ScriptDescriptor::Inline(text) => assert!(text.contains("var y = 2")),
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
            ScriptDescriptor::External(url) => {
                assert_eq!(url, "https://example.com/real.js");
            }
            ScriptDescriptor::Inline(_) => panic!("should be External when src is present"),
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
}
