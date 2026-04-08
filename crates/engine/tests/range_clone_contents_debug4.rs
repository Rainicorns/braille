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
fn narrow_common_js_breakage() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let common_js = std::fs::read_to_string(root.join("tests/wpt/dom/common.js")).unwrap();
    let lines: Vec<&str> = common_js.lines().collect();
    eprintln!("common.js has {} lines", lines.len());

    // Binary search: which lines of common.js break test()+cloneContents
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    for checkpoint in [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1089] {
        let cp = checkpoint.min(lines.len());
        let mut engine = make_engine_with_harness();
        let chunk = lines[..cp].join("\n");
        engine.eval_js(&chunk).unwrap_or_else(|e| {
            eprintln!("common.js lines 1-{} LOAD error: {}", cp, &e[..e.len().min(100)]);
            String::new()
        });
        let r = engine.eval_js(test_code);
        if let Err(e) = &r {
            eprintln!("BREAKS at common.js lines 1-{}: {}", cp, &e[..e.len().min(100)]);
            // Narrow 10 at a time from previous checkpoint
            let prev = if checkpoint > 100 { checkpoint - 100 } else { 0 };
            for step in (prev..=cp).step_by(10) {
                let mut e2 = make_engine_with_harness();
                let chunk2 = lines[..step].join("\n");
                e2.eval_js(&chunk2).ok();
                let r2 = e2.eval_js(test_code);
                if let Err(err) = &r2 {
                    eprintln!("  Breaks at lines 1-{}: {}", step, &err[..err.len().min(100)]);
                    let prev2 = if step > 10 { step - 10 } else { 0 };
                    for line_n in prev2..=step {
                        let mut e3 = make_engine_with_harness();
                        let chunk3 = lines[..line_n].join("\n");
                        e3.eval_js(&chunk3).ok();
                        let r3 = e3.eval_js(test_code);
                        if let Err(err2) = &r3 {
                            eprintln!("  EXACT: breaks at line {}: {}", line_n, &err2[..err2.len().min(100)]);
                            eprintln!("  Line content: {:?}", lines.get(line_n.saturating_sub(1)));
                            eprintln!("  Prev line:    {:?}", lines.get(line_n.saturating_sub(2)));
                            break;
                        }
                    }
                    break;
                }
            }
            break;
        }
    }
}
