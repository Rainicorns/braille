use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    property::Attribute,
    Context, JsData, JsError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};

// ---------------------------------------------------------------------------
// Token validation helper
// ---------------------------------------------------------------------------

/// Validate a DOMTokenList token per the spec:
/// - Empty string -> SyntaxError
/// - Contains ASCII whitespace -> InvalidCharacterError
fn validate_token(token: &str) -> JsResult<()> {
    if token.is_empty() {
        return Err(JsError::from_opaque(
            js_string!("SyntaxError: The token must not be empty").into(),
        ));
    }
    if token.contains([' ', '\t', '\n', '\r', '\x0C']) {
        return Err(JsError::from_opaque(
            js_string!("InvalidCharacterError: The token must not contain whitespace").into(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// JsClassList — the Class-based wrapper for element.classList
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsClassList {
    #[unsafe_ignore_trace]
    node_id: NodeId,
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
}

impl JsClassList {
    pub fn new(node_id: NodeId, tree: Rc<RefCell<DomTree>>) -> Self {
        Self { node_id, tree }
    }

    /// Returns true if the element has a "class" attribute (even if empty).
    fn has_class_attribute(&self) -> bool {
        self.tree.borrow().get_attribute(self.node_id, "class").is_some()
    }

    /// Returns the raw class attribute string, or empty string if no attribute.
    fn get_raw_class_value(&self) -> String {
        self.tree
            .borrow()
            .get_attribute(self.node_id, "class")
            .unwrap_or_default()
    }

    /// Helper to parse class attribute into a deduplicated vector of class names.
    fn get_classes(&self) -> Vec<String> {
        self.tree
            .borrow()
            .get_attribute(self.node_id, "class")
            .map(|class_str| {
                let mut seen = Vec::new();
                for token in class_str.split_whitespace() {
                    let s = token.to_string();
                    if !seen.contains(&s) {
                        seen.push(s);
                    }
                }
                seen
            })
            .unwrap_or_default()
    }

    /// Helper to write class names back to the class attribute.
    /// Per spec, sets to empty string when all classes are removed (does not remove the attribute).
    fn set_classes(&self, classes: Vec<String>, ctx: &mut Context) {
        let class_str = classes.join(" ");
        super::mutation_observer::set_attribute_with_observer(ctx, &self.tree, self.node_id, "class", &class_str);
    }

    /// Native implementation of classList.add(...classNames)
    fn add(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.add: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.add: `this` is not a ClassList").into()))?;

        // Validate all tokens first (before any mutation)
        let mut tokens = Vec::new();
        for arg in args {
            let token = arg.to_string(ctx)?.to_std_string_escaped();
            validate_token(&token)?;
            tokens.push(token);
        }

        let mut classes = class_list.get_classes();

        // Add each token if not already present
        for token in &tokens {
            if !classes.contains(token) {
                classes.push(token.clone());
            }
        }

        class_list.set_classes(classes, ctx);
        Ok(JsValue::undefined())
    }

    /// Native implementation of classList.remove(...classNames)
    fn remove(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.remove: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.remove: `this` is not a ClassList").into()))?;

        // Validate all tokens first (before any mutation)
        let mut tokens = Vec::new();
        for arg in args {
            let token = arg.to_string(ctx)?.to_std_string_escaped();
            validate_token(&token)?;
            tokens.push(token);
        }

        // If attribute doesn't exist, don't create it
        if !class_list.has_class_attribute() {
            return Ok(JsValue::undefined());
        }

        let mut classes = class_list.get_classes();

        // Remove each token
        for token in &tokens {
            classes.retain(|c| c != token);
        }

        class_list.set_classes(classes, ctx);
        Ok(JsValue::undefined())
    }

    /// Native implementation of classList.toggle(className [, force])
    fn toggle(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.toggle: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.toggle: `this` is not a ClassList").into()))?;

        let class_name = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.toggle: missing argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        validate_token(&class_name)?;

        // Check for force parameter (second argument)
        let force = args
            .get(1)
            .and_then(|v| if v.is_undefined() { None } else { Some(v.to_boolean()) });

        let mut classes = class_list.get_classes();
        let has_token = classes.contains(&class_name);

        match force {
            Some(true) => {
                // Force add: if already present, it's a noop (don't update attribute for noop)
                if !has_token {
                    classes.push(class_name);
                    class_list.set_classes(classes, ctx);
                } else if class_list.has_class_attribute() {
                    // Even if noop, dedup if attribute exists (for force toggle noop case,
                    // the spec says don't run update steps, so we skip set_classes)
                    // Actually: per the test, force toggle noop returns `before` unchanged.
                    // So do NOT set_classes here.
                }
                Ok(JsValue::from(true))
            }
            Some(false) => {
                // Force remove: if not present, it's a noop
                if has_token {
                    classes.retain(|c| c != &class_name);
                    class_list.set_classes(classes, ctx);
                } else if class_list.has_class_attribute() {
                    // Noop - don't run update steps
                }
                Ok(JsValue::from(false))
            }
            None => {
                // No force: toggle
                let result = if has_token {
                    classes.retain(|c| c != &class_name);
                    false
                } else {
                    classes.push(class_name);
                    true
                };
                class_list.set_classes(classes, ctx);
                Ok(JsValue::from(result))
            }
        }
    }

    /// Native implementation of classList.replace(oldToken, newToken)
    fn replace(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.replace: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.replace: `this` is not a ClassList").into()))?;

        let old_token = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.replace: missing old token argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        let new_token = args
            .get(1)
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.replace: missing new token argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        // Validate both tokens: empty first (SyntaxError), then whitespace (InvalidCharacterError)
        // Per spec, validate old_token first, then new_token
        validate_token(&old_token)?;
        validate_token(&new_token)?;

        // If attribute doesn't exist, return false (no modification)
        if !class_list.has_class_attribute() {
            return Ok(JsValue::from(false));
        }

        let mut classes = class_list.get_classes();

        // Find the first occurrence of old_token
        let pos = match classes.iter().position(|c| c == &old_token) {
            Some(p) => p,
            None => return Ok(JsValue::from(false)), // Not found, no modification
        };

        // Replace that element with new_token
        classes[pos] = new_token.clone();

        // Deduplicate: remove later occurrences of new_token (keep first)
        let mut seen = Vec::new();
        let mut deduped = Vec::new();
        for class in classes {
            if !seen.contains(&class) {
                seen.push(class.clone());
                deduped.push(class);
            }
        }

        class_list.set_classes(deduped, ctx);
        Ok(JsValue::from(true))
    }

    /// Native implementation of classList.contains(className)
    fn contains(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.contains: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.contains: `this` is not a ClassList").into()))?;

        let class_name = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.contains: missing argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        let classes = class_list.get_classes();
        let has_class = classes.contains(&class_name);
        Ok(JsValue::from(has_class))
    }

    /// Native implementation of classList.item(index)
    fn item(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.item: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.item: `this` is not a ClassList").into()))?;

        let index = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.item: missing argument").into()))?
            .to_i32(ctx)? as usize;

        let classes = class_list.get_classes();
        match classes.get(index) {
            Some(class_name) => Ok(JsValue::from(js_string!(class_name.clone()))),
            None => Ok(JsValue::null()),
        }
    }

    /// Native getter for classList.length
    fn get_length(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.length: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.length: `this` is not a ClassList").into()))?;

        let classes = class_list.get_classes();
        Ok(JsValue::from(classes.len()))
    }

    /// Native getter for classList.value — returns the raw class attribute string
    fn get_value(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.value: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.value: `this` is not a ClassList").into()))?;

        let raw = class_list.get_raw_class_value();
        Ok(JsValue::from(js_string!(raw)))
    }

    /// Native setter for classList.value — sets the raw class attribute
    fn set_value(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.value: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.value: `this` is not a ClassList").into()))?;

        let value = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();

        super::mutation_observer::set_attribute_with_observer(
            ctx,
            &class_list.tree,
            class_list.node_id,
            "class",
            &value,
        );
        Ok(JsValue::undefined())
    }

    /// Native implementation of classList.toString() — returns the raw class attribute string
    fn to_string_method(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.toString: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.toString: `this` is not a ClassList").into()))?;

        let raw = class_list.get_raw_class_value();
        Ok(JsValue::from(js_string!(raw)))
    }
}

impl Class for JsClassList {
    const NAME: &'static str = "ClassList";
    const LENGTH: usize = 0;

    fn data_constructor(_new_target: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<Self> {
        Err(JsError::from_opaque(
            js_string!("ClassList cannot be constructed directly from JS").into(),
        ))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        // Methods
        class.method(js_string!("add"), 0, NativeFunction::from_fn_ptr(Self::add));

        class.method(js_string!("remove"), 0, NativeFunction::from_fn_ptr(Self::remove));

        class.method(js_string!("toggle"), 1, NativeFunction::from_fn_ptr(Self::toggle));

        class.method(js_string!("replace"), 2, NativeFunction::from_fn_ptr(Self::replace));

        class.method(js_string!("contains"), 1, NativeFunction::from_fn_ptr(Self::contains));

        class.method(js_string!("item"), 1, NativeFunction::from_fn_ptr(Self::item));

        class.method(
            js_string!("toString"),
            0,
            NativeFunction::from_fn_ptr(Self::to_string_method),
        );

        // Length getter
        let realm = class.context().realm().clone();
        let length_getter = NativeFunction::from_fn_ptr(Self::get_length);

        class.accessor(
            js_string!("length"),
            Some(length_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Value getter/setter
        let value_getter = NativeFunction::from_fn_ptr(Self::get_value);
        let value_setter = NativeFunction::from_fn_ptr(Self::set_value);

        class.accessor(
            js_string!("value"),
            Some(value_getter.to_js_function(&realm)),
            Some(value_setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Registration function to be called by the parent module
// ---------------------------------------------------------------------------

/// IMPORTANT: This function must be called during context initialization
/// to register the JsClassList class globally. Call this from wherever
/// you register the other DOM classes (e.g., in document.rs).
pub(crate) fn register_class_list_class(context: &mut Context) {
    context.register_global_class::<JsClassList>().unwrap();
}
