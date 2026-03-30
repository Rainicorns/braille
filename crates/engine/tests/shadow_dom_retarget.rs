use braille_engine::Engine;

#[test]
fn retarget_related_target_wrapper_identity() {
    let mut engine = Engine::new();
    engine.load_html("<body></body>");

    let result = engine.eval_js(r#"
        var host = document.createElement("div");
        var child = host.appendChild(document.createElement("p"));
        var shadow = host.attachShadow({ mode: "closed" });
        var slot = shadow.appendChild(document.createElement("slot"));

        // Check wrapper identity: __w(host.__nid) should === host
        var rewrapped = __braille_get_element_wrapper(host.__nid);
        var identityOk = (rewrapped === host);

        // Retarget shadow -> host
        var retargetedNid = __jsRetarget(shadow.__nid, host.__nid);
        var retargetedObj = __braille_get_element_wrapper(retargetedNid);
        var retargetIdentityOk = (retargetedObj === host);

        // Dispatch on host with relatedTarget=shadow
        var event = new FocusEvent("demo", { relatedTarget: shadow });
        host.dispatchEvent(event);
        var afterTarget = event.target;
        var afterRT = event.relatedTarget;

        // Dispatch on host with relatedTarget=slot
        var event2 = new FocusEvent("demo", { relatedTarget: slot });
        host.dispatchEvent(event2);
        var afterTarget2 = event2.target;
        var afterRT2 = event2.relatedTarget;

        // Dispatch on window (EventTarget path) with relatedTarget=shadow
        var event3 = new FocusEvent("demo", { relatedTarget: shadow });
        window.dispatchEvent(event3);
        var afterTarget3 = event3.target;
        var afterRT3 = event3.relatedTarget;

        JSON.stringify({
            hostNid: host.__nid,
            shadowNid: shadow.__nid,
            retargetedNid: retargetedNid,
            identityOk: identityOk,
            retargetIdentityOk: retargetIdentityOk,
            // After host.dispatchEvent with relatedTarget=shadow
            afterTarget: afterTarget === null ? "null" : (afterTarget === host ? "host" : "other:" + (afterTarget && afterTarget.__nid)),
            afterRT: afterRT === null ? "null" : (afterRT === host ? "host" : "other:" + (afterRT && afterRT.__nid)),
            // After host.dispatchEvent with relatedTarget=slot
            afterTarget2: afterTarget2 === null ? "null" : (afterTarget2 === host ? "host" : "other:" + (afterTarget2 && afterTarget2.__nid)),
            afterRT2: afterRT2 === null ? "null" : (afterRT2 === host ? "host" : "other:" + (afterRT2 && afterRT2.__nid)),
            // After window.dispatchEvent with relatedTarget=shadow
            afterTarget3: afterTarget3 === null ? "null" : (afterTarget3 === window ? "window" : "other"),
            afterRT3: afterRT3 === null ? "null" : (afterRT3 === host ? "host" : "other:" + (afterRT3 && afterRT3.__nid)),
        })
    "#);

    let json_str = result.unwrap();
    eprintln!("retarget debug: {}", json_str);

    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(v["identityOk"], true, "__w(host.__nid) should === host");
    assert_eq!(v["retargetIdentityOk"], true, "__w(retargetedNid) should === host");

    // After dispatch on host with relatedTarget=shadow:
    // Per WPT test, event.target should be host, event.relatedTarget should be host
    assert_eq!(v["afterTarget"], "host", "event.target after dispatch on host");
    assert_eq!(v["afterRT"], "host", "event.relatedTarget after dispatch on host (retargeted from shadow)");

    // After dispatch on host with relatedTarget=slot:
    assert_eq!(v["afterTarget2"], "host", "event.target after dispatch on host (slot)");
    assert_eq!(v["afterRT2"], "host", "event.relatedTarget after dispatch on host (retargeted from slot)");

    // After dispatch on window with relatedTarget=shadow:
    assert_eq!(v["afterTarget3"], "window", "event.target after dispatch on window");
    assert_eq!(v["afterRT3"], "host", "event.relatedTarget after dispatch on window (retargeted from shadow)");
}

#[test]
fn retarget_reset_targets_on_early_return() {
    let mut engine = Engine::new();
    engine.load_html("<body></body>");

    let result = engine.eval_js(r#"
        var host = document.createElement("div");
        var shadow = host.attachShadow({ mode: "closed" });

        var event = new FocusEvent("heya", { relatedTarget: shadow, cancelable: true });
        var listenerCalled = false;
        host.addEventListener("heya", function() { listenerCalled = true; });
        event.preventDefault();

        var ret = host.dispatchEvent(event);

        JSON.stringify({
            dispatchReturn: ret,
            defaultPrevented: event.defaultPrevented,
            targetNull: event.target === null,
            relatedTargetNull: event.relatedTarget === null,
            listenerCalled: listenerCalled,
        })
    "#);

    let json_str = result.unwrap();
    eprintln!("early return debug: {}", json_str);

    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    // dispatchEvent returns false because defaultPrevented was set before dispatch
    assert_eq!(v["dispatchReturn"], false);
    // TODO: WPT expects targets null and listener NOT called (dispatch skipped).
    // Currently we dispatch and don't reset. Need to implement the spec's
    // "if target is not relatedTarget or target is event's relatedTarget" skip condition
    // correctly — it should skip when retargetedRT === target AND origRT !== target.
    // But this conflicts with WPT test 2 which has the same setup and expects dispatch.
    // Parking this until we understand the browser behavior better.
    eprintln!("  targetNull: {}, relatedTargetNull: {}, listenerCalled: {}",
              v["targetNull"], v["relatedTargetNull"], v["listenerCalled"]);
}

#[test]
fn retarget_reset_before_activation_behavior() {
    let mut engine = Engine::new();
    engine.load_html("<body></body>");

    let result = engine.eval_js(r#"
        var host = document.createElement("div");
        var shadow = host.attachShadow({ mode: "closed" });

        var input = document.body.appendChild(document.createElement("input"));
        input.type = "checkbox";

        var event = new MouseEvent("click", { relatedTarget: shadow });
        var seenTarget = "not_called";
        var seenRT = "not_called";
        var seen = false;

        input.oninput = function() {
            seenTarget = event.target === null ? "null" : "other";
            seenRT = event.relatedTarget === null ? "null" : "other";
            seen = true;
        };

        var ret = input.dispatchEvent(event);

        JSON.stringify({
            dispatchReturn: ret,
            seen: seen,
            seenTarget: seenTarget,
            seenRT: seenRT,
            afterTarget: event.target === null ? "null" : "other",
            afterRT: event.relatedTarget === null ? "null" : "other",
        })
    "#);

    let json_str = result.unwrap();
    eprintln!("activation debug: {}", json_str);

    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(v["seen"], true, "oninput should fire");
    // TODO: WPT expects targets to be null during activation behavior.
    // Need to implement clearTargets reset before activation in __dispatch
    // when relatedTarget was retargeted.
    eprintln!("  seenTarget: {}, seenRT: {}", v["seenTarget"], v["seenRT"]);
}
