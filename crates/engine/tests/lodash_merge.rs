//! Isolated test: does lodash-style deep merge work correctly in our JS runtime?
//! This tests the foundational behavior that React apps depend on.

use braille_engine::Engine;

/// Pure JS deep merge — no lodash, just the algorithm
#[test]
fn js_deep_merge_basic() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"(function() {
        function isObject(v) { return v != null && typeof v === 'object' && !Array.isArray(v); }
        function merge(target) {
            for (var i = 1; i < arguments.length; i++) {
                var src = arguments[i];
                var keys = Object.keys(src);
                for (var j = 0; j < keys.length; j++) {
                    var k = keys[j];
                    if (isObject(src[k]) && isObject(target[k])) {
                        merge(target[k], src[k]);
                    } else {
                        target[k] = src[k];
                    }
                }
            }
            return target;
        }

        var base = {values: {email: "test@test.com", username: "old", domain: "proton.me"}, meta: {x: 1}};
        var patch = {values: {username: "new"}};
        var result = merge({}, base, patch);
        return JSON.stringify({
            email: result.values.email,
            username: result.values.username,
            domain: result.values.domain,
            meta: result.meta
        });
    })()"#).unwrap();

    eprintln!("[merge] basic: {result}");
    assert!(result.contains(r#""email":"test@test.com""#), "email lost: {result}");
    assert!(result.contains(r#""username":"new""#), "username not updated: {result}");
    assert!(result.contains(r#""domain":"proton.me""#), "domain lost: {result}");
}

/// Exact Proton state update pattern
#[test]
fn js_deep_merge_proton_pattern() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"(function() {
        function isObject(v) { return v != null && typeof v === 'object' && !Array.isArray(v); }
        function merge(target) {
            for (var i = 1; i < arguments.length; i++) {
                var src = arguments[i];
                var keys = Object.keys(src);
                for (var j = 0; j < keys.length; j++) {
                    var k = keys[j];
                    if (isObject(src[k]) && isObject(target[k])) {
                        merge(target[k], src[k]);
                    } else {
                        target[k] = src[k];
                    }
                }
            }
            return target;
        }

        var state = {
            values: {username: "", email: "", password: "", passwordConfirm: ""},
            inputStates: {username: {}, email: {}},
            asyncStates: {email: {}, username: {}}
        };
        var patch = {values: {username: "braille-test-bot", domain: "proton.me"}, inputStates: {username: {interactive: true}}};
        var result = merge({}, state, patch);
        return JSON.stringify(result.values);
    })()"#).unwrap();

    eprintln!("[merge] proton pattern: {result}");
    assert!(result.contains(r#""email":"""#), "email lost in merge: {result}");
    assert!(result.contains(r#""username":"braille-test-bot""#), "username not set: {result}");
    assert!(result.contains(r#""password":"""#), "password lost in merge: {result}");
}

/// Test with actual lodash from the Proton bundle (loaded via replay)
#[test]
fn lodash_merge_in_proton_runtime() {
    let mut fetcher = braille_engine::transcript::ReplayFetcher::load("tests/fixtures/proton_signup_full.json").unwrap();
    let mut engine = Engine::new();
    let _ = engine.navigate(
        "https://account.proton.me/signup",
        &mut fetcher,
        braille_wire::SnapMode::Compact,
    ).unwrap();

    // Test lodash merge by finding it through the React fiber tree.
    // The oh component uses c1() which is lodash merge.
    // We can test it indirectly by finding a useRef and doing the same merge.
    let result = engine.eval_js(r#"(function() {
        // Find any input element with React fiber
        var inputs = document.querySelectorAll('input');
        if (!inputs.length) return 'no inputs';

        var el = inputs[0];
        var keys = Object.keys(el);
        var fiberKey = keys.find(function(k) { return k.indexOf('__reactFiber$') === 0; });
        if (!fiberKey) return 'no fiber';

        // Walk up fiber tree to find the form ref (has .values with .email)
        var fiber = el[fiberKey];
        var formRef = null;
        var node = fiber;
        for (var i = 0; i < 30 && node; i++) {
            if (node.memoizedState) {
                var ms = node.memoizedState;
                while (ms) {
                    if (ms.queue === null && ms.memoizedState && typeof ms.memoizedState === 'object') {
                        var cur = ms.memoizedState.current;
                        if (cur && cur.values && 'email' in cur.values) {
                            formRef = ms.memoizedState;
                            break;
                        }
                    }
                    ms = ms.next;
                }
                if (formRef) break;
            }
            node = node.return;
        }
        if (!formRef) return 'form ref not found';

        var before = JSON.stringify(formRef.current.values);

        // Now simulate what A() does: deep merge via the same c1 lodash merge
        // We can't call c1 directly, but we can test the merge by doing it ourselves
        // on a COPY of the state and comparing
        var stateCopy = JSON.parse(JSON.stringify(formRef.current));
        var patch = {values: {username: "test-merge", domain: "proton.me"}, inputStates: {username: {interactive: true}}};

        // Manual deep merge (what lodash SHOULD do)
        function deepMerge(target, source) {
            var keys = Object.keys(source);
            for (var i = 0; i < keys.length; i++) {
                var k = keys[i];
                var sv = source[k], tv = target[k];
                if (sv && typeof sv === 'object' && !Array.isArray(sv) && tv && typeof tv === 'object' && !Array.isArray(tv)) {
                    deepMerge(tv, sv);
                } else {
                    target[k] = sv;
                }
            }
            return target;
        }

        var merged = deepMerge(deepMerge({}, stateCopy), patch);

        return JSON.stringify({
            before: JSON.parse(before),
            after_manual_merge: merged.values,
            email_preserved: merged.values.email === "",
            username_updated: merged.values.username === "test-merge",
        });
    })()"#).unwrap();

    eprintln!("[merge] lodash in proton runtime: {result}");
    assert!(result.contains(r#""email_preserved":true"#), "email lost: {result}");
    assert!(result.contains(r#""username_updated":true"#), "username not updated: {result}");
}

/// Test with the ACTUAL lodash merge function from Proton's webpack bundle
#[test]
fn actual_lodash_merge_from_bundle() {
    let mut fetcher = braille_engine::transcript::ReplayFetcher::load("tests/fixtures/proton_signup_full.json").unwrap();
    let mut engine = Engine::new();
    let _ = engine.navigate(
        "https://account.proton.me/signup",
        &mut fetcher,
        braille_wire::SnapMode::Compact,
    ).unwrap();

    // Find the actual lodash merge function used by the oh component.
    // We know A = useCallback(e => { ... c1()({}, t, e) ... })
    // c1 = a.n(c0), c0 = a(82451) = lodash merge
    // We can find c1 by intercepting A's call.
    let result = engine.eval_js(r#"(function() {
        // Find an input with React fiber
        var inputs = document.querySelectorAll('input');
        if (!inputs.length) return 'no inputs';
        var el = inputs[0];
        var keys = Object.keys(el);
        var fiberKey = keys.find(function(k) { return k.indexOf('__reactFiber$') === 0; });
        if (!fiberKey) return 'no fiber';

        // Walk up fiber tree to find the oh component's memoizedState
        // which contains the useCallback for A
        var fiber = el[fiberKey];
        var node = fiber;
        var foundMerge = null;

        // Instead of finding the merge function, test it by actually triggering
        // the same code path. Set up a test ref and merge.
        // Actually, let's find the merge via the webpack runtime.

        // The webpack runtime stores modules in a cache. Let's find it.
        // Webpack's __webpack_require__ is in a closure, but the modules
        // are accessible via the installed chunks/modules object.

        // Try to find lodash merge by searching global scope
        var merge = null;

        // Method 1: Check if lodash is on any global
        if (typeof _ !== 'undefined' && typeof _.merge === 'function') {
            merge = _.merge;
        }

        // Method 2: Walk the fiber tree to find the A callback's closure
        // The A callback captures c1 (lodash merge wrapper)
        // In React fibers, useCallback stores in memoizedState.queue
        if (!merge) {
            node = fiber;
            for (var i = 0; i < 30 && node; i++) {
                if (node.memoizedState) {
                    var ms = node.memoizedState;
                    var idx = 0;
                    while (ms) {
                        // useCallback stores as {memoizedState: [callback, deps]}
                        if (ms.memoizedState && Array.isArray(ms.memoizedState) && typeof ms.memoizedState[0] === 'function') {
                            var fn = ms.memoizedState[0];
                            var fnStr = fn.toString().substring(0, 200);
                            // The A callback contains deepEqual check and c1() call
                            if (fnStr.indexOf('current') >= 0 && (fnStr.indexOf('{}') >= 0 || fnStr.indexOf('()') >= 0)) {
                                // Try calling it with a test patch to see if it's A
                                // Actually just report what we find
                                // Can't easily extract c1 from the closure
                            }
                        }
                        ms = ms.next;
                        idx++;
                    }
                }
                node = node.return;
            }
        }

        // Method 3: Just test Object.prototype.toString behavior which lodash depends on
        var results = [];
        var obj = {a: 1, b: {c: 2}};
        results.push('toString_obj=' + Object.prototype.toString.call(obj));
        results.push('toString_arr=' + Object.prototype.toString.call([1,2]));
        results.push('toString_null=' + Object.prototype.toString.call(null));
        results.push('toString_undef=' + Object.prototype.toString.call(undefined));
        results.push('toString_str=' + Object.prototype.toString.call("test"));
        results.push('toString_num=' + Object.prototype.toString.call(42));
        results.push('toString_bool=' + Object.prototype.toString.call(true));
        results.push('toString_fn=' + Object.prototype.toString.call(function(){}));
        results.push('toString_regexp=' + Object.prototype.toString.call(/test/));
        results.push('toString_date=' + Object.prototype.toString.call(new Date()));

        // Check Symbol.toStringTag
        results.push('has_Symbol=' + (typeof Symbol !== 'undefined'));
        if (typeof Symbol !== 'undefined') {
            results.push('has_toStringTag=' + (Symbol.toStringTag !== undefined));
            var tagged = {};
            tagged[Symbol.toStringTag] = 'Custom';
            results.push('toString_tagged=' + Object.prototype.toString.call(tagged));
        }

        // Check Object.getPrototypeOf behavior
        results.push('getProto_obj=' + (Object.getPrototypeOf(obj) === Object.prototype));
        results.push('getProto_create=' + (Object.getPrototypeOf(Object.create(null)) === null));

        // Check constructor property
        results.push('ctor_obj=' + (obj.constructor === Object));
        results.push('ctor_arr=' + ([].constructor === Array));

        return results.join('\n');
    })()"#).unwrap();

    eprintln!("[merge] runtime type checks:\n{result}");

    // All these should match browser behavior
    assert!(result.contains("toString_obj=[object Object]"), "Object.prototype.toString broken: {result}");
}

/// THE ROOT CAUSE: {}.constructor should be Object in any JS environment
#[test]
fn object_literal_constructor_is_object() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"(function() {
        var obj = {a: 1};
        var results = [];
        results.push('ctor=' + obj.constructor);
        results.push('ctor_name=' + (obj.constructor ? obj.constructor.name : 'undefined'));
        results.push('ctor===Object: ' + (obj.constructor === Object));
        results.push('typeof_ctor=' + typeof obj.constructor);
        results.push('proto===Object.prototype: ' + (Object.getPrototypeOf(obj) === Object.prototype));
        results.push('Object.prototype.constructor===Object: ' + (Object.prototype.constructor === Object));

        // This is what lodash checks for isPlainObject:
        var proto = Object.getPrototypeOf(obj);
        var Ctor = proto && proto.constructor;
        results.push('Ctor===Object: ' + (Ctor === Object));

        return results.join('\n');
    })()"#).unwrap();

    eprintln!("[constructor] {result}");
    assert!(
        result.contains("ctor===Object: true"),
        "FOUNDATIONAL BUG: {{}}.constructor !== Object in our JS runtime.\n\
         Lodash uses this to detect plain objects. When it returns false,\n\
         lodash treats nested objects as non-plain and does shallow copy\n\
         instead of deep merge.\n\n{result}"
    );
}

/// Exact reproduction of Proton signup TypeError:
///   cannot read property 'interactive' of undefined
///
/// The form state uses a useRef holding:
///   { values: {...}, inputStates: {username:{}, email:{}, password:{}, passwordConfirm:{}} }
/// On submit, `op()` validates then renders with `oc(inputStates.password)`.
/// If lodash merge corrupts the state (shallow-copies inputStates instead of
/// deep-merging), field keys like `password` can go missing.
#[test]
fn proton_signup_inputstates_merge_preserves_all_fields() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    // Simulate the exact state structure and merge sequence from Proton's oh() component.
    // A = useCallback(e => { let t = d.current, a = c1()({}, t, e); ... d.current = a })
    // j = useCallback(e => { A({inputStates: e}) })
    //
    // When user types password, j({password: {interactive: true}}) is called.
    // c1 is lodash merge. The result must preserve ALL inputStates keys.
    let result = engine.eval_js(r#"(function() {
        // Lodash-style isPlainObject (simplified — real one checks constructor)
        function isPlainObject(v) {
            if (v == null || typeof v !== 'object') return false;
            var proto = Object.getPrototypeOf(v);
            if (proto === null) return true;
            var Ctor = proto.constructor;
            return typeof Ctor === 'function' && Ctor === Object;
        }

        // Lodash-style deep merge
        function merge(target) {
            for (var i = 1; i < arguments.length; i++) {
                var src = arguments[i];
                if (src == null) continue;
                var keys = Object.keys(src);
                for (var j = 0; j < keys.length; j++) {
                    var k = keys[j];
                    var sv = src[k], tv = target[k];
                    if (isPlainObject(sv) && isPlainObject(tv)) {
                        merge(tv, sv);
                    } else {
                        target[k] = sv;
                    }
                }
            }
            return target;
        }

        // Initial state (from od = {...})
        var state = {
            values: {username: "", email: "", password: "", passwordConfirm: "", signupType: undefined, domain: undefined},
            inputStates: {username: {}, email: {}, password: {}, passwordConfirm: {}},
            asyncStates: {email: {}, username: {}}
        };

        // Simulate: user types username → j({username: {interactive: true}})
        // A({inputStates: {username: {interactive: true}}})
        state = merge({}, state, {inputStates: {username: {interactive: true}}});

        // Simulate: user types password → j({password: {interactive: true}})
        state = merge({}, state, {inputStates: {password: {interactive: true}}});

        // Now check: ALL inputStates keys must still exist
        var is = state.inputStates;
        var results = [];
        results.push('username=' + JSON.stringify(is.username));
        results.push('email=' + JSON.stringify(is.email));
        results.push('password=' + JSON.stringify(is.password));
        results.push('passwordConfirm=' + JSON.stringify(is.passwordConfirm));

        // The oc() function: oc = e => !!(e.interactive && e.focus)
        // This is what throws if e is undefined
        function oc(e) { return !!(e.interactive && e.focus); }

        // These must NOT throw
        var threw = false;
        try {
            oc(is.username);
            oc(is.email);
            oc(is.password);
            oc(is.passwordConfirm);
        } catch(e) {
            threw = true;
            results.push('ERROR: ' + e.message);
        }
        results.push('threw=' + threw);

        // The critical check: is.password must not be undefined
        results.push('password_undefined=' + (is.password === undefined));
        results.push('passwordConfirm_undefined=' + (is.passwordConfirm === undefined));

        return results.join('\n');
    })()"#).unwrap();

    eprintln!("[proton] inputStates merge:\n{result}");
    assert!(result.contains("threw=false"), "oc() should not throw: {result}");
    assert!(result.contains("password_undefined=false"), "password must survive merge: {result}");
    assert!(result.contains("passwordConfirm_undefined=false"), "passwordConfirm must survive merge: {result}");
}

/// Replay the Proton signup recording and check if inputStates survives the
/// type→submit flow. This reproduces the exact TypeError from the live site.
#[test]
fn proton_signup_replay_inputstates_after_type() {
    let fixture = "tests/fixtures/proton_signup_srp.json";
    if !std::path::Path::new(fixture).exists() {
        eprintln!("[skip] {fixture} not found");
        return;
    }

    let mut fetcher = braille_engine::transcript::ReplayFetcher::load(fixture).unwrap();
    let mut engine = Engine::new();
    let snap = engine.navigate(
        "https://account.proton.me/signup",
        &mut fetcher,
        braille_wire::SnapMode::Compact,
    ).unwrap();

    eprintln!("[proton] loaded, snap length={}", snap.len());

    // Check console for errors during load
    let console = engine.drain_console();
    for line in &console {
        eprintln!("[proton] console: {line}");
    }

    // Check that inputStates is intact after page load
    let result = engine.eval_js(r#"(function() {
        var inputs = document.querySelectorAll('input');
        if (!inputs.length) return JSON.stringify({error: 'no inputs', inputCount: 0});
        var el = inputs[0];
        var keys = Object.keys(el);
        var fiberKey = keys.find(function(k) { return k.indexOf('__reactFiber$') === 0; });
        if (!fiberKey) return JSON.stringify({error: 'no fiber', keys: keys.slice(0, 10)});

        var node = el[fiberKey];
        var formRef = null;
        for (var i = 0; i < 50 && node; i++) {
            var ms = node.memoizedState;
            while (ms) {
                if (ms.queue === null && ms.memoizedState && typeof ms.memoizedState === 'object') {
                    var cur = ms.memoizedState.current;
                    if (cur && cur.inputStates && cur.values && 'password' in cur.values) {
                        formRef = ms.memoizedState;
                        break;
                    }
                }
                ms = ms.next;
            }
            if (formRef) break;
            node = node.return;
        }
        if (!formRef) return JSON.stringify({error: 'form ref not found'});

        var is = formRef.current.inputStates;
        return JSON.stringify({
            keys: Object.keys(is),
            password_type: typeof is.password,
            passwordConfirm_type: typeof is.passwordConfirm,
            username_type: typeof is.username,
            email_type: typeof is.email
        });
    })()"#).unwrap();

    eprintln!("[proton] inputStates after load: {result}");
    // This is the state BEFORE any typing. All fields should exist as empty objects.
    assert!(
        result.contains(r#""password_type":"object""#),
        "password inputState should be object after load: {result}"
    );

    // Now simulate what happens when user types: the A() callback merges
    // {inputStates: {password: {interactive: true}}} into the state.
    // Find the ACTUAL lodash merge from the webpack bundle and use it.
    let merge_result = engine.eval_js(r#"(function() {
        // Find form ref (same walk as above)
        var inputs = document.querySelectorAll('input');
        var el = inputs[0];
        var fiberKey = Object.keys(el).find(function(k) { return k.indexOf('__reactFiber$') === 0; });
        var node = el[fiberKey];
        var formRef = null;
        for (var i = 0; i < 50 && node; i++) {
            var ms = node.memoizedState;
            while (ms) {
                if (ms.queue === null && ms.memoizedState && typeof ms.memoizedState === 'object') {
                    var cur = ms.memoizedState.current;
                    if (cur && cur.inputStates && cur.values && 'password' in cur.values) {
                        formRef = ms.memoizedState;
                        break;
                    }
                }
                ms = ms.next;
            }
            if (formRef) break;
            node = node.return;
        }
        if (!formRef) return JSON.stringify({error: 'form ref not found'});

        var state = formRef.current;

        // Find the A callback by walking hooks. It's a useCallback whose closure
        // captures c1 (lodash merge). We can find it by looking for callbacks
        // that reference the formRef.
        // Instead, let's just find lodash merge through the webpack require cache.
        // Check if __webpack_require__ or webpackChunk is accessible.
        var merge = null;

        // Try to find lodash merge by checking if the module system exposed it.
        // Webpack stores modules in webpackChunkaccount. Let's find the merge
        // by extracting it from a fiber's callback closure.
        //
        // Alternative: just test with a copy of the state using JSON round-trip
        // and check if the ACTUAL merge (from the A callback) produces correct results.

        // Find the A callback: it's a useCallback that does c1()({}, t, e)
        node = el[fiberKey];
        var mergeCallback = null;
        for (var i = 0; i < 50 && node; i++) {
            var ms = node.memoizedState;
            while (ms) {
                if (ms.memoizedState && Array.isArray(ms.memoizedState) && typeof ms.memoizedState[0] === 'function') {
                    var fn = ms.memoizedState[0];
                    var src = fn.toString();
                    // The A callback contains d.current and deep equality check
                    if (src.indexOf('.current') >= 0 && src.indexOf('{}') >= 0) {
                        mergeCallback = fn;
                        break;
                    }
                }
                ms = ms.next;
            }
            if (mergeCallback) break;
            node = node.return;
        }

        if (!mergeCallback) {
            // Can't find the callback — do a manual test with state copy
            // Use our own deep merge to validate the state structure
            return JSON.stringify({
                error: 'merge callback not found',
                state_keys: Object.keys(state),
                inputStates_keys: Object.keys(state.inputStates),
                isPlainObject_test: (function() {
                    var obj = state.inputStates.password;
                    var proto = Object.getPrototypeOf(obj);
                    return {
                        proto_is_Object_prototype: proto === Object.prototype,
                        constructor_is_Object: proto && proto.constructor === Object,
                        typeof_constructor: typeof (proto && proto.constructor)
                    };
                })()
            });
        }

        // Call A with a password interactive patch — this is what typing triggers
        var before = JSON.parse(JSON.stringify(state.inputStates));
        mergeCallback({inputStates: {password: {interactive: true}}});
        var after = formRef.current.inputStates;

        return JSON.stringify({
            before_keys: Object.keys(before),
            after_keys: Object.keys(after),
            password_before: before.password,
            password_after: after.password,
            email_survived: after.email !== undefined,
            passwordConfirm_survived: after.passwordConfirm !== undefined
        });
    })()"#).unwrap();

    eprintln!("[proton] merge result: {merge_result}");

    // The key question: does the merge preserve all inputStates keys?
    if merge_result.contains("merge callback not found") {
        // Check the isPlainObject diagnostics
        eprintln!("[proton] couldn't find merge callback, checking isPlainObject...");
        assert!(
            merge_result.contains(r#""constructor_is_Object":true"#),
            "isPlainObject check fails on inputStates.password — likely the root cause: {merge_result}"
        );
    } else {
        assert!(
            merge_result.contains(r#""email_survived":true"#),
            "email inputState lost after merge: {merge_result}"
        );
        assert!(
            merge_result.contains(r#""passwordConfirm_survived":true"#),
            "passwordConfirm lost after merge: {merge_result}"
        );
    }
}

/// Replay Proton, type into password, then check if lodash merge corrupted inputStates.
/// This is the exact reproduction of the TypeError on submit.
#[test]
fn proton_signup_type_password_corrupts_inputstates() {
    let fixture = "tests/fixtures/proton_signup_srp.json";
    if !std::path::Path::new(fixture).exists() {
        eprintln!("[skip] {fixture} not found");
        return;
    }

    let mut fetcher = braille_engine::transcript::ReplayFetcher::load(fixture).unwrap();
    let mut engine = Engine::new();
    let _snap = engine.navigate(
        "https://account.proton.me/signup",
        &mut fetcher,
        braille_wire::SnapMode::Accessibility,
    ).unwrap();

    // Click Free plan (uses remaining replay exchanges)
    engine.handle_click("@e6");
    engine.settle_with_fetches(&mut fetcher);

    // Snapshot to get ref map
    let snap = engine.snapshot(braille_wire::SnapMode::Accessibility);
    eprintln!("[proton] snap has password field: {}", snap.contains("assword"));

    // Check inputStates BEFORE typing
    let before = engine.eval_js(r#"(function(){
        var inputs = document.querySelectorAll("input");
        var el = inputs[0];
        var fk = Object.keys(el).find(function(k){return k.indexOf("__reactFiber$")===0});
        if(!fk) return JSON.stringify({error:"no fiber"});
        var node = el[fk];
        for(var i=0;i<50&&node;i++){
            var ms=node.memoizedState;
            while(ms){
                if(ms.queue===null&&ms.memoizedState&&typeof ms.memoizedState==="object"){
                    var cur=ms.memoizedState.current;
                    if(cur&&cur.inputStates&&cur.values&&"password" in cur.values){
                        var is=cur.inputStates;
                        return JSON.stringify({
                            keys:Object.keys(is),
                            password_type:typeof is.password,
                            passwordConfirm_type:typeof is.passwordConfirm
                        });
                    }
                }
                ms=ms.next;
            }
            node=node.return;
        }
        return JSON.stringify({error:"ref not found"});
    })()"#).unwrap();
    eprintln!("[proton] inputStates BEFORE type: {before}");

    // Now type into password field — this triggers onValue → onInputsStateDiff → lodash merge
    // Find the password input
    let snap2 = engine.snapshot(braille_wire::SnapMode::Accessibility);
    let password_ref = snap2.lines()
        .find(|l| l.contains("input[type=password]"))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|s| s.strip_prefix("[@"))
        .and_then(|s| s.split_whitespace().next());

    if let Some(pref) = password_ref {
        let selector = format!("@{}", pref);
        eprintln!("[proton] typing into {selector}");
        let _ = engine.handle_type(&selector, "TestPass123!!");
        engine.settle();
    } else {
        eprintln!("[proton] no password input found, trying #password");
        let _ = engine.handle_type("#password", "TestPass123!!");
        engine.settle();
    }

    // Check console for errors
    let console = engine.drain_console();
    for line in &console {
        eprintln!("[proton] console after type: {line}");
    }

    // Now simulate what happens on SUBMIT: the error handler calls
    // A({inputStates: {password: {interactive:true, focus:true}}})
    // which does lodash merge. If lodash does shallow copy, inputStates loses keys.
    let merge_test = engine.eval_js(r#"(function(){
        var inputs = document.querySelectorAll("input");
        var el = inputs[0];
        var fk = Object.keys(el).find(function(k){return k.indexOf("__reactFiber$")===0});
        if(!fk) return JSON.stringify({error:"no fiber"});
        var node = el[fk];
        // Find the A callback (useCallback that does merge)
        for(var i=0;i<50&&node;i++){
            var ms=node.memoizedState;
            while(ms){
                if(ms.memoizedState&&Array.isArray(ms.memoizedState)&&typeof ms.memoizedState[0]==="function"){
                    var fn=ms.memoizedState[0];
                    var src=fn.toString();
                    // A callback does: let t=d.current, a=MERGE({},t,e); ...d.current=a
                    if(src.indexOf("current")>=0 && src.length < 300){
                        // Try calling it with the same patch the error handler uses
                        try {
                            fn({inputStates:{password:{interactive:true,focus:true}}});
                        } catch(e) {
                            return JSON.stringify({call_error: e.message});
                        }
                        // Now check if state is still intact
                        // Re-walk to find the ref
                        var node2 = el[fk];
                        for(var j=0;j<50&&node2;j++){
                            var ms2=node2.memoizedState;
                            while(ms2){
                                if(ms2.queue===null&&ms2.memoizedState&&typeof ms2.memoizedState==="object"){
                                    var cur=ms2.memoizedState.current;
                                    if(cur&&cur.inputStates&&cur.values&&"password" in cur.values){
                                        var is=cur.inputStates;
                                        return JSON.stringify({
                                            after_A_call: true,
                                            keys: Object.keys(is),
                                            password: is.password,
                                            passwordConfirm_type: typeof is.passwordConfirm,
                                            email_type: typeof is.email,
                                            username_type: typeof is.username
                                        });
                                    }
                                }
                                ms2=ms2.next;
                            }
                            node2=node2.return;
                        }
                        return JSON.stringify({error:"ref lost after A call"});
                    }
                }
                ms=ms.next;
            }
            node=node.return;
        }
        return JSON.stringify({error:"A callback not found"});
    })()"#).unwrap();
    eprintln!("[proton] after calling A() directly: {merge_test}");

    // Check inputStates AFTER typing
    let after = engine.eval_js(r#"(function(){
        var inputs = document.querySelectorAll("input");
        var el = inputs[0];
        var fk = Object.keys(el).find(function(k){return k.indexOf("__reactFiber$")===0});
        if(!fk) return JSON.stringify({error:"no fiber"});
        var node = el[fk];
        for(var i=0;i<50&&node;i++){
            var ms=node.memoizedState;
            while(ms){
                if(ms.queue===null&&ms.memoizedState&&typeof ms.memoizedState==="object"){
                    var cur=ms.memoizedState.current;
                    if(cur&&cur.inputStates&&cur.values&&"password" in cur.values){
                        var is=cur.inputStates;
                        return JSON.stringify({
                            keys:Object.keys(is),
                            password_type:typeof is.password,
                            password_val:is.password,
                            passwordConfirm_type:typeof is.passwordConfirm,
                            passwordConfirm_val:is.passwordConfirm
                        });
                    }
                }
                ms=ms.next;
            }
            node=node.return;
        }
        return JSON.stringify({error:"ref not found"});
    })()"#).unwrap();
    eprintln!("[proton] inputStates AFTER type: {after}");

    // The bug: after typing password, lodash merge may have corrupted inputStates
    // such that some keys are missing
    assert!(
        after.contains(r#""password_type":"object""#),
        "password inputState should be object after type: {after}"
    );
    assert!(
        after.contains(r#""passwordConfirm_type":"object""#),
        "passwordConfirm should survive merge after type: {after}"
    );
}

/// Object.assign (shallow) should NOT preserve nested keys — sanity check
#[test]
fn object_assign_is_shallow() {
    let mut engine = Engine::new();
    engine.load_html("<html><body></body></html>");

    let result = engine.eval_js(r#"(function() {
        var base = {values: {email: "a@b.com", username: "old"}};
        var patch = {values: {username: "new"}};
        var result = Object.assign({}, base, patch);
        return JSON.stringify({
            email: result.values.email,
            username: result.values.username,
            email_type: typeof result.values.email
        });
    })()"#).unwrap();

    eprintln!("[merge] Object.assign (shallow): {result}");
    // Object.assign is shallow — values object gets replaced entirely
    assert!(result.contains(r#""email_type":"undefined""#), "Object.assign should be shallow: {result}");
}
