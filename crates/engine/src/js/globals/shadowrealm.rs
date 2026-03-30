use rquickjs::Ctx;

pub(super) fn register_shadowrealm(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(
        r#"
(function() {
    // Web-only globals that should NOT exist in a ShadowRealm.
    // ECMAScript intrinsics (Object, Array, Promise, etc.) remain.
    var WEB_ONLY_GLOBALS = [
        'window', 'self', 'document', 'navigator', 'location', 'history',
        'screen', 'performance', 'localStorage', 'sessionStorage',
        'setTimeout', 'setInterval', 'clearTimeout', 'clearInterval',
        'fetch', 'Request', 'Response', 'Headers',
        'XMLHttpRequest', 'Worker', 'MessageChannel', 'MessagePort',
        'Blob', 'FormData', 'CSS',
        'MutationObserver', 'IntersectionObserver', 'ResizeObserver',
        'HTMLElement', 'HTMLInputElement', 'HTMLFormElement', 'HTMLIFrameElement',
        'Node', 'Element', 'Document',
        'crypto', 'SubtleCrypto', 'CryptoKey',
        'isSecureContext'
    ];

    // APIs that are [Exposed=*] but have members that are NOT [Exposed=*].
    // Map of global name -> array of member names to strip.
    var STRIP_MEMBERS = {
        'AbortSignal': ['timeout']
    };

    function ShadowRealm() {
        this._globals = Object.create(null);
    }

    ShadowRealm.prototype.evaluate = function(sourceText) {
        if (typeof sourceText !== 'string') {
            throw new TypeError("ShadowRealm.prototype.evaluate requires a string");
        }

        // Build parameter list: web-only globals get shadowed with undefined
        var paramNames = WEB_ONLY_GLOBALS.slice();
        var paramValues = new Array(paramNames.length);
        // all undefined by default

        // For [Exposed=*] APIs with stripped members, create modified copies
        for (var apiName in STRIP_MEMBERS) {
            if (typeof globalThis[apiName] !== 'undefined') {
                var original = globalThis[apiName];
                var stripped = Object.create(original.prototype !== undefined ? original : Object.getPrototypeOf(original));
                var descs = Object.getOwnPropertyDescriptors(original);
                var toStrip = STRIP_MEMBERS[apiName];
                for (var key in descs) {
                    if (toStrip.indexOf(key) === -1) {
                        Object.defineProperty(stripped, key, descs[key]);
                    }
                }
                // Also need the prototype to work for instanceof
                if (original.prototype) {
                    stripped.prototype = original.prototype;
                }
                paramNames.push(apiName);
                paramValues.push(stripped);
            }
        }

        // Realm globals: inject stored globals as parameters
        for (var k in this._globals) {
            paramNames.push(k);
            paramValues.push(this._globals[k]);
        }

        // Build and call the function
        var fn;
        try {
            fn = new Function(paramNames.join(','),
                '"use strict";\n' + sourceText + '\n');
        } catch(e) {
            throw new SyntaxError(e.message);
        }

        var result = fn.apply(undefined, paramValues);

        // Callable boundary: only primitives and callables cross
        if (result === undefined || result === null) return result;
        var t = typeof result;
        if (t === 'string' || t === 'number' || t === 'boolean' ||
            t === 'bigint' || t === 'symbol' || t === 'function') {
            return result;
        }
        throw new TypeError(
            "ShadowRealm evaluate: return value is not a primitive or callable"
        );
    };

    ShadowRealm.prototype.importValue = function() {
        return Promise.reject(new TypeError("importValue is not supported"));
    };

    ShadowRealm.prototype[Symbol.toStringTag] = 'ShadowRealm';

    globalThis.ShadowRealm = ShadowRealm;
})();
"#,
    )
    .unwrap();
}
