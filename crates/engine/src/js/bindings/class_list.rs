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

    /// Helper to parse class attribute into a vector of class names
    fn get_classes(&self) -> Vec<String> {
        self.tree
            .borrow()
            .get_attribute(self.node_id, "class")
            .map(|class_str| {
                class_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Helper to write class names back to the class attribute
    fn set_classes(&self, classes: Vec<String>) {
        if classes.is_empty() {
            self.tree
                .borrow_mut()
                .remove_attribute(self.node_id, "class");
        } else {
            let class_str = classes.join(" ");
            self.tree
                .borrow_mut()
                .set_attribute(self.node_id, "class", &class_str);
        }
    }

    /// Native implementation of classList.add(...classNames)
    fn add(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.add: `this` is not an object").into()))?;
        let class_list = obj
            .downcast_ref::<JsClassList>()
            .ok_or_else(|| JsError::from_opaque(js_string!("classList.add: `this` is not a ClassList").into()))?;

        let mut classes = class_list.get_classes();

        // Add each argument to the classes if not already present
        for arg in args {
            let class_name = arg.to_string(ctx)?.to_std_string_escaped();
            if !class_name.is_empty() && !classes.contains(&class_name) {
                classes.push(class_name);
            }
        }

        class_list.set_classes(classes);
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

        let mut classes = class_list.get_classes();

        // Remove each argument from the classes
        for arg in args {
            let class_name = arg.to_string(ctx)?.to_std_string_escaped();
            classes.retain(|c| c != &class_name);
        }

        class_list.set_classes(classes);
        Ok(JsValue::undefined())
    }

    /// Native implementation of classList.toggle(className)
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

        let mut classes = class_list.get_classes();
        let added = if let Some(pos) = classes.iter().position(|c| c == &class_name) {
            // Remove it
            classes.remove(pos);
            false
        } else {
            // Add it
            classes.push(class_name);
            true
        };

        class_list.set_classes(classes);
        Ok(JsValue::from(added))
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
}

impl Class for JsClassList {
    const NAME: &'static str = "ClassList";
    const LENGTH: usize = 0;

    fn data_constructor(
        _new_target: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<Self> {
        Err(JsError::from_opaque(
            js_string!("ClassList cannot be constructed directly from JS").into(),
        ))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        // Methods
        class.method(
            js_string!("add"),
            0,
            NativeFunction::from_fn_ptr(Self::add),
        );

        class.method(
            js_string!("remove"),
            0,
            NativeFunction::from_fn_ptr(Self::remove),
        );

        class.method(
            js_string!("toggle"),
            1,
            NativeFunction::from_fn_ptr(Self::toggle),
        );

        class.method(
            js_string!("contains"),
            1,
            NativeFunction::from_fn_ptr(Self::contains),
        );

        class.method(
            js_string!("item"),
            1,
            NativeFunction::from_fn_ptr(Self::item),
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
