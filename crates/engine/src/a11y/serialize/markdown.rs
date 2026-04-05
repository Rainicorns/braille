use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;

use super::helpers::{
    collect_deep_text, is_display_none, is_parent_block_level, is_visibility_hidden, SKIP_ELEMENTS,
};

/// Context passed through the recursive walk to track list nesting, etc.
struct MdContext {
    /// Stack of list types: true = ordered, false = unordered.
    /// Each entry also tracks the current item counter for ordered lists.
    list_stack: Vec<ListInfo>,
    /// Whether we are inside a <pre> element (suppress formatting).
    in_pre: bool,
    /// Whether we are inside a <blockquote>.
    blockquote_depth: usize,
}

struct ListInfo {
    ordered: bool,
    counter: usize,
}

impl MdContext {
    fn new() -> Self {
        Self {
            list_stack: Vec::new(),
            in_pre: false,
            blockquote_depth: 0,
        }
    }

    fn list_indent(&self) -> String {
        // Each nesting level adds 2 spaces of indent (beyond the first level).
        if self.list_stack.len() > 1 {
            "  ".repeat(self.list_stack.len() - 1)
        } else {
            String::new()
        }
    }
}

pub fn serialize_markdown(tree: &DomTree) -> String {
    let mut output = String::new();
    let mut ctx = MdContext::new();
    walk_md(tree, tree.document(), &mut output, &mut ctx);
    // Clean up: collapse 3+ newlines to 2, trim
    let cleaned = collapse_excessive_newlines(&output);
    cleaned.trim().to_string()
}

