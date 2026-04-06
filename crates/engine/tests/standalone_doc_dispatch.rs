use braille_engine::Engine;

#[test]
fn standalone_doc_event_dispatch_targets() {
    let html = r#"<!DOCTYPE html>
<table id="table"><tbody id="table-body">
<tr id="table-row"><td id="table-cell">Shady Grove</td><td>Aeolian</td></tr>
<tr id="parent"><td id="target">Over the river</td><td>Dorian</td></tr>
</tbody></table>"#;
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();
    let result = engine.eval_js(r#"
(function() {
    var newDoc = new Document();
    newDoc.appendChild(document.documentElement.cloneNode(true));

    var targets = [
        newDoc,
        newDoc.documentElement,
        newDoc.getElementsByTagName("body")[0],
        newDoc.getElementById("table"),
        newDoc.getElementById("table-body"),
        newDoc.getElementById("parent")
    ];
    var target = newDoc.getElementById("target");
    targets.push(target);

    var actual_targets = [];
    for (var i = 0; i < targets.length; i++) {
        if (!targets[i]) { return "null target at index " + i; }
        targets[i].addEventListener("click", function(e) {
            actual_targets.push(e.currentTarget);
        }, true);
        targets[i].addEventListener("click", function(e) {
            actual_targets.push(e.currentTarget);
        }, false);
    }

    var evt = newDoc.createEvent("Event");
    evt.initEvent("click", true, true);
    target.dispatchEvent(evt);

    var expected = [];
    for (var i = 0; i < targets.length; i++) expected.push(targets[i]);
    for (var i = targets.length - 1; i >= 0; i--) expected.push(targets[i]);

    if (actual_targets.length !== expected.length) {
        var names = [];
        for (var j = 0; j < actual_targets.length; j++) {
            names.push(actual_targets[j] ? (actual_targets[j].tagName || actual_targets[j].nodeName || '?') : 'null');
        }
        return "Length mismatch: expected " + expected.length + " got " + actual_targets.length + " [" + names.join(", ") + "]";
    }
    for (var i = 0; i < expected.length; i++) {
        if (actual_targets[i] !== expected[i]) {
            var aTag = actual_targets[i] ? (actual_targets[i].tagName || actual_targets[i].nodeName || '?') : 'null';
            var eTag = expected[i] ? (expected[i].tagName || expected[i].nodeName || '?') : 'null';
            return "Mismatch at index " + i + ": expected " + eTag + " got " + aTag;
        }
    }
    return "PASS";
})()
    "#);
    eprintln!("Result: {:?}", result);
    assert_eq!(result.unwrap(), "PASS");
}

#[test]
fn cloned_doc_event_dispatch_targets() {
    let html = r#"<!DOCTYPE html>
<table id="table"><tbody id="table-body">
<tr id="table-row"><td id="table-cell">Shady Grove</td><td>Aeolian</td></tr>
<tr id="parent"><td id="target">Over the river</td><td>Dorian</td></tr>
</tbody></table>"#;
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();
    let result = engine.eval_js(r#"
(function() {
    var docClone = document.cloneNode(true);

    var targets = [
        docClone,
        docClone.documentElement,
        docClone.getElementsByTagName("body")[0],
        docClone.getElementById("table"),
        docClone.getElementById("table-body"),
        docClone.getElementById("parent")
    ];
    var target = docClone.getElementById("target");
    targets.push(target);

    var actual_targets = [];
    for (var i = 0; i < targets.length; i++) {
        if (!targets[i]) { return "null target at index " + i; }
        targets[i].addEventListener("click", function(e) {
            actual_targets.push(e.currentTarget);
        }, true);
        targets[i].addEventListener("click", function(e) {
            actual_targets.push(e.currentTarget);
        }, false);
    }

    var evt = docClone.createEvent("Event");
    evt.initEvent("click", true, true);
    target.dispatchEvent(evt);

    var expected = [];
    for (var i = 0; i < targets.length; i++) expected.push(targets[i]);
    for (var i = targets.length - 1; i >= 0; i--) expected.push(targets[i]);

    if (actual_targets.length !== expected.length) {
        var names = [];
        for (var j = 0; j < actual_targets.length; j++) {
            names.push(actual_targets[j] ? (actual_targets[j].tagName || actual_targets[j].nodeName || '?') : 'null');
        }
        return "Length mismatch: expected " + expected.length + " got " + actual_targets.length + " [" + names.join(", ") + "]";
    }
    for (var i = 0; i < expected.length; i++) {
        if (actual_targets[i] !== expected[i]) {
            var aTag = actual_targets[i] ? (actual_targets[i].tagName || actual_targets[i].nodeName || '?') : 'null';
            var eTag = expected[i] ? (expected[i].tagName || expected[i].nodeName || '?') : 'null';
            return "Mismatch at index " + i + ": expected " + eTag + " got " + aTag;
        }
    }
    return "PASS";
})()
    "#);
    eprintln!("Result: {:?}", result);
    assert_eq!(result.unwrap(), "PASS");
}
