# API Reference

Detailed API documentation for the key dependencies used in Braille. This is a reference for implementing agents — not a design document.

## Boa JavaScript Engine (v0.21)

### Cargo Dependencies

```toml
[dependencies]
boa_engine = "0.21.0"
boa_gc = "0.21.0"          # Trace, Finalize derives
boa_runtime = "0.21.0"     # Optional: console.log, setTimeout, etc.
```

`boa_engine` re-exports `JsData` derive macro and `js_string!` macro. `Trace` and `Finalize` are re-exported from `boa_gc`.

### Creating a Context

```rust
use boa_engine::{Context, Source};

let mut context = Context::default();
// Gives you a fully initialized JS environment with all standard built-ins
```

### Evaluating JavaScript

```rust
use boa_engine::{Context, Source, JsValue};

let mut context = Context::default();

let result = context.eval(Source::from_bytes(r#"
    let x = 2 + 2;
    x * 10
"#)).unwrap();

let as_number = result.to_number(&mut context).unwrap(); // 40.0
let as_string = result.to_string(&mut context).unwrap().to_std_string_escaped(); // "40"
```

### Exposing Rust Objects to JS: ObjectInitializer (ad-hoc singletons)

Use this for objects like `document` that don't need a JS constructor.

```rust
use boa_engine::{
    Context, JsValue, JsObject,
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};

fn build_document(context: &mut Context) -> JsObject {
    ObjectInitializer::new(context)
        .property(
            js_string!("title"),
            js_string!("My Page"),
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(create_element),
            js_string!("createElement"),
            1,  // expected argument count
        )
        .build()
}

fn setup(context: &mut Context) {
    let doc = build_document(context);
    context.register_global_property(
        js_string!("document"),
        doc,
        Attribute::all(),
    );
}
```

### ObjectInitializer with Native Data (ad-hoc object backed by Rust state)

```rust
use boa_gc::{Trace, Finalize};

#[derive(Debug, Trace, Finalize, JsData)]
struct Document {
    title: String,
}

fn build_document(context: &mut Context) -> JsObject {
    let doc = Document { title: "My Page".to_string() };

    ObjectInitializer::with_native_data(doc, context)
        .function(
            NativeFunction::from_fn_ptr(create_element),
            js_string!("createElement"),
            1,
        )
        .build()
}
```

### Exposing Rust Objects to JS: Class Trait (for types JS can `new`)

Use this for types like `Element` that JS code should be able to construct.

```rust
use boa_engine::{
    class::{Class, ClassBuilder},
    Context, JsValue, JsResult, JsObject,
    js_string,
    native_function::NativeFunction,
    property::Attribute,
};
use boa_gc::{Trace, Finalize};

#[derive(Debug, Trace, Finalize, JsData)]
struct Element {
    tag_name: String,
    text_content: String,
}

impl Class for Element {
    const NAME: &'static str = "Element";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<Self> {
        let tag = args.get(0)
            .map(|v| v.to_string(context))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        Ok(Element {
            tag_name: tag,
            text_content: String::new(),
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        // Methods and accessors defined here (see below)
        Ok(())
    }
}

// Register so JS can use `new Element(...)` and `Element::from_data` works:
context.register_global_class::<Element>().unwrap();
```

### Defining Methods on a Class

Inside `Class::init`:

```rust
fn init(class: &mut ClassBuilder) -> JsResult<()> {
    class.method(
        js_string!("getAttribute"),
        1,
        NativeFunction::from_fn_ptr(Self::get_attribute),
    );
    Ok(())
}

// The method implementation:
fn get_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let obj = this.as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("expected object").into()))?;
    let element = obj.downcast_ref::<Element>()
        .ok_or_else(|| JsError::from_opaque(js_string!("`this` is not an Element").into()))?;

    let attr_name = args.get(0)
        .map(|v| v.to_string(context))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Your logic here...
    Ok(JsValue::undefined())
}
```

### Defining Property Getters/Setters on a Class

Inside `Class::init`:

```rust
fn init(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    let getter = NativeFunction::from_fn_ptr(|this, _, _| {
        let obj = this.as_object().unwrap();
        let el = obj.downcast_ref::<Element>().unwrap();
        Ok(JsValue::from(js_string!(el.text_content.as_str())))
    });

    let setter = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().unwrap();
        let mut el = obj.downcast_mut::<Element>().unwrap();
        el.text_content = args.get(0)
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        Ok(JsValue::undefined())
    });

    class.accessor(
        js_string!("textContent"),
        Some(getter.to_js_function(&realm)),
        Some(setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    Ok(())
}
```