fn walk_md(tree: &DomTree, node_id: NodeId, output: &mut String, ctx: &mut MdContext) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Text { content } | NodeData::CDATASection { content } => {
            if ctx.in_pre {
                output.push_str(content);
            } else {
                let collapsed = collapse_inline_whitespace(content);
                if !collapsed.is_empty() {
                    // In block-level parents, whitespace-only text nodes are
                    // inter-block spacing and should be suppressed — just like
                    // browsers strip them during layout. In inline parents,
                    // they're inter-element spacing (e.g., between two <span>s)
                    // and must be preserved as a single space.
                    let is_whitespace_only = collapsed.trim().is_empty();
                    if is_whitespace_only && is_parent_block_level(tree, node_id) {
                        // Suppress: whitespace between block children
                    } else {
                        output.push_str(&collapsed);
                    }
                }
            }
        }
        NodeData::Element { tag_name, attributes, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) {
                return;
            }
            if is_visibility_hidden(node) {
                return;
            }
            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            let children: Vec<NodeId> = node.children.clone();

            match tag.as_str() {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level: usize = tag[1..].parse().unwrap_or(1);
                    let hashes = "#".repeat(level);
                    ensure_blank_line(output);
                    output.push_str(&hashes);
                    output.push(' ');
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    // Trim trailing whitespace from the heading line
                    let trimmed_len = output.trim_end().len();
                    output.truncate(trimmed_len);
                    output.push_str("\n\n");
                }
                "p" => {
                    ensure_blank_line(output);
                    if ctx.blockquote_depth > 0 {
                        output.push_str(&"> ".repeat(ctx.blockquote_depth));
                    }
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    output.push_str("\n\n");
                }
                "br" => {
                    output.push('\n');
                }
                "hr" => {
                    ensure_blank_line(output);
                    output.push_str("---\n\n");
                }
                "a" => {
                    let href = attributes.iter().find(|a| a.local_name == "href");
                    let text = collect_deep_text(tree, node_id).trim().to_string();
                    if let Some(href_attr) = href {
                        output.push_str(&format!("[{}]({})", text, href_attr.value));
                    } else {
                        output.push_str(&text);
                    }
                }
                "img" => {
                    let src = attributes
                        .iter()
                        .find(|a| a.local_name == "src")
                        .map(|a| &*a.value)
                        .unwrap_or("");
                    let alt = attributes
                        .iter()
                        .find(|a| a.local_name == "alt")
                        .map(|a| &*a.value)
                        .unwrap_or("");
                    output.push_str(&format!("![{}]({})", alt, src));
                }
                "strong" | "b" => {
                    output.push_str("**");
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    output.push_str("**");
                }
                "em" | "i" => {
                    output.push('*');
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    output.push('*');
                }
                "code" => {
                    if ctx.in_pre {
                        // Inside <pre><code>, just emit children (the pre handler wraps in ```)
                        for child_id in children {
                            walk_md(tree, child_id, output, ctx);
                        }
                    } else {
                        // Inline code
                        output.push('`');
                        let text = collect_deep_text(tree, node_id);
                        output.push_str(text.trim());
                        output.push('`');
                    }
                }
                "pre" => {
                    ensure_blank_line(output);
                    let lang = detect_code_language(tree, &children);
                    output.push_str("```");
                    output.push_str(&lang);
                    output.push('\n');
                    ctx.in_pre = true;
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    ctx.in_pre = false;
                    // Ensure trailing newline before closing fence
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("```\n\n");
                }
                "blockquote" => {
                    ctx.blockquote_depth += 1;
                    ensure_blank_line(output);
                    // Walk children, which should be block elements (p, etc.)
                    // For simple blockquotes with just text, wrap in > prefix
                    let has_block_children = children.iter().any(|&cid| {
                        let child = tree.get_node(cid);
                        matches!(&child.data, NodeData::Element { tag_name, .. } if {
                            let t = tag_name.to_ascii_lowercase();
                            matches!(t.as_str(), "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "blockquote")
                        })
                    });

                    if has_block_children {
                        for child_id in children {
                            walk_md(tree, child_id, output, ctx);
                        }
                    } else {
                        // Simple blockquote with just text content
                        let prefix = "> ".repeat(ctx.blockquote_depth);
                        output.push_str(&prefix);
                        for child_id in children {
                            walk_md(tree, child_id, output, ctx);
                        }
                        output.push_str("\n\n");
                    }
                    ctx.blockquote_depth -= 1;
                }
                "ul" => {
                    ensure_blank_line(output);
                    ctx.list_stack.push(ListInfo {
                        ordered: false,
                        counter: 0,
                    });
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    ctx.list_stack.pop();
                    ensure_newline(output);
                }
                "ol" => {
                    ensure_blank_line(output);
                    ctx.list_stack.push(ListInfo {
                        ordered: true,
                        counter: 0,
                    });
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    ctx.list_stack.pop();
                    ensure_newline(output);
                }
                "li" => {
                    let indent = ctx.list_indent();
                    let marker = if let Some(list_info) = ctx.list_stack.last_mut() {
                        if list_info.ordered {
                            list_info.counter += 1;
                            format!("{}. ", list_info.counter)
                        } else {
                            "- ".to_string()
                        }
                    } else {
                        // Orphan li without list parent
                        "- ".to_string()
                    };
                    output.push_str(&indent);
                    output.push_str(&marker);
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                    ensure_newline(output);
                }
                "table" => {
                    ensure_blank_line(output);
                    render_table(tree, node_id, output);
                    output.push('\n');
                }
                // Transparent containers: just recurse
                "div" | "span" | "section" | "article" | "header" | "footer"
                | "html" | "body" | "main" | "nav" | "form" | "fieldset"
                | "figure" | "figcaption" | "details" | "summary" | "dl"
                | "dt" | "dd" => {
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                }
                // Skip table sub-elements when encountered outside render_table
                "thead" | "tbody" | "tfoot" | "tr" | "th" | "td" | "caption"
                | "colgroup" | "col" => {
                    // These are handled by render_table; skip if encountered raw
                }
                // Unknown elements: just recurse into children
                _ => {
                    for child_id in children {
                        walk_md(tree, child_id, output, ctx);
                    }
                }
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_md(tree, child_id, output, ctx);
            }
        }
        _ => {}
    }
}

