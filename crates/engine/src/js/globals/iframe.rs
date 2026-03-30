use rquickjs::{Ctx, Function};

use crate::js::dom_bridge::{with_state, with_tree};

/// Register iframe realm creation and contentWindow/contentDocument support.
pub(super) fn register_iframe(ctx: &Ctx<'_>) {
    // Native function: look up pre-fetched iframe content by URL
    let lookup_fn =
        Function::new(ctx.clone(), move |url: String| -> String {
            with_state(|st| {
                st.iframe_src_content.get(&url).cloned().unwrap_or_default()
            })
        })
        .unwrap();
    ctx.globals()
        .set("__braille_iframe_lookup_content", lookup_fn)
        .unwrap();

    // Native function: get iframe src attribute from the DomTree
    let get_src_fn =
        Function::new(ctx.clone(), move |node_id: u32| -> String {
            with_tree(|tree| {
                tree.get_attribute(node_id as usize, "src")
                    .unwrap_or_default()
            })
        })
        .unwrap();
    ctx.globals()
        .set("__braille_iframe_get_src", get_src_fn)
        .unwrap();

    // Native function: find all iframe elements in the DomTree
    let find_iframes_fn = Function::new(ctx.clone(), move || -> Vec<u32> {
        with_tree(|tree| {
            tree.find_descendants_by_tag(0, "iframe")
                .into_iter()
                .map(|id| id as u32)
                .collect()
        })
    })
    .unwrap();
    ctx.globals()
        .set("__braille_find_iframes", find_iframes_fn)
        .unwrap();

    // JS-side iframe realm system
    ctx.eval::<(), _>(
        r#"
        (function() {
            var iframeRealms = {};

            function buildIframeWindow(iframeEl, iframeDoc) {
                var iframeWindow;

                var parentProxy = new Proxy(window, {
                    get: function(target, prop) {
                        if (prop === 'postMessage') {
                            return function(data, targetOrigin) {
                                var serialized = data;
                                if (typeof data === 'object' && data !== null) {
                                    serialized = JSON.parse(JSON.stringify(data));
                                }
                                setTimeout(function() {
                                    var event = new MessageEvent('message', {
                                        data: serialized,
                                        origin: (typeof location !== 'undefined' && location.origin) || '',
                                        source: iframeWindow
                                    });
                                    if (target.__et_listeners) {
                                        var cbs = target.__et_listeners['message_b'];
                                        if (cbs) { var s = cbs.slice(); for (var j = 0; j < s.length; j++) s[j].call(target, event); }
                                        cbs = target.__et_listeners['message_c'];
                                        if (cbs) { var s = cbs.slice(); for (var j = 0; j < s.length; j++) s[j].call(target, event); }
                                    }
                                    if (typeof target.onmessage === 'function') {
                                        target.onmessage(event);
                                    }
                                }, 0);
                            };
                        }
                        var val = target[prop];
                        if (typeof val === 'function') return val.bind(target);
                        return val;
                    }
                });

                iframeWindow = {
                    document: iframeDoc,
                    parent: parentProxy,
                    top: parentProxy,
                    self: null,
                    window: null,
                    frameElement: iframeEl,
                    location: (typeof location !== 'undefined') ? {
                        href: location.href,
                        origin: location.origin,
                        protocol: location.protocol,
                        host: location.host,
                        hostname: location.hostname,
                        pathname: location.pathname,
                        search: location.search,
                        hash: location.hash
                    } : {},
                    navigator: (typeof navigator !== 'undefined') ? navigator : {},
                    setTimeout: setTimeout,
                    setInterval: setInterval,
                    clearTimeout: clearTimeout,
                    clearInterval: clearInterval,
                    console: console,
                    Event: Event,
                    MessageEvent: MessageEvent,
                    JSON: JSON,
                    Object: Object,
                    Array: Array,
                    Promise: Promise,
                    Math: Math,
                    Date: Date,
                    RegExp: RegExp,
                    Error: Error,
                    TypeError: TypeError,
                    parseInt: parseInt,
                    parseFloat: parseFloat,
                    isNaN: isNaN,
                    isFinite: isFinite,
                    encodeURIComponent: encodeURIComponent,
                    decodeURIComponent: decodeURIComponent,
                    encodeURI: encodeURI,
                    decodeURI: decodeURI,
                    atob: (typeof atob !== 'undefined') ? atob : undefined,
                    btoa: (typeof btoa !== 'undefined') ? btoa : undefined,
                    crypto: (typeof crypto !== 'undefined') ? crypto : undefined,
                    TextEncoder: (typeof TextEncoder !== 'undefined') ? TextEncoder : undefined,
                    TextDecoder: (typeof TextDecoder !== 'undefined') ? TextDecoder : undefined,
                    AbortSignal: (typeof AbortSignal !== 'undefined') ? AbortSignal : undefined,
                    AbortController: (typeof AbortController !== 'undefined') ? AbortController : undefined,
                    DOMException: (typeof DOMException !== 'undefined') ? DOMException : undefined,
                    EventTarget: (typeof EventTarget !== 'undefined') ? EventTarget : undefined,
                    CustomEvent: (typeof CustomEvent !== 'undefined') ? CustomEvent : undefined,
                    XMLHttpRequest: (typeof XMLHttpRequest !== 'undefined') ? XMLHttpRequest : undefined,
                    HTMLElement: (typeof HTMLElement !== 'undefined') ? HTMLElement : undefined,
                    Element: (typeof Element !== 'undefined') ? Element : undefined,
                    Node: (typeof Node !== 'undefined') ? Node : undefined,
                    DocumentFragment: (typeof DocumentFragment !== 'undefined') ? DocumentFragment : undefined,
                    ShadowRoot: (typeof ShadowRoot !== 'undefined') ? ShadowRoot : undefined,
                    customElements: (typeof customElements !== 'undefined') ? customElements : undefined,
                    Uint8Array: Uint8Array,
                    ArrayBuffer: ArrayBuffer,
                    DataView: DataView,
                    Map: Map,
                    Set: Set,
                    Symbol: Symbol,
                    _onmessage: null,
                    _listeners: {},
                    addEventListener: function(type, cb, opts) {
                        if (!iframeWindow._listeners[type]) iframeWindow._listeners[type] = [];
                        iframeWindow._listeners[type].push(cb);
                    },
                    removeEventListener: function(type, cb) {
                        if (iframeWindow._listeners[type]) {
                            iframeWindow._listeners[type] = iframeWindow._listeners[type].filter(function(f) { return f !== cb; });
                        }
                    },
                    dispatchEvent: function(event) {
                        var cbs = iframeWindow._listeners[event.type];
                        if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(iframeWindow, event); }
                        if (typeof iframeWindow['on' + event.type] === 'function') {
                            iframeWindow['on' + event.type].call(iframeWindow, event);
                        }
                        return true;
                    },
                    postMessage: function(data, targetOrigin) {
                        var serialized = data;
                        if (typeof data === 'object' && data !== null) {
                            serialized = JSON.parse(JSON.stringify(data));
                        }
                        setTimeout(function() {
                            var event = new MessageEvent('message', {
                                data: serialized,
                                origin: (typeof location !== 'undefined' && location.origin) || '',
                                source: window
                            });
                            iframeWindow.dispatchEvent(event);
                        }, 0);
                    }
                };

                iframeWindow.self = iframeWindow;
                iframeWindow.window = iframeWindow;
                iframeDoc.defaultView = iframeWindow;

                // Window scroll properties
                iframeWindow.__scrollX = 0;
                iframeWindow.__scrollY = 0;
                Object.defineProperty(iframeWindow, 'scrollX', {
                    get: function() { return iframeWindow.__scrollX; },
                    configurable: true
                });
                Object.defineProperty(iframeWindow, 'scrollY', {
                    get: function() { return iframeWindow.__scrollY; },
                    configurable: true
                });
                Object.defineProperty(iframeWindow, 'pageXOffset', {
                    get: function() { return iframeWindow.__scrollX; },
                    configurable: true
                });
                Object.defineProperty(iframeWindow, 'pageYOffset', {
                    get: function() { return iframeWindow.__scrollY; },
                    configurable: true
                });
                iframeWindow.innerWidth = 200;
                iframeWindow.innerHeight = 200;
                iframeWindow.scrollTo = function(xOrOpts, y) {
                    var nx, ny;
                    if (typeof xOrOpts === 'object' && xOrOpts !== null) {
                        nx = ('left' in xOrOpts) ? xOrOpts.left|0 : iframeWindow.__scrollX;
                        ny = ('top' in xOrOpts) ? xOrOpts.top|0 : iframeWindow.__scrollY;
                    } else {
                        nx = (xOrOpts|0);
                        ny = (y|0);
                    }
                    if (nx < 0) nx = 0;
                    if (ny < 0) ny = 0;
                    var docEl = iframeDoc.documentElement;
                    if (docEl) {
                        var maxX = (docEl.scrollWidth || 0) - (iframeWindow.innerWidth || 200);
                        var maxY = (docEl.scrollHeight || 0) - (iframeWindow.innerHeight || 200);
                        if (maxX > 0 && nx > maxX) nx = maxX;
                        if (maxY > 0 && ny > maxY) ny = maxY;
                    }
                    var changed = (nx !== iframeWindow.__scrollX || ny !== iframeWindow.__scrollY);
                    iframeWindow.__scrollX = nx;
                    iframeWindow.__scrollY = ny;
                    if (changed) {
                        iframeWindow.dispatchEvent(new Event('scroll', {bubbles: false}));
                    }
                };
                iframeWindow.scroll = iframeWindow.scrollTo;
                iframeWindow.scrollBy = function(xOrOpts, y) {
                    var dx, dy;
                    if (typeof xOrOpts === 'object' && xOrOpts !== null) {
                        dx = xOrOpts.left || 0;
                        dy = xOrOpts.top || 0;
                    } else {
                        dx = xOrOpts || 0;
                        dy = y || 0;
                    }
                    iframeWindow.scrollTo(iframeWindow.__scrollX + dx, iframeWindow.__scrollY + dy);
                };

                return { window: iframeWindow, parentProxy: parentProxy };
            }

            function execScriptInIframe(realm, code) {
                var iw = realm.window;
                var pp = realm._parentProxy;
                var fn = new Function(
                    'window', 'document', 'self', 'parent', 'top',
                    'postMessage', 'addEventListener', 'removeEventListener',
                    'setTimeout', 'setInterval', 'clearTimeout', 'clearInterval',
                    'console', 'location', 'navigator', 'JSON', 'MessageEvent',
                    'crypto', 'TextEncoder', 'TextDecoder',
                    code
                );
                fn(
                    iw, iw.document, iw, pp, pp,
                    function(data, targetOrigin) {
                        var serialized = data;
                        if (typeof data === 'object' && data !== null) {
                            serialized = JSON.parse(JSON.stringify(data));
                        }
                        setTimeout(function() {
                            var event = new MessageEvent('message', {
                                data: serialized,
                                origin: (typeof location !== 'undefined' && location.origin) || '',
                                source: iw
                            });
                            if (window.__et_listeners) {
                                var cbs = window.__et_listeners['message_b'];
                                if (cbs) { var s = cbs.slice(); for (var j = 0; j < s.length; j++) s[j].call(window, event); }
                                cbs = window.__et_listeners['message_c'];
                                if (cbs) { var s = cbs.slice(); for (var j = 0; j < s.length; j++) s[j].call(window, event); }
                            }
                            if (typeof window.onmessage === 'function') {
                                window.onmessage(event);
                            }
                        }, 0);
                    },
                    iw.addEventListener,
                    iw.removeEventListener,
                    setTimeout, setInterval, clearTimeout, clearInterval,
                    console,
                    iw.location, iw.navigator, JSON, MessageEvent,
                    (typeof crypto !== 'undefined') ? crypto : undefined,
                    (typeof TextEncoder !== 'undefined') ? TextEncoder : undefined,
                    (typeof TextDecoder !== 'undefined') ? TextDecoder : undefined
                );
            }

            function buildRealDomDocument(iframeNodeId) {
                var htmlNid = __n_createElement('html');
                __n_appendChild(iframeNodeId, htmlNid);
                var headNid = __n_createElement('head');
                __n_appendChild(htmlNid, headNid);
                var bodyNid = __n_createElement('body');
                __n_appendChild(htmlNid, bodyNid);
                var htmlEl = __w(htmlNid);
                return __makeDocumentLike(htmlEl);
            }

            // Initialize about:blank iframe realm on appendChild
            globalThis.__braille_maybe_init_iframe = function(node) {
                if (!node || node.tagName !== 'IFRAME') return;
                if (node.__nid === undefined) return;
                if (iframeRealms[node.__nid]) return;

                var src = node.getAttribute('src');
                if (src) return;

                var iframeDoc = buildRealDomDocument(node.__nid);
                var built = buildIframeWindow(node, iframeDoc);

                var realm = {
                    window: built.window,
                    document: iframeDoc,
                    _parentProxy: built.parentProxy,
                    _iframeNodeId: node.__nid
                };
                iframeRealms[node.__nid] = realm;
            };

            // Find the iframe realm that owns a given node (walk up parent chain)
            globalThis.__braille_find_owning_iframe_realm = function(node) {
                if (!node || node.__nid === undefined) return null;
                var nid = node.__nid;
                var cur = __n_getParent(nid);
                while (cur >= 0) {
                    if (__n_getTagName(cur) === 'IFRAME') {
                        return iframeRealms[cur] || null;
                    }
                    cur = __n_getParent(cur);
                }
                return null;
            };

            // Execute code in an iframe's scoped context
            globalThis.__braille_exec_in_iframe = function(realm, code) {
                execScriptInIframe(realm, code);
            };

            // Extract inline <script> content from HTML (simple regex)
            function extractScripts(html) {
                var scripts = [];
                var re = /<script[^>]*>([\s\S]*?)<\/script>/gi;
                var m;
                while ((m = re.exec(html)) !== null) {
                    var tag = m[0];
                    if (/\bsrc\s*=/i.test(tag.substring(0, tag.indexOf('>')))) continue;
                    if (m[1].trim()) scripts.push(m[1]);
                }
                return scripts;
            }

            // Create an iframe realm for a given iframe nodeId and HTML content
            globalThis.__braille_create_iframe_realm = function(iframeNodeId, html) {
                var iframeEl = __braille_get_element_wrapper(iframeNodeId);

                // Build real DOM subtree under the iframe element
                var iframeDoc;
                if (html) {
                    // Parse HTML into real nodes via a temp container, then move under iframe
                    var htmlNid = __n_createElement('html');
                    __n_appendChild(iframeNodeId, htmlNid);
                    __n_setInnerHTML(htmlNid, html);

                    // Ensure <head> and <body> exist
                    var htmlEl = __w(htmlNid);
                    var hasHead = false, hasBody = false;
                    var kids = htmlEl.childNodes;
                    for (var i = 0; i < kids.length; i++) {
                        if (kids[i].tagName === 'HEAD') hasHead = true;
                        if (kids[i].tagName === 'BODY') hasBody = true;
                    }
                    if (!hasHead) {
                        var headNid = __n_createElement('head');
                        if (kids.length > 0 && kids[0].__nid !== undefined) {
                            __n_insertBefore(htmlNid, headNid, kids[0].__nid);
                        } else {
                            __n_appendChild(htmlNid, headNid);
                        }
                    }
                    if (!hasBody) {
                        var bodyNid = __n_createElement('body');
                        __n_appendChild(htmlNid, bodyNid);
                    }

                    iframeDoc = __makeDocumentLike(htmlEl);
                } else {
                    iframeDoc = buildRealDomDocument(iframeNodeId);
                }

                var built = buildIframeWindow(iframeEl, iframeDoc);

                var realm = {
                    window: built.window,
                    document: iframeDoc,
                    _parentProxy: built.parentProxy,
                    _iframeNodeId: iframeNodeId
                };
                iframeRealms[iframeNodeId] = realm;

                // Execute inline scripts from the HTML content
                var scripts = extractScripts(html || '');
                for (var i = 0; i < scripts.length; i++) {
                    execScriptInIframe(realm, scripts[i]);
                }

                return realm;
            };

            // Get iframe realm by nodeId
            globalThis.__braille_get_iframe_realm = function(nodeId) {
                return iframeRealms[nodeId] || null;
            };

            // Process all iframes: called from Rust process_iframe_loads
            globalThis.__braille_process_iframes = function() {
                var iframeIds = __braille_find_iframes();
                for (var i = 0; i < iframeIds.length; i++) {
                    var nid = iframeIds[i];
                    if (iframeRealms[nid]) continue;

                    var src = __braille_iframe_get_src(nid);
                    if (!src) continue;

                    var content = __braille_iframe_lookup_content(src);
                    if (!content) continue;

                    __braille_create_iframe_realm(nid, content);

                    var el = __braille_get_element_wrapper(nid);
                    if (el) {
                        var loadEvent = new Event('load');
                        if (typeof el.onload === 'function') {
                            el.onload(loadEvent);
                        }
                        if (el.dispatchEvent) {
                            el.dispatchEvent(loadEvent);
                        }
                    }
                }
            };

            Object.defineProperty(__ElemProto, 'contentWindow', {
                get: function() {
                    if (this.__nid === undefined) return undefined;
                    if (__n_getTagName(this.__nid) !== 'IFRAME') return undefined;
                    if (!iframeRealms[this.__nid]) __braille_maybe_init_iframe(this);
                    var realm = iframeRealms[this.__nid];
                    return realm ? realm.window : null;
                },
                configurable: true
            });

            Object.defineProperty(__ElemProto, 'contentDocument', {
                get: function() {
                    if (this.__nid === undefined) return undefined;
                    if (__n_getTagName(this.__nid) !== 'IFRAME') return undefined;
                    if (!iframeRealms[this.__nid]) __braille_maybe_init_iframe(this);
                    var realm = iframeRealms[this.__nid];
                    return realm ? realm.document : null;
                },
                configurable: true
            });

            var origReset = globalThis.__braille_reset_dom_cache;
            globalThis.__braille_reset_dom_cache = function() {
                if (origReset) origReset();
                for (var k in iframeRealms) delete iframeRealms[k];
            };
        })();
    "#,
    )
    .unwrap();
}
