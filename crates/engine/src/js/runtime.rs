use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, JsObject, ObjectInitializer},
    property::Attribute,
    Context, JsError, JsResult, JsValue, Source,
};

use crate::dom::DomTree;

use super::bindings;
use super::bindings::event_target::{ListenerMap, EVENT_LISTENERS};
use super::bindings::element::{
    get_or_create_js_element, DomPrototypes, NodeCache, DOM_PROTOTYPES, DOM_TREE, NODE_CACHE,
};

pub struct JsRuntime {
    pub(crate) context: Context,
    tree: Rc<RefCell<DomTree>>,
    console_buffer: Rc<RefCell<Vec<String>>>,
    pub(crate) listeners: Rc<RefCell<ListenerMap>>,
    pub(crate) node_cache: Rc<RefCell<NodeCache>>,
}

impl JsRuntime {
    /// Creates a new JS runtime wired to the given DomTree.
    /// Registers the `document` global, the `Element` class,
    /// the `window` global, and the `console` object.
    pub fn new(tree: Rc<RefCell<DomTree>>) -> Self {
        let mut context = Context::default();
        let console_buffer = Rc::new(RefCell::new(Vec::new()));
        let listeners: Rc<RefCell<ListenerMap>> = Rc::new(RefCell::new(HashMap::new()));
        let node_cache: Rc<RefCell<NodeCache>> = Rc::new(RefCell::new(HashMap::new()));

        // Store the listeners Rc in the thread-local so NativeFunction callbacks
        // (addEventListener, removeEventListener) can access it.
        EVENT_LISTENERS.with(|el| {
            *el.borrow_mut() = Some(Rc::clone(&listeners));
        });

        // Store the node cache Rc in the thread-local so NativeFunction callbacks
        // can return the same JsObject for the same NodeId (object identity).
        NODE_CACHE.with(|cell| {
            *cell.borrow_mut() = Some(Rc::clone(&node_cache));
        });

        // Store the DomTree in the thread-local so Text/Comment constructors can access it
        DOM_TREE.with(|cell| {
            *cell.borrow_mut() = Some(Rc::clone(&tree));
        });

        bindings::register_document(Rc::clone(&tree), &mut context);
        bindings::window::register_window(&mut context, Rc::clone(&console_buffer), Rc::clone(&tree));

        // Register Event and CustomEvent classes
        context.register_global_class::<bindings::event::JsEvent>().unwrap();
        context.register_global_class::<bindings::event::JsCustomEvent>().unwrap();
        bindings::event::register_event_constants(&mut context);

        // Register CSSStyleDeclaration class for getComputedStyle
        context.register_global_class::<bindings::computed_style::JsComputedStyle>().unwrap();

        // Register global Node, CharacterData, Text, Comment with proper prototype chain
        Self::register_dom_type_hierarchy(&mut context);

        Self { context, tree, console_buffer, listeners, node_cache }
    }

