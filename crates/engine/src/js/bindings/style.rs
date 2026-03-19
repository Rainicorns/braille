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


#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsStyleDeclaration {
    #[unsafe_ignore_trace]
    node_id: NodeId,
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
}

impl JsStyleDeclaration {
    pub fn new(node_id: NodeId, tree: Rc<RefCell<DomTree>>) -> Self {
        Self { node_id, tree }
    }
}

fn parse_style_attr(style: &str) -> Vec<(String, String)> {
    let mut props = Vec::new();
    for part in style.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(colon_pos) = part.find(':') {
            let name = part[..colon_pos].trim().to_string();
            let value = part[colon_pos + 1..].trim().to_string();
            if !name.is_empty() {
                props.push((name, value));
            }
        }
    }
    props
}

fn serialize_style(props: &[(String, String)]) -> String {
    if props.is_empty() {
        return String::new();
    }
    props
        .iter()
        .map(|(name, value)| format!("{}: {};", name, value))
        .collect::<Vec<_>>()
        .join(" ")
}

fn get_property_value(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.getPropertyValue: `this` is not an object").into()))?;
    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
        JsError::from_opaque(js_string!("style.getPropertyValue: `this` is not a StyleDeclaration").into())
    })?;

    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree = style.tree.borrow();
    let style_attr = tree.get_attribute(style.node_id, "style").unwrap_or_default();
    let props = parse_style_attr(&style_attr);

    let value = props
        .iter()
        .find(|(n, _)| n == &name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    Ok(JsValue::from(js_string!(value)))
}

fn set_property(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.setProperty: `this` is not an object").into()))?;
    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
        JsError::from_opaque(js_string!("style.setProperty: `this` is not a StyleDeclaration").into())
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

    let node_id = style.node_id;
    let tree = style.tree.clone();

    let style_attr = tree.borrow().get_attribute(node_id, "style").unwrap_or_default();
    let mut props = parse_style_attr(&style_attr);

    if let Some(existing) = props.iter_mut().find(|(n, _)| n == &name) {
        existing.1 = value;
    } else {
        props.push((name, value));
    }

    let serialized = serialize_style(&props);
    super::mutation_observer::set_attribute_with_observer(ctx, &tree, node_id, "style", &serialized);

    Ok(JsValue::undefined())
}

fn remove_property(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.removeProperty: `this` is not an object").into()))?;
    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
        JsError::from_opaque(js_string!("style.removeProperty: `this` is not a StyleDeclaration").into())
    })?;

    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = style.node_id;
    let tree = style.tree.clone();

    let style_attr = tree.borrow().get_attribute(node_id, "style").unwrap_or_default();
    let mut props = parse_style_attr(&style_attr);

    let old_value = if let Some(pos) = props.iter().position(|(n, _)| n == &name) {
        let (_, old_val) = props.remove(pos);
        old_val
    } else {
        String::new()
    };

    let serialized = serialize_style(&props);
    if serialized.is_empty() {
        super::mutation_observer::remove_attribute_with_observer(ctx, &tree, node_id, "style");
    } else {
        super::mutation_observer::set_attribute_with_observer(ctx, &tree, node_id, "style", &serialized);
    }

    Ok(JsValue::from(js_string!(old_value)))
}

fn get_css_text(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.cssText getter: `this` is not an object").into()))?;
    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
        JsError::from_opaque(js_string!("style.cssText getter: `this` is not a StyleDeclaration").into())
    })?;

    let tree = style.tree.borrow();
    let style_attr = tree.get_attribute(style.node_id, "style").unwrap_or_default();

    Ok(JsValue::from(js_string!(style_attr)))
}

fn set_css_text(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.cssText setter: `this` is not an object").into()))?;
    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
        JsError::from_opaque(js_string!("style.cssText setter: `this` is not a StyleDeclaration").into())
    })?;

    let value = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = style.node_id;
    let tree = style.tree.clone();

    if value.is_empty() {
        super::mutation_observer::remove_attribute_with_observer(ctx, &tree, node_id, "style");
    } else {
        super::mutation_observer::set_attribute_with_observer(ctx, &tree, node_id, "style", &value);
    }

    Ok(JsValue::undefined())
}

fn get_length(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.length: `this` is not an object").into()))?;
    let style = obj
        .downcast_ref::<JsStyleDeclaration>()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.length: `this` is not a StyleDeclaration").into()))?;

    let tree = style.tree.borrow();
    let style_attr = tree.get_attribute(style.node_id, "style").unwrap_or_default();
    let props = parse_style_attr(&style_attr);

    Ok(JsValue::from(props.len()))
}