/// Render an HTML table as a markdown table.
fn render_table(tree: &DomTree, table_id: NodeId, output: &mut String) {
    let rows = collect_table_rows(tree, table_id);
    if rows.is_empty() {
        return;
    }

    // Determine column count from the widest row
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return;
    }

    // First row is the header
    render_table_row(&rows[0], col_count, output);
    output.push('\n');

    // Separator
    let sep_cells: Vec<String> = (0..col_count).map(|_| "---".to_string()).collect();
    output.push_str("| ");
    output.push_str(&sep_cells.join(" | "));
    output.push_str(" |\n");

    // Body rows
    for row in &rows[1..] {
        render_table_row(row, col_count, output);
        output.push('\n');
    }
}

fn render_table_row(cells: &[String], col_count: usize, output: &mut String) {
    output.push_str("| ");
    for i in 0..col_count {
        if i > 0 {
            output.push_str(" | ");
        }
        if i < cells.len() {
            output.push_str(&cells[i]);
        }
        // If fewer cells than col_count, empty cells are already handled by not pushing content
    }
    output.push_str(" |");
}

/// Collect all rows from a table, including rows inside thead/tbody/tfoot.
fn collect_table_rows(tree: &DomTree, table_id: NodeId) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let table_node = tree.get_node(table_id);
    for &child_id in &table_node.children {
        let child = tree.get_node(child_id);
        if let NodeData::Element { tag_name, .. } = &child.data {
            let tag = tag_name.to_ascii_lowercase();
            match tag.as_str() {
                "tr" => {
                    rows.push(collect_row_cells(tree, child_id));
                }
                "thead" | "tbody" | "tfoot" => {
                    let section = tree.get_node(child_id);
                    for &row_id in &section.children {
                        let row = tree.get_node(row_id);
                        if let NodeData::Element { tag_name: row_tag, .. } = &row.data {
                            if row_tag.eq_ignore_ascii_case("tr") {
                                rows.push(collect_row_cells(tree, row_id));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    rows
}

fn collect_row_cells(tree: &DomTree, tr_id: NodeId) -> Vec<String> {
    let mut cells = Vec::new();
    let tr_node = tree.get_node(tr_id);
    for &child_id in &tr_node.children {
        let child = tree.get_node(child_id);
        if let NodeData::Element { tag_name, attributes, .. } = &child.data {
            let tag = tag_name.to_ascii_lowercase();
            if tag == "td" || tag == "th" {
                let text = collect_deep_text(tree, child_id).trim().to_string();
                let colspan: usize = attributes
                    .iter()
                    .find(|a| a.local_name == "colspan")
                    .and_then(|a| a.value.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                for _ in 0..colspan {
                    cells.push(text.clone());
                }
            }
        }
    }
    cells
}

/// Check if a `<pre>` element's first `<code>` child has a `language-*` or `lang-*` class.
fn detect_code_language(tree: &DomTree, children: &[NodeId]) -> String {
    for &child_id in children {
        let child = tree.get_node(child_id);
        if let NodeData::Element { tag_name, attributes, .. } = &child.data {
            if tag_name.eq_ignore_ascii_case("code") {
                if let Some(class_attr) = attributes.iter().find(|a| a.local_name == "class") {
                    for cls in class_attr.value.split_whitespace() {
                        if let Some(lang) = cls.strip_prefix("language-").or_else(|| cls.strip_prefix("lang-")) {
                            if !lang.is_empty() {
                                return lang.to_string();
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    String::new()
}

fn ensure_blank_line(output: &mut String) {
    if output.is_empty() {
        return;
    }
    if !output.ends_with("\n\n") {
        if output.ends_with('\n') {
            output.push('\n');
        } else {
            output.push_str("\n\n");
        }
    }
}

fn ensure_newline(output: &mut String) {
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
}

fn collapse_inline_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_ascii_whitespace() {
            if !prev_ws {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            prev_ws = false;
            result.push(c);
        }
    }
    result
}

fn collapse_excessive_newlines(s: &str) -> String {
    let mut result = String::new();
    let mut newline_count = 0;
    for c in s.chars() {
        if c == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                result.push(c);
            }
        } else {
            newline_count = 0;
            result.push(c);
        }
    }
    result
}
