use crate::dom::{DomTree, NodeId};
use crate::Engine;
use braille_wire::{HttpMethod, NavigateRequest};

impl Engine {
    /// Collect form data and build a NavigateRequest for form submission.
    /// `button_id` is the NodeId of the clicked submit button.
    /// Returns Err with a clear message if no associated form element is found.
    pub fn handle_form_submit(&self, button_id: NodeId) -> Result<NavigateRequest, String> {
        let tree = self.tree.borrow();

        // 1. Find the associated <form> element.
        //    First check if the button has a `form` attribute pointing to a form by ID.
        //    Fall back to ancestor walking.
        let form_id = find_associated_form(&tree, button_id)
            .ok_or_else(|| format!("no parent <form> found for submit button (node {})", button_id))?;

        // 2. Get form's action attribute (default to current URL or "")
        let action = tree.get_attribute(form_id, "action").unwrap_or_default();

        // 3. Get form's method attribute (default to "get")
        let method_str = tree
            .get_attribute(form_id, "method")
            .unwrap_or_else(|| "get".to_string())
            .to_ascii_lowercase();

        let method = if method_str == "post" {
            HttpMethod::Post
        } else {
            HttpMethod::Get
        };

        // 4. Collect form data
        let form_data = collect_form_data(&tree, form_id);

        // 5. Build URL-encoded body or query string
        let encoded = url_encode_form_data(&form_data);

        // 6. Return NavigateRequest
        match method {
            HttpMethod::Get => {
                // For GET, append query string to URL
                let url = if encoded.is_empty() {
                    action
                } else if action.contains('?') {
                    format!("{}&{}", action, encoded)
                } else {
                    format!("{}?{}", action, encoded)
                };

                Ok(NavigateRequest {
                    url,
                    method: HttpMethod::Get,
                    body: None,
                    content_type: None,
                })
            }
            HttpMethod::Post => {
                // For POST, put data in body
                Ok(NavigateRequest {
                    url: action,
                    method: HttpMethod::Post,
                    body: Some(encoded),
                    content_type: Some("application/x-www-form-urlencoded".to_string()),
                })
            }
        }
    }
}

/// Find the form associated with an element.
/// First checks for a `form` attribute pointing to a form by ID,
/// then falls back to ancestor walking.
fn find_associated_form(tree: &DomTree, element_id: NodeId) -> Option<NodeId> {
    // Check for form attribute
    if let Some(form_attr) = tree.get_attribute(element_id, "form") {
        if !form_attr.is_empty() {
            if let Some(target_id) = tree.get_element_by_id(&form_attr) {
                if let crate::dom::NodeData::Element { ref tag_name, .. } = tree.get_node(target_id).data {
                    if tag_name.eq_ignore_ascii_case("form") {
                        return Some(target_id);
                    }
                }
            }
            return None;
        }
    }
    // Fallback: walk up ancestors
    tree.find_ancestor(element_id, "form")
}

/// Find all elements in the document that reference the given form via form="<id>" attribute.
/// Only returns elements with the given tag names.
fn find_form_associated_elements(tree: &DomTree, form_id: NodeId, tag: &str) -> Vec<NodeId> {
    let form_id_attr = match tree.get_attribute(form_id, "id") {
        Some(id) if !id.is_empty() => id,
        _ => return Vec::new(),
    };

    let doc = tree.document();
    let all_of_tag = tree.find_descendants_by_tag(doc, tag);
    let mut results = Vec::new();
    for el_id in all_of_tag {
        // Only include elements NOT already descendants of this form
        if tree.find_ancestor(el_id, "form") == Some(form_id) {
            continue;
        }
        if let Some(ref attr_val) = tree.get_attribute(el_id, "form") {
            if attr_val == &form_id_attr {
                results.push(el_id);
            }
        }
    }
    results
}

