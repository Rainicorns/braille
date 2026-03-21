use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, FunctionObjectBuilder, ObjectInitializer},
    property::PropertyDescriptor,
    Context, JsObject, JsValue,
};
use std::cell::RefCell;

use crate::dom::{NodeData, NodeId};
use crate::js::prop_desc;

/// Native data for FormData instances.
#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsFormData {
    #[unsafe_ignore_trace]
    entries: RefCell<Vec<(String, String)>>,
}

impl JsFormData {
    fn from_entries(entries: Vec<(String, String)>) -> Self {
        Self {
            entries: RefCell::new(entries),
        }
    }
}

/// Collect name/value pairs from a form element's descendant controls.
fn collect_form_entries(tree: &crate::dom::DomTree, form_id: NodeId) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut stack = tree.get_node(form_id).children.clone();
    while let Some(node_id) = stack.pop() {
        let node = tree.get_node(node_id);
        if let NodeData::Element {
            tag_name,
            attributes,
            ..
        } = &node.data
        {
            let tag = tag_name.to_ascii_lowercase();
            let name = attributes.iter().find(|a| a.local_name == "name").map(|a| a.value.clone());

            // Skip elements without a name attribute
            if let Some(ref name) = name {
                if !name.is_empty() {
                    match tag.as_str() {
                        "input" => {
                            let input_type = attributes
                                .iter()
                                .find(|a| a.local_name == "type")
                                .map(|a| a.value.to_ascii_lowercase())
                                .unwrap_or_else(|| "text".to_string());

                            match input_type.as_str() {
                                "checkbox" | "radio" => {
                                    let checked = attributes.iter().any(|a| a.local_name == "checked");
                                    if checked {
                                        let value = attributes
                                            .iter()
                                            .find(|a| a.local_name == "value")
                                            .map(|a| a.value.clone())
                                            .unwrap_or_else(|| "on".to_string());
                                        entries.push((name.clone(), value));
                                    }
                                }
                                "submit" | "image" | "button" | "reset" | "file" => {
                                    // Skip these types
                                }
                                _ => {
                                    let value = attributes
                                        .iter()
                                        .find(|a| a.local_name == "value")
                                        .map(|a| a.value.clone())
                                        .unwrap_or_default();
                                    entries.push((name.clone(), value));
                                }
                            }
                        }
                        "select" => {
                            // Find selected option value
                            let value = find_selected_option_value(tree, node_id);
                            entries.push((name.clone(), value));
                        }
                        "textarea" => {
                            let value = tree.get_text_content(node_id);
                            entries.push((name.clone(), value));
                        }
                        _ => {}
                    }
                }
            }
        }
        // Push children in reverse order so we process in document order
        let children = &tree.get_node(node_id).children;
        for &child in children.iter().rev() {
            stack.push(child);
        }
    }
    entries
}

fn find_selected_option_value(tree: &crate::dom::DomTree, select_id: NodeId) -> String {
    let node = tree.get_node(select_id);
    for &child in &node.children {
        let child_node = tree.get_node(child);
        if let NodeData::Element {
            tag_name,
            attributes,
            ..
        } = &child_node.data
        {
            if tag_name.eq_ignore_ascii_case("option") {
                let selected = attributes.iter().any(|a| a.local_name == "selected");
                if selected {
                    return attributes
                        .iter()
                        .find(|a| a.local_name == "value")
                        .map(|a| a.value.clone())
                        .unwrap_or_else(|| tree.get_text_content(child));
                }
            }
        }
    }
    // No selected option — return first option's value if any
    for &child in &node.children {
        let child_node = tree.get_node(child);
        if let NodeData::Element {
            tag_name,
            attributes,
            ..
        } = &child_node.data
        {
            if tag_name.eq_ignore_ascii_case("option") {
                return attributes
                    .iter()
                    .find(|a| a.local_name == "value")
                    .map(|a| a.value.clone())
                    .unwrap_or_else(|| tree.get_text_content(child));
            }
        }
    }
    String::new()
}