fn item(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.item: `this` is not an object").into()))?;
    let style = obj
        .downcast_ref::<JsStyleDeclaration>()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.item: `this` is not a StyleDeclaration").into()))?;

    let index = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("style.item: missing argument").into()))?
        .to_i32(ctx)? as usize;

    let tree = style.tree.borrow();
    let style_attr = tree.get_attribute(style.node_id, "style").unwrap_or_default();
    let props = parse_style_attr(&style_attr);

    match props.get(index) {
        Some((name, _)) => Ok(JsValue::from(js_string!(name.clone()))),
        None => Ok(JsValue::from(js_string!(""))),
    }
}

impl Class for JsStyleDeclaration {
    const NAME: &'static str = "CSSStyleDeclaration";
    const LENGTH: usize = 0;

    fn data_constructor(_new_target: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<Self> {
        Err(JsError::from_opaque(
            js_string!("CSSStyleDeclaration cannot be constructed directly from JS").into(),
        ))
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("getPropertyValue"),
            1,
            NativeFunction::from_fn_ptr(get_property_value),
        );
        class.method(js_string!("setProperty"), 2, NativeFunction::from_fn_ptr(set_property));
        class.method(
            js_string!("removeProperty"),
            1,
            NativeFunction::from_fn_ptr(remove_property),
        );
        class.method(js_string!("item"), 1, NativeFunction::from_fn_ptr(item));

        let realm = class.context().realm().clone();
        let css_text_getter = NativeFunction::from_fn_ptr(get_css_text);
        let css_text_setter = NativeFunction::from_fn_ptr(set_css_text);
        class.accessor(
            js_string!("cssText"),
            Some(css_text_getter.to_js_function(&realm)),
            Some(css_text_setter.to_js_function(&realm)),
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );
        let length_getter = NativeFunction::from_fn_ptr(get_length);
        class.accessor(
            js_string!("length"),
            Some(length_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Register camelCase getter/setter accessors for common CSS properties
        let style_props: &[(&str, &str)] = &[
            ("color", "color"),
            ("display", "display"),
            ("visibility", "visibility"),
            ("fontSize", "font-size"),
            ("fontFamily", "font-family"),
            ("fontWeight", "font-weight"),
            ("fontStyle", "font-style"),
            ("backgroundColor", "background-color"),
            ("margin", "margin"),
            ("marginTop", "margin-top"),
            ("marginRight", "margin-right"),
            ("marginBottom", "margin-bottom"),
            ("marginLeft", "margin-left"),
            ("padding", "padding"),
            ("paddingTop", "padding-top"),
            ("paddingRight", "padding-right"),
            ("paddingBottom", "padding-bottom"),
            ("paddingLeft", "padding-left"),
            ("width", "width"),
            ("height", "height"),
            ("border", "border"),
            ("position", "position"),
            ("top", "top"),
            ("right", "right"),
            ("bottom", "bottom"),
            ("left", "left"),
            ("textAlign", "text-align"),
            ("textDecoration", "text-decoration"),
            ("lineHeight", "line-height"),
            ("overflow", "overflow"),
            ("cursor", "cursor"),
            ("opacity", "opacity"),
            ("zIndex", "z-index"),
            ("cssFloat", "float"),
            ("maxWidth", "max-width"),
            ("maxHeight", "max-height"),
            ("minWidth", "min-width"),
            ("minHeight", "min-height"),
            ("borderRadius", "border-radius"),
            ("boxSizing", "box-sizing"),
            ("flexDirection", "flex-direction"),
            ("justifyContent", "justify-content"),
            ("alignItems", "align-items"),
            ("flexWrap", "flex-wrap"),
            ("gap", "gap"),
            ("gridTemplateColumns", "grid-template-columns"),
            ("gridTemplateRows", "grid-template-rows"),
            ("transform", "transform"),
            ("transition", "transition"),
            ("pointerEvents", "pointer-events"),
            ("whiteSpace", "white-space"),
            ("textOverflow", "text-overflow"),
            ("overflowX", "overflow-x"),
            ("overflowY", "overflow-y"),
            ("outline", "outline"),
            ("boxShadow", "box-shadow"),
            ("textTransform", "text-transform"),
            ("letterSpacing", "letter-spacing"),
            ("wordSpacing", "word-spacing"),
            ("verticalAlign", "vertical-align"),
            ("borderTop", "border-top"),
            ("borderRight", "border-right"),
            ("borderBottom", "border-bottom"),
            ("borderLeft", "border-left"),
            ("borderColor", "border-color"),
            ("borderWidth", "border-width"),
            ("borderStyle", "border-style"),
            ("content", "content"),
            ("listStyle", "list-style"),
            ("listStyleType", "list-style-type"),
            ("fontVariant", "font-variant"),
            ("textIndent", "text-indent"),
            ("userSelect", "user-select"),
            ("objectFit", "object-fit"),
            ("backgroundImage", "background-image"),
            ("backgroundSize", "background-size"),
            ("backgroundPosition", "background-position"),
            ("backgroundRepeat", "background-repeat"),
        ];

        for &(camel, kebab) in style_props {
            let kebab_getter = kebab.to_string();
            let kebab_setter = kebab.to_string();

            let getter = unsafe {
                NativeFunction::from_closure(move |this, _args, _ctx| {
                    let obj = this.as_object().ok_or_else(|| {
                        JsError::from_opaque(js_string!("style getter: this is not an object").into())
                    })?;
                    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
                        JsError::from_opaque(
                            js_string!("style getter: this is not a CSSStyleDeclaration").into(),
                        )
                    })?;
                    let tree = style.tree.borrow();
                    let style_attr = tree.get_attribute(style.node_id, "style").unwrap_or_default();
                    let props = parse_style_attr(&style_attr);
                    let val = props
                        .iter()
                        .find(|(n, _)| n == &kebab_getter)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default();
                    Ok(JsValue::from(js_string!(val)))
                })
            };

            let setter = unsafe {
                NativeFunction::from_closure(move |this, args, ctx| {
                    let obj = this.as_object().ok_or_else(|| {
                        JsError::from_opaque(js_string!("style setter: this is not an object").into())
                    })?;
                    let style = obj.downcast_ref::<JsStyleDeclaration>().ok_or_else(|| {
                        JsError::from_opaque(
                            js_string!("style setter: this is not a CSSStyleDeclaration").into(),
                        )
                    })?;
                    let value = args
                        .first()
                        .map(|v| v.to_string(ctx))
                        .transpose()?
                        .map(|s| s.to_std_string_escaped())
                        .unwrap_or_default();

                    let node_id = style.node_id;
                    let tree = style.tree.clone();

                    let style_attr = tree.borrow().get_attribute(node_id, "style").unwrap_or_default();
                    let mut props = parse_style_attr(&style_attr);

                    if value.is_empty() {
                        // Empty string removes the property (per spec)
                        props.retain(|(n, _)| n != &kebab_setter);
                    } else if let Some(existing) = props.iter_mut().find(|(n, _)| n == &kebab_setter) {
                        existing.1 = value;
                    } else {
                        props.push((kebab_setter.clone(), value));
                    }

                    let serialized = serialize_style(&props);
                    if serialized.is_empty() {
                        super::mutation_observer::remove_attribute_with_observer(ctx, &tree, node_id, "style");
                    } else {
                        super::mutation_observer::set_attribute_with_observer(
                            ctx, &tree, node_id, "style", &serialized,
                        );
                    }

                    Ok(JsValue::undefined())
                })
            };

            class.accessor(
                js_string!(camel),
                Some(getter.to_js_function(&realm)),
                Some(setter.to_js_function(&realm)),
                Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
            );
        }

        Ok(())
    }
}

