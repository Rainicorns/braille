use braille_engine::Engine;

#[test]
fn detached_arraybuffer_property_access() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html><html><body><script>
var buf = new ArrayBuffer(16);
var view = new Uint8Array(buf);
view[0] = 42;
buf.transfer();

var tests = [
    ["view.byteLength", function() { return view.byteLength; }],
    ["view.length", function() { return view.length; }],
    ["view.buffer", function() { return typeof view.buffer; }],
    ["view.buffer.byteLength", function() { return view.buffer.byteLength; }],
    ["view[0]", function() { return view[0]; }],
    ["Array.from(view)", function() { return Array.from(view).length; }],
];

tests.forEach(function(t) {
    try {
        var r = t[1]();
        console.log("OK " + t[0] + " = " + JSON.stringify(r));
    } catch(e) {
        console.log("THROW " + t[0] + " = " + e.message);
    }
});
</script></body></html>"#,
    );
    engine.settle();
    for line in engine.drain_console() {
        eprintln!("  {}", line);
    }
}

#[test]
fn detached_arraybuffer_transfer_twice() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html><html><body><script>
// Simulate the WPT pattern: transfer called via getter on the view's own buffer
var view = new Uint8Array([1, 2, 3, 4]);

var algo = {
    get name() {
        console.log("getter called, transferring...");
        view.buffer.transfer();
        console.log("transfer done");
        return "SHA3-256";
    }
};

// This is what digest() does internally
try {
    var h = algo.name; // triggers getter, detaches buffer
    console.log("algo name: " + h);
    console.log("view.length after detach: " + view.length);
    console.log("view.byteLength after detach: " + view.byteLength);
    var arr = [];
    for (var i = 0; i < view.length; i++) arr.push(view[i]);
    console.log("manual iteration result length: " + arr.length);
} catch(e) {
    console.log("THROW: " + e.message);
}

// Test: transfer() on a buffer obtained via .buffer property
var view2 = new Uint8Array([10, 20, 30]);
try {
    var newBuf = view2.buffer.transfer();
    console.log("transfer via .buffer OK, new byteLength: " + newBuf.byteLength);
    console.log("view2.length after: " + view2.length);
} catch(e) {
    console.log("transfer via .buffer THROW: " + e.message);
}

// Test: transfer() on ArrayBuffer that has a view
var buf3 = new ArrayBuffer(8);
var view3 = new Uint8Array(buf3);
view3[0] = 99;
try {
    var newBuf3 = buf3.transfer();
    console.log("transfer with view OK, new byteLength: " + newBuf3.byteLength);
    console.log("view3.length after: " + view3.length);
} catch(e) {
    console.log("transfer with view THROW: " + e.message);
}
</script></body></html>"#,
    );
    engine.settle();
    for line in engine.drain_console() {
        eprintln!("  {}", line);
    }
}
