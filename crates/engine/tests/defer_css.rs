use braille_engine::Engine;

/// Test the ifixit deferCss pattern: link.relList.supports('preload')
/// should return true, enabling the primary CSS loading path.
#[test]
fn dom_token_list_supports_preload() {
    let mut engine = Engine::new();
    engine.load_html("<!DOCTYPE html><html><head></head><body></body></html>");
    engine.settle();

    let result = engine.eval_js(r#"
(function() {
    var link = document.createElement('link');
    var results = [];

    // relList should exist for link elements
    results.push('relList=' + (link.relList ? 'exists' : 'missing'));

    // supports should be a function
    results.push('supports=' + (typeof link.relList.supports));

    // preload should be supported
    results.push('preload=' + link.relList.supports('preload'));

    // stylesheet should be supported
    results.push('stylesheet=' + link.relList.supports('stylesheet'));

    // nonsense should not be supported
    results.push('nonsense=' + link.relList.supports('nonsense'));

    return results.join('; ');
})()
    "#);
    eprintln!("Result: {:?}", result);
    let r = result.unwrap();
    assert!(r.contains("relList=exists"), "relList missing: {r}");
    assert!(r.contains("supports=function"), "supports not a function: {r}");
    assert!(r.contains("preload=true"), "preload not supported: {r}");
    assert!(r.contains("stylesheet=true"), "stylesheet not supported: {r}");
    assert!(r.contains("nonsense=false"), "nonsense should not be supported: {r}");
}

/// Test the full deferCss unhide pattern from ifixit.
/// The cssHide style should be removed after deferCss runs.
#[test]
fn defer_css_unhide_pattern() {
    let html = r#"<!DOCTYPE html>
<html>
<head>
<script>
var deferCss = {
    hidden: true,
    timeout: null,
    supportsPreload: function() {
        try {
            return document.createElement('link').relList.supports('preload');
        } catch (e) {
            return false;
        }
    },
    fallbackNoPreload: function() {
        if (deferCss.supportsPreload()) {
            return;
        }
        var applyFallback = function() {
            var links = document.querySelectorAll('.cssPreload');
            if (!links.length) { return; }
            for (var i = 0; i < links.length; ++i) {
                var link = links[i];
                link.onload = null;
                link.rel = 'stylesheet';
                link.className = 'cssReady';
            }
            deferCss.applyCssWhenDomLoaded();
        };
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', applyFallback);
        } else {
            applyFallback();
        }
    },
    unhide: function() {
        if (deferCss.hidden) {
            var hider = document.getElementById('cssHide');
            hider && hider.parentElement.removeChild(hider);
            deferCss.hidden = false;
        }
    },
    applyAllCss: function() {
        if (!deferCss.hidden) { return; }
        clearTimeout(deferCss.timeout);
        var links = document.querySelectorAll('.cssReady, .cssPreload');
        var link;
        for (var i = 0; i < links.length; ++i) {
            link = links[i];
            link.onload = null;
            link.rel = 'stylesheet';
        }
        deferCss.unhide();
    },
    cssLoaded: function(link, success) {
        link.className = success ? "cssReady" : "cssFailed";
        var stillWaiting = document.querySelector('.cssPreload');
        if (!stillWaiting) {
            deferCss.applyCssWhenDomLoaded();
        }
    },
    applyCssWhenDomLoaded: function() {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', function() { deferCss.applyAllCss(); });
        } else {
            deferCss.applyAllCss();
        }
    },
    setTimeout: function(timeout) {
        deferCss.timeout = setTimeout(function() {
            deferCss.applyCssWhenDomLoaded();
        }, timeout);
    }
};
deferCss.fallbackNoPreload();
deferCss.setTimeout(7000);
</script>
<style id="cssHide">
    .hide-until-css-loaded { display: none !important; }
</style>
<link rel="preload" href="fake.css" as="style" class="cssPreload"
      onload="deferCss.cssLoaded(this, true)">
</head>
<body>
<div id="page" class="hide-until-css-loaded">
    <h1>Teardown Content</h1>
</div>
</body>
</html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    // Check what happened
    let result = engine.eval_js(r#"
(function() {
    var results = [];
    results.push('hidden=' + deferCss.hidden);
    results.push('supportsPreload=' + deferCss.supportsPreload());
    var hider = document.getElementById('cssHide');
    results.push('cssHide=' + (hider ? 'present' : 'removed'));
    var page = document.getElementById('page');
    results.push('pageVisible=' + (page ? 'exists' : 'missing'));
    var preloadLinks = document.querySelectorAll('.cssPreload');
    results.push('cssPreloadCount=' + preloadLinks.length);
    var readyLinks = document.querySelectorAll('.cssReady');
    results.push('cssReadyCount=' + readyLinks.length);
    return results.join('; ');
})()
    "#);
    eprintln!("deferCss state: {:?}", result);
    let r = result.unwrap();

    // The cssHide element should have been removed by unhide()
    assert!(
        r.contains("cssHide=removed"),
        "cssHide style should have been removed by unhide(): {r}"
    );
}

