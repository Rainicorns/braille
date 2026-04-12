use std::collections::HashMap;
use braille_engine::{Engine, FetchedResources};

#[test]
fn iframe_loads_external_script_from_fetched_resources() {
    let mut engine = Engine::new();

    let html = r#"<!DOCTYPE html>
<html><head></head>
<body>
<iframe id="child" src="child.html"></iframe>
<script>
    // After iframe loads, check if the external script ran
</script>
</body></html>"#;

    let child_html = r#"<!DOCTYPE html>
<html><head>
<script src="/lib.js"></script>
</head>
<body>
<script>
    // lib.js should have set window.libLoaded = true
    if (typeof libLoaded !== 'undefined' && libLoaded === true) {
        document.title = 'SUCCESS';
    } else {
        document.title = 'FAIL: libLoaded=' + (typeof libLoaded);
    }
</script>
</body></html>"#;

    let lib_js = "self.libLoaded = true;";

    let mut scripts = HashMap::new();
    scripts.insert("/lib.js".to_string(), lib_js.to_string());

    let mut iframes = HashMap::new();
    iframes.insert("child.html".to_string(), child_html.to_string());

    let resources = FetchedResources {
        scripts,
        iframes,
        css: HashMap::new(),
    };

    let errors = engine.load_html_with_resources_lossy(html, &resources);
    eprintln!("JS errors: {:?}", errors);

    // Check that the child iframe's external script executed
    let result = engine.eval_js(
        "document.querySelector('iframe').contentDocument.title"
    ).unwrap_or_default();
    eprintln!("Child iframe title: {}", result);
    assert_eq!(result, "SUCCESS");
}

#[test]
fn iframe_external_and_inline_scripts_execute_in_order() {
    let mut engine = Engine::new();

    let html = r#"<!DOCTYPE html>
<html><body>
<iframe id="child" src="child.html"></iframe>
</body></html>"#;

    let child_html = r#"<!DOCTYPE html>
<html><body>
<script src="/first.js"></script>
<script>window.order.push('inline1');</script>
<script src="/second.js"></script>
<script>window.order.push('inline2');</script>
</body></html>"#;

    let mut scripts = HashMap::new();
    scripts.insert("/first.js".to_string(), "window.order = ['ext1'];".to_string());
    scripts.insert("/second.js".to_string(), "window.order.push('ext2');".to_string());

    let mut iframes = HashMap::new();
    iframes.insert("child.html".to_string(), child_html.to_string());

    let resources = FetchedResources {
        scripts,
        iframes,
        css: HashMap::new(),
    };

    let errors = engine.load_html_with_resources_lossy(html, &resources);
    eprintln!("JS errors: {:?}", errors);

    let result = engine.eval_js(
        "JSON.stringify(document.querySelector('iframe').contentWindow.order)"
    ).unwrap_or_default();
    eprintln!("Script execution order: {}", result);
    assert_eq!(result, r#"["ext1","inline1","ext2","inline2"]"#);
}