    /// Register the full DOM type hierarchy:
    ///   Node (interface object + prototype with constants)
    ///   CharacterData (prototype inherits from Node.prototype)
    ///   Text (constructor + prototype inherits from CharacterData.prototype)
    ///   Comment (constructor + prototype inherits from CharacterData.prototype)
    ///
    /// Also stores Text.prototype and Comment.prototype in the DOM_PROTOTYPES thread-local
    /// so that get_or_create_js_element can set the right prototype on created objects.
    fn register_dom_type_hierarchy(context: &mut Context) {
        // Get the Element class prototype — this is what all JsElement instances inherit from.
        // We'll make Node.prototype the parent of this prototype,
        // so Element instances get the Node constants via prototype chain.
        let element_constructor = context.global_object()
            .get(js_string!("Element"), context)
            .expect("Element should be registered");
        let element_proto = element_constructor
            .as_object()
            .expect("Element should be an object")
            .get(js_string!("prototype"), context)
            .expect("Element.prototype should exist");
        let element_proto_obj = element_proto.as_object().expect("Element.prototype should be an object").clone();

        // ---------------------------------------------------------------
        // Node.prototype — the base prototype with node type constants
        // ---------------------------------------------------------------
        let node_proto = ObjectInitializer::new(context).build();

        // Add all Node constants to Node.prototype
        let node_constants: &[(&str, i32)] = &[
            ("ELEMENT_NODE", 1), ("ATTRIBUTE_NODE", 2), ("TEXT_NODE", 3),
            ("CDATA_SECTION_NODE", 4), ("ENTITY_REFERENCE_NODE", 5), ("ENTITY_NODE", 6),
            ("PROCESSING_INSTRUCTION_NODE", 7), ("COMMENT_NODE", 8), ("DOCUMENT_NODE", 9),
            ("DOCUMENT_TYPE_NODE", 10), ("DOCUMENT_FRAGMENT_NODE", 11), ("NOTATION_NODE", 12),
        ];
        let doc_position_constants: &[(&str, i32)] = &[
            ("DOCUMENT_POSITION_DISCONNECTED", 0x01), ("DOCUMENT_POSITION_PRECEDING", 0x02),
            ("DOCUMENT_POSITION_FOLLOWING", 0x04), ("DOCUMENT_POSITION_CONTAINS", 0x08),
            ("DOCUMENT_POSITION_CONTAINED_BY", 0x10), ("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC", 0x20),
        ];

        for (name, value) in node_constants.iter().chain(doc_position_constants.iter()) {
            node_proto.define_property_or_throw(
                js_string!(*name),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(JsValue::from(*value))
                    .writable(false)
                    .configurable(false)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to define Node.prototype constant");
        }

        // Make Element.prototype inherit from Node.prototype
        element_proto_obj.set_prototype(Some(node_proto.clone()));

        // ---------------------------------------------------------------
        // Node interface object — must be a callable function so that
        // `obj instanceof Node` works (requires [[Call]] on the RHS).
        // Node is abstract; calling `new Node()` throws "Illegal constructor".
        // ---------------------------------------------------------------
        let node_ctor = unsafe {
            NativeFunction::from_closure(|_this, _args, _ctx| {
                Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
            })
        };
        let node_ctor_fn = FunctionObjectBuilder::new(context.realm(), node_ctor)
            .name(js_string!("Node"))
            .length(0)
            .constructor(true)
            .build();
        // Set Node.prototype
        node_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(node_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Node.prototype");
        // Add constants to Node constructor itself (e.g. Node.ELEMENT_NODE)
        for (name, value) in node_constants.iter().chain(doc_position_constants.iter()) {
            node_ctor_fn.define_property_or_throw(
                js_string!(*name),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(JsValue::from(*value))
                    .writable(false)
                    .configurable(false)
                    .enumerable(false)
                    .build(),
                context,
            ).expect("failed to define Node constant");
        }

        context
            .register_global_property(js_string!("Node"), node_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register Node global");

        // ---------------------------------------------------------------
        // CharacterData.prototype — inherits from Node.prototype
        // We copy all properties from Element.prototype onto it so that
        // CharacterData instances (Text, Comment) get access to .data,
        // .nodeType, .textContent, etc. without Element.prototype being
        // in the chain (which would break the WPT prototype chain checks).
        // ---------------------------------------------------------------
        let char_data_proto = ObjectInitializer::new(context).build();
        char_data_proto.set_prototype(Some(node_proto.clone()));

        // Store Element.prototype and CharacterData.prototype as JS globals temporarily,
        // then use JS to copy all property descriptors.
        context.register_global_property(
            js_string!("__braille_elem_proto"),
            element_proto_obj.clone(),
            Attribute::all(),
        ).expect("failed to register temp elem proto");
        context.register_global_property(
            js_string!("__braille_cd_proto"),
            char_data_proto.clone(),
            Attribute::all(),
        ).expect("failed to register temp cd proto");

        // Use JS to copy all property descriptors from Element.prototype to CharacterData.prototype
        context.eval(Source::from_bytes(
            r#"
            (function() {
                var src = __braille_elem_proto;
                var dst = __braille_cd_proto;
                var names = Object.getOwnPropertyNames(src);
                for (var i = 0; i < names.length; i++) {
                    var name = names[i];
                    if (name === 'constructor') continue;
                    var desc = Object.getOwnPropertyDescriptor(src, name);
                    if (desc) {
                        Object.defineProperty(dst, name, desc);
                    }
                }
                delete self.__braille_elem_proto;
                delete self.__braille_cd_proto;
            })();
            "#,
        )).expect("failed to copy Element.prototype properties to CharacterData.prototype");

        // CharacterData is abstract; calling `new CharacterData()` throws.
        // Must be a callable function for `obj instanceof CharacterData` to work.
        let char_data_ctor = unsafe {
            NativeFunction::from_closure(|_this, _args, _ctx| {
                Err(JsError::from_opaque(JsValue::from(js_string!("Illegal constructor"))))
            })
        };
        let char_data_ctor_fn = FunctionObjectBuilder::new(context.realm(), char_data_ctor)
            .name(js_string!("CharacterData"))
            .length(0)
            .constructor(true)
            .build();
        char_data_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(char_data_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define CharacterData.prototype");

        context
            .register_global_property(js_string!("CharacterData"), char_data_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register CharacterData global");

        // ---------------------------------------------------------------
        // Text.prototype — inherits from CharacterData.prototype
        // ---------------------------------------------------------------
        let text_proto = ObjectInitializer::new(context).build();
        text_proto.set_prototype(Some(char_data_proto.clone()));

        // Text constructor: new Text(data?) creates a Text node
        let text_proto_for_closure = text_proto.clone();
        let text_ctor = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let data = if args.is_empty() || args[0].is_undefined() {
                    String::new()
                } else {
                    args[0].to_string(ctx)?.to_std_string_escaped()
                };

                let tree = DOM_TREE.with(|cell| {
                    let rc = cell.borrow();
                    rc.as_ref().expect("DOM_TREE not initialized").clone()
                });

                let node_id = tree.borrow_mut().create_text(&data);
                let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
                // Ensure prototype is Text.prototype (get_or_create_js_element may already do this)
                js_obj.set_prototype(Some(text_proto_for_closure.clone()));
                Ok(JsValue::from(js_obj))
            })
        };

        // Build the Text constructor function object (constructor: true enables `new Text()`)
        let text_ctor_fn = FunctionObjectBuilder::new(context.realm(), text_ctor)
            .name(js_string!("Text"))
            .length(0)
            .constructor(true)
            .build();
        text_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(text_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Text.prototype");

        // Set Text.prototype.constructor = Text
        text_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(text_ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Text.prototype.constructor");

        context
            .register_global_property(js_string!("Text"), text_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register Text global");

        // ---------------------------------------------------------------
        // Comment.prototype — inherits from CharacterData.prototype
        // ---------------------------------------------------------------
        let comment_proto = ObjectInitializer::new(context).build();
        comment_proto.set_prototype(Some(char_data_proto.clone()));

        // Comment constructor: new Comment(data?) creates a Comment node
        let comment_proto_for_closure = comment_proto.clone();
        let comment_ctor = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let data = if args.is_empty() || args[0].is_undefined() {
                    String::new()
                } else {
                    args[0].to_string(ctx)?.to_std_string_escaped()
                };

                let tree = DOM_TREE.with(|cell| {
                    let rc = cell.borrow();
                    rc.as_ref().expect("DOM_TREE not initialized").clone()
                });

                let node_id = tree.borrow_mut().create_comment(&data);
                let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
                js_obj.set_prototype(Some(comment_proto_for_closure.clone()));
                Ok(JsValue::from(js_obj))
            })
        };

        let comment_ctor_fn = FunctionObjectBuilder::new(context.realm(), comment_ctor)
            .name(js_string!("Comment"))
            .length(0)
            .constructor(true)
            .build();
        comment_ctor_fn.define_property_or_throw(
            js_string!("prototype"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(comment_proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Comment.prototype");

        comment_proto.define_property_or_throw(
            js_string!("constructor"),
            boa_engine::property::PropertyDescriptor::builder()
                .value(comment_ctor_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        ).expect("failed to define Comment.prototype.constructor");

        context
            .register_global_property(js_string!("Comment"), comment_ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
            .expect("failed to register Comment global");

        // ---------------------------------------------------------------
        // Store prototypes in thread-local for get_or_create_js_element
        // ---------------------------------------------------------------
        DOM_PROTOTYPES.with(|cell| {
            *cell.borrow_mut() = Some(DomPrototypes {
                text_proto,
                comment_proto,
            });
        });

        // ---------------------------------------------------------------
        // Copy Node/CharacterData/Text/Comment globals onto window object
        // so that `window.Text`, `window.Node`, etc. work (used by WPT tests)
        // ---------------------------------------------------------------
        let global = context.global_object();
        let window_val = global.get(js_string!("window"), context)
            .expect("window global should exist");
        if let Some(window_obj) = window_val.as_object() {
            for name in &["Node", "CharacterData", "Text", "Comment"] {
                let val = global.get(js_string!(*name), context)
                    .expect("global should have this property");
                window_obj.define_property_or_throw(
                    js_string!(*name),
                    boa_engine::property::PropertyDescriptor::builder()
                        .value(val)
                        .writable(true)
                        .configurable(true)
                        .enumerable(false)
                        .build(),
                    context,
                ).expect("failed to set window property");
            }
        }
    }

    /// Evaluates a JS source string and returns the result.
    pub fn eval(&mut self, code: &str) -> JsResult<JsValue> {
        self.context.eval(Source::from_bytes(code))
    }

    /// Returns a reference to the shared DomTree.
    pub fn tree(&self) -> &Rc<RefCell<DomTree>> {
        &self.tree
    }

    /// Returns a clone of the console output buffer.
    pub fn console_output(&self) -> Vec<String> {
        self.console_buffer.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{NodeData, NodeId};

    /// Helper: build a DomTree with document > html > body > div#app
    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");

            // Set id="app" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(("id".to_string(), "app".to_string()));
            }

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn create_element_adds_node_to_tree() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(r#"document.createElement("p")"#).unwrap();

        // The tree should now have an extra "p" node (unattached)
        let t = tree.borrow();
        // Nodes: [0]=Document, [1]=html, [2]=body, [3]=div#app, [4]=p
        let p_node = t.get_node(4);
        match &p_node.data {
            NodeData::Element { tag_name, .. } => assert_eq!(tag_name, "p"),
            other => panic!("expected Element, got {:?}", other),
        }
        // Unattached — no parent
        assert!(p_node.parent.is_none());
    }

    #[test]
    fn get_element_by_id_returns_element() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("app")"#).unwrap();

        // Should not be null or undefined
        assert!(!result.is_null());
        assert!(!result.is_undefined());
        // Should be an object
        assert!(result.is_object());
    }

    #[test]
    fn get_element_by_id_returns_null_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("nonexistent")"#).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn text_content_getter_and_setter() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Set textContent on the div#app
        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.textContent = "hello";
        "#,
        )
        .unwrap();

        // Verify via DomTree
        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        assert_eq!(t.get_text_content(div_id), "hello");

        drop(t); // release borrow before eval

        // Read back through JS
        let result = rt
            .eval(
                r#"
            var el2 = document.getElementById("app");
            el2.textContent
        "#,
            )
            .unwrap();

        let text = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(text, "hello");
    }

    #[test]
    fn append_child_wires_parent_and_child() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var p = document.createElement("p");
            p.textContent = "new paragraph";
            var app = document.getElementById("app");
            app.appendChild(p);
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app

        // div#app's children should include the new <p>
        let div_children = &t.get_node(div_id).children;
        // The <p> was created as node 4, then set_text_content created a text node as 5
        // and appended it as child of 4. Then we appended 4 to div_id(3).
        assert!(div_children.contains(&4));

        // Verify the text content through the tree
        assert_eq!(t.get_text_content(4), "new paragraph");
        // The <p> node's parent should be div#app
        assert_eq!(t.get_node(4).parent, Some(div_id));
    }

    #[test]
    fn full_spike_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // This mirrors the spike's JS test script:
        // 1. Create a <p> element
        // 2. Set its textContent
        // 3. Find div#app by id
        // 4. Append the <p> to div#app
        rt.eval(
            r#"
            var p = document.createElement("p");
            p.textContent = "Hello from JS!";
            var app = document.getElementById("app");
            app.appendChild(p);
        "#,
        )
        .unwrap();

        let t = tree.borrow();

        // div#app (node 3) should have the <p> as a child
        let div_children = &t.get_node(3).children;
        let p_id: NodeId = 4;
        assert!(div_children.contains(&p_id), "div#app should contain the <p>");

        // The <p> should contain the text "Hello from JS!"
        assert_eq!(t.get_text_content(p_id), "Hello from JS!");

        // Verify the tag name of the new element
        match &t.get_node(p_id).data {
            NodeData::Element { tag_name, .. } => assert_eq!(tag_name, "p"),
            other => panic!("expected Element('p'), got {:?}", other),
        }

        // Verify the full text content of div#app includes the paragraph
        assert_eq!(t.get_text_content(3), "Hello from JS!");
    }

    #[test]
    fn document_body_returns_body_element() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Access document.body
        rt.eval(
            r#"
            var body = document.body;
            body.textContent = "body content";
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let body_id: NodeId = 2; // body is node 2 in make_test_tree
        assert_eq!(t.get_text_content(body_id), "body content");
    }

    #[test]
    fn document_head_returns_head_element() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let body = t.create_element("body");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(html, body);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Access document.head
        let result = rt.eval(r#"document.head"#).unwrap();
        assert!(!result.is_null());

        // Verify we can manipulate it
        rt.eval(
            r#"
            var head = document.head;
            head.textContent = "head content";
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let head_id: NodeId = 1; // head is node 1
        assert_eq!(t.get_text_content(head_id), "head content");
    }

    #[test]
    fn document_head_returns_null_when_absent() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.head"#).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn document_create_text_node_creates_text() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var textNode = document.createTextNode("hello world");
            var app = document.getElementById("app");
            app.appendChild(textNode);
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let text = t.get_text_content(div_id);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn document_title_getter_returns_empty_when_no_title() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(title, "");
    }

    #[test]
    fn document_title_getter_reads_title_element() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let title = t.create_element("title");

            t.set_text_content(title, "My Page Title");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(head, title);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(title, "My Page Title");
    }

    #[test]
    fn document_title_setter_creates_or_updates_title() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Set title (should create <title> element)
        rt.eval(r#"document.title = "New Title""#).unwrap();

        // Read it back
        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(title, "New Title");

        // Verify through DomTree
        let t = tree.borrow();
        let titles = t.get_elements_by_tag_name("title");
        assert_eq!(titles.len(), 1);
        assert_eq!(t.get_text_content(titles[0]), "New Title");
    }

    #[test]
    fn document_title_setter_updates_existing_title() {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let title = t.create_element("title");

            t.set_text_content(title, "Old Title");

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(head, title);
        }

        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Update title
        rt.eval(r#"document.title = "Updated Title""#).unwrap();

        // Read it back
        let result = rt.eval(r#"document.title"#).unwrap();
        let title = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(title, "Updated Title");

        // Verify only one title element exists
        let t = tree.borrow();
        let titles = t.get_elements_by_tag_name("title");
        assert_eq!(titles.len(), 1);
    }

    #[test]
    fn class_list_add() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo");
            el.classList.add("bar");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        assert_eq!(class_attr, Some("foo bar".to_string()));
    }

    #[test]
    fn class_list_add_multiple() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        assert_eq!(class_attr, Some("foo bar baz".to_string()));
    }

