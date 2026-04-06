use braille_engine::Engine;
use std::collections::HashMap;

#[test]
fn debug_new_document_dispatch() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html>
<html><head></head><body>
<table id="table" border="1">
    <tbody id="table-body">
    <tr id="table-row">
        <td id="table-cell">Shady Grove</td>
        <td>Aeolian</td>
    </tr>
    <tr id="parent">
        <td id="target">Over the river, Charlie</td>
        <td>Dorian</td>
    </tr>
    </tbody>
</table>
</body></html>"#);

    let result = engine.eval_js(r#"
        var newDocument = new Document();
        var cloned = document.documentElement.cloneNode(true);
        newDocument.appendChild(cloned);

        var de = newDocument.documentElement;
        var body = de ? (function() {
            var kids = de.childNodes || [];
            for (var i = 0; i < kids.length; i++) {
                if (kids[i].tagName === 'BODY') return kids[i];
            }
            return null;
        })() : null;

        var table = newDocument.getElementById ? newDocument.getElementById("table") : null;

        JSON.stringify({
            hasDE: !!de,
            deTag: de ? de.tagName : null,
            hasBody: !!body,
            bodyTag: body ? body.tagName : null,
            hasGetElementById: typeof (newDocument.getElementById),
            hasTable: !!table,
            tableTag: table ? table.tagName : null,
        })
    "#).unwrap();
    eprintln!("Result: {}", result);

    // Now test event dispatch
    let result2 = engine.eval_js(r#"
        var newDocument = new Document();
        newDocument.appendChild(document.documentElement.cloneNode(true));

        var target = newDocument.getElementById("target");
        var count = 0;
        if (target) {
            target.addEventListener("click", function() { count++; }, true);
            target.addEventListener("click", function() { count++; }, false);
            var evt = newDocument.createEvent("Event");
            evt.initEvent("click", false, true);
            target.dispatchEvent(evt);
        }
        JSON.stringify({hasTarget: !!target, count: count})
    "#).unwrap();
    eprintln!("Result2: {}", result2);
}

/// Simulate the WPT runner path for Event-dispatch-bubbles-false.html
#[test]
fn debug_bubbles_false_via_runner() {
    let html = r#"<!DOCTYPE html>
<meta charset=utf-8>
<title> Event.bubbles attribute is set to false </title>
<script src="/resources/testharness.js"></script>
<script src="/resources/testharnessreport.js"></script>
<div id=log></div>
<table id="table" border="1" style="display: none">
    <tbody id="table-body">
    <tr id="table-row">
        <td id="table-cell">Shady Grove</td>
        <td>Aeolian</td>
    </tr>
    <tr id="parent">
        <td id="target">Over the river, Charlie</td>
        <td>Dorian</td>
    </tr>
    </tbody>
</table>
<script>
function targetsForDocumentChain(document) {
    return [
        document,
        document.documentElement,
        document.getElementsByTagName("body")[0],
        document.getElementById("table"),
        document.getElementById("table-body"),
        document.getElementById("parent")
    ];
}

function testChain(document, targetParents, phases, event_type) {
    var target = document.getElementById("target");
    var targets = targetParents.concat(target);
    var expected_targets = targets.concat(target);

    var actual_targets = [], actual_phases = [];
    var test_event = function(evt) {
        actual_targets.push(evt.currentTarget);
        actual_phases.push(evt.eventPhase);
    }

    for (var i = 0; i < targets.length; i++) {
        targets[i].addEventListener(event_type, test_event, true);
        targets[i].addEventListener(event_type, test_event, false);
    }

    var evt = document.createEvent("Event");
    evt.initEvent(event_type, false, true);

    target.dispatchEvent(evt);

    assert_array_equals(actual_targets, expected_targets, "targets");
    assert_array_equals(actual_phases, phases, "phases");
}

var phasesForDocumentChain = [
    Event.CAPTURING_PHASE,
    Event.CAPTURING_PHASE,
    Event.CAPTURING_PHASE,
    Event.CAPTURING_PHASE,
    Event.CAPTURING_PHASE,
    Event.CAPTURING_PHASE,
    Event.AT_TARGET,
    Event.AT_TARGET,
];

test(function () {
    var chainWithWindow = [window].concat(targetsForDocumentChain(document));
    testChain(
        document, chainWithWindow, [Event.CAPTURING_PHASE].concat(phasesForDocumentChain), "click");
}, "In window.document with click event");

test(function () {
    testChain(document, targetsForDocumentChain(document), phasesForDocumentChain, "load");
}, "In window.document with load event");

test(function () {
    var documentClone = document.cloneNode(true);
    testChain(
        documentClone, targetsForDocumentChain(documentClone), phasesForDocumentChain, "click");
}, "In window.document.cloneNode(true)");

test(function () {
    var newDocument = new Document();
    newDocument.appendChild(document.documentElement.cloneNode(true));
    testChain(
        newDocument, targetsForDocumentChain(newDocument), phasesForDocumentChain, "click");
}, "In new Document()");

test(function () {
    var HTMLDocument = document.implementation.createHTMLDocument();
    HTMLDocument.body.appendChild(document.getElementById("table").cloneNode(true));
    testChain(
        HTMLDocument, targetsForDocumentChain(HTMLDocument), phasesForDocumentChain, "click");
}, "In DOMImplementation.createHTMLDocument()");
globalThis.__diag = 'done';
</script>"#;

    // Inline a minimal WPT test harness
    let preamble = r#"
(function() {
    var results = [];
    self.test = function(fn, name) {
        try { fn(); results.push({name: name, status: 0}); }
        catch(e) { results.push({name: name, status: 1, message: String(e).substring(0,200)}); }
    };
    self.assert_array_equals = function(actual, expected, desc) {
        if (actual.length !== expected.length)
            throw new Error(desc + ': length ' + actual.length + ' !== ' + expected.length);
        for (var i = 0; i < actual.length; i++) {
            if (actual[i] !== expected[i])
                throw new Error(desc + '[' + i + ']: ' + actual[i] + ' !== ' + expected[i]);
        }
    };
    self.__wpt_get_results = function() { return results; };
})();
"#;
    let mut scripts = HashMap::new();
    scripts.insert("/resources/testharness.js".to_string(), preamble.to_string());
    scripts.insert("/resources/testharnessreport.js".to_string(), String::new());

    let mut engine = Engine::new();
    let errors = engine.load_html_with_scripts_lossy(html, &scripts);
    for e in &errors {
        eprintln!("JS error: {}", e);
    }
    engine.settle();

    let diag = engine.eval_js("globalThis.__diag").unwrap_or_default();
    eprintln!("Diag: {}", diag);
    let results = engine.eval_js("JSON.stringify(__wpt_get_results())").unwrap_or_default();
    eprintln!("Results count: {}", results.matches("\"status\"").count());
    eprintln!("Results: {}", &results[..std::cmp::min(2000, results.len())]);
}