/// Register the FormData global constructor and prototype.
pub(crate) fn register_form_data_global(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();
    let realm = ctx.realm().clone();

    // -- Prototype methods --

    let append_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.append called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.append called on non-FormData")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let value = args
            .get(1)
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        fd.entries.borrow_mut().push((name, value));
        Ok(JsValue::undefined())
    });

    let get_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.get called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.get called on non-FormData")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let entries = fd.entries.borrow();
        for (k, v) in entries.iter() {
            if k == &name {
                return Ok(JsValue::from(js_string!(v.clone())));
            }
        }
        Ok(JsValue::null())
    });

    let get_all_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.getAll called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.getAll called on non-FormData")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let entries = fd.entries.borrow();
        let arr = JsArray::new(ctx);
        for (k, v) in entries.iter() {
            if k == &name {
                arr.push(JsValue::from(js_string!(v.clone())), ctx)?;
            }
        }
        Ok(JsValue::from(arr))
    });

    let set_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.set called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.set called on non-FormData")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let value = args
            .get(1)
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let mut entries = fd.entries.borrow_mut();
        entries.retain(|(k, _)| k != &name);
        entries.push((name, value));
        Ok(JsValue::undefined())
    });

    let has_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.has called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.has called on non-FormData")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let entries = fd.entries.borrow();
        let found = entries.iter().any(|(k, _)| k == &name);
        Ok(JsValue::from(found))
    });

    let delete_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.delete called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.delete called on non-FormData")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        fd.entries.borrow_mut().retain(|(k, _)| k != &name);
        Ok(JsValue::undefined())
    });

    let entries_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.entries called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.entries called on non-FormData")
        })?;
        let entries = fd.entries.borrow();
        let arr = JsArray::new(ctx);
        for (k, v) in entries.iter() {
            let pair = JsArray::new(ctx);
            pair.push(JsValue::from(js_string!(k.clone())), ctx)?;
            pair.push(JsValue::from(js_string!(v.clone())), ctx)?;
            arr.push(JsValue::from(pair), ctx)?;
        }
        // Return the array's iterator
        let iterator_fn = arr.get(boa_engine::JsSymbol::iterator(), ctx)?;
        if let Some(callable) = iterator_fn.as_callable() {
            return callable.call(&JsValue::from(arr), &[], ctx);
        }
        Ok(JsValue::from(arr))
    });

    let keys_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.keys called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.keys called on non-FormData")
        })?;
        let entries = fd.entries.borrow();
        let arr = JsArray::new(ctx);
        for (k, _) in entries.iter() {
            arr.push(JsValue::from(js_string!(k.clone())), ctx)?;
        }
        let iterator_fn = arr.get(boa_engine::JsSymbol::iterator(), ctx)?;
        if let Some(callable) = iterator_fn.as_callable() {
            return callable.call(&JsValue::from(arr), &[], ctx);
        }
        Ok(JsValue::from(arr))
    });

    let values_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.values called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.values called on non-FormData")
        })?;
        let entries = fd.entries.borrow();
        let arr = JsArray::new(ctx);
        for (_, v) in entries.iter() {
            arr.push(JsValue::from(js_string!(v.clone())), ctx)?;
        }
        let iterator_fn = arr.get(boa_engine::JsSymbol::iterator(), ctx)?;
        if let Some(callable) = iterator_fn.as_callable() {
            return callable.call(&JsValue::from(arr), &[], ctx);
        }
        Ok(JsValue::from(arr))
    });

    let for_each_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.forEach called on non-object")
        })?;
        let fd = obj.downcast_ref::<JsFormData>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.forEach called on non-FormData")
        })?;
        let callback = args.first().cloned().unwrap_or(JsValue::undefined());
        let callable = callback.as_callable().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData.forEach callback is not callable")
        })?;
        let entries: Vec<(String, String)> = fd.entries.borrow().clone();
        for (k, v) in entries {
            callable.call(
                &JsValue::undefined(),
                &[
                    JsValue::from(js_string!(v)),
                    JsValue::from(js_string!(k)),
                    JsValue::from(obj.clone()),
                ],
                ctx,
            )?;
        }
        Ok(JsValue::undefined())
    });

    proto
        .define_property_or_throw(js_string!("append"), prop_desc::data_prop(append_fn.to_js_function(&realm)), ctx)
        .expect("define FormData.prototype.append");
    proto
        .define_property_or_throw(js_string!("get"), prop_desc::data_prop(get_fn.to_js_function(&realm)), ctx)
        .expect("define FormData.prototype.get");
    proto
        .define_property_or_throw(
            js_string!("getAll"),
            prop_desc::data_prop(get_all_fn.to_js_function(&realm)),
            ctx,
        )
        .expect("define FormData.prototype.getAll");
    proto
        .define_property_or_throw(js_string!("set"), prop_desc::data_prop(set_fn.to_js_function(&realm)), ctx)
        .expect("define FormData.prototype.set");
    proto
        .define_property_or_throw(js_string!("has"), prop_desc::data_prop(has_fn.to_js_function(&realm)), ctx)
        .expect("define FormData.prototype.has");
    proto
        .define_property_or_throw(js_string!("delete"), prop_desc::data_prop(delete_fn.to_js_function(&realm)), ctx)
        .expect("define FormData.prototype.delete");
    proto
        .define_property_or_throw(
            js_string!("entries"),
            prop_desc::data_prop(entries_fn.to_js_function(&realm)),
            ctx,
        )
        .expect("define FormData.prototype.entries");
    proto
        .define_property_or_throw(js_string!("keys"), prop_desc::data_prop(keys_fn.to_js_function(&realm)), ctx)
        .expect("define FormData.prototype.keys");
    proto
        .define_property_or_throw(
            js_string!("values"),
            prop_desc::data_prop(values_fn.to_js_function(&realm)),
            ctx,
        )
        .expect("define FormData.prototype.values");
    proto
        .define_property_or_throw(
            js_string!("forEach"),
            prop_desc::data_prop(for_each_fn.to_js_function(&realm)),
            ctx,
        )
        .expect("define FormData.prototype.forEach");

    // Symbol.iterator → entries
    let entries_fn2 = NativeFunction::from_fn_ptr(|this, args, ctx| {
        // Delegate to entries()
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("FormData[Symbol.iterator] called on non-object")
        })?;
        let entries_method = obj.get(js_string!("entries"), ctx)?;
        if let Some(callable) = entries_method.as_callable() {
            return callable.call(this, args, ctx);
        }
        Ok(JsValue::undefined())
    });
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::iterator(),
            prop_desc::data_prop(entries_fn2.to_js_function(&realm)),
            ctx,
        )
        .expect("define FormData.prototype[Symbol.iterator]");

    // Symbol.toStringTag
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            PropertyDescriptor::builder()
                .value(js_string!("FormData"))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("define FormData.prototype[Symbol.toStringTag]");

    // -- Constructor --
    let proto_for_ctor = proto.clone();
    let form_data_ctor = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let entries = if let Some(form_val) = args.first() {
                if !form_val.is_null() && !form_val.is_undefined() {
                    let form_obj = form_val.as_object().ok_or_else(|| {
                        boa_engine::JsNativeError::typ()
                            .with_message("FormData constructor argument must be a form element")
                    })?;
                    // Check if it's a JsElement (form element)
                    let result = if let Some(el) = form_obj.downcast_ref::<super::element::JsElement>() {
                        let tree = el.tree.borrow();
                        collect_form_entries(&tree, el.node_id)
                    } else {
                        Vec::new()
                    };
                    result
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let data = JsFormData::from_entries(entries);
            let obj = ObjectInitializer::with_native_data(data, ctx).build();
            obj.set_prototype(Some(proto_for_ctor.clone()));
            Ok(JsValue::from(obj))
        })
    };

    let ctor: JsObject = FunctionObjectBuilder::new(ctx.realm(), form_data_ctor)
        .name(js_string!("FormData"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor.define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("set FormData.prototype");
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor.clone()), ctx)
        .expect("set FormData.prototype.constructor");

    ctx.global_object()
        .set(js_string!("FormData"), JsValue::from(ctor), false, ctx)
        .expect("set FormData global");
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::dom::DomTree;
    use crate::js::JsRuntime;

    fn make_runtime() -> JsRuntime {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
        }
        JsRuntime::new(tree)
    }

    #[test]
    fn form_data_empty_constructor() {
        let mut rt = make_runtime();
        let result = rt
            .eval("var fd = new FormData(); fd instanceof FormData && fd.has('x') === false")
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_append_and_get() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var fd = new FormData();
            fd.append('name', 'Alice');
            fd.append('age', '30');
            fd.get('name') === 'Alice' && fd.get('age') === '30'
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_get_all() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var fd = new FormData();
            fd.append('color', 'red');
            fd.append('color', 'blue');
            var all = fd.getAll('color');
            all.length === 2 && all[0] === 'red' && all[1] === 'blue'
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_set_replaces() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var fd = new FormData();
            fd.append('x', '1');
            fd.append('x', '2');
            fd.set('x', '3');
            fd.getAll('x').length === 1 && fd.get('x') === '3'
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_has_and_delete() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var fd = new FormData();
            fd.append('a', '1');
            var had = fd.has('a');
            fd.delete('a');
            had === true && fd.has('a') === false
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_get_missing_returns_null() {
        let mut rt = make_runtime();
        let result = rt.eval("new FormData().get('nope') === null").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_for_each() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var fd = new FormData();
            fd.append('a', '1');
            fd.append('b', '2');
            var keys = [];
            fd.forEach(function(val, key) { keys.push(key); });
            keys.join(',') === 'a,b'
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_symbol_to_string_tag() {
        let mut rt = make_runtime();
        let result = rt.eval("Object.prototype.toString.call(new FormData())").unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "[object FormData]");
    }

    #[test]
    fn form_data_from_form_element() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var form = document.createElement('form');
            var input = document.createElement('input');
            input.setAttribute('name', 'user');
            input.setAttribute('value', 'Bob');
            form.appendChild(input);
            var fd = new FormData(form);
            fd.get('user') === 'Bob'
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn form_data_iteration() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var fd = new FormData();
            fd.append('x', '1');
            fd.append('y', '2');
            var pairs = [];
            for (var pair of fd) {
                pairs.push(pair[0] + '=' + pair[1]);
            }
            pairs.join('&') === 'x=1&y=2'
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }
}
