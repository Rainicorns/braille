use libtest_mimic::{Arguments, Failed, Trial};
use std::collections::HashMap;
use std::path::Path;

use braille_engine::dom::node::{NodeData, NodeId};
use braille_engine::dom::tree::DomTree;
use braille_engine::html::{parse_html_fragment_scripting, parse_html_scripting};

// ---------------------------------------------------------------------------
// .dat file parser
// ---------------------------------------------------------------------------

struct TestCase {
    data: String,
    expected: String,
    fragment_context: Option<String>, // e.g. "svg path" or "body"
    script_on: bool,
    file_name: String,
    index: usize,
}

fn parse_dat_file(path: &Path) -> Vec<TestCase> {
    let contents = std::fs::read_to_string(path).unwrap();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();

    let mut tests = Vec::new();

    // Split on "#data\n" to get individual tests.
    // The first element before the first #data is usually empty or a comment.
    let parts: Vec<&str> = contents.split("\n#data\n").collect();

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            // Check if the file starts with #data (no leading newline)
            if part.starts_with("#data") {
                // Remove the leading "#data\n" since our split won't handle the very first one
                let stripped = &part["#data".len()..];
                let stripped = stripped.strip_prefix('\n').unwrap_or(stripped);
                if let Some(tc) = parse_single_test(stripped, &file_name, tests.len()) {
                    tests.push(tc);
                }
            }
            continue;
        }

        if let Some(tc) = parse_single_test(part, &file_name, tests.len()) {
            tests.push(tc);
        }
    }

    tests
}

fn parse_single_test(part: &str, file_name: &str, index: usize) -> Option<TestCase> {
    let mut sections: HashMap<String, String> = HashMap::new();
    let mut current_section = String::from("data");
    let mut current_content = String::new();
    // Default: scripting enabled (matches html5ever default)
    let mut script_on = true;

    for line in part.lines() {
        if line.starts_with('#') && !line.starts_with("| ") {
            // Save previous section
            if !current_section.is_empty() {
                let val = if current_section == "data" {
                    current_content.clone()
                } else {
                    current_content.trim_end_matches('\n').to_string()
                };
                sections.insert(current_section.clone(), val);
            }

            // Handle script flags (per-test section markers)
            if line == "#script-on" {
                script_on = true;
                current_section.clear();
                current_content.clear();
                continue;
            }
            if line == "#script-off" {
                script_on = false;
                current_section.clear();
                current_content.clear();
                continue;
            }

            current_section = line[1..].to_string();
            current_content = String::new();
        } else {
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
        }
    }
    // Save last section
    if !current_section.is_empty() {
        let val = if current_section == "data" {
            current_content.clone()
        } else {
            current_content.trim_end_matches('\n').to_string()
        };
        sections.insert(current_section, val);
    }

    let data = sections.get("data")?;
    let expected = sections.get("document")?;

    Some(TestCase {
        data: data.clone(),
        expected: expected.clone(),
        fragment_context: sections.get("document-fragment").cloned(),
        script_on,
        file_name: file_name.to_string(),
        index,
    })
}

// ---------------------------------------------------------------------------
// DOM serializer (html5lib pipe-indented format)
// ---------------------------------------------------------------------------