pub(crate) fn register_style(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();
    let getter = NativeFunction::from_fn_ptr(get_style);
    class.accessor(
        js_string!("style"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );
    Ok(())
}

fn get_style(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "style getter");

    let style_decl = JsStyleDeclaration::new(el.node_id, el.tree.clone());
    let js_obj = JsStyleDeclaration::from_data(style_decl, ctx)?;
    Ok(js_obj.into())
}

pub(crate) fn register_style_class(context: &mut Context) {
    context.register_global_class::<JsStyleDeclaration>().unwrap();
}

#[cfg(test)]
mod tests {
    use crate::dom::NodeId;
    use crate::dom::{DomTree, NodeData};
    use crate::js::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "app"));
            }
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn style_set_property_sets_inline_style_attribute() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red");"#)
            .unwrap();
        let t = tree.borrow();
        let div_id: NodeId = 3;
        assert_eq!(t.get_attribute(div_id, "style"), Some("color: red;".to_string()));
    }

    #[test]
    fn style_get_property_value_reads_correct_value() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red");"#)
            .unwrap();
        let result = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.getPropertyValue("color");"#)
            .unwrap();
        let value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(value, "red");
    }

    #[test]
    fn style_remove_property_removes_and_returns_old_value() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.setProperty("font-size", "16px");"#).unwrap();
        let result = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.removeProperty("color");"#)
            .unwrap();
        let old_value = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(old_value, "red");
        let t = tree.borrow();
        assert_eq!(t.get_attribute(3, "style"), Some("font-size: 16px;".to_string()));
    }

    #[test]
    fn style_css_text_getter_returns_full_style_string() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.setProperty("font-size", "16px");"#).unwrap();
        let result = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.cssText;"#)
            .unwrap();
        let css_text = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(css_text, "color: red; font-size: 16px;");
    }

    #[test]
    fn style_css_text_setter_replaces_style() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.cssText = "background: blue; margin: 10px;";"#).unwrap();
        let t = tree.borrow();
        assert_eq!(
            t.get_attribute(3, "style"),
            Some("background: blue; margin: 10px;".to_string())
        );
    }

    #[test]
    fn style_length_returns_property_count() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let r0 = rt
            .eval(r#"var el = document.getElementById("app"); el.style.length;"#)
            .unwrap();
        assert_eq!(r0.as_number(), Some(0.0));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.setProperty("font-size", "16px"); el.style.setProperty("margin", "10px");"#).unwrap();
        let r3 = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.length;"#)
            .unwrap();
        assert_eq!(r3.as_number(), Some(3.0));
    }

    #[test]
    fn style_item_returns_property_at_index() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.setProperty("font-size", "16px");"#).unwrap();
        let r0 = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.item(0);"#)
            .unwrap();
        assert_eq!(r0.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "color");
        let r1 = rt
            .eval(r#"var el3 = document.getElementById("app"); el3.style.item(1);"#)
            .unwrap();
        assert_eq!(
            r1.to_string(&mut rt.context).unwrap().to_std_string_escaped(),
            "font-size"
        );
        let roob = rt
            .eval(r#"var el4 = document.getElementById("app"); el4.style.item(99);"#)
            .unwrap();
        assert_eq!(roob.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "");
    }

    #[test]
    fn style_multiple_set_property_calls_accumulate() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.setProperty("font-size", "16px"); el.style.setProperty("margin", "10px");"#).unwrap();
        let t = tree.borrow();
        assert_eq!(
            t.get_attribute(3, "style"),
            Some("color: red; font-size: 16px; margin: 10px;".to_string())
        );
    }

    #[test]
    fn style_set_property_overwrites_existing_property() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.setProperty("color", "blue");"#).unwrap();
        let result = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.getPropertyValue("color");"#)
            .unwrap();
        assert_eq!(
            result.to_string(&mut rt.context).unwrap().to_std_string_escaped(),
            "blue"
        );
        let t = tree.borrow();
        assert_eq!(t.get_attribute(3, "style"), Some("color: blue;".to_string()));
    }

    #[test]
    fn style_on_element_with_no_style_attribute_returns_empty_values() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let rv = rt
            .eval(r#"var el = document.getElementById("app"); el.style.getPropertyValue("color");"#)
            .unwrap();
        assert_eq!(rv.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "");
        let rc = rt
            .eval(r#"var el2 = document.getElementById("app"); el2.style.cssText;"#)
            .unwrap();
        assert_eq!(rc.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "");
        let rl = rt
            .eval(r#"var el3 = document.getElementById("app"); el3.style.length;"#)
            .unwrap();
        assert_eq!(rl.as_number(), Some(0.0));
    }

    #[test]
    fn style_remove_property_returns_empty_for_missing() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(r#"var el = document.getElementById("app"); el.style.removeProperty("nonexistent");"#)
            .unwrap();
        assert_eq!(result.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "");
    }

    #[test]
    fn style_css_text_setter_empty_clears_style() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"var el = document.getElementById("app"); el.style.setProperty("color", "red"); el.style.cssText = "";"#,
        )
        .unwrap();
        let t = tree.borrow();
        assert_eq!(t.get_attribute(3, "style"), None);
    }

    #[test]
    fn style_workflow_integration() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var el = document.getElementById("app");
            if (el.style.length !== 0) throw new Error("Expected length 0");
            if (el.style.cssText !== "") throw new Error("Expected empty cssText");
            el.style.setProperty("color", "red");
            el.style.setProperty("font-size", "16px");
            el.style.setProperty("margin", "10px");
            if (el.style.length !== 3) throw new Error("Expected length 3");
            if (el.style.getPropertyValue("color") !== "red") throw new Error("Expected color red");
            el.style.setProperty("color", "blue");
            if (el.style.getPropertyValue("color") !== "blue") throw new Error("Expected color blue");
            var old = el.style.removeProperty("font-size");
            if (old !== "16px") throw new Error("Expected 16px");
            if (el.style.length !== 2) throw new Error("Expected length 2");
            el.style.cssText = "display: flex;";
            if (el.style.length !== 1) throw new Error("Expected length 1");
            if (el.style.getPropertyValue("display") !== "flex") throw new Error("Expected display flex");
            el.style.cssText = "";
            if (el.style.length !== 0) throw new Error("Expected length 0");
        "#,
        )
        .unwrap();
    }
}
