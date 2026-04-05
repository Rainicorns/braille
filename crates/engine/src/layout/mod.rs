mod cache;
mod convert;
mod grid;

pub use cache::{LayoutCache, LayoutRect};

use std::collections::HashMap;

use crate::dom::node::{Node, NodeData, NodeId};
use convert::{measure_text, to_taffy_style};
use taffy::prelude::*;

/// Rebuild the taffy layout tree from the DOM and compute layout.
/// Results are stored in the LayoutCache.
pub fn ensure_computed(cache: &mut LayoutCache, nodes: &[Node]) {
    if !cache.is_dirty() {
        return;
    }

    let mut taffy = TaffyTree::<NodeId>::new();
    let mut dom_to_taffy: HashMap<NodeId, taffy::NodeId> = HashMap::new();

    // Phase 1: Create taffy nodes for every DOM node in tree order.
    // We walk linearly since node IDs are indices.
    for node in nodes {
        let nid = node.id;

        match &node.data {
            NodeData::Element { .. } => {
                // display:contents — element generates no box; children promoted to parent
                let is_contents = node.computed_style.as_ref()
                    .and_then(|cs| cs.get("display"))
                    .map(|v| v == "contents")
                    .unwrap_or(false);
                if is_contents {
                    // No taffy node — children will be reparented in Phase 2
                    continue;
                }
                let style = to_taffy_style(node);
                let taffy_node = taffy.new_leaf(style).unwrap();
                taffy.set_node_context(taffy_node, Some(nid)).unwrap();
                dom_to_taffy.insert(nid, taffy_node);
            }
            NodeData::Text { content } => {
                // Get font-size and line-height from parent's computed style
                let (font_size, line_height) = parent_text_metrics(nodes, node);
                let text = content.clone();
                let vw = cache.viewport.0;

                let taffy_node = taffy
                    .new_leaf_with_context(
                        Style::default(),
                        nid,
                    )
                    .unwrap();

                // Set measure function for text
                let (tw, th) = measure_text(&text, font_size, line_height, vw);
                taffy
                    .set_style(
                        taffy_node,
                        Style {
                            size: Size {
                                width: Dimension::Length(tw),
                                height: Dimension::Length(th),
                            },
                            ..Style::default()
                        },
                    )
                    .unwrap();
                dom_to_taffy.insert(nid, taffy_node);
            }
            NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                // Container nodes — create a block-level taffy node
                let style = Style {
                    display: taffy::Display::Block,
                    size: Size {
                        width: Dimension::Length(cache.viewport.0),
                        height: Dimension::Auto,
                    },
                    ..Style::default()
                };
                let taffy_node = taffy.new_leaf(style).unwrap();
                taffy.set_node_context(taffy_node, Some(nid)).unwrap();
                dom_to_taffy.insert(nid, taffy_node);
            }
            // Skip comments, doctypes, PIs, etc.
            _ => {}
        }
    }

    // Phase 2: Wire up parent-child relationships.
    // For display:contents elements (no taffy node), promote their children
    // to the nearest ancestor that has a taffy node.
    fn collect_taffy_children(
        nodes: &[Node],
        nid: NodeId,
        dom_to_taffy: &HashMap<NodeId, taffy::NodeId>,
    ) -> Vec<taffy::NodeId> {
        let node = &nodes[nid];
        let mut result = Vec::new();
        for &child_nid in &node.children {
            if let Some(&taffy_id) = dom_to_taffy.get(&child_nid) {
                result.push(taffy_id);
            } else {
                // Child has no taffy node (display:contents) — promote its children
                result.extend(collect_taffy_children(nodes, child_nid, dom_to_taffy));
            }
        }
        result
    }

    for node in nodes {
        let nid = node.id;
        let Some(&taffy_parent) = dom_to_taffy.get(&nid) else {
            continue;
        };

        let child_taffy_ids = collect_taffy_children(nodes, nid, &dom_to_taffy);

        if !child_taffy_ids.is_empty() {
            taffy.set_children(taffy_parent, &child_taffy_ids).unwrap();
        }
    }

    // Phase 3: Find the root (Document node = 0) and compute layout.
    if let Some(&root_taffy) = dom_to_taffy.get(&0) {
        let available = Size {
            width: AvailableSpace::Definite(cache.viewport.0),
            height: AvailableSpace::Definite(cache.viewport.1),
        };
        taffy.compute_layout(root_taffy, available).unwrap();

        // Phase 4: Extract layout results.
        cache.results.clear();
        collect_layout_results(&taffy, root_taffy, 0.0, 0.0, &mut cache.results);
    }

    cache.set_clean();
}

/// Recursively collect layout rects, accumulating absolute positions.
fn collect_layout_results(
    taffy: &TaffyTree<NodeId>,
    taffy_node: taffy::NodeId,
    parent_x: f32,
    parent_y: f32,
    results: &mut HashMap<NodeId, LayoutRect>,
) {
    let layout = taffy.layout(taffy_node).unwrap();
    let abs_x = parent_x + layout.location.x;
    let abs_y = parent_y + layout.location.y;

    if let Some(&dom_nid) = taffy.get_node_context(taffy_node) {
        results.insert(
            dom_nid,
            LayoutRect {
                x: abs_x,
                y: abs_y,
                width: layout.size.width,
                height: layout.size.height,
            },
        );
    }

    for &child in taffy.children(taffy_node).unwrap().iter() {
        collect_layout_results(taffy, child, abs_x, abs_y, results);
    }
}

/// Get font_size and line_height from the parent element's computed style.
fn parent_text_metrics(nodes: &[Node], text_node: &Node) -> (f32, f32) {
    if let Some(parent_id) = text_node.parent {
        if let Some(cs) = &nodes[parent_id].computed_style {
            let font_size = cs
                .get("font-size")
                .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
                .unwrap_or(16.0);
            let line_height = cs
                .get("line-height")
                .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
                .unwrap_or(font_size * 1.2);
            return (font_size, line_height);
        }
    }
    (16.0, 19.2)
}
