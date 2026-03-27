use crate::dom::node::NodeData;
use crate::dom::tree::DomTree;
use crate::dom::NodeId;

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
pub(crate) fn is_javascript_type(type_value: &str) -> bool {
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

use super::Engine;

impl Engine {
    /// Walk the DomTree recursively from the document root, collecting the text
    /// content of each `<script>` element in document order.
    pub(crate) fn collect_scripts(&self) -> Vec<String> {
        let tree = self.tree.borrow();
        let mut scripts = Vec::new();
        Self::walk_for_scripts(&tree, tree.document(), &mut scripts);
        scripts
    }

    pub(crate) fn walk_for_scripts(tree: &DomTree, root: NodeId, scripts: &mut Vec<String>) {
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
    pub(crate) fn collect_script_descriptors(&self) -> Vec<ScriptDescriptor> {
        let tree = self.tree.borrow();
        let mut descriptors = Vec::new();
        Self::walk_for_script_descriptors(&tree, tree.document(), &mut descriptors);
        descriptors
    }

    pub(crate) fn walk_for_script_descriptors(tree: &DomTree, root: NodeId, descriptors: &mut Vec<ScriptDescriptor>) {
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

    /// Parse an import map JSON and register bare specifier → URL mappings.
    /// For each entry in `imports`, if the URL has been pre-fetched, register it as a module.
    pub(crate) fn process_import_map(runtime: &mut crate::js::JsRuntime, json: &str, fetched: &super::FetchedResources) {
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
}
