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
                    .map(|v| v.to_string())
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
            var __iframeDocMap = {};

            // Single source of truth: does this src mean "about:blank" (no fetch needed)?
            function __isAboutBlankSrc(src) {
                return !src || src === 'about:blank';
            }

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
                                    target.dispatchEvent(event);
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
                    DOMException: (function() {
                        // Each iframe gets its own DOMException constructor per spec
                        if (typeof DOMException === 'undefined') return undefined;
                        var codeMap = {
                            IndexSizeError: 1, HierarchyRequestError: 3, WrongDocumentError: 4,
                            InvalidCharacterError: 5, NoModificationAllowedError: 7, NotFoundError: 8,
                            NotSupportedError: 9, InUseAttributeError: 10, InvalidStateError: 11,
                            SyntaxError: 12, InvalidModificationError: 13, NamespaceError: 14,
                            InvalidAccessError: 15, TypeMismatchError: 17, SecurityError: 18,
                            NetworkError: 19, AbortError: 20, URLMismatchError: 21,
                            QuotaExceededError: 22, TimeoutError: 23, InvalidNodeTypeError: 24,
                            DataCloneError: 25
                        };
                        var legacyNames = {
                            INDEX_SIZE_ERR: 1, DOMSTRING_SIZE_ERR: 2, HIERARCHY_REQUEST_ERR: 3,
                            WRONG_DOCUMENT_ERR: 4, INVALID_CHARACTER_ERR: 5, NO_DATA_ALLOWED_ERR: 6,
                            NO_MODIFICATION_ALLOWED_ERR: 7, NOT_FOUND_ERR: 8, NOT_SUPPORTED_ERR: 9,
                            INUSE_ATTRIBUTE_ERR: 10, INVALID_STATE_ERR: 11, SYNTAX_ERR: 12,
                            INVALID_MODIFICATION_ERR: 13, NAMESPACE_ERR: 14, INVALID_ACCESS_ERR: 15,
                            VALIDATION_ERR: 16, TYPE_MISMATCH_ERR: 17, SECURITY_ERR: 18,
                            NETWORK_ERR: 19, ABORT_ERR: 20, URL_MISMATCH_ERR: 21,
                            QUOTA_EXCEEDED_ERR: 22, TIMEOUT_ERR: 23, INVALID_NODE_TYPE_ERR: 24,
                            DATA_CLONE_ERR: 25
                        };
                        function IframeDOMException(message, name) {
                            this.message = message || '';
                            this.name = name || 'Error';
                            this.code = codeMap[this.name] || 0;
                            this.stack = (new Error()).stack;
                        }
                        IframeDOMException.prototype = Object.create(Object.prototype);
                        IframeDOMException.prototype.constructor = IframeDOMException;
                        IframeDOMException.prototype.toString = function() { return this.name + ': ' + this.message; };
                        for (var n in codeMap) { if (codeMap[n] > 0) { IframeDOMException[n] = codeMap[n]; IframeDOMException.prototype[n] = codeMap[n]; } }
                        for (var ln in legacyNames) { IframeDOMException[ln] = legacyNames[ln]; IframeDOMException.prototype[ln] = legacyNames[ln]; }
                        return IframeDOMException;
                    })(),
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
                iframeWindow.globalThis = iframeWindow;
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
                        iframeWindow.dispatchEvent(new Event('scrollend', {bubbles: false}));
                    }
                };
                iframeWindow.scroll = iframeWindow.scrollTo;
                iframeWindow.onscrollend = null;
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

                // Auto-copy all Web API constructors from parent window to iframe window.
                // This ensures AbortSignal, URLSearchParams, DOMParser, Blob, File,
                // FileReader, Headers, Request, Response, FormData, ReadableStream,
                // PerformanceObserver, MutationObserver, ResizeObserver, IntersectionObserver,
                // EventSource, etc. are all available in the iframe context.
                var IFRAME_GLOBALS = [
                    'URL', 'URLSearchParams', 'DOMParser', 'Blob', 'File', 'FileReader',
                    'Headers', 'Request', 'Response', 'FormData', 'ReadableStream',
                    'PerformanceObserver', 'MutationObserver', 'ResizeObserver', 'IntersectionObserver',
                    'EventSource', 'Worker', 'SharedWorker',
                    'Proxy', 'Reflect', 'WeakMap', 'WeakSet', 'WeakRef',
                    'Float32Array', 'Float64Array', 'Int8Array', 'Int16Array', 'Int32Array',
                    'Uint16Array', 'Uint32Array', 'Uint8ClampedArray',
                    'SharedArrayBuffer', 'Atomics',
                    'fetch', 'queueMicrotask', 'requestAnimationFrame', 'cancelAnimationFrame',
                    'requestIdleCallback', 'cancelIdleCallback',
                    'getComputedStyle', 'getSelection', 'matchMedia',
                    'structuredClone', 'reportError',
                    'UIEvent', 'FocusEvent', 'MouseEvent', 'KeyboardEvent', 'PointerEvent',
                    'InputEvent', 'WheelEvent', 'TouchEvent', 'AnimationEvent', 'TransitionEvent',
                    'ClipboardEvent', 'PopStateEvent', 'HashChangeEvent', 'StorageEvent',
                    'PromiseRejectionEvent', 'ErrorEvent',
                    'CSSStyleSheet', 'DOMRect', 'DOMRectReadOnly', 'DOMPoint', 'DOMPointReadOnly',
                    'DOMMatrix', 'DOMMatrixReadOnly', 'Range',
                    'NodeFilter', 'NodeIterator', 'TreeWalker', 'NodeList', 'HTMLCollection',
                    'MessageChannel', 'BroadcastChannel',
                    'Notification', 'OffscreenCanvas', 'ImageBitmap', 'createImageBitmap', 'ImageData',
                    'CanvasRenderingContext2D', 'Path2D', 'CanvasGradient', 'CanvasPattern',
                    'indexedDB', 'IDBKeyRange',
                    'DOMTokenList', 'DOMImplementation',
                    'alert', 'confirm', 'prompt',
                    'localStorage', 'sessionStorage',
                    'screen', 'visualViewport',
                    'CharacterData', 'Text', 'Comment', 'Document', 'XMLDocument',
                    'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement',
                    'HTMLFormElement', 'HTMLAnchorElement', 'HTMLImageElement',
                    'HTMLButtonElement', 'HTMLOptionElement', 'HTMLCanvasElement',
                    'HTMLVideoElement', 'HTMLAudioElement', 'HTMLIFrameElement',
                    'HTMLTemplateElement', 'HTMLScriptElement', 'HTMLStyleElement',
                    'SVGElement', 'Window',
                    'CustomElementRegistry', 'ShadowRoot',
                ];
                for (var gi = 0; gi < IFRAME_GLOBALS.length; gi++) {
                    var gname = IFRAME_GLOBALS[gi];
                    if (!(gname in iframeWindow) && typeof globalThis[gname] !== 'undefined') {
                        iframeWindow[gname] = globalThis[gname];
                    }
                }

                return { window: iframeWindow, parentProxy: parentProxy };
            }

            function execScriptInIframe(realm, code) {
                var iw = realm.window;
                var pp = realm._parentProxy;
                // Pre-populate common event handler properties on iw so that bare
                // assignments like `onload = function()...` go through the with() scope.
                // Without this, the assignment creates a global instead of setting iw.onload.
                var evtProps = ['onload','onerror','onmessage','onunload','onresize','onscroll',
                                'onclick','onkeydown','onkeyup','onkeypress','onfocus','onblur',
                                'onhashchange','onpopstate','onbeforeunload','onsubmit'];
                for (var ei = 0; ei < evtProps.length; ei++) {
                    if (!(evtProps[ei] in iw)) iw[evtProps[ei]] = null;
                }
                // Use with(window) — the real iw object, NOT a Proxy — to provide
                // bare-name access to iframe window properties. Unlike Proxy has:()=>true,
                // with(plainObject) correctly respects closure variable scope in QuickJS.
                var fn = new Function(
                    'window', 'document', 'self', 'parent', 'top', 'globalThis',
                    'postMessage', 'addEventListener', 'removeEventListener',
                    'setTimeout', 'setInterval', 'clearTimeout', 'clearInterval',
                    'console', 'location', 'navigator', 'JSON', 'MessageEvent',
                    'crypto', 'TextEncoder', 'TextDecoder',
                    'with(window) {\n' + code + '\n}'
                );
                fn(
                    iw, iw.document, iw, pp, pp, iw,
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
                            window.dispatchEvent(event);
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

            function __initSingleIframe(node) {
                if (!node || node.tagName !== 'IFRAME') return;
                if (node.__nid === undefined) return;
                if (iframeRealms[node.__nid]) return;

                var src = node.getAttribute('src');
                if (!__isAboutBlankSrc(src)) return;

                // Only init when connected to the document (not in a disconnected fragment)
                if (!node.isConnected) return;

                var iframeDoc = buildRealDomDocument(node.__nid);
                iframeDoc.contentType = 'text/html';
                __iframeDocMap[iframeDoc.__nid] = node.__nid;
                var built = buildIframeWindow(node, iframeDoc);

                var realm = {
                    window: built.window,
                    document: iframeDoc,
                    _parentProxy: built.parentProxy,
                    _iframeNodeId: node.__nid
                };
                iframeRealms[node.__nid] = realm;

                // Fire load event synchronously for about:blank iframes (per spec)
                // dispatchEvent invokes on<type> handlers via fireOnHandler — no manual call needed
                if (node.dispatchEvent) {
                    node.dispatchEvent(new Event('load'));
                }
            }

            // Initialize about:blank iframe realm on appendChild (+ scan descendants)
            globalThis.__braille_maybe_init_iframe = function(node) {
                if (!node || node.__nid === undefined) return;
                __initSingleIframe(node);
                // Also init any iframe descendants (e.g., div containing iframes)
                if (node.querySelectorAll) {
                    var iframes = node.querySelectorAll('iframe');
                    for (var i = 0; i < iframes.length; i++) {
                        __initSingleIframe(iframes[i]);
                    }
                }
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
                    // Check if we hit an iframe's document node
                    if (__n_getNodeType(cur) === 9 && __iframeDocMap[cur] !== undefined) {
                        return iframeRealms[__iframeDocMap[cur]] || null;
                    }
                    cur = __n_getParent(cur);
                }
                return null;
            };

            // Execute code in an iframe's scoped context
            globalThis.__braille_exec_in_iframe = function(realm, code) {
                execScriptInIframe(realm, code);
            };

            // Extract scripts from HTML in document order (both external and inline).
            // External scripts are resolved from __braille_worker_scripts (pre-fetched).
            // Skips non-JavaScript types (e.g. type="text/json", type="application/json").
            function extractScripts(html) {
                var scripts = [];
                var re = /<script[^>]*>([\s\S]*?)<\/script>/gi;
                var m;
                while ((m = re.exec(html)) !== null) {
                    var tag = m[0];
                    var header = tag.substring(0, tag.indexOf('>'));
                    // Skip non-JS script types (json, template, etc.)
                    var typeMatch = header.match(/\btype\s*=\s*["']([^"']+)["']/i);
                    if (typeMatch) {
                        var stype = typeMatch[1].toLowerCase();
                        if (stype !== 'text/javascript' && stype !== 'application/javascript' && stype !== 'module' && stype !== '') {
                            continue;
                        }
                    }
                    var srcMatch = header.match(/\bsrc\s*=\s*["']([^"']+)["']/i) || header.match(/\bsrc\s*=\s*([^\s>]+)/i);
                    if (srcMatch) {
                        var src = srcMatch[1];
                        var content = (typeof __braille_worker_scripts !== 'undefined' && __braille_worker_scripts[src]) ? __braille_worker_scripts[src] : '';
                        if (content) scripts.push(content);
                    } else {
                        if (m[1].trim()) scripts.push(m[1]);
                    }
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
                iframeDoc.contentType = 'text/html';

                var built = buildIframeWindow(iframeEl, iframeDoc);

                var realm = {
                    window: built.window,
                    document: iframeDoc,
                    _parentProxy: built.parentProxy,
                    _iframeNodeId: iframeNodeId
                };
                iframeRealms[iframeNodeId] = realm;
                __iframeDocMap[iframeDoc.__nid] = iframeNodeId;

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
            // Create about:blank realms for parser-inserted iframes (no onload).
            // Called before scripts so contentWindow is available per spec.
            globalThis.__braille_init_iframe_realms = function() {
                var iframeIds = __braille_find_iframes();
                for (var i = 0; i < iframeIds.length; i++) {
                    var nid = iframeIds[i];
                    if (iframeRealms[nid]) continue;
                    var src = __braille_iframe_get_src(nid);
                    if (!__isAboutBlankSrc(src)) continue;
                    __braille_create_iframe_realm(nid, '<html><head></head><body></body></html>');
                }
            };

            globalThis.__braille_process_iframes = function() {
                var iframeIds = __braille_find_iframes();
                for (var i = 0; i < iframeIds.length; i++) {
                    var nid = iframeIds[i];
                    if (iframeRealms[nid]) continue;

                    var src = __braille_iframe_get_src(nid);
                    var content;
                    var fragment = null;
                    if (__isAboutBlankSrc(src)) {
                        content = '<html><head></head><body></body></html>';
                    } else {
                        // Strip URL fragment before content lookup
                        var srcNoFrag = src;
                        var hashIdx = src.indexOf('#');
                        if (hashIdx >= 0) {
                            fragment = src.substring(hashIdx + 1);
                            srcNoFrag = src.substring(0, hashIdx);
                        }
                        content = __braille_iframe_lookup_content(srcNoFrag);
                        if (!content) content = __braille_iframe_lookup_content(src);
                        if (!content) continue;
                    }

                    var realm = __braille_create_iframe_realm(nid, content);

                    // Set URL fragment for :target pseudo-class support
                    if (fragment && realm && realm.document && realm.document.__nid !== undefined) {
                        __n_setUrlFragment(realm.document.__nid, fragment);
                    }

                    // Mark XML documents: if src ends with .xml or .xhtml, flag as non-HTML
                    if (src && realm && realm.document) {
                        var srcLower = src.toLowerCase();
                        if (srcLower.indexOf('.xml') === srcLower.length - 4 ||
                            srcLower.indexOf('.xhtml') === srcLower.length - 6 ||
                            srcLower.indexOf('.svg') === srcLower.length - 4) {
                            realm.document.__isXML = true;
                            realm.document.contentType = 'application/xml';
                        }
                    }

                    // Fire load event on the iframe's window (for window.onload handlers)
                    if (realm && realm.window) {
                        var loadEvt = new Event('load');
                        if (typeof realm.window.onload === 'function') {
                            realm.window.onload(loadEvt);
                        }
                    }
                    // Fire load event on the iframe element
                    var el = __braille_get_element_wrapper(nid);
                    if (el) {
                        if (el.dispatchEvent) {
                            el.dispatchEvent(new Event('load'));
                        }
                    }
                }
            };

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

            // Dynamic iframe.src setter: when src is set to a real URL (not about:blank),
            // look up pre-fetched content and create the iframe realm.
            Object.defineProperty(HTMLIFrameElement.prototype, 'src', {
                get: function() {
                    if (this.__nid === undefined) return '';
                    return __n_getAttribute(this.__nid, 'src') || '';
                },
                set: function(v) {
                    var val = String(v);
                    if (this.__nid === undefined) return;
                    __n_setAttribute(this.__nid, 'src', val);
                    // If already has a realm, destroy it first
                    if (iframeRealms[this.__nid]) {
                        // Clear children
                        var kids = __n_getAllChildIds(this.__nid);
                        for (var ci = kids.length - 1; ci >= 0; ci--) {
                            __n_removeChild(this.__nid, kids[ci]);
                        }
                        delete iframeRealms[this.__nid];
                    }
                    if (__isAboutBlankSrc(val)) {
                        __initSingleIframe(this);
                        return;
                    }
                    // Strip fragment for content lookup
                    var srcNoFrag = val;
                    var fragment = null;
                    var hashIdx = val.indexOf('#');
                    if (hashIdx >= 0) {
                        fragment = val.substring(hashIdx + 1);
                        srcNoFrag = val.substring(0, hashIdx);
                    }
                    var content = __braille_iframe_lookup_content(srcNoFrag);
                    if (!content) content = __braille_iframe_lookup_content(val);
                    if (!content) return;
                    var realm = __braille_create_iframe_realm(this.__nid, content);
                    // Set URL fragment for :target
                    if (fragment && realm && realm.document && realm.document.__nid !== undefined) {
                        __n_setUrlFragment(realm.document.__nid, fragment);
                    }
                    // Fire load on iframe's window (for window.onload handlers)
                    if (realm && realm.window && typeof realm.window.onload === 'function') {
                        realm.window.onload(new Event('load'));
                    }
                    // Fire load on iframe element
                    if (this.dispatchEvent) {
                        this.dispatchEvent(new Event('load'));
                    }
                },
                configurable: true, enumerable: true,
            });

            // window.open() — creates a new window with its own document, reusing
            // the same iframe realm infrastructure (buildIframeWindow, __makeDocumentLike).
            var popupRealms = [];
            globalThis.open = function(url, target, features) {
                // Create a standalone document node with html>head+body
                var docNid = __n_createDocumentNode();
                var htmlNid = __n_createElement('html');
                __n_appendChild(docNid, htmlNid);
                var headNid = __n_createElement('head');
                __n_appendChild(htmlNid, headNid);
                var bodyNid = __n_createElement('body');
                __n_appendChild(htmlNid, bodyNid);
                var htmlEl = __w(htmlNid);
                var popupDoc = __makeDocumentLike(htmlEl);
                popupDoc.contentType = 'text/html';

                // Build a window object around this document (null iframeEl — it's a popup)
                var built = buildIframeWindow(null, popupDoc);
                var popupWin = built.window;
                popupWin.opener = window;
                popupWin.closed = false;
                popupWin.close = function() { popupWin.closed = true; };
                popupRealms.push(popupWin);
                return popupWin;
            };

            var origReset = globalThis.__braille_reset_dom_cache;
            globalThis.__braille_reset_dom_cache = function() {
                if (origReset) origReset();
                for (var k in iframeRealms) delete iframeRealms[k];
                for (var k in __iframeDocMap) delete __iframeDocMap[k];
                popupRealms = [];
            };
        })();
    "#,
    )
    .unwrap();
}
