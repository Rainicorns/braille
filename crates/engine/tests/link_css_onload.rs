//! Tests for <link> stylesheet/preload onload firing.
//! Sites gate content visibility on CSS load callbacks (e.g. ifixit's deferCss).
//! The engine doesn't fetch CSS, but must fire onload so these gates open.

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

#[test]
fn link_stylesheet_onload_fires() {
    let mut e = engine_with_html(r#"<html><head>
        <link rel="stylesheet" href="https://example.com/style.css"
              onload="document.body.setAttribute('data-css', 'loaded')">
    </head><body></body></html>"#);
    let result = e.eval_js("document.body.getAttribute('data-css')").unwrap();
    eprintln!("link_stylesheet_onload_fires: {}", result);
    assert_eq!(result, "loaded");
}

#[test]
fn link_preload_onload_fires() {
    let mut e = engine_with_html(r#"<html><head>
        <link rel="preload" as="style" href="https://example.com/style.css"
              onload="document.body.setAttribute('data-css', 'preloaded')">
    </head><body></body></html>"#);
    let result = e.eval_js("document.body.getAttribute('data-css')").unwrap();
    eprintln!("link_preload_onload_fires: {}", result);
    assert_eq!(result, "preloaded");
}

#[test]
fn link_onload_removes_hiding_style() {
    // Simulates the ifixit deferCss pattern:
    // A <style> hides content with display:none, and link onload removes it.
    let mut e = engine_with_html(r#"<html><head>
        <style id="hider">.hidden-until-css { display: none !important; }</style>
        <script>
            var cssReady = false;
            function cssLoaded() {
                cssReady = true;
                var hider = document.getElementById('hider');
                if (hider) hider.parentElement.removeChild(hider);
            }
        </script>
        <link rel="preload" as="style" href="https://example.com/style.css"
              onload="cssLoaded()">
    </head><body>
        <div class="hidden-until-css" id="content">Teardown listings here</div>
    </body></html>"#);
    let css_ready = e.eval_js("String(cssReady)").unwrap();
    let hider_exists = e.eval_js("String(!!document.getElementById('hider'))").unwrap();
    eprintln!("cssReady={} hider_exists={}", css_ready, hider_exists);
    assert_eq!(css_ready, "true", "cssLoaded() should have been called");
    assert_eq!(hider_exists, "false", "hider style element should be removed");
}

#[test]
fn link_js_onload_property_fires() {
    // Test that link.onload set via JS (not HTML attribute) also fires
    let mut e = engine_with_html(r#"<html><head>
        <script>
            var loaded = false;
            document.addEventListener('DOMContentLoaded', function() {
                // This won't work because link onload should fire BEFORE DOMContentLoaded
            });
        </script>
    </head><body>
        <script>
            var link = document.createElement('link');
            link.rel = 'stylesheet';
            link.href = 'https://example.com/dynamic.css';
            link.onload = function() { loaded = true; };
            document.head.appendChild(link);
        </script>
    </body></html>"#);
    let result = e.eval_js("String(loaded)").unwrap();
    eprintln!("link_js_onload_property_fires: {}", result);
    assert_eq!(result, "true");
}
