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
fn size_vs_content() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let common_js = std::fs::read_to_string(root.join("tests/wpt/dom/common.js")).unwrap();
    let lines: Vec<&str> = common_js.lines().collect();

    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // Split common.js into two halves and eval separately
    let half = lines.len() / 2;
    let mut engine = make_engine_with_harness();
    let part1 = lines[..half].join("\n");
    let part2 = lines[half..].join("\n");
    engine.eval_js(&part1).ok();
    engine.eval_js(&part2).ok();
    let r1 = engine.eval_js(test_code);
    eprintln!("two halves: {:?}", r1);

    // Split into 4 quarters
    let q1 = lines.len() / 4;
    let q2 = lines.len() / 2;
    let q3 = 3 * lines.len() / 4;
    let mut engine2 = make_engine_with_harness();
    engine2.eval_js(&lines[..q1].join("\n")).ok();
    engine2.eval_js(&lines[q1..q2].join("\n")).ok();
    engine2.eval_js(&lines[q2..q3].join("\n")).ok();
    engine2.eval_js(&lines[q3..].join("\n")).ok();
    let r2 = engine2.eval_js(test_code);
    eprintln!("four quarters: {:?}", r2);

    // Full common.js in one eval (broken)
    let mut engine3 = make_engine_with_harness();
    engine3.eval_js(&common_js).ok();
    let r3 = engine3.eval_js(test_code);
    eprintln!("single eval: {:?}", r3);

    // Now test: same size, but padded with comments instead
    let pad_lines = 1089;
    let mut padding = String::new();
    for i in 0..pad_lines {
        padding.push_str(&format!("// padding line {}\n", i));
    }
    padding.push_str("var __padded = true;\n");
    let mut engine4 = make_engine_with_harness();
    engine4.eval_js(&padding).unwrap();
    let r4 = engine4.eval_js(test_code);
    eprintln!("padded comments (same line count): {:?}", r4);

    // Try: common.js content but without the setup() call
    let mut no_setup = common_js.clone();
    no_setup = no_setup.replace(
        "if (\"setup\" in window) {\n    setup(setupRangeTests);\n} else {\n    // Presumably we're running from within an iframe or something\n    setupRangeTests();\n}",
        "// setup call removed\nsetupRangeTests();"
    );
    let mut engine5 = make_engine_with_harness();
    engine5.eval_js(&no_setup).ok();
    let r5 = engine5.eval_js(test_code);
    eprintln!("without setup() wrapper: {:?}", r5);

    // Try: skip setupRangeTests entirely
    let mut no_init = common_js.clone();
    no_init = no_init.replace(
        "if (\"setup\" in window) {\n    setup(setupRangeTests);\n} else {\n    // Presumably we're running from within an iframe or something\n    setupRangeTests();\n}",
        "// skipped init"
    );
    let mut engine6 = make_engine_with_harness();
    engine6.eval_js(&no_init).ok();
    let r6 = engine6.eval_js(test_code);
    eprintln!("without any init: {:?}", r6);
}
