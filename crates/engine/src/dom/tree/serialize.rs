use crate::dom::node::{NodeData, NodeId};

use super::DomTree;

/// Work item for iterative HTML serialization.
enum Work {
    Open(NodeId),
    Close(String),
}

impl DomTree {
    fn is_void_element(tag: &str) -> bool {
        matches!(
            tag.to_ascii_lowercase().as_str(),
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
    }

    fn escape_html(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(ch),
            }
        }
        out
    }

    pub fn serialize_children_html(&self, nid: NodeId) -> String {
        let mut out = String::new();
        let mut stack: Vec<Work> = self.nodes[nid].children.iter().rev().map(|&c| Work::Open(c)).collect();
        self.serialize_iterative(&mut stack, &mut out);
        out
    }

    pub fn serialize_node_html(&self, nid: NodeId) -> String {
        let mut out = String::new();
        let mut stack = vec![Work::Open(nid)];
        self.serialize_iterative(&mut stack, &mut out);
        out
    }

    fn serialize_iterative(&self, stack: &mut Vec<Work>, out: &mut String) {
        while let Some(work) = stack.pop() {
            match work {
                Work::Open(nid) => {
                    let nd = &self.nodes[nid];
                    match &nd.data {
                        NodeData::Text { content } => {
                            out.push_str(&Self::escape_html(content));
                        }
                        NodeData::Comment { content } => {
                            out.push_str("<!--");
                            out.push_str(content);
                            out.push_str("-->");
                        }
                        NodeData::Doctype { name, .. } => {
                            out.push_str("<!DOCTYPE ");
                            out.push_str(name);
                            out.push('>');
                        }
                        NodeData::Element {
                            tag_name, attributes, ..
                        } => {
                            out.push('<');
                            out.push_str(tag_name);
                            for a in attributes {
                                out.push(' ');
                                out.push_str(&a.qualified_name());
                                out.push_str("=\"");
                                out.push_str(&Self::escape_html(&a.value));
                                out.push('"');
                            }
                            out.push('>');
                            if !Self::is_void_element(tag_name) {
                                stack.push(Work::Close(tag_name.clone()));
                                for &c in nd.children.iter().rev() {
                                    stack.push(Work::Open(c));
                                }
                            }
                        }
                        NodeData::ProcessingInstruction { target, data } => {
                            out.push_str("<?");
                            out.push_str(target);
                            if !data.is_empty() {
                                out.push(' ');
                                out.push_str(data);
                            }
                            out.push_str("?>");
                        }
                        NodeData::CDATASection { content } => {
                            out.push_str("<![CDATA[");
                            out.push_str(content);
                            out.push_str("]]>");
                        }
                        NodeData::Attr { .. } => {} // Attr nodes are not serialized as children
                        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                            for &c in nd.children.iter().rev() {
                                stack.push(Work::Open(c));
                            }
                        }
                    }
                }
                Work::Close(tag_name) => {
                    out.push_str("</");
                    out.push_str(&tag_name);
                    out.push('>');
                }
            }
        }
    }
}
