//! Form validation tests: requestSubmit, checkValidity, reportValidity, validity constraints.

use braille_engine::Engine;
use braille_wire::SnapMode;

#[test]
fn test_request_submit_validates_before_submitting() {
    let html = r#"
    <html><body>
      <form id="myform">
        <input name="email" type="email" required value="" />
        <button type="submit">Submit</button>
      </form>
      <script>
        var submitted = false;
        var form = document.getElementById('myform');
        form.addEventListener('submit', function(e) {
          submitted = true;
          e.preventDefault();
        });
        form.requestSubmit();
        window.__submitted = submitted;
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // Validation should fail (required email is empty), so submit event should NOT fire
    let result = engine.eval_js("window.__submitted").unwrap();
    assert_eq!(result, "false", "requestSubmit should not fire submit when validation fails");
}

#[test]
fn test_request_submit_fires_submit_when_valid() {
    let html = r#"
    <html><body>
      <form id="myform">
        <input name="email" type="email" value="test@example.com" />
        <button type="submit">Submit</button>
      </form>
      <script>
        var submitted = false;
        var form = document.getElementById('myform');
        form.addEventListener('submit', function(e) {
          submitted = true;
          e.preventDefault();
        });
        form.requestSubmit();
        window.__submitted = submitted;
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // Validation passes, submit event should fire
    let result = engine.eval_js("window.__submitted").unwrap();
    assert_eq!(result, "true", "requestSubmit should fire submit when form is valid");
}

#[test]
fn test_request_submit_respects_prevent_default() {
    let html = r#"
    <html><body>
      <form id="myform">
        <input name="name" value="hello" />
      </form>
      <script>
        var submitFired = false;
        var preventDefaultCalled = false;
        var form = document.getElementById('myform');
        form.addEventListener('submit', function(e) {
          submitFired = true;
          preventDefaultCalled = true;
          e.preventDefault();
        });
        form.requestSubmit();
        window.__submitFired = submitFired;
        window.__preventDefaultCalled = preventDefaultCalled;
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let fired = engine.eval_js("window.__submitFired").unwrap();
    assert_eq!(fired, "true", "submit event should fire");
    let prevented = engine.eval_js("window.__preventDefaultCalled").unwrap();
    assert_eq!(prevented, "true", "preventDefault should have been called");
}

#[test]
fn test_request_submit_with_submitter() {
    let html = r#"
    <html><body>
      <form id="myform">
        <input name="name" value="hello" />
        <button id="btn" type="submit">Go</button>
      </form>
      <script>
        var capturedSubmitter = null;
        var form = document.getElementById('myform');
        var btn = document.getElementById('btn');
        form.addEventListener('submit', function(e) {
          capturedSubmitter = e.submitter;
          e.preventDefault();
        });
        form.requestSubmit(btn);
        window.__submitterTag = capturedSubmitter ? capturedSubmitter.tagName : 'none';
        window.__submitterId = capturedSubmitter ? capturedSubmitter.id : 'none';
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let tag = engine.eval_js("window.__submitterTag").unwrap();
    assert_eq!(tag, "BUTTON", "submitter should be the button element");
    let id = engine.eval_js("window.__submitterId").unwrap();
    assert_eq!(id, "btn", "submitter should have id=btn");
}

#[test]
fn test_request_submit_fires_invalid_on_failed_validation() {
    let html = r#"
    <html><body>
      <form id="myform">
        <input id="inp" name="email" type="email" required value="" />
      </form>
      <script>
        var invalidFired = false;
        var inp = document.getElementById('inp');
        inp.addEventListener('invalid', function(e) {
          invalidFired = true;
        });
        var form = document.getElementById('myform');
        form.requestSubmit();
        window.__invalidFired = invalidFired;
      </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("window.__invalidFired").unwrap();
    assert_eq!(result, "true", "invalid event should fire on failed validation");
}

#[test]
fn test_step_mismatch_number_input() {
    let html = r#"<html><body>
        <input id="a" type="number" step="3" min="0" value="5">
        <input id="b" type="number" step="3" min="0" value="6">
        <input id="c" type="number" value="1.5">
        <input id="d" type="number" step="any" value="3.14159">
        <input id="e" type="number" step="0.1" min="0" value="0.3">
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // 5 is not divisible by step=3 from min=0 -> stepMismatch=true
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('a').validity)")
        .unwrap();
    assert!(
        result.contains("\"stepMismatch\":true"),
        "5 is not a multiple of step 3 from min 0: {}",
        result
    );
    assert!(
        result.contains("\"valid\":false"),
        "should be invalid: {}",
        result
    );

    // 6 IS divisible by step=3 from min=0 -> stepMismatch=false
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('b').validity)")
        .unwrap();
    assert!(
        result.contains("\"stepMismatch\":false"),
        "6 is a multiple of step 3 from min 0: {}",
        result
    );

    // default step=1 for number, 1.5 is not a whole number -> stepMismatch=true
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('c').validity)")
        .unwrap();
    assert!(
        result.contains("\"stepMismatch\":true"),
        "1.5 is not a multiple of default step 1: {}",
        result
    );

    // step="any" means no step mismatch
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('d').validity)")
        .unwrap();
    assert!(
        result.contains("\"stepMismatch\":false"),
        "step=any should never have stepMismatch: {}",
        result
    );

    // 0.3 with step=0.1 from min=0 -> should be valid
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('e').validity)")
        .unwrap();
    assert!(
        result.contains("\"stepMismatch\":false"),
        "0.3 is a multiple of step 0.1 from min 0: {}",
        result
    );
}

