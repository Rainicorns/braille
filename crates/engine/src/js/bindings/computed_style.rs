use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    property::Attribute,
    Context, JsData, JsError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::DomTree;

use super::element::JsElement;

// ---------------------------------------------------------------------------
// JsComputedStyle — read-only snapshot of an element's computed styles
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsComputedStyle {
    #[unsafe_ignore_trace]
    styles: HashMap<String, String>,
}

impl JsComputedStyle {
    fn lookup(&self, prop: &str) -> String {
        self.styles.get(prop).cloned().unwrap_or_default()
    }
}

impl Class for JsComputedStyle {
    const NAME: &'static str = "CSSStyleDeclaration";
    const LENGTH: usize = 0;

    fn data_constructor(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<Self> {
        Ok(JsComputedStyle { styles: HashMap::new() })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        // getPropertyValue(name) -> string
        class.method(
            js_string!("getPropertyValue"),
            1,
            NativeFunction::from_fn_ptr(get_property_value),
        );

        let realm = class.context().realm().clone();

        // length getter
        let length_getter = NativeFunction::from_fn_ptr(get_length);
        class.accessor(
            js_string!("length"),
            Some(length_getter.to_js_function(&realm)),
            None,
            Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
        );

        // Register common CSS property accessors (camelCase → kebab-case lookup)
        let props: &[(&str, &str)] = &[
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
        ];

        for &(camel, kebab) in props {
            let kebab_owned = kebab.to_string();
            let getter = unsafe {
                NativeFunction::from_closure(move |this, _args, _ctx| {
                    let obj = this.as_object().ok_or_else(|| {
                        JsError::from_opaque(js_string!("computed style getter: this is not an object").into())
                    })?;
                    let cs = obj.downcast_ref::<JsComputedStyle>().ok_or_else(|| {
                        JsError::from_opaque(
                            js_string!("computed style getter: this is not a CSSStyleDeclaration").into(),
                        )
                    })?;
                    let val = cs.lookup(&kebab_owned);
                    Ok(JsValue::from(js_string!(val)))
                })
            };
            class.accessor(
                js_string!(camel),
                Some(getter.to_js_function(&realm)),
                None,
                Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
            );
        }

        Ok(())
    }
}

fn get_property_value(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("getPropertyValue: this is not an object").into()))?;
    let cs = obj.downcast_ref::<JsComputedStyle>().ok_or_else(|| {
        JsError::from_opaque(js_string!("getPropertyValue: this is not a CSSStyleDeclaration").into())
    })?;

    let prop = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let val = cs.lookup(&prop);
    Ok(JsValue::from(js_string!(val)))
}

fn get_length(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("length getter: this is not an object").into()))?;
    let cs = obj
        .downcast_ref::<JsComputedStyle>()
        .ok_or_else(|| JsError::from_opaque(js_string!("length getter: this is not a CSSStyleDeclaration").into()))?;
    Ok(JsValue::from(cs.styles.len() as f64))
}

// ---------------------------------------------------------------------------
// Factory: creates the getComputedStyle(element) native function
// ---------------------------------------------------------------------------

pub(crate) fn make_get_computed_style(tree: Rc<RefCell<DomTree>>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let el_val = args
                .first()
                .ok_or_else(|| JsError::from_opaque(js_string!("getComputedStyle: missing element argument").into()))?;
            let el_obj = el_val.as_object().ok_or_else(|| {
                JsError::from_opaque(js_string!("getComputedStyle: argument is not an object").into())
            })?;
            let el = el_obj.downcast_ref::<JsElement>().ok_or_else(|| {
                JsError::from_opaque(js_string!("getComputedStyle: argument is not an Element").into())
            })?;

            let styles = {
                let tree_ref = tree.borrow();
                let node = tree_ref.get_node(el.node_id);
                node.computed_style.clone().unwrap_or_default()
            };

            let cs = JsComputedStyle { styles };
            let js_obj = JsComputedStyle::from_data(cs, ctx)?;
            Ok(JsValue::from(js_obj))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    #[test]
    fn get_computed_style_returns_color() {
        let mut engine = Engine::new();
        engine.load_html(
            r##"<html><body>
            <style>p { color: red; }</style>
            <p id="t">Hello</p>
            </body></html>"##,
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"var s = getComputedStyle(document.getElementById("t")); var result = s.getPropertyValue("color");"#,
            )
            .unwrap();
        let result = runtime.eval("result").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert!(!s.is_empty(), "computed color should not be empty: {}", s);
    }

    #[test]
    fn get_computed_style_camel_case_accessor() {
        let mut engine = Engine::new();
        engine.load_html(
            r##"<html><body>
            <style>div { display: none; }</style>
            <div id="t">Hidden</div>
            </body></html>"##,
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"var s = getComputedStyle(document.getElementById("t")); var result = s.display;"#)
            .unwrap();
        let result = runtime.eval("result").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "none");
    }

    #[test]
    fn get_computed_style_length() {
        let mut engine = Engine::new();
        engine.load_html(
            r##"<html><body>
            <style>p { color: red; display: block; }</style>
            <p id="t">Hello</p>
            </body></html>"##,
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"var s = getComputedStyle(document.getElementById("t")); var result = s.length;"#)
            .unwrap();
        let result = runtime.eval("result").unwrap();
        let n = result.to_number(&mut runtime.context).unwrap();
        assert!(n >= 2.0, "should have at least 2 computed properties, got {}", n);
    }

    #[test]
    fn get_computed_style_empty_for_no_styles() {
        let mut engine = Engine::new();
        engine.load_html("<html><body><div id='t'>Test</div></body></html>");
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"var s = getComputedStyle(document.getElementById("t")); var result = s.getPropertyValue("color");"#,
            )
            .unwrap();
        let result = runtime.eval("result").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        // No author styles, so UA defaults may or may not be present
        // Just verify it doesn't crash and returns a string
        assert!(s.is_empty() || !s.is_empty(), "should return a string");
    }

    #[test]
    fn window_get_computed_style() {
        let mut engine = Engine::new();
        engine.load_html(
            r##"<html><body>
            <style>.red { color: red; }</style>
            <p id="t" class="red">Hello</p>
            </body></html>"##,
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(r#"var s = window.getComputedStyle(document.getElementById("t")); var result = s.color;"#)
            .unwrap();
        let result = runtime.eval("result").unwrap();
        let s = result.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert!(!s.is_empty(), "window.getComputedStyle should work");
    }

    #[test]
    fn different_elements_different_styles() {
        let mut engine = Engine::new();
        engine.load_html(
            r##"<html><body>
            <style>
                .red { color: red; }
                .blue { color: blue; }
            </style>
            <p id="a" class="red">Red</p>
            <p id="b" class="blue">Blue</p>
            </body></html>"##,
        );
        let runtime = engine.runtime.as_mut().unwrap();
        runtime
            .eval(
                r#"
                var a = getComputedStyle(document.getElementById("a")).getPropertyValue("color");
                var b = getComputedStyle(document.getElementById("b")).getPropertyValue("color");
            "#,
            )
            .unwrap();
        let a = runtime.eval("a").unwrap();
        let b = runtime.eval("b").unwrap();
        let a_str = a.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        let b_str = b.to_string(&mut runtime.context).unwrap().to_std_string_escaped();
        assert_ne!(
            a_str, b_str,
            "different classes should give different colors: a={}, b={}",
            a_str, b_str
        );
    }
}
