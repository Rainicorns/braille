//! ScriptDescriptor / external script execution tests.

use std::collections::HashMap;

use braille_engine::{Engine, FetchedResources, ScriptDescriptor};
use braille_wire::SnapMode;

#[test]
fn test_parse_and_collect_scripts_identifies_inline() {
    let html = r#"
    <html><body>
      <script>console.log("hello")</script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);

    assert_eq!(descriptors.len(), 1);
    match &descriptors[0] {
        ScriptDescriptor::Inline(text, _) => {
            assert!(text.contains("console.log"), "inline script text: {}", text);
        }
        other => panic!("expected Inline, got {:?}", other),
    }
}

#[test]
fn test_parse_and_collect_scripts_identifies_external() {
    let html = r#"
    <html><body>
      <script src="https://example.com/app.js"></script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);

    assert_eq!(descriptors.len(), 1);
    match &descriptors[0] {
        ScriptDescriptor::External(url, _) => {
            assert_eq!(url, "https://example.com/app.js");
        }
        other => panic!("expected External, got {:?}", other),
    }
}

#[test]
fn test_parse_and_collect_scripts_mixed_document_order() {
    let html = r#"
    <html><body>
      <script>var x = 1;</script>
      <script src="https://cdn.example.com/lib.js"></script>
      <script>var y = 2;</script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);

    assert_eq!(descriptors.len(), 3, "should find 3 scripts");

    match &descriptors[0] {
        ScriptDescriptor::Inline(text, _) => assert!(text.contains("var x = 1")),
        _ => panic!("first script should be Inline"),
    }
    match &descriptors[1] {
        ScriptDescriptor::External(url, _) => assert_eq!(url, "https://cdn.example.com/lib.js"),
        _ => panic!("second script should be External"),
    }
    match &descriptors[2] {
        ScriptDescriptor::Inline(text, _) => assert!(text.contains("var y = 2")),
        _ => panic!("third script should be Inline"),
    }
}

#[test]
fn test_execute_scripts_runs_inline() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script>
        let el = document.createElement("p");
        el.textContent = "inline works";
        document.getElementById("target").appendChild(el);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);
    let fetched = HashMap::new();
    engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("inline works"),
        "inline script should execute: {}",
        snapshot
    );
}

#[test]
fn test_execute_scripts_runs_external_from_fetched() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script src="https://example.com/app.js"></script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);

    let mut fetched = HashMap::new();
    fetched.insert(
        "https://example.com/app.js".to_string(),
        concat!(
            "let el = document.createElement(\"p\");",
            "el.textContent = \"external works\";",
            "document.getElementById(\"target\").appendChild(el);"
        )
        .to_string(),
    );

    engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("external works"),
        "external script should execute: {}",
        snapshot
    );
}

#[test]
fn test_execute_scripts_skips_missing_external() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script src="https://example.com/missing.js"></script>
      <script>
        let el = document.createElement("p");
        el.textContent = "after missing";
        document.getElementById("target").appendChild(el);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);
    let fetched = HashMap::new();
    engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("after missing"),
        "inline script after missing external should run: {}",
        snapshot
    );
}

#[test]
fn test_load_html_with_scripts_end_to_end() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script src="https://example.com/lib.js"></script>
      <script>
        let el = document.createElement("p");
        el.textContent = "value is " + globalValue;
        document.getElementById("target").appendChild(el);
      </script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert(
        "https://example.com/lib.js".to_string(),
        "var globalValue = 42;".to_string(),
    );

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("value is 42"),
        "external script should set global used by inline: {}",
        snapshot
    );
}

#[test]
fn test_mixed_inline_and_external_execute_in_order() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script>var order = [];</script>
      <script src="https://example.com/a.js"></script>
      <script>order.push("inline2");</script>
      <script src="https://example.com/b.js"></script>
      <script>
        let el = document.createElement("p");
        el.textContent = order.join(",");
        document.getElementById("target").appendChild(el);
      </script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert(
        "https://example.com/a.js".to_string(),
        "order.push(\"extA\");".to_string(),
    );
    fetched.insert(
        "https://example.com/b.js".to_string(),
        "order.push(\"extB\");".to_string(),
    );

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("extA,inline2,extB"),
        "scripts should execute in document order: {}",
        snapshot
    );
}

#[test]
fn test_script_with_src_and_text_src_wins() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script src="https://example.com/real.js">
        let bad = document.createElement("p");
        bad.textContent = "INLINE SHOULD NOT RUN";
        document.getElementById("target").appendChild(bad);
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);

    assert_eq!(descriptors.len(), 1);
    match &descriptors[0] {
        ScriptDescriptor::External(url, _) => {
            assert_eq!(url, "https://example.com/real.js");
        }
        other => panic!("should be External when src is present, got {:?}", other),
    }

    let mut fetched = HashMap::new();
    fetched.insert(
        "https://example.com/real.js".to_string(),
        concat!(
            "let el = document.createElement(\"p\");",
            "el.textContent = \"EXTERNAL RAN\";",
            "document.getElementById(\"target\").appendChild(el);"
        )
        .to_string(),
    );

    engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("EXTERNAL RAN"),
        "external content should run: {}",
        snapshot
    );
    assert!(
        !snapshot.contains("INLINE SHOULD NOT RUN"),
        "inline text should be ignored when src present: {}",
        snapshot
    );
}
