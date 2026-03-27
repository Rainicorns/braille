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
fn window_exists_and_self_referential() {
    let mut rt = make_runtime();
    let result = rt.eval("window.window === window").unwrap();
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn window_dot_window_dot_window() {
    let mut rt = make_runtime();
    let result = rt.eval("window.window.window === window").unwrap();
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn window_document_exists() {
    let mut rt = make_runtime();
    let result = rt
        .eval("window.document !== undefined && window.document !== null")
        .unwrap();
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn window_document_same_as_global_document() {
    let mut rt = make_runtime();
    let result = rt.eval("typeof window.document.createElement === 'function'").unwrap();
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn window_location_href_default() {
    let mut rt = make_runtime();
    let result = rt.eval("window.location.href").unwrap();
    let href = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(href, "about:blank");
}

#[test]
fn window_location_href_setter() {
    let mut rt = make_runtime();
    rt.eval(r#"window.location.href = "https://example.com/path?q=1#sec""#)
        .unwrap();
    let result = rt.eval("window.location.href").unwrap();
    let href = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(href, "https://example.com/path?q=1#sec");
}

#[test]
fn window_location_parts() {
    let mut rt = make_runtime();
    rt.eval(r#"window.location.href = "https://example.com:8080/foo/bar?q=hello&b=2#section""#)
        .unwrap();

    let protocol = rt.eval("window.location.protocol").unwrap();
    let protocol_str = protocol.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(protocol_str, "https:");

    let hostname = rt.eval("window.location.hostname").unwrap();
    let hostname_str = hostname.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(hostname_str, "example.com");

    let pathname = rt.eval("window.location.pathname").unwrap();
    let pathname_str = pathname.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(pathname_str, "/foo/bar");

    let search = rt.eval("window.location.search").unwrap();
    let search_str = search.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(search_str, "?q=hello&b=2");

    let hash = rt.eval("window.location.hash").unwrap();
    let hash_str = hash.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(hash_str, "#section");
}

#[test]
fn window_location_pathname_default() {
    let mut rt = make_runtime();
    let result = rt.eval("window.location.pathname").unwrap();
    let path = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(path, "/");
}

#[test]
fn console_log_stores_message() {
    let mut rt = make_runtime();
    rt.eval(r#"console.log("hello world")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output, vec!["hello world"]);
}

#[test]
fn console_warn_prefixes() {
    let mut rt = make_runtime();
    rt.eval(r#"console.warn("something bad")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output, vec!["WARN: something bad"]);
}

#[test]
fn console_error_prefixes() {
    let mut rt = make_runtime();
    rt.eval(r#"console.error("fatal")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output, vec!["ERROR: fatal"]);
}

#[test]
fn console_info_prefixes() {
    let mut rt = make_runtime();
    rt.eval(r#"console.info("note")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output, vec!["INFO: note"]);
}

#[test]
fn console_log_multiple_args_joined() {
    let mut rt = make_runtime();
    rt.eval(r#"console.log("a", "b", "c")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output, vec!["a b c"]);
}

#[test]
fn console_multiple_calls_accumulate() {
    let mut rt = make_runtime();
    rt.eval(r#"console.log("first")"#).unwrap();
    rt.eval(r#"console.log("second")"#).unwrap();
    rt.eval(r#"console.warn("third")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output, vec!["first", "second", "WARN: third"]);
}

#[test]
fn set_timeout_returns_numeric_id() {
    let mut rt = make_runtime();
    let result = rt.eval("window.setTimeout(function(){}, 100)").unwrap();
    assert!(result.is_number(), "setTimeout should return a number");
    let id = result.as_number().unwrap();
    assert!(id >= 1.0, "timer ID should be >= 1");
}

#[test]
fn set_interval_returns_numeric_id() {
    let mut rt = make_runtime();
    let result = rt.eval("window.setInterval(function(){}, 100)").unwrap();
    assert!(result.is_number(), "setInterval should return a number");
}

#[test]
fn set_timeout_ids_increment() {
    let mut rt = make_runtime();
    let r1 = rt.eval("window.setTimeout(function(){}, 100)").unwrap();
    let r2 = rt.eval("window.setTimeout(function(){}, 200)").unwrap();
    let id1 = r1.as_number().unwrap();
    let id2 = r2.as_number().unwrap();
    assert!(id2 > id1, "timer IDs should increment");
}

#[test]
fn clear_timeout_does_not_crash() {
    let mut rt = make_runtime();
    rt.eval("var id = window.setTimeout(function(){}, 100); window.clearTimeout(id)")
        .unwrap();
}

#[test]
fn clear_interval_does_not_crash() {
    let mut rt = make_runtime();
    rt.eval("var id = window.setInterval(function(){}, 100); window.clearInterval(id)")
        .unwrap();
}

#[test]
fn navigator_user_agent() {
    let mut rt = make_runtime();
    let result = rt.eval("window.navigator.userAgent").unwrap();
    let ua = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(ua, "Braille/0.1");
}

#[test]
fn console_output_accessible_from_runtime() {
    let mut rt = make_runtime();
    rt.eval(r#"console.log("from runtime")"#).unwrap();
    let output = rt.console_output();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0], "from runtime");
}

#[test]
fn on_animation_end_returns_null() {
    let mut rt = make_runtime();
    let result = rt
        .eval("var d = document.createElement('div'); d.onanimationend === null")
        .unwrap();
    assert_eq!(result.as_boolean(), Some(true), "onanimationend should be null on fresh div");
}

#[test]
fn on_animation_end_after_setting_prefixed() {
    let mut rt = make_runtime();
    let result = rt
        .eval(
            r#"
                var d = document.createElement('div');
                d.onwebkitanimationend = function(){};
                d.onanimationend === null
                "#,
        )
        .unwrap();
    assert_eq!(
        result.as_boolean(),
        Some(true),
        "onanimationend should still be null after setting onwebkitanimationend"
    );
}

#[test]
fn history_exists() {
    let mut rt = make_runtime();
    let result = rt.eval("typeof window.history === 'object'").unwrap();
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn history_push_state_updates_location() {
    let mut rt = make_runtime();
    rt.eval(r#"window.location.href = "https://example.com/page1""#)
        .unwrap();
    rt.eval(r#"window.history.pushState({page: 2}, "", "/page2")"#)
        .unwrap();
    let result = rt.eval("window.location.pathname").unwrap();
    let path = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(path, "/page2");
}

#[test]
fn history_replace_state() {
    let mut rt = make_runtime();
    rt.eval(r#"window.location.href = "https://example.com/page1""#)
        .unwrap();
    rt.eval(r#"window.history.replaceState({replaced: true}, "", "/replaced")"#)
        .unwrap();
    let result = rt.eval("window.location.pathname").unwrap();
    let path = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(path, "/replaced");
}

#[test]
fn history_length_increments() {
    let mut rt = make_runtime();
    rt.eval(r#"window.location.href = "https://example.com/""#)
        .unwrap();
    let len1 = rt.eval("window.history.length").unwrap();
    rt.eval(r#"window.history.pushState(null, "", "/page2")"#)
        .unwrap();
    let len2 = rt.eval("window.history.length").unwrap();
    assert_eq!(len1.as_number(), Some(1.0));
    assert_eq!(len2.as_number(), Some(2.0));
}

#[test]
fn history_state_getter() {
    let mut rt = make_runtime();
    rt.eval(r#"window.location.href = "https://example.com/""#)
        .unwrap();
    rt.eval(r#"window.history.pushState({myKey: "myVal"}, "", "/s")"#)
        .unwrap();
    let result = rt.eval("window.history.state.myKey").unwrap();
    let val = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(val, "myVal");
}

#[test]
fn history_back_fires_popstate() {
    let mut rt = make_runtime();
    rt.eval(
        r#"
            window.location.href = "https://example.com/";
            var popstateUrl = null;
            window.onpopstate = function(e) { popstateUrl = window.location.pathname; };
            window.history.pushState(null, "", "/page2");
            window.history.back();
        "#,
    )
    .unwrap();
    let result = rt.eval("popstateUrl").unwrap();
    let val = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
    assert_eq!(val, "/");
}

#[test]
fn history_forward_fires_popstate() {
    let mut rt = make_runtime();
    rt.eval(
        r#"
            window.location.href = "https://example.com/";
            var fwdState = null;
            window.onpopstate = function(e) { fwdState = e.state; };
            window.history.pushState({p: 2}, "", "/page2");
            window.history.back();
            window.history.forward();
        "#,
    )
    .unwrap();
    let result = rt.eval("fwdState && fwdState.p === 2").unwrap();
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn style_sheet_insert_rule_works() {
    let mut rt = make_runtime();
    let result = rt.eval(r#"
            var style = document.createElement('style');
            document.body.appendChild(style);
            var sheet = style.sheet;
            typeof sheet === 'object' && sheet !== null && typeof sheet.insertRule === 'function'
        "#).unwrap();
    assert_eq!(result.as_boolean(), Some(true), "style.sheet.insertRule should be a function");
}
