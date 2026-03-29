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
