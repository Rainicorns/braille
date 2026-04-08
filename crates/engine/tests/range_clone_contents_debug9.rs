use braille_engine::Engine;

fn make_engine_with_harness() -> Engine {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let harness_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharness.js")).unwrap();
    let report_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharnessreport.js")).unwrap();

    let mut engine = Engine::new();
    engine.load_html("<!doctype html><div id=log></div><body></body>");
    engine.eval_js(&harness_js).unwrap();
    engine.eval_js(&report_js).unwrap();
    engine
}

#[test]
fn progressive_setup_range_tests() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // From setupRangeTests, progressively add chunks
    let chunks = [
        // Chunk 0: basic div creation
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
        });"#,

        // Chunk 1: add paragraphs
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var p = document.createElement("p");
            p.textContent = "hello";
            testDiv.appendChild(p);
        });"#,

        // Chunk 2: add foreignDoc
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var foreignDoc = document.implementation.createHTMLDocument("");
        });"#,

        // Chunk 3: foreignDoc + createElement
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var foreignDoc = document.implementation.createHTMLDocument("");
            var foreignPara1 = foreignDoc.createElement("p");
            foreignPara1.appendChild(foreignDoc.createTextNode("Efghijkl"));
            foreignDoc.body.appendChild(foreignPara1);
        });"#,

        // Chunk 4: xmlDoc
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var xmlDoctype = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
            var xmlDoc = document.implementation.createDocument(null, null, xmlDoctype);
        });"#,

        // Chunk 5: xmlDoc + createElement on xmlDoc
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var xmlDoctype = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
            var xmlDoc = document.implementation.createDocument(null, null, xmlDoctype);
            var xmlElement = xmlDoc.createElement("igiveuponcreativenames");
        });"#,

        // Chunk 6: xmlDoc + appendChild on xmlDoc
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var xmlDoctype = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
            var xmlDoc = document.implementation.createDocument(null, null, xmlDoctype);
            var xmlElement = xmlDoc.createElement("igiveuponcreativenames");
            xmlDoc.appendChild(xmlElement);
        });"#,

        // Chunk 7: new Document() + createCDATASection
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var p = document.createElement("p");
            var xmlDocument = new Document();
            p.appendChild(xmlDocument.createCDATASection("1234"));
            testDiv.appendChild(p);
        });"#,

        // Chunk 8: createRange inside setup
        r#"setup(function() {
            document.createRange();
        });"#,

        // Chunk 9: full testRanges array with eval
        r#"setup(function() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
            var paras = [];
            paras.push(document.createElement("p"));
            paras[0].setAttribute("id", "a");
            paras[0].textContent = "Abcdefgh\n";
            testDiv.appendChild(paras[0]);
            paras.push(document.createElement("p"));
            paras[1].setAttribute("id", "b");
            paras[1].textContent = "Ijklmnop\n";
            testDiv.appendChild(paras[1]);
        });"#,
    ];

    for (i, chunk) in chunks.iter().enumerate() {
        let mut engine = make_engine_with_harness();
        let r = engine.eval_js(chunk);
        if let Err(e) = &r {
            eprintln!("Chunk {} SETUP ERROR: {}", i, &e[..e.len().min(100)]);
            continue;
        }
        let r2 = engine.eval_js(test_code);
        if let Err(e) = &r2 {
            eprintln!("Chunk {} BREAKS test: {}", i, &e[..e.len().min(100)]);
        } else {
            eprintln!("Chunk {} OK", i);
        }
    }
}