### Creating Class Instances from Rust (for createElement etc.)

```rust
// Inside a native function that needs to return an Element to JS:
fn create_element_fn(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let tag = args.get(0)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "div".to_string());

    let element = Element {
        tag_name: tag,
        text_content: String::new(),
    };
    let js_obj = Element::from_data(element, ctx)?;
    Ok(js_obj.into())
}
```

### Native Function Patterns

```rust
// Function pointer (most efficient, no allocation):
let native = NativeFunction::from_fn_ptr(my_rust_fn);
// Signature: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>

// Copy closure (captures only Copy types):
let multiplier: i32 = 10;
let native = NativeFunction::from_copy_closure(move |_this, args, ctx| {
    let val = args.get(0)
        .map(|v| v.to_number(ctx))
        .transpose()?
        .unwrap_or(0.0);
    Ok(JsValue::from(val * multiplier as f64))
});

// Copy closure with GC-traced captures:
let native = NativeFunction::from_copy_closure_with_captures(
    |_this, args, captures, ctx| {
        captures.counter += 1;
        Ok(JsValue::from(captures.counter))
    },
    CapturedState { counter: 0 },
);

// Register as global function:
context.register_global_property(
    js_string!("myCallback"),
    NativeFunction::from_fn_ptr(my_rust_fn)
        .to_js_function(context.realm()),
    Attribute::all(),
);
```

### Key Boa Types

| Type | Purpose |
|------|---------|
| `Context` | JS execution environment, holds realm + global object |
| `Source` | Wraps JS source code (`Source::from_bytes(...)`) |
| `JsValue` | Any JS value (NaN-boxed in v0.21) |
| `JsObject` | GC'd JS object |
| `JsString` | Immutable JS string (`js_string!("...")`) |
| `NativeFunction` | Wraps a Rust fn for JS to call |
| `Class` trait | Full JS class with constructor + prototype |
| `JsData` derive | Marks a Rust struct as embeddable native data |
| `Trace + Finalize` | GC integration (derive from `boa_gc`) |
| `ClassBuilder` | Builder for methods/properties/accessors inside `Class::init` |
| `ObjectInitializer` | Builder for ad-hoc JS objects |
| `Attribute` | Property attributes (writable, enumerable, configurable) |

---

## html5ever (v0.38)

### Cargo Dependencies

```toml
[dependencies]
html5ever = "0.38"
markup5ever = "0.38"     # for Attribute, QualName, ExpandedName, etc.
tendril = "0.5"          # for StrTendril (string type used throughout)
```

### Parsing HTML (with reference RcDom)

```rust
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::RcDom;

let dom = parse_document(RcDom::default(), Default::default())
    .one(html_string);
```

### Parser Entry Points

```rust
// Parse a full document:
pub fn parse_document<Sink: TreeSink>(sink: Sink, opts: ParseOpts) -> Parser<Sink>

// Parse an HTML fragment:
pub fn parse_fragment<Sink: TreeSink>(
    sink: Sink,
    opts: ParseOpts,
    context_name: QualName,
    context_attrs: Vec<Attribute>,
    context_element_allows_scripting: bool,
) -> Parser<Sink>
```

`Parser<Sink>` implements `TendrilSink<UTF8>`:
- `.one(string)` — parse a single string, return `Sink::Output`
- `.process(tendril)` — feed incremental input
- `.finish()` — finalize and return `Sink::Output`

### The TreeSink Trait (complete)

html5ever does NOT provide its own DOM tree. It calls methods on your `TreeSink` implementation. You build whatever data structure you want.

```rust
pub trait TreeSink {
    type Handle: Clone;
    type Output;
    type ElemName<'a>: ElemName where Self: 'a;

    // Required:
    fn finish(self) -> Self::Output;
    fn parse_error(&self, msg: Cow<'static, str>);
    fn get_document(&self) -> Self::Handle;
    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> Self::ElemName<'a>;
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Self::Handle;
    fn create_comment(&self, text: StrTendril) -> Self::Handle;
    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle;
    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>);
    fn append_before_sibling(&self, sibling: &Self::Handle, child: NodeOrText<Self::Handle>);
    fn append_based_on_parent_node(&self, element: &Self::Handle, prev_element: &Self::Handle, child: NodeOrText<Self::Handle>);
    fn append_doctype_to_document(&self, name: StrTendril, public_id: StrTendril, system_id: StrTendril);
    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle;
    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool;
    fn set_quirks_mode(&self, mode: QuirksMode);
    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>);
    fn remove_from_parent(&self, target: &Self::Handle);
    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle);

    // Optional (provided defaults):
    fn mark_script_already_started(&self, _node: &Self::Handle) { }
    fn pop(&self, _node: &Self::Handle) { }
    fn associate_with_form(&self, _target: &Self::Handle, _form: &Self::Handle, _nodes: (&Self::Handle, Option<&Self::Handle>)) { }
    fn is_mathml_annotation_xml_integration_point(&self, _handle: &Self::Handle) -> bool { false }
    fn set_current_line(&self, _line_number: u64) { }
}
```

