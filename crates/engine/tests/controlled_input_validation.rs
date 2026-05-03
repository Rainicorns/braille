use braille_engine::Engine;

// ---------------------------------------------------------------------------
// 1. Value persists in controlled input after type + settle
// ---------------------------------------------------------------------------

#[test]
fn controlled_input_value_persists_after_type() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><body>
    <input id="email" type="text" value="" />
    <script>
        // Simulate a controlled input: onChange updates an internal state,
        // which is used to set the value back (like React controlled inputs)
        var state = '';
        var input = document.getElementById('email');
        input.addEventListener('input', function(e) {
            state = e.target.value;
        });
        // After each change cycle, "re-render" with state value
        input.addEventListener('change', function(e) {
            input.value = state;
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    let result = engine.handle_type("#email", "test@example.com");
    assert!(result.is_ok(), "type failed: {:?}", result);
    engine.settle();

    let value = engine.eval_js("document.getElementById('email').value").unwrap();
    eprintln!("input value after type: {}", value);
    assert_eq!(value, "test@example.com", "Input value should persist after type + settle");
}

// ---------------------------------------------------------------------------
// 2. onBlur validation sees current value (not stale)
// ---------------------------------------------------------------------------

#[test]
fn onblur_validation_sees_current_value() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><body>
    <input id="password" type="password" value="" />
    <div id="error" style="display:none"></div>
    <script>
        var passwordState = '';
        var input = document.getElementById('password');
        var errorDiv = document.getElementById('error');

        // Controlled input pattern
        input.addEventListener('input', function(e) {
            passwordState = e.target.value;
        });
        input.addEventListener('change', function(e) {
            input.value = passwordState;
        });

        // Validation on blur — checks length
        input.addEventListener('blur', function(e) {
            var val = e.target.value;
            if (val.length > 0 && val.length < 8) {
                errorDiv.textContent = 'Password must contain at least 8 characters';
                errorDiv.style.display = 'block';
            } else {
                errorDiv.textContent = '';
                errorDiv.style.display = 'none';
            }
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    // Type a 16-character password
    let result = engine.handle_type("#password", "securepassword16");
    assert!(result.is_ok(), "type failed: {:?}", result);
    engine.settle();

    let value = engine.eval_js("document.getElementById('password').value").unwrap();
    eprintln!("password value: {}", value);
    assert_eq!(value, "securepassword16");

    let error_text = engine
        .eval_js("document.getElementById('error').textContent")
        .unwrap();
    eprintln!("error text: '{}'", error_text);
    assert_eq!(
        error_text, "",
        "No validation error should appear for 16-char password"
    );

    let error_display = engine
        .eval_js("document.getElementById('error').style.display")
        .unwrap();
    assert_eq!(error_display, "none", "Error div should be hidden");
}

// ---------------------------------------------------------------------------
// 3. Value tracker reset triggers change detection
// ---------------------------------------------------------------------------

#[test]
fn value_tracker_reset_triggers_change_detection() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><body>
    <input id="name" type="text" value="" />
    <script>
        // Simulate React's _valueTracker
        var input = document.getElementById('name');
        var trackerValue = '';
        input._valueTracker = {
            getValue: function() { return trackerValue; },
            setValue: function(v) { trackerValue = v; },
            stopTracking: function() {}
        };

        var changeDetected = false;
        input.addEventListener('input', function(e) {
            // React checks: did the value actually change from what tracker has?
            var tracker = e.target._valueTracker;
            if (tracker) {
                var lastValue = tracker.getValue();
                if (lastValue !== e.target.value) {
                    changeDetected = true;
                    tracker.setValue(e.target.value);
                }
            }
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    let result = engine.handle_type("#name", "hello");
    assert!(result.is_ok(), "type failed: {:?}", result);
    engine.settle();

    let detected = engine.eval_js("String(changeDetected)").unwrap();
    eprintln!("change detected: {}", detected);
    assert_eq!(detected, "true", "Change should be detected via _valueTracker");

    let value = engine.eval_js("document.getElementById('name').value").unwrap();
    assert_eq!(value, "hello", "Input value should be 'hello'");
}

// ---------------------------------------------------------------------------
// 4. Type then settle then read value — value matches what was typed
// ---------------------------------------------------------------------------

#[test]
fn type_then_settle_then_read_value() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><body>
    <input id="username" type="text" value="" />
    <div id="display"></div>
    <script>
        var input = document.getElementById('username');
        var display = document.getElementById('display');

        // Simulate React controlled input with re-render on change
        var currentValue = '';
        input.addEventListener('input', function(e) {
            currentValue = e.target.value;
        });
        input.addEventListener('change', function(e) {
            // "Re-render": set value from state and update display
            input.value = currentValue;
            display.textContent = 'Value: ' + currentValue;
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    let result = engine.handle_type("#username", "braille-test-bot");
    assert!(result.is_ok(), "type failed: {:?}", result);
    engine.settle();

    let value = engine.eval_js("document.getElementById('username').value").unwrap();
    eprintln!("username value: {}", value);
    assert_eq!(value, "braille-test-bot", "Value must match what was typed");

    let display = engine.eval_js("document.getElementById('display').textContent").unwrap();
    eprintln!("display: {}", display);
    assert_eq!(display, "Value: braille-test-bot", "Display should show typed value");
}
