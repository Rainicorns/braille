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

            // Extract inline <script> content from HTML (simple regex, like Worker does)
            function extractScripts(html) {
                var scripts = [];
                var re = /<script[^>]*>([\s\S]*?)<\/script>/gi;
                var m;
                while ((m = re.exec(html)) !== null) {
                    var tag = m[0];
                    // Skip external scripts (those with src attribute)
                    if (/\bsrc\s*=/i.test(tag.substring(0, tag.indexOf('>')))) continue;
                    if (m[1].trim()) scripts.push(m[1]);
                }
                return scripts;
            }

            // Create an iframe realm for a given iframe nodeId and HTML content
            globalThis.__braille_create_iframe_realm = function(iframeNodeId, html) {
                var iframeEl = __braille_get_element_wrapper(iframeNodeId);

                // Build the realm's window proxy and document
                var realm = {
                    _listeners: {},
                    _onmessage: null
                };

                // Iframe document stub — minimal DOM for challenge scripts
                var iframeDoc = {
                    nodeType: 9,
                    nodeName: '#document',
                    readyState: 'complete',
                    cookie: '',
                    title: '',
                    _elements: [],
                    _elementsById: {},
                    createElement: function(tag) {
                        var el = {
                            tagName: tag.toUpperCase(),
                            nodeName: tag.toUpperCase(),
                            nodeType: 1,
                            childNodes: [],
                            children: [],
                            attributes: {},
                            style: {},
                            className: '',
                            id: '',
                            textContent: '',
                            innerHTML: '',
                            _listeners: {},
                            appendChild: function(child) {
                                this.childNodes.push(child);
                                this.children.push(child);
                                child.parentNode = this;
                                child.parentElement = this;
                                // Track in doc's element lists
                                iframeDoc._elements.push(child);
                                if (child.id) iframeDoc._elementsById[child.id] = child;
                                return child;
                            },
                            removeChild: function(child) {
                                this.childNodes = this.childNodes.filter(function(c) { return c !== child; });
                                this.children = this.children.filter(function(c) { return c !== child; });
                                return child;
                            },
                            getAttribute: function(name) { return this.attributes[name] || null; },
                            setAttribute: function(name, value) {
                                this.attributes[name] = String(value);
                                if (name === 'id') {
                                    this.id = String(value);
                                    iframeDoc._elementsById[this.id] = this;
                                }
                            },
                            removeAttribute: function(name) { delete this.attributes[name]; },
                            hasAttribute: function(name) { return name in this.attributes; },
                            addEventListener: function(type, cb) {
                                if (!this._listeners[type]) this._listeners[type] = [];
                                this._listeners[type].push(cb);
                            },
                            removeEventListener: function(type, cb) {
                                if (this._listeners[type]) {
                                    this._listeners[type] = this._listeners[type].filter(function(f) { return f !== cb; });
                                }
                            },
                            dispatchEvent: function(event) {
                                var cbs = this._listeners[event.type];
                                if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(this, event); }
                                return true;
                            },
                            querySelector: function() { return null; },
                            querySelectorAll: function() { return []; },
                            getBoundingClientRect: function() { return {top:0,left:0,right:0,bottom:0,width:0,height:0}; },
                            cloneNode: function() { return iframeDoc.createElement(this.tagName.toLowerCase()); }
                        };
                        return el;
                    },
                    createTextNode: function(text) {
                        return { nodeType: 3, textContent: text, nodeName: '#text', data: text };
                    },
                    createDocumentFragment: function() {
                        return {
                            nodeType: 11,
                            childNodes: [],
                            children: [],
                            appendChild: function(child) { this.childNodes.push(child); this.children.push(child); return child; },
                            querySelectorAll: function() { return []; }
                        };
                    },
                    getElementById: function(id) {
                        return iframeDoc._elementsById[id] || null;
                    },
                    querySelector: function(sel) {
                        // Very basic: only supports #id selectors
                        if (sel.charAt(0) === '#') return iframeDoc._elementsById[sel.substring(1)] || null;
                        return null;
                    },
                    querySelectorAll: function() { return []; },
                    addEventListener: function(type, cb) {
                        if (!iframeDoc._docListeners) iframeDoc._docListeners = {};
                        if (!iframeDoc._docListeners[type]) iframeDoc._docListeners[type] = [];
                        iframeDoc._docListeners[type].push(cb);
                    },
                    removeEventListener: function(type, cb) {
                        if (iframeDoc._docListeners && iframeDoc._docListeners[type]) {
                            iframeDoc._docListeners[type] = iframeDoc._docListeners[type].filter(function(f) { return f !== cb; });
                        }
                    },
                    dispatchEvent: function(event) {
                        if (iframeDoc._docListeners) {
                            var cbs = iframeDoc._docListeners[event.type];
                            if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(iframeDoc, event); }
                        }
                        return true;
                    },
                    createEvent: function(type) {
                        return new Event(type);
                    }
                };

                // Body element
                var body = iframeDoc.createElement('body');
                iframeDoc.body = body;
                iframeDoc.documentElement = iframeDoc.createElement('html');
                iframeDoc.documentElement.appendChild(body);
                iframeDoc.head = iframeDoc.createElement('head');

                // Parent proxy: wraps the real parent window but overrides postMessage
                // so that event.source is the iframe's window, not the parent's.
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

                // Build the iframe window proxy
                var iframeWindow = {
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
                    // iframe's postMessage: receives messages FROM parent
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

                // Store the realm
                realm.window = iframeWindow;
                realm.document = iframeDoc;
                iframeRealms[iframeNodeId] = realm;

                // Extract and execute scripts from the iframe HTML
                var scripts = extractScripts(html || '');
                for (var i = 0; i < scripts.length; i++) {
                    var fn = new Function(
                        'window', 'document', 'self', 'parent', 'top',
                        'postMessage', 'addEventListener', 'removeEventListener',
                        'setTimeout', 'setInterval', 'clearTimeout', 'clearInterval',
                        'console', 'location', 'navigator', 'JSON', 'MessageEvent',
                        'crypto', 'TextEncoder', 'TextDecoder',
                        scripts[i]
                    );
                    fn(
                        iframeWindow, iframeDoc, iframeWindow, parentProxy, parentProxy,
                        function(data, targetOrigin) {
                            // iframe calling postMessage -> deliver to parent
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
                                // Deliver to parent window message listeners
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
                        iframeWindow.addEventListener,
                        iframeWindow.removeEventListener,
                        setTimeout, setInterval, clearTimeout, clearInterval,
                        console,
                        iframeWindow.location, iframeWindow.navigator, JSON, MessageEvent,
                        (typeof crypto !== 'undefined') ? crypto : undefined,
                        (typeof TextEncoder !== 'undefined') ? TextEncoder : undefined,
                        (typeof TextDecoder !== 'undefined') ? TextDecoder : undefined
                    );
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
                    // Skip if already processed
                    if (iframeRealms[nid]) continue;

                    var src = __braille_iframe_get_src(nid);
                    if (!src) continue;

                    var content = __braille_iframe_lookup_content(src);
                    if (!content) continue;

                    __braille_create_iframe_realm(nid, content);

                    // Fire iframe.onload
                    var el = __braille_get_element_wrapper(nid);
                    if (el) {
                        var loadEvent = new Event('load');
                        if (typeof el.onload === 'function') {
                            el.onload(loadEvent);
                        }
                        // Also dispatch via addEventListener
                        if (el.dispatchEvent) {
                            el.dispatchEvent(loadEvent);
                        }
                    }
                }
            };

            // contentWindow/contentDocument on __ElemProto (the actual prototype
            // chain used by wrapper objects). Check tag name to only work on iframes.
            Object.defineProperty(__ElemProto, 'contentWindow', {
                get: function() {
                    if (this.__nid === undefined) return undefined;
                    if (__n_getTagName(this.__nid) !== 'IFRAME') return undefined;
                    var realm = iframeRealms[this.__nid];
                    return realm ? realm.window : null;
                },
                configurable: true
            });

            Object.defineProperty(__ElemProto, 'contentDocument', {
                get: function() {
                    if (this.__nid === undefined) return undefined;
                    if (__n_getTagName(this.__nid) !== 'IFRAME') return undefined;
                    var realm = iframeRealms[this.__nid];
                    return realm ? realm.document : null;
                },
                configurable: true
            });

            // Reset iframe realms on page rebind
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