### Key Supporting Types

```rust
// What gets appended to the tree:
pub enum NodeOrText<Handle> {
    AppendNode(Handle),
    AppendText(StrTendril),
}

// Flags passed when creating elements:
pub struct ElementFlags {
    pub template: bool,
    pub mathml_annotation_xml_integration_point: bool,
}

// Qualified name for elements/attributes:
pub struct QualName {
    pub prefix: Option<Prefix>,
    pub ns: Namespace,
    pub local: LocalName,
}

// An attribute:
pub struct Attribute {
    pub name: QualName,
    pub value: StrTendril,
}

// Parser options:
pub struct TreeBuilderOpts {
    pub exact_errors: bool,       // default: false
    pub scripting_enabled: bool,  // default: true
    pub iframe_srcdoc: bool,      // default: false
    pub drop_doctype: bool,       // default: false
    pub quirks_mode: QuirksMode,  // default: NoQuirks
}
```

### Implementing a Custom TreeSink

Critical design decisions:

**Handle type:** Must be `Clone`. Common choices:
- `Rc<Node>` — what RcDom uses. Simplest. `elem_name` borrows from the handle itself.
- `NodeId(usize)` — index into a Vec. More cache-friendly. But `elem_name` has a lifetime issue (see below).

**The `elem_name` lifetime problem with index-based handles:**

`elem_name` returns `ElemName<'a>` tied to `&'a self`. With index-based handles and `RefCell<Vec<Node>>`, you can't return a reference into the vec because the `Ref` guard doesn't live long enough. Solutions:
1. Use `Rc<Node>` handles (like RcDom) — simplest
2. Store `QualName` data directly accessible without RefCell (e.g., separate Vec<QualName> indexed by NodeId)
3. Use an arena allocator so nodes have stable `&'a` references
4. Use unsafe code

**Recommended approach for Braille:** Store a parallel `Vec<QualName>` (or include QualName in a non-RefCell-wrapped part of the node) so `elem_name` can return a reference without going through RefCell.

### How `<script>` Tags Work in html5ever

- `<script>` elements are created as regular elements via `create_element()`
- Content between `<script>` and `</script>` is parsed as raw text (not HTML)
- The content appears as a text child node of the script element
- html5ever does NOT execute scripts — it just parses them into the tree
- `scripting_enabled` option only affects `<noscript>` parsing, not `<script>`

### Walking the Parsed Tree

No built-in query/selector API. Walk the tree manually:

```rust
fn find_elements_by_tag(handle: &Handle, tag: &str, results: &mut Vec<Handle>) {
    if let NodeData::Element { ref name, .. } = handle.data {
        if &*name.local == tag {
            results.push(handle.clone());
        }
    }
    for child in handle.children.borrow().iter() {
        find_elements_by_tag(child, tag, results);
    }
}
```

### Extracting Text Content

```rust
fn get_text_content(handle: &Handle) -> String {
    let mut text = String::new();
    collect_text(handle, &mut text);
    text
}

fn collect_text(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => out.push_str(&contents.borrow()),
        _ => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
    }
}
```

---

## Sources

- [boa_engine docs.rs](https://docs.rs/boa_engine/latest/boa_engine/)
- [Boa v0.21 release](https://boajs.dev/blog/2025/10/22/boa-release-21)
- [Boa GitHub](https://github.com/boa-dev/boa)
- [html5ever on crates.io](https://crates.io/crates/html5ever)
- [html5ever docs.rs](https://docs.rs/html5ever/latest/html5ever/)
- [servo/html5ever GitHub](https://github.com/servo/html5ever)
- [TreeSink trait docs](https://docs.rs/html5ever/latest/html5ever/interface/trait.TreeSink.html)