    #[test]
    fn class_list_remove() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
            el.classList.remove("bar");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        assert_eq!(class_attr, Some("foo baz".to_string()));
    }

    #[test]
    fn class_list_remove_all() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar");
            el.classList.remove("foo", "bar");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3; // div#app
        let class_attr = t.get_attribute(div_id, "class");
        // When all classes are removed, the attribute should be removed
        assert_eq!(class_attr, None);
    }

    #[test]
    fn class_list_toggle() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        // Toggle adds the class when not present, returns true
        let result1 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.toggle("foo");
        "#,
        )
        .unwrap();
        assert_eq!(result1.as_boolean(), Some(true));

        let t = tree.borrow();
        let div_id: NodeId = 3;
        assert_eq!(t.get_attribute(div_id, "class"), Some("foo".to_string()));
        drop(t);

        // Toggle removes the class when present, returns false
        let result2 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.toggle("foo");
        "#,
        )
        .unwrap();
        assert_eq!(result2.as_boolean(), Some(false));

        let t = tree.borrow();
        assert_eq!(t.get_attribute(div_id, "class"), None);
    }

    #[test]
    fn class_list_contains() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar");
        "#,
        )
        .unwrap();

        let result1 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.contains("foo");
        "#,
        )
        .unwrap();
        assert_eq!(result1.as_boolean(), Some(true));

        let result2 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.contains("baz");
        "#,
        )
        .unwrap();
        assert_eq!(result2.as_boolean(), Some(false));
    }

    #[test]
    fn class_list_item() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
        "#,
        )
        .unwrap();

        let result0 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.item(0);
        "#,
        )
        .unwrap();
        let text0 = result0
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(text0, "foo");

        let result1 = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.item(1);
        "#,
        )
        .unwrap();
        let text1 = result1
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(text1, "bar");

        let result_out_of_bounds = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.item(99);
        "#,
        )
        .unwrap();
        assert!(result_out_of_bounds.is_null());
    }

    #[test]
    fn class_list_length() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result_empty = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.length;
        "#,
        )
        .unwrap();
        assert_eq!(result_empty.as_number(), Some(0.0));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo", "bar", "baz");
        "#,
        )
        .unwrap();

        let result_three = rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.length;
        "#,
        )
        .unwrap();
        assert_eq!(result_three.as_number(), Some(3.0));
    }

    #[test]
    fn class_list_no_duplicate_add() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");
            el.classList.add("foo");
            el.classList.add("foo");
            el.classList.add("foo");
        "#,
        )
        .unwrap();

        let t = tree.borrow();
        let div_id: NodeId = 3;
        let class_attr = t.get_attribute(div_id, "class");
        // Should only have "foo" once
        assert_eq!(class_attr, Some("foo".to_string()));
    }

    #[test]
    fn class_list_workflow_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        rt.eval(
            r#"
            var el = document.getElementById("app");

            // Start empty
            if (el.classList.length !== 0) throw new Error("Expected length 0");

            // Add some classes
            el.classList.add("foo", "bar");
            if (el.classList.length !== 2) throw new Error("Expected length 2");
            if (!el.classList.contains("foo")) throw new Error("Expected foo");
            if (!el.classList.contains("bar")) throw new Error("Expected bar");

            // Toggle off foo
            var removed = el.classList.toggle("foo");
            if (removed !== false) throw new Error("Expected toggle to return false");
            if (el.classList.contains("foo")) throw new Error("foo should be removed");
            if (el.classList.length !== 1) throw new Error("Expected length 1");

            // Toggle on baz
            var added = el.classList.toggle("baz");
            if (added !== true) throw new Error("Expected toggle to return true");
            if (!el.classList.contains("baz")) throw new Error("Expected baz");
            if (el.classList.length !== 2) throw new Error("Expected length 2");

            // Check items
            if (el.classList.item(0) !== "bar") throw new Error("Expected bar at index 0");
            if (el.classList.item(1) !== "baz") throw new Error("Expected baz at index 1");

            // Remove all
            el.classList.remove("bar", "baz");
            if (el.classList.length !== 0) throw new Error("Expected length 0");
        "#,
        )
        .unwrap();

        // All assertions passed in JS; verify final state in Rust
        let t = tree.borrow();
        let div_id: NodeId = 3;
        assert_eq!(t.get_attribute(div_id, "class"), None);
    }

    #[test]
    fn text_constructor_debug() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval("typeof Text").unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "function", "Text should be a function");

        let result2 = rt.eval("var t = new Text('hello'); t.data").unwrap();
        let s2 = result2.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s2, "hello", "Text data should be 'hello'");

        // Check window.Text === Text
        let result3 = rt.eval("typeof window.Text").unwrap();
        let s3 = result3.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s3, "function", "window.Text should be a function");

        // Check window[ctor] pattern used by WPT
        let result4 = rt.eval("var ctor = 'Text'; new window[ctor]('test').data").unwrap();
        let s4 = result4.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s4, "test", "window['Text'] constructor should work");
    }
}