#[test]
fn test_bad_input_number() {
    let html = r#"<html><body>
        <input id="a" type="number" value="abc">
        <input id="b" type="number" value="42">
        <input id="c" type="number" value="">
        <input id="d" type="date" value="not-a-date">
        <input id="e" type="date" value="2024-01-15">
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // "abc" is not a valid number -> badInput=true
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('a').validity)")
        .unwrap();
    assert!(
        result.contains("\"badInput\":true"),
        "abc is not a valid number: {}",
        result
    );
    assert!(
        result.contains("\"valid\":false"),
        "should be invalid: {}",
        result
    );

    // "42" is a valid number -> badInput=false
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('b').validity)")
        .unwrap();
    assert!(
        result.contains("\"badInput\":false"),
        "42 is a valid number: {}",
        result
    );

    // empty value -> badInput=false (no input to be bad)
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('c').validity)")
        .unwrap();
    assert!(
        result.contains("\"badInput\":false"),
        "empty value should not be badInput: {}",
        result
    );

    // "not-a-date" is not a valid date -> badInput=true
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('d').validity)")
        .unwrap();
    assert!(
        result.contains("\"badInput\":true"),
        "not-a-date is not a valid date: {}",
        result
    );

    // "2024-01-15" is a valid date -> badInput=false
    let result = engine
        .eval_js("JSON.stringify(document.getElementById('e').validity)")
        .unwrap();
    assert!(
        result.contains("\"badInput\":false"),
        "2024-01-15 is a valid date: {}",
        result
    );
}

#[test]
fn test_check_validity_fires_invalid_event() {
    let html = r#"<html><body>
        <input id="inp" type="text" required value="">
        <script>
            window.__invalidFired = false;
            window.__invalidBubbled = false;
            window.__invalidCancelable = null;
            var inp = document.getElementById('inp');
            inp.addEventListener('invalid', function(e) {
                window.__invalidFired = true;
                window.__invalidCancelable = e.cancelable;
            });
            document.body.addEventListener('invalid', function(e) {
                window.__invalidBubbled = true;
            });
            window.__checkResult = inp.checkValidity();
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // checkValidity should return false for required empty field
    let result = engine.eval_js("String(window.__checkResult)").unwrap();
    assert_eq!(result, "false", "checkValidity should return false");

    // invalid event should have fired
    let result = engine.eval_js("String(window.__invalidFired)").unwrap();
    assert_eq!(result, "true", "invalid event should fire on checkValidity");

    // invalid event should NOT bubble
    let result = engine.eval_js("String(window.__invalidBubbled)").unwrap();
    assert_eq!(result, "false", "invalid event should not bubble");

    // invalid event should be cancelable
    let result = engine.eval_js("String(window.__invalidCancelable)").unwrap();
    assert_eq!(result, "true", "invalid event should be cancelable");
}

#[test]
fn test_check_validity_no_event_when_valid() {
    let html = r#"<html><body>
        <input id="inp" type="text" value="hello">
        <script>
            window.__invalidFired = false;
            var inp = document.getElementById('inp');
            inp.addEventListener('invalid', function(e) {
                window.__invalidFired = true;
            });
            window.__checkResult = inp.checkValidity();
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("String(window.__checkResult)").unwrap();
    assert_eq!(result, "true", "checkValidity should return true");

    let result = engine.eval_js("String(window.__invalidFired)").unwrap();
    assert_eq!(result, "false", "invalid event should not fire when valid");
}

#[test]
fn test_report_validity_fires_invalid_event() {
    let html = r#"<html><body>
        <input id="inp" type="number" required value="">
        <script>
            window.__invalidFired = false;
            var inp = document.getElementById('inp');
            inp.addEventListener('invalid', function(e) {
                window.__invalidFired = true;
            });
            window.__reportResult = inp.reportValidity();
        </script>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine.eval_js("String(window.__reportResult)").unwrap();
    assert_eq!(result, "false", "reportValidity should return false");

    let result = engine.eval_js("String(window.__invalidFired)").unwrap();
    assert_eq!(result, "true", "invalid event should fire on reportValidity");
}

#[test]
fn test_validation_message_step_mismatch_and_bad_input() {
    let html = r#"<html><body>
        <input id="step" type="number" step="5" min="0" value="3">
        <input id="bad" type="number" value="xyz">
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    let result = engine
        .eval_js("document.getElementById('step').validationMessage")
        .unwrap();
    assert!(
        result.contains("step"),
        "validationMessage should mention step: {}",
        result
    );

    let result = engine
        .eval_js("document.getElementById('bad').validationMessage")
        .unwrap();
    assert!(
        result.contains("valid value"),
        "validationMessage should mention valid value: {}",
        result
    );
}

#[test]
fn textarea_validity_too_long() {
    let html = r#"
    <html><body>
      <textarea id="t" maxlength="3"></textarea>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(SnapMode::Accessibility);

    // Use the public .value setter via handle_type (not __props._value directly)
    engine.handle_type("#t", "hello").unwrap();

    let too_long = engine.eval_js(
        "document.getElementById('t').validity.tooLong"
    ).unwrap();
    assert_eq!(
        too_long, "true",
        "textarea with maxlength=3 and value='hello' should have validity.tooLong=true, got: {}",
        too_long
    );

    let valid = engine.eval_js(
        "document.getElementById('t').validity.valid"
    ).unwrap();
    assert_eq!(
        valid, "false",
        "textarea with tooLong should not be valid, got: {}",
        valid
    );
}