fn serialize_tree(tree: &DomTree) -> String {
    let mut out = String::new();
    let doc = tree.document();
    let node = tree.get_node(doc);
    for &child_id in &node.children {
        serialize_node(tree, child_id, 0, &mut out);
    }
    // Remove trailing newline
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn serialize_node(tree: &DomTree, node_id: NodeId, depth: usize, out: &mut String) {
    let node = tree.get_node(node_id);
    let indent = "  ".repeat(depth);

    match &node.data {
        NodeData::Document => {
            for &child_id in &node.children {
                serialize_node(tree, child_id, depth, out);
            }
        }
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            if public_id.is_empty() && system_id.is_empty() {
                out.push_str(&format!("| {indent}<!DOCTYPE {name}>\n"));
            } else {
                out.push_str(&format!(
                    "| {indent}<!DOCTYPE {name} \"{public_id}\" \"{system_id}\">\n"
                ));
            }
        }
        NodeData::Element {
            tag_name,
            attributes,
            namespace,
        } => {
            if namespace.is_empty() || namespace == "http://www.w3.org/1999/xhtml" {
                out.push_str(&format!("| {indent}<{tag_name}>\n"));
            } else {
                // html5lib format uses short prefixes for SVG/MathML
                let ns_label = match namespace.as_str() {
                    "http://www.w3.org/2000/svg" => "svg",
                    "http://www.w3.org/1998/Math/MathML" => "math",
                    other => other,
                };
                out.push_str(&format!("| {indent}<{ns_label} {tag_name}>\n"));
            }

            // Sort attributes lexicographically by key name for deterministic output.
            // html5lib tests expect sorted attributes.
            let mut sorted_attrs: Vec<_> = attributes.iter().collect();
            sorted_attrs.sort_by(|a, b| {
                let key_a = if !a.prefix.is_empty() {
                    format!("{} {}", a.prefix, a.local_name)
                } else {
                    a.local_name.clone()
                };
                let key_b = if !b.prefix.is_empty() {
                    format!("{} {}", b.prefix, b.local_name)
                } else {
                    b.local_name.clone()
                };
                key_a.cmp(&key_b)
            });

            for attr in sorted_attrs {
                // html5lib test format uses "prefix localname" (space-separated) for
                // namespaced attributes, not "prefix:localname" (colon-separated).
                let key = if !attr.prefix.is_empty() {
                    format!("{} {}", attr.prefix, attr.local_name)
                } else {
                    attr.local_name.clone()
                };
                let value = &attr.value;
                out.push_str(&format!("| {indent}  {key}=\"{value}\"\n"));
            }

            // For <template> elements, emit the "content" line and serialize
            // the content fragment's children at depth+2.
            if let Some(content_id) = node.template_contents {
                let content_indent = "  ".repeat(depth + 1);
                out.push_str(&format!("| {content_indent}content\n"));
                let content_node = tree.get_node(content_id);
                for &child_id in &content_node.children {
                    serialize_node(tree, child_id, depth + 2, out);
                }
            } else {
                // Recurse into children
                for &child_id in &node.children {
                    serialize_node(tree, child_id, depth + 1, out);
                }
            }
        }
        NodeData::Text { content } => {
            out.push_str(&format!("| {indent}\"{content}\"\n"));
        }
        NodeData::Comment { content } => {
            out.push_str(&format!("| {indent}<!-- {content} -->\n"));
        }
        NodeData::ProcessingInstruction { target, data } => {
            out.push_str(&format!("| {indent}<?{target} {data}?>\n"));
        }
        NodeData::DocumentFragment => {
            for &child_id in &node.children {
                serialize_node(tree, child_id, depth, out);
            }
        }
        NodeData::Attr { .. } => {
            // Attr nodes don't appear in tree-construction serialization
        }
        NodeData::CDATASection { content } => {
            out.push_str(&format!("| {indent}\"{content}\"\n"));
        }
        NodeData::ShadowRoot { .. } => {
            for &child_id in &node.children {
                serialize_node(tree, child_id, depth, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Expected failures
// ---------------------------------------------------------------------------

fn should_skip(_test: &TestCase) -> Option<&'static str> {
    None
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

fn run_test(test: &TestCase) -> Result<(), Failed> {
    let tree_rc = if let Some(ref ctx) = test.fragment_context {
        // Parse context: "namespace tag" or just "tag"
        let parts: Vec<&str> = ctx.split_whitespace().collect();
        let (ns, tag) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", parts[0])
        };
        parse_html_fragment_scripting(&test.data, tag, ns, test.script_on)
    } else {
        parse_html_scripting(&test.data, test.script_on)
    };

    let tree = tree_rc.borrow();

    let actual = if test.fragment_context.is_some() {
        // For fragment tests, the tree has a Document root with an html node.
        // The html node's children are what we want to serialize.
        // html5ever fragment parsing creates: document > html > [children...]
        // The expected output is the children of the html element.
        let doc = tree.document();
        let doc_node = tree.get_node(doc);
        if let Some(&html_id) = doc_node.children.first() {
            let html_node = tree.get_node(html_id);
            let mut out = String::new();
            for &child_id in &html_node.children {
                serialize_node(&tree, child_id, 0, &mut out);
            }
            if out.ends_with('\n') {
                out.pop();
            }
            out
        } else {
            String::new()
        }
    } else {
        serialize_tree(&tree)
    };

    let expected = &test.expected;

    if actual == *expected {
        Ok(())
    } else {
        Err(format!(
            "Input: {:?}\n\nExpected:\n{}\n\nActual:\n{}\n",
            test.data, expected, actual
        )
        .into())
    }
}

fn main() {
    let args = Arguments::from_args();

    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/html5lib-tests/tree-construction");

    let mut trials = Vec::new();

    let mut dat_files: Vec<_> = std::fs::read_dir(&test_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "dat").unwrap_or(false))
        .collect();
    dat_files.sort_by_key(|e| e.file_name());

    for entry in dat_files {
        let path = entry.path();
        let tests = parse_dat_file(&path);

        for test in tests {
            let name = format!(
                "{}::{}::{}",
                test.file_name.trim_end_matches(".dat"),
                test.index,
                test.data.chars().take(60).collect::<String>().replace('\n', "\\n")
            );

            let ignored = should_skip(&test).is_some();

            trials.push(Trial::test(name, move || run_test(&test)).with_ignored_flag(ignored));
        }
    }

    libtest_mimic::run(&args, trials).exit();
}
