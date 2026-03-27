use std::collections::HashMap;

use braille_engine::{Engine, FetchedResources};

/// Helper: load HTML with fetched resources, return console output.
fn run_with_resources(html: &str, fetched: &FetchedResources) -> Vec<String> {
    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(html);
    engine.execute_scripts(&descriptors, fetched);
    engine.console_output()
}

/// Helper: load HTML with no external resources, return console output.
fn run(html: &str) -> Vec<String> {
    run_with_resources(html, &FetchedResources::default())
}

// ---------------------------------------------------------------------------
// 1. Basic inline <script type="module"> executes
// ---------------------------------------------------------------------------
#[test]
fn inline_module_executes() {
    let output = run(r#"
        <html><body>
        <script type="module">
            console.log("module-ran");
        </script>
        </body></html>
    "#);
    assert!(
        output.iter().any(|s| s.contains("module-ran")),
        "inline module should execute, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. import { greet } from './a.js' between pre-registered modules
// ---------------------------------------------------------------------------
#[test]
fn import_between_preregistered_modules() {
    let html = r#"
        <html><body>
        <script type="module" src="./a.js"></script>
        <script type="module">
            import { greet } from './a.js';
            console.log(greet("world"));
        </script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "./a.js".to_string(),
        "export function greet(name) { return 'hello ' + name; }".to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("hello world")),
        "should import and call greet, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Import map resolves bare specifiers
// ---------------------------------------------------------------------------
#[test]
fn import_map_bare_specifier() {
    let html = r#"
        <html><body>
        <script type="importmap">
        {
            "imports": {
                "my-lib": "./lib/my-lib.js"
            }
        }
        </script>
        <script type="module">
            import { x } from "my-lib";
            console.log("x=" + x);
        </script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "./lib/my-lib.js".to_string(),
        "export const x = 42;".to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("x=42")),
        "import map should resolve bare specifier, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. Dynamic import('./helper.js').then(...)
// ---------------------------------------------------------------------------
#[test]
fn dynamic_import() {
    let html = r#"
        <html><body>
        <script type="module" src="./helper.js"></script>
        <script type="module">
            import('./helper.js').then(m => {
                console.log("dynamic=" + m.value);
            });
        </script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "./helper.js".to_string(),
        "export const value = 99;".to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("dynamic=99")),
        "dynamic import should resolve, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. Top-level await in module
// ---------------------------------------------------------------------------
#[test]
fn top_level_await() {
    let output = run(r#"
        <html><body>
        <script type="module">
            const val = await Promise.resolve(7);
            console.log("tla=" + val);
        </script>
        </body></html>
    "#);
    assert!(
        output.iter().any(|s| s.contains("tla=7")),
        "top-level await should work in modules, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. Module scope isolation (var doesn't leak to global)
// ---------------------------------------------------------------------------
#[test]
fn module_scope_isolation() {
    let output = run(r#"
        <html><body>
        <script type="module">
            var secretVar = "inside-module";
        </script>
        <script>
            console.log("leaked=" + (typeof secretVar));
        </script>
        </body></html>
    "#);
    assert!(
        output.iter().any(|s| s.contains("leaked=undefined")),
        "module vars should not leak to global, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 7. External module script (<script type="module" src="...">) with fetched content
// ---------------------------------------------------------------------------
#[test]
fn external_module_script() {
    let html = r#"
        <html><body>
        <script type="module" src="https://cdn.example.com/app.js"></script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "https://cdn.example.com/app.js".to_string(),
        r#"console.log("external-module-ok");"#.to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("external-module-ok")),
        "external module src should execute, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 8. Module dependency chain: A exports -> B imports from A -> inline imports from B
// ---------------------------------------------------------------------------
#[test]
fn module_dependency_chain() {
    let html = r#"
        <html><body>
        <script type="module" src="./a.js"></script>
        <script type="module" src="./b.js"></script>
        <script type="module">
            import { doubled } from './b.js';
            console.log("chain=" + doubled);
        </script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "./a.js".to_string(),
        "export const base = 5;".to_string(),
    );
    scripts.insert(
        "./b.js".to_string(),
        "import { base } from './a.js'; export const doubled = base * 2;".to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("chain=10")),
        "dependency chain should resolve, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 9. Multiple import map entries
// ---------------------------------------------------------------------------
#[test]
fn multiple_import_map_entries() {
    let html = r#"
        <html><body>
        <script type="importmap">
        {
            "imports": {
                "utils": "./lib/utils.js",
                "config": "./lib/config.js"
            }
        }
        </script>
        <script type="module">
            import { add } from "utils";
            import { multiplier } from "config";
            console.log("result=" + add(3, 4) * multiplier);
        </script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "./lib/utils.js".to_string(),
        "export function add(a, b) { return a + b; }".to_string(),
    );
    scripts.insert(
        "./lib/config.js".to_string(),
        "export const multiplier = 10;".to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("result=70")),
        "multiple import map entries should work, got: {output:?}"
    );
}

// ---------------------------------------------------------------------------
// 10. Re-exports: export { x } from './base.js'
// ---------------------------------------------------------------------------
#[test]
fn reexports() {
    let html = r#"
        <html><body>
        <script type="module" src="./base.js"></script>
        <script type="module" src="./reexporter.js"></script>
        <script type="module">
            import { x } from './reexporter.js';
            console.log("reexport=" + x);
        </script>
        </body></html>
    "#;
    let mut scripts = HashMap::new();
    scripts.insert(
        "./base.js".to_string(),
        "export const x = 'from-base';".to_string(),
    );
    scripts.insert(
        "./reexporter.js".to_string(),
        "export { x } from './base.js';".to_string(),
    );
    let fetched = FetchedResources::scripts_only(scripts);
    let output = run_with_resources(html, &fetched);
    assert!(
        output.iter().any(|s| s.contains("reexport=from-base")),
        "re-exports should work, got: {output:?}"
    );
}