/// Debug test: trace the deferCss flow with the navigate path (matching real ifixit test)
#[test]
fn defer_css_debug_navigate_path() {
    // Navigate path debug - uses load_html + settle (same as load_html path)

    let html = r#"<!DOCTYPE html>
<html>
<head>
<script>
var __debug = [];
var deferCss = {
    hidden: true,
    timeout: null,
    supportsPreload: function() {
        try {
            var result = document.createElement('link').relList.supports('preload');
            __debug.push('supportsPreload=' + result);
            return result;
        } catch (e) {
            __debug.push('supportsPreload=error:' + e.message);
            return false;
        }
    },
    fallbackNoPreload: function() {
        if (deferCss.supportsPreload()) {
            __debug.push('fallback:skipped(preload supported)');
            return;
        }
        __debug.push('fallback:running');
        var applyFallback = function() {
            var links = document.querySelectorAll('.cssPreload');
            __debug.push('fallback:links=' + links.length);
            if (!links.length) { return; }
            for (var i = 0; i < links.length; ++i) {
                links[i].onload = null;
                links[i].rel = 'stylesheet';
                links[i].className = 'cssReady';
            }
            deferCss.applyCssWhenDomLoaded();
        };
        __debug.push('readyState=' + document.readyState);
        if (document.readyState === 'loading') {
            __debug.push('fallback:deferred to DOMContentLoaded');
            document.addEventListener('DOMContentLoaded', applyFallback);
        } else {
            __debug.push('fallback:immediate');
            applyFallback();
        }
    },
    unhide: function() {
        __debug.push('unhide:called hidden=' + deferCss.hidden);
        if (deferCss.hidden) {
            var hider = document.getElementById('cssHide');
            __debug.push('unhide:hider=' + (hider ? 'found' : 'null'));
            hider && hider.parentElement.removeChild(hider);
            deferCss.hidden = false;
        }
    },
    applyAllCss: function() {
        __debug.push('applyAllCss:hidden=' + deferCss.hidden);
        if (!deferCss.hidden) { return; }
        clearTimeout(deferCss.timeout);
        var links = document.querySelectorAll('.cssReady, .cssPreload');
        __debug.push('applyAllCss:links=' + links.length);
        for (var i = 0; i < links.length; ++i) {
            links[i].onload = null;
            links[i].rel = 'stylesheet';
        }
        deferCss.unhide();
    },
    cssLoaded: function(link, success) {
        __debug.push('cssLoaded:success=' + success + ' class=' + link.className);
        link.className = success ? "cssReady" : "cssFailed";
        var stillWaiting = document.querySelector('.cssPreload');
        __debug.push('cssLoaded:stillWaiting=' + !!stillWaiting);
        if (!stillWaiting) {
            deferCss.applyCssWhenDomLoaded();
        }
    },
    applyCssWhenDomLoaded: function() {
        __debug.push('applyCssWhenDomLoaded:readyState=' + document.readyState);
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', function() { deferCss.applyAllCss(); });
        } else {
            deferCss.applyAllCss();
        }
    },
    setTimeout: function(timeout) {
        deferCss.timeout = setTimeout(function() {
            __debug.push('timeout:fired');
            deferCss.applyCssWhenDomLoaded();
        }, timeout);
    }
};
deferCss.fallbackNoPreload();
deferCss.setTimeout(7000);
</script>
<style id="cssHide">
    .hide-until-css-loaded { display: none !important; }
</style>
<link rel="preload" href="fake.css" as="style" class="cssPreload"
      onload="deferCss.cssLoaded(this, true)">
</head>
<body>
<div id="page" class="hide-until-css-loaded">
    <h1>Teardown Content</h1>
</div>
</body>
</html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    engine.settle();

    let debug = engine.eval_js("__debug.join('\\n')").unwrap();
    eprintln!("=== deferCss debug trace ===\n{debug}");

    let state = engine.eval_js(r#"
(function() {
    var hider = document.getElementById('cssHide');
    return 'cssHide=' + (hider ? 'present' : 'removed') + ' hidden=' + deferCss.hidden;
})()
    "#).unwrap();
    eprintln!("Final state: {state}");

    assert!(state.contains("cssHide=removed"), "cssHide should be removed: {state}\nDebug:\n{debug}");
}
