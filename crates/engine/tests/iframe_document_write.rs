use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine
}

#[test]
fn iframe_document_write_executes_scripts() {
    let mut engine = engine_with_html(r#"<html><head></head><body></body></html>"#);

    let result = engine.eval_js(r#"
        (function() {
            var iframe = document.createElement('iframe');
            document.body.appendChild(iframe);

            var src = '<!DOCTYPE HTML><html><head></head><body>' +
                '<script>window.__testResult = 42;</' + 'script>' +
                '</body></html>';

            iframe.contentDocument.write(src);
            iframe.contentDocument.close();

            return String(iframe.contentWindow.__testResult);
        })()
    "#).unwrap_or_default();
    eprintln!("iframe_document_write_executes_scripts result: {:?}", result);
    assert_eq!(result.trim_matches('"'), "42");
}

#[test]
fn iframe_document_write_postmessage_to_parent() {
    let mut engine = engine_with_html(r#"<html><head></head><body></body></html>"#);

    let result = engine.eval_js(r#"
        (function() {
            var iframe = document.createElement('iframe');
            document.body.appendChild(iframe);

            var src = '<!DOCTYPE HTML><html><head></head><body>' +
                '<script>window.parent.postMessage("hello from iframe", "*");</' + 'script>' +
                '</body></html>';

            iframe.contentDocument.write(src);
            iframe.contentDocument.close();

            return "wrote";
        })()
    "#).unwrap_or_default();
    eprintln!("iframe_document_write_postmessage result: {:?}", result);
    assert!(result.contains("wrote"));
}

#[test]
fn iframe_document_open_write_close() {
    let mut engine = engine_with_html(r#"<html><head></head><body></body></html>"#);

    let result = engine.eval_js(r#"
        (function() {
            var iframe = document.createElement('iframe');
            document.body.appendChild(iframe);
            var doc = iframe.contentDocument;

            doc.open();
            doc.write('<html><body><script>window.__val = 1;</' + 'script></body></html>');
            doc.close();

            return String(iframe.contentWindow.__val);
        })()
    "#).unwrap_or_default();
    eprintln!("iframe_document_open_write_close result: {:?}", result);
    assert_eq!(result.trim_matches('"'), "1");
}
