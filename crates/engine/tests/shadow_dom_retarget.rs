use braille_engine::Engine;

#[test]
fn debug_wpt_test2_retarget_shadow_related_target() {
    let mut engine = Engine::new();
    engine.load_html("<body></body>");

    let result = engine.eval_js(r#"
        var host = document.createElement("div"),
            child = host.appendChild(document.createElement("p")),
            shadow = host.attachShadow({ mode: "closed" }),
            slot = shadow.appendChild(document.createElement("slot"));

        var results = [];

        // Direct retarget checks first
        var r1 = __jsRetarget(shadow.__nid, -1);
        results.push("retarget(shadow,-1)=" + r1 + " host.__nid=" + host.__nid + " shadow.__nid=" + shadow.__nid);
        results.push("__n_getParent(shadow)=" + __n_getParent(shadow.__nid));
        results.push("__n_isShadowRoot(shadow)=" + __n_isShadowRoot(shadow.__nid));
        results.push("__n_rootOf(shadow)=" + __n_rootOf(shadow.__nid));
        results.push("__n_getShadowHost(shadow)=" + __n_getShadowHost(shadow.__nid));

        // Check FocusEvent relatedTarget preservation
        var fe = new FocusEvent("demo", { relatedTarget: shadow });
        results.push("FocusEvent.relatedTarget === shadow: " + (fe.relatedTarget === shadow));
        results.push("FocusEvent.relatedTarget.__nid: " + (fe.relatedTarget && fe.relatedTarget.__nid));

        var targets = [new XMLHttpRequest(), self, host];
        var targetNames = ["XHR", "self", "host"];
        var relTargets = [shadow, slot];
        var rtNames = ["shadow", "slot"];

        for (var ri = 0; ri < relTargets.length; ri++) {
            for (var ti = 0; ti < targets.length; ti++) {
                var evt = new FocusEvent("demo", { relatedTarget: relTargets[ri] });
                targets[ti].dispatchEvent(evt);
                var t = targets[ti];
                results.push(
                    rtNames[ri] + "→" + targetNames[ti] +
                    ": target=" + (evt.target === null ? "null" : (evt.target === t ? "SAME" : "DIFF(nid=" + (evt.target && evt.target.__nid) + ")")) +
                    " rt=" + (evt.relatedTarget === null ? "null" : (evt.relatedTarget === host ? "HOST_SAME" : "DIFF(nid=" + (evt.relatedTarget && evt.relatedTarget.__nid) + ",hostNid=" + host.__nid + ")"))
                );
            }
        }
        JSON.stringify(results)
    "#);

    let json_str = result.unwrap();
    let arr: Vec<String> = serde_json::from_str(&json_str).unwrap();
    for line in &arr {
        eprintln!("  {}", line);
    }
    // WPT expects: event.target === target, event.relatedTarget === host
    // Check for wrapper identity issues
    for line in &arr {
        assert!(!line.contains("DIFF"), "Wrapper identity or null issue: {}", line);
    }
}

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
        var evt1 = new FocusEvent("demo", { relatedTarget: shadow });
        host.dispatchEvent(evt1);
        var afterTarget = evt1.target;
        var afterRT = evt1.relatedTarget;

        // Dispatch on host with relatedTarget=slot
        var evt2 = new FocusEvent("demo", { relatedTarget: slot });
        host.dispatchEvent(evt2);
        var afterTarget2 = evt2.target;
        var afterRT2 = evt2.relatedTarget;

        // Dispatch on window (EventTarget path) with relatedTarget=shadow
        var evt3 = new FocusEvent("demo", { relatedTarget: shadow });
        window.dispatchEvent(evt3);
        var afterTarget3 = evt3.target;
        var afterRT3 = evt3.relatedTarget;

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
    // retarget(shadow, host) === host and origRT (shadow) !== host → early return, no clearTargets
    assert_eq!(v["afterTarget"], "host", "event.target after dispatch on host (early return)");
    assert_eq!(v["afterRT"], "host", "event.relatedTarget after dispatch on host (retargeted)");

    // After dispatch on host with relatedTarget=slot:
    // retarget(slot, host) === host and origRT (slot) !== host → early return, no clearTargets
    assert_eq!(v["afterTarget2"], "host", "event.target after dispatch on host (slot, early return)");
    assert_eq!(v["afterRT2"], "host", "event.relatedTarget after dispatch on host (slot, retargeted)");

    // After dispatch on window with relatedTarget=shadow:
    // Window dispatch retargets but doesn't clear targets
    assert_eq!(v["afterTarget3"], "window", "event.target after dispatch on window");
    assert_eq!(v["afterRT3"], "host", "event.relatedTarget after dispatch on window (retargeted)");
}

#[test]
fn retarget_reset_targets_on_early_return() {
    let mut engine = Engine::new();
    engine.load_html("<body></body>");

    let result = engine.eval_js(r#"
        var host = document.createElement("div");
        var shadow = host.attachShadow({ mode: "closed" });

        var evt = new FocusEvent("heya", { relatedTarget: shadow, cancelable: true });
        var listenerCalled = false;
        host.addEventListener("heya", function() { listenerCalled = true; });
        evt.preventDefault();

        var ret = host.dispatchEvent(evt);

        JSON.stringify({
            dispatchReturn: ret,
            defaultPrevented: evt.defaultPrevented,
            targetNull: evt.target === null,
            relatedTargetNull: evt.relatedTarget === null,
            listenerCalled: listenerCalled,
        })
    "#);

    let json_str = result.unwrap();
    eprintln!("early return debug: {}", json_str);

    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    // dispatchEvent returns false because defaultPrevented was set before dispatch
    assert_eq!(v["dispatchReturn"], false);
    // Dispatch is skipped (retargetedRT === target && origRT !== target)
    assert_eq!(v["listenerCalled"], false, "listener should NOT be called (dispatch skipped)");
    // clearTargets on early return when defaultPrevented + origRT is shadow root
    assert_eq!(v["targetNull"], true, "event.target should be null after early return + clearTargets");
    assert_eq!(v["relatedTargetNull"], true, "event.relatedTarget should be null after early return + clearTargets");
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

        var clickEvt = new MouseEvent("click", { relatedTarget: shadow });
        var seenTarget = "not_called";
        var seenRT = "not_called";
        var seen = false;

        input.oninput = function() {
            seenTarget = clickEvt.target === null ? "null" : "other";
            seenRT = clickEvt.relatedTarget === null ? "null" : "other";
            seen = true;
        };

        var ret = input.dispatchEvent(clickEvt);

        JSON.stringify({
            dispatchReturn: ret,
            seen: seen,
            seenTarget: seenTarget,
            seenRT: seenRT,
            afterTarget: clickEvt.target === null ? "null" : "other",
            afterRT: clickEvt.relatedTarget === null ? "null" : "other",
        })
    "#);

    let json_str = result.unwrap();
    eprintln!("activation debug: {}", json_str);

    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(v["seen"], true, "oninput should fire");
    // clearTargets: targets should be null during activation behavior and after
    assert_eq!(v["seenTarget"], "null", "event.target should be null during activation (clearTargets)");
    assert_eq!(v["seenRT"], "null", "event.relatedTarget should be null during activation (clearTargets)");
    assert_eq!(v["afterTarget"], "null", "event.target should be null after dispatch");
    assert_eq!(v["afterRT"], "null", "event.relatedTarget should be null after dispatch");
}
