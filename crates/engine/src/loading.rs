use std::collections::HashMap;
use std::rc::Rc;

use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;
use crate::js::JsRuntime;

use super::{Engine, FetchedResources, ScriptDescriptor};

impl Engine {
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
        if self.cookies_pending_js_sync { self.sync_cookies_to_js(); }

        // 5. Reset focus when loading new page
        self.focused_element = None;

        // 6. Compute CSS styles after script execution
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
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

        // Make all fetched scripts available to inline Worker execution
        Self::populate_worker_scripts(&fetched.scripts, &mut runtime);

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
        if self.cookies_pending_js_sync { self.sync_cookies_to_js(); }
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

        // Make all fetched scripts available to inline Worker execution
        Self::populate_worker_scripts(&fetched.scripts, &mut runtime);

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
        if self.cookies_pending_js_sync { self.sync_cookies_to_js(); }
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
        if self.cookies_pending_js_sync { self.sync_cookies_to_js(); }
        self.focused_element = None;
        crate::css::style_tree::compute_all_styles(&mut self.tree.borrow_mut());
        errors
    }

    /// Fire `window.onload` handler after all scripts and iframe loads have completed.
    pub(crate) fn fire_window_load(runtime: &mut JsRuntime) {
        runtime.fire_window_load();
    }

    /// Store pre-fetched iframe HTML content in the realm state.
    pub(crate) fn populate_iframe_src_content(iframes: &HashMap<String, String>, runtime: &JsRuntime) {
        runtime.populate_iframe_content(iframes);
    }

    /// Make fetched scripts available to inline Worker execution via a JS-side map.
    fn populate_worker_scripts(scripts: &HashMap<String, String>, runtime: &mut JsRuntime) {
        if scripts.is_empty() {
            return;
        }
        let _ = runtime.eval("if (!globalThis.__braille_worker_scripts) globalThis.__braille_worker_scripts = {};");
        for (url, content) in scripts {
            let url_json = serde_json::to_string(url).unwrap();
            let content_json = serde_json::to_string(content).unwrap();
            let _ = runtime.eval(&format!(
                "globalThis.__braille_worker_scripts[{}] = {};",
                url_json, content_json
            ));
        }
    }

    /// After scripts have executed, walk the DOM for `<iframe>` elements with a `src`
    /// attribute. For each one whose content was pre-fetched, ensure the content doc
    /// is populated and fire any `onload` handler.
    pub(crate) fn process_iframe_loads(runtime: &mut JsRuntime, tree: &Rc<std::cell::RefCell<DomTree>>) {
        runtime.process_iframe_loads(tree);
    }
}