/// Collect name=value pairs from all form controls associated with a form element.
/// Finds all <input>, <select>, <textarea> descendants and also elements that
/// reference the form via a `form` attribute.
/// Skips inputs without a name attribute.
/// For checkboxes/radios, only includes them if they have a "checked" attribute.
pub fn collect_form_data(tree: &DomTree, form_id: NodeId) -> Vec<(String, String)> {
    let mut data = Vec::new();

    // Find all input elements (descendants + form-associated)
    let mut inputs = tree.find_descendants_by_tag(form_id, "input");
    inputs.extend(find_form_associated_elements(tree, form_id, "input"));
    for input_id in inputs {
        // Skip if no name attribute
        let name = match tree.get_attribute(input_id, "name") {
            Some(n) => n,
            None => continue,
        };

        // Check input type
        let input_type = tree
            .get_attribute(input_id, "type")
            .unwrap_or_else(|| "text".to_string())
            .to_ascii_lowercase();

        // For checkbox/radio, only include if checked
        if (input_type == "checkbox" || input_type == "radio") && !tree.has_attribute(input_id, "checked") {
            continue;
        }

        // Get value (default to empty string)
        let value = tree.get_attribute(input_id, "value").unwrap_or_default();

        data.push((name, value));
    }

    // Find all select elements (descendants + form-associated)
    let mut selects = tree.find_descendants_by_tag(form_id, "select");
    selects.extend(find_form_associated_elements(tree, form_id, "select"));
    for select_id in selects {
        // Skip if no name attribute
        let name = match tree.get_attribute(select_id, "name") {
            Some(n) => n,
            None => continue,
        };

        // Get value attribute
        let value = tree.get_attribute(select_id, "value").unwrap_or_default();

        data.push((name, value));
    }

    // Find all textarea elements (descendants + form-associated)
    let mut textareas = tree.find_descendants_by_tag(form_id, "textarea");
    textareas.extend(find_form_associated_elements(tree, form_id, "textarea"));
    for textarea_id in textareas {
        // Skip if no name attribute
        let name = match tree.get_attribute(textarea_id, "name") {
            Some(n) => n,
            None => continue,
        };

        // Get value attribute
        let value = tree.get_attribute(textarea_id, "value").unwrap_or_default();

        data.push((name, value));
    }

    data
}

/// URL-encode form data into a query string like "name=value&name2=value2".
/// Implements simple percent-encoding for special characters.
pub fn url_encode_form_data(data: &[(String, String)]) -> String {
    data.iter()
        .map(|(name, value)| format!("{}={}", url_encode(name), url_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Simple URL encoding helper.
/// Encodes spaces as '+' and special characters as %XX.
fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => {
                result.push('+');
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::node::DomAttribute;
    use crate::dom::DomTree;

    #[test]
    fn collect_form_data_from_form_with_text_inputs() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let input1 = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "text"),
                DomAttribute::new("name", "username"),
                DomAttribute::new("value", "alice"),
            ],
        );
        let input2 = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "email"),
                DomAttribute::new("name", "email"),
                DomAttribute::new("value", "alice@example.com"),
            ],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, input1);
        tree.append_child(form, input2);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0], ("username".to_string(), "alice".to_string()));
        assert_eq!(data[1], ("email".to_string(), "alice@example.com".to_string()));
    }

    #[test]
    fn collect_form_data_skips_inputs_without_name() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let input1 = tree.create_element_with_attrs(
            "input",
            vec![DomAttribute::new("type", "text"), DomAttribute::new("value", "no-name")],
        );
        let input2 = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "text"),
                DomAttribute::new("name", "username"),
                DomAttribute::new("value", "alice"),
            ],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, input1);
        tree.append_child(form, input2);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 1);
        assert_eq!(data[0], ("username".to_string(), "alice".to_string()));
    }

    #[test]
    fn collect_form_data_includes_checked_checkbox() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let checkbox = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "checkbox"),
                DomAttribute::new("name", "subscribe"),
                DomAttribute::new("value", "yes"),
                DomAttribute::new("checked", ""),
            ],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, checkbox);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 1);
        assert_eq!(data[0], ("subscribe".to_string(), "yes".to_string()));
    }

    #[test]
    fn collect_form_data_skips_unchecked_checkbox() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let checkbox = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "checkbox"),
                DomAttribute::new("name", "subscribe"),
                DomAttribute::new("value", "yes"),
            ],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, checkbox);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 0);
    }

    #[test]
    fn collect_form_data_includes_select_elements() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let select = tree.create_element_with_attrs(
            "select",
            vec![DomAttribute::new("name", "country"), DomAttribute::new("value", "USA")],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, select);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 1);
        assert_eq!(data[0], ("country".to_string(), "USA".to_string()));
    }

    #[test]
    fn collect_form_data_includes_textarea_elements() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let textarea = tree.create_element_with_attrs(
            "textarea",
            vec![
                DomAttribute::new("name", "message"),
                DomAttribute::new("value", "Hello world"),
            ],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, textarea);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 1);
        assert_eq!(data[0], ("message".to_string(), "Hello world".to_string()));
    }

    #[test]
    fn collect_form_data_handles_nested_elements() {
        let mut tree = DomTree::new();
        let form = tree.create_element("form");
        let div = tree.create_element("div");
        let input = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "text"),
                DomAttribute::new("name", "nested"),
                DomAttribute::new("value", "value"),
            ],
        );

        tree.append_child(tree.document(), form);
        tree.append_child(form, div);
        tree.append_child(div, input);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 1);
        assert_eq!(data[0], ("nested".to_string(), "value".to_string()));
    }

    #[test]
    fn url_encode_form_data_encodes_special_characters() {
        let data = vec![
            ("name".to_string(), "Alice Smith".to_string()),
            ("email".to_string(), "alice+test@example.com".to_string()),
        ];

        let encoded = url_encode_form_data(&data);

        assert_eq!(encoded, "name=Alice+Smith&email=alice%2Btest%40example.com");
    }

    #[test]
    fn url_encode_form_data_handles_empty_values() {
        let data = vec![
            ("name".to_string(), "alice".to_string()),
            ("comment".to_string(), "".to_string()),
        ];

        let encoded = url_encode_form_data(&data);

        assert_eq!(encoded, "name=alice&comment=");
    }

    #[test]
    fn url_encode_form_data_handles_empty_input() {
        let data: Vec<(String, String)> = vec![];

        let encoded = url_encode_form_data(&data);

        assert_eq!(encoded, "");
    }

    #[test]
    fn handle_form_submit_with_method_get_builds_url_with_query_string() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/search" method="get">
                    <input type="text" name="q" value="rust" />
                    <button type="submit" id="submit-btn">Search</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(request.url, "/search?q=rust");
        assert_eq!(request.body, None);
        assert_eq!(request.content_type, None);
    }

    #[test]
    fn handle_form_submit_with_method_post_builds_body() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/submit" method="post">
                    <input type="text" name="username" value="alice" />
                    <input type="email" name="email" value="alice@example.com" />
                    <button type="submit" id="submit-btn">Submit</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.url, "/submit");
        assert_eq!(
            request.body,
            Some("username=alice&email=alice%40example.com".to_string())
        );
        assert_eq!(
            request.content_type,
            Some("application/x-www-form-urlencoded".to_string())
        );
    }

    #[test]
    fn handle_form_submit_returns_error_when_no_ancestor_form_found() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <button type="submit" id="orphan-btn">Submit</button>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("orphan-btn").unwrap();
        drop(tree);

        let result = engine.handle_form_submit(button_id);

        assert!(result.is_err(), "expected Err when no parent form");
        let err = result.unwrap_err();
        assert!(
            err.contains("no parent <form> found"),
            "error should mention no parent form, got: {}",
            err
        );
    }

    #[test]
    fn handle_form_submit_defaults_to_get_when_no_method() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/search">
                    <input type="text" name="q" value="test" />
                    <button type="submit" id="submit-btn">Search</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(request.url, "/search?q=test");
    }

    #[test]
    fn handle_form_submit_defaults_to_empty_action_when_no_action() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form method="post">
                    <input type="text" name="data" value="test" />
                    <button type="submit" id="submit-btn">Submit</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.url, "");
    }

    #[test]
    fn handle_form_submit_with_get_appends_to_existing_query_string() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/search?category=books" method="get">
                    <input type="text" name="q" value="rust" />
                    <button type="submit" id="submit-btn">Search</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.url, "/search?category=books&q=rust");
    }

    #[test]
    fn handle_form_submit_handles_radio_buttons() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/vote" method="post">
                    <input type="radio" name="option" value="a" />
                    <input type="radio" name="option" value="b" checked />
                    <input type="radio" name="option" value="c" />
                    <button type="submit" id="submit-btn">Vote</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.body, Some("option=b".to_string()));
    }

    #[test]
    fn handle_form_submit_with_multiple_checkboxes() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/prefs" method="post">
                    <input type="checkbox" name="feature1" value="on" checked />
                    <input type="checkbox" name="feature2" value="on" />
                    <input type="checkbox" name="feature3" value="on" checked />
                    <button type="submit" id="submit-btn">Save</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.body, Some("feature1=on&feature3=on".to_string()));
    }

    #[test]
    fn url_encode_encodes_ampersand_and_equals() {
        let data = vec![("param".to_string(), "a&b=c".to_string())];

        let encoded = url_encode_form_data(&data);

        assert_eq!(encoded, "param=a%26b%3Dc");
    }

    #[test]
    fn collect_form_data_includes_external_inputs_with_form_attribute() {
        let mut tree = DomTree::new();
        let body = tree.create_element("body");
        let form = tree.create_element_with_attrs(
            "form",
            vec![DomAttribute::new("id", "myform"), DomAttribute::new("action", "/submit")],
        );
        let inside_input = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "text"),
                DomAttribute::new("name", "inside"),
                DomAttribute::new("value", "val1"),
            ],
        );
        let outside_input = tree.create_element_with_attrs(
            "input",
            vec![
                DomAttribute::new("type", "text"),
                DomAttribute::new("name", "outside"),
                DomAttribute::new("value", "val2"),
                DomAttribute::new("form", "myform"),
            ],
        );

        tree.append_child(tree.document(), body);
        tree.append_child(body, form);
        tree.append_child(form, inside_input);
        tree.append_child(body, outside_input);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0], ("inside".to_string(), "val1".to_string()));
        assert_eq!(data[1], ("outside".to_string(), "val2".to_string()));
    }

    #[test]
    fn collect_form_data_external_select_and_textarea_with_form_attribute() {
        let mut tree = DomTree::new();
        let body = tree.create_element("body");
        let form = tree.create_element_with_attrs(
            "form",
            vec![DomAttribute::new("id", "myform")],
        );
        let external_select = tree.create_element_with_attrs(
            "select",
            vec![
                DomAttribute::new("name", "color"),
                DomAttribute::new("value", "blue"),
                DomAttribute::new("form", "myform"),
            ],
        );
        let external_textarea = tree.create_element_with_attrs(
            "textarea",
            vec![
                DomAttribute::new("name", "notes"),
                DomAttribute::new("value", "hello"),
                DomAttribute::new("form", "myform"),
            ],
        );

        tree.append_child(tree.document(), body);
        tree.append_child(body, form);
        tree.append_child(body, external_select);
        tree.append_child(body, external_textarea);

        let data = collect_form_data(&tree, form);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0], ("color".to_string(), "blue".to_string()));
        assert_eq!(data[1], ("notes".to_string(), "hello".to_string()));
    }

    #[test]
    fn handle_form_submit_button_with_form_attribute() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form id="myform" action="/submit" method="post">
                    <input type="text" name="inside" value="val1" />
                </form>
                <input type="text" name="outside" value="val2" form="myform" />
                <button type="submit" id="submit-btn" form="myform">Submit</button>
            </body></html>
        "#,
        );

        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.url, "/submit");
        assert_eq!(
            request.body,
            Some("inside=val1&outside=val2".to_string())
        );
    }

    #[test]
    fn handle_form_submit_with_empty_form() {
        let mut engine = Engine::new();
        engine.load_html(
            r#"
            <html><body>
                <form action="/empty" method="get">
                    <button type="submit" id="submit-btn">Submit</button>
                </form>
            </body></html>
        "#,
        );

        // Find the button
        let tree = engine.tree.borrow();
        let button_id = tree.get_element_by_id("submit-btn").unwrap();
        drop(tree);

        let request = engine.handle_form_submit(button_id);

        assert!(request.is_ok(), "expected Ok, got {:?}", request);
        let request = request.unwrap();
        assert_eq!(request.url, "/empty");
        assert_eq!(request.body, None);
    }
}
