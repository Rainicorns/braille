use rquickjs::{Ctx, Function};

use crate::js::dom_bridge::with_state_mut;

pub(super) fn register_dom_stubs(ctx: &Ctx<'_>) {
    // Register __braille_navigate — called by location.href setter to signal pending navigation
    let navigate_fn = Function::new(ctx.clone(), move |url: String| {
        with_state_mut(|s| s.pending_navigation = Some(url));
    }).unwrap();
    ctx.globals().set("__braille_navigate", navigate_fn).unwrap();
    // Comprehensive DOM/Web API stubs so real-world JS doesn't crash on missing globals.
    // These are JS-level stubs that provide the right shape but no real DOM integration.
    // Critical DOM operations (createElement, appendChild, etc.) are backed by native
    // Rust functions that operate on the real DomTree.
    ctx.eval::<(), _>(r#"
        globalThis.window = globalThis;
        globalThis.self = globalThis;
        globalThis.isSecureContext = true;
        globalThis.document = { nodeType: 9, nodeName: '#document', readyState: 'complete', cookie: '', title: '', defaultView: globalThis };

        // Shared HTMLCollection factory — returns a live Proxy with:
        //   - Brand check on .length (TypeError if receiver !== the proxy)
        //   - Named item access (elements by id/name attribute)
        //   - namedItem() method
        //   - Proper iteration
        // Array index check: non-negative integer < 2^32 - 1 (per ECMAScript spec)
        function __isArrayIndex(p) {
            if (typeof p !== 'string') return false;
            var n = Number(p);
            return n === (n >>> 0) && n !== 0xFFFFFFFF && String(n >>> 0) === p;
        }

        function __findNamed(live, name) {
            var s = String(name);
            if (!s) return null;
            for (var i = 0; i < live.length; i++) {
                var el = live[i];
                if (!el.getAttribute) continue;
                if (el.getAttribute('id') === s) return el;
                // name attribute only applies to elements in HTML namespace
                var ns = el.namespaceURI;
                if ((!ns || ns === 'http://www.w3.org/1999/xhtml') && el.getAttribute('name') === s) return el;
            }
            return null;
        }

        globalThis.__makeHTMLCollection = function(queryFn) {
            var proxy;
            proxy = new Proxy(Object.create(null), {
                get: function(t, p, receiver) {
                    if (p === Symbol.toStringTag) return undefined;
                    if (p === Symbol.iterator) {
                        var items = queryFn();
                        return function() { return items[Symbol.iterator](); };
                    }
                    var live = queryFn();
                    if (p === 'length') {
                        if (receiver !== proxy) throw new TypeError("Illegal invocation");
                        return live.length;
                    }
                    if (p === 'item') return function(i) { var idx = i >>> 0; return idx < live.length ? live[idx] : null; };
                    if (p === 'namedItem') return function(name) { return __findNamed(live, name); };
                    // Array index → indexed access
                    if (__isArrayIndex(p)) return live[p >>> 0];
                    // HTMLCollection does NOT have Array iterable methods
                    if (p === 'forEach' || p === 'values' || p === 'entries' || p === 'keys' ||
                        p === 'map' || p === 'filter' || p === 'reduce' || p === 'find' ||
                        p === 'findIndex' || p === 'some' || p === 'every' || p === 'includes' ||
                        p === 'indexOf' || p === 'flat' || p === 'flatMap' || p === 'fill' ||
                        p === 'copyWithin' || p === 'at' || p === 'push' || p === 'pop' ||
                        p === 'shift' || p === 'unshift' || p === 'splice' || p === 'slice' ||
                        p === 'concat' || p === 'join' || p === 'reverse' || p === 'sort' ||
                        p === 'toString' || p === 'toLocaleString' || p === 'toReversed' ||
                        p === 'toSorted' || p === 'toSpliced' || p === 'with') return undefined;
                    // Own properties on target shadow named item access
                    if (typeof p === 'string' && Object.prototype.hasOwnProperty.call(t, p)) return t[p];
                    // Named item access (skip empty string and JS internals)
                    if (typeof p === 'string' && p !== '' && p !== 'then' && p !== 'toJSON' && p !== 'constructor' && p !== '__proto__') {
                        var found = __findNamed(live, p);
                        if (found) return found;
                    }
                    return undefined;
                },
                has: function(t, p) {
                    if (p === Symbol.iterator || p === 'length' || p === 'item' || p === 'namedItem') return true;
                    var live = queryFn();
                    if (__isArrayIndex(p)) return (p >>> 0) < live.length;
                    if (typeof p === 'string' && p !== '') {
                        if (Object.prototype.hasOwnProperty.call(t, p)) return true;
                        if (__findNamed(live, p)) return true;
                    }
                    return false;
                },
                ownKeys: function(t) {
                    var live = queryFn();
                    var keys = [];
                    for (var i = 0; i < live.length; i++) keys.push(String(i));
                    var seen = {};
                    for (var i = 0; i < live.length; i++) {
                        var el = live[i];
                        if (!el.getAttribute) continue;
                        var id = el.getAttribute('id');
                        if (id && !seen[id]) { keys.push(id); seen[id] = true; }
                        var ns = el.namespaceURI;
                        if (!ns || ns === 'http://www.w3.org/1999/xhtml') {
                            var nm = el.getAttribute('name');
                            if (nm && !seen[nm]) { keys.push(nm); seen[nm] = true; }
                        }
                    }
                    // Include expando keys from target
                    var tKeys = Object.keys(t);
                    for (var i = 0; i < tKeys.length; i++) {
                        if (seen[tKeys[i]] === undefined && keys.indexOf(tKeys[i]) === -1) keys.push(tKeys[i]);
                    }
                    return keys;
                },
                getOwnPropertyDescriptor: function(t, p) {
                    var live = queryFn();
                    if (__isArrayIndex(p)) {
                        var idx = p >>> 0;
                        if (idx < live.length) return { value: live[idx], writable: false, enumerable: true, configurable: true };
                        return undefined;
                    }
                    if (typeof p === 'string' && p !== '') {
                        // Check expando first
                        if (Object.prototype.hasOwnProperty.call(t, p)) {
                            return { value: t[p], writable: true, enumerable: true, configurable: true };
                        }
                        var found = __findNamed(live, p);
                        if (found) return { value: found, writable: false, enumerable: false, configurable: true };
                    }
                    return undefined;
                },
                set: function(t, p, value) {
                    // Array indices: always reject
                    if (__isArrayIndex(p)) return false;
                    // Named properties: reject if matching element exists
                    if (typeof p === 'string') {
                        var live = queryFn();
                        if (__findNamed(live, p)) return false;
                    }
                    // No matching element: store on target
                    t[p] = value;
                    return true;
                },
                defineProperty: function(t, p, desc) {
                    // Reject defining indexed properties
                    if (__isArrayIndex(p)) return false;
                    // Reject if it shadows a named element
                    if (typeof p === 'string') {
                        var live = queryFn();
                        if (__findNamed(live, p)) return false;
                    }
                    Object.defineProperty(t, p, desc);
                    return true;
                },
                deleteProperty: function(t, p) {
                    var live = queryFn();
                    // If there's an expando on target, delete it (even if named element exists)
                    if (Object.prototype.hasOwnProperty.call(t, p)) {
                        var desc = Object.getOwnPropertyDescriptor(t, p);
                        if (desc && !desc.configurable) return false;
                        delete t[p];
                        return true;
                    }
                    // In-range array index: reject
                    if (__isArrayIndex(p) && (p >>> 0) < live.length) return false;
                    // Named element exists: reject
                    if (typeof p === 'string' && __findNamed(live, p)) return false;
                    // Out-of-range index or no matching element: allow
                    return true;
                }
            });
            return proxy;
        };

        // Shared getter for Event.isTrusted (unforgeable, same function on all instances per spec)
        var __isTrustedGetter = function() { return this._isTrusted; };

        // Event classes
        globalThis.Event = globalThis.Event || class Event {
            constructor(type, opts) {
                if (arguments.length < 1) throw new TypeError("Failed to construct 'Event': 1 argument required, but only 0 present.");
                this.type = String(type);
                this.bubbles = (opts && opts.bubbles) || false;
                this.cancelable = (opts && opts.cancelable) || false;
                this.composed = (opts && opts.composed) || false;
                this.defaultPrevented = false;
                this._returnValue = true;
                this.target = null;
                this.currentTarget = null;
                this.srcElement = null;
                this.eventPhase = 0;
                this._isTrusted = false;
                Object.defineProperty(this, 'isTrusted', {get: __isTrustedGetter, configurable: false});
                this.timeStamp = performance.now();
                this._stopPropagation = false;
                this._stopImmediate = false;
                this._dispatching = false;
                this._initialized = true;
                this._inPassiveListener = false;
            }
            get returnValue() { return this._returnValue; }
            set returnValue(v) {
                if (!v && this.cancelable && !this._inPassiveListener) {
                    this._returnValue = false;
                    this.defaultPrevented = true;
                }
            }
            get cancelBubble() { return this._stopPropagation; }
            set cancelBubble(v) { if (v) this._stopPropagation = true; }
            preventDefault() { if (this.cancelable && !this._inPassiveListener) { this.defaultPrevented = true; this._returnValue = false; } }
            stopPropagation() { this._stopPropagation = true; }
            stopImmediatePropagation() { this._stopImmediate = true; this._stopPropagation = true; }
            composedPath() { if (!this._dispatching && this.eventPhase === 0) return []; return this._path || []; }
            initEvent(type, bubbles, cancelable) {
                if (arguments.length < 1) throw new TypeError("Failed to execute 'initEvent' on 'Event': 1 argument required, but only 0 present.");
                if (this._dispatching) return;
                this._stopPropagation = false;
                this._stopImmediate = false;
                this.defaultPrevented = false;
                this._returnValue = true;
                this._isTrusted = false;
                this.target = null;
                this.srcElement = null;
                this.type = String(type);
                this.bubbles = !!bubbles;
                this.cancelable = !!cancelable;
                this._initialized = true;
            }
        };
        Event.NONE = 0;
        Event.CAPTURING_PHASE = 1;
        Event.AT_TARGET = 2;
        Event.BUBBLING_PHASE = 3;
        Event.prototype.NONE = 0;
        Event.prototype.CAPTURING_PHASE = 1;
        Event.prototype.AT_TARGET = 2;
        Event.prototype.BUBBLING_PHASE = 3;
        globalThis.CustomEvent = class CustomEvent extends Event {
            constructor(type, opts) { super(type, opts); this.detail = (opts && opts.detail !== undefined ? opts.detail : null); }
            initCustomEvent(type) {
                if (arguments.length < 1) throw new TypeError("Failed to execute 'initCustomEvent' on 'CustomEvent': 1 argument required, but only 0 present.");
                this.initEvent(type, arguments[1], arguments[2]);
                this.detail = arguments.length > 3 ? arguments[3] : null;
            }
        };
        globalThis.UIEvent = class UIEvent extends Event {
            constructor(type, opts) {
                super(type, opts);
                var v = opts && opts.view !== undefined ? opts.view : null;
                if (v !== null && v !== window) {
                    throw new TypeError("Failed to construct 'UIEvent': member view is not of type Window.");
                }
                this.view = v;
                this.detail = (opts && opts.detail) || 0;
            }
        };
        globalThis.FocusEvent = class FocusEvent extends UIEvent {
            constructor(type, opts) {
                super(type, opts);
                this.relatedTarget = (opts && opts.relatedTarget !== undefined) ? opts.relatedTarget : null;
            }
        };
        globalThis.MouseEvent = class MouseEvent extends UIEvent {
            constructor(type, opts) {
                super(type, opts);
                this.screenX = (opts && opts.screenX) || 0;
                this.screenY = (opts && opts.screenY) || 0;
                this.clientX = (opts && opts.clientX) || 0;
                this.clientY = (opts && opts.clientY) || 0;
                this.button = (opts && opts.button) || 0;
                this.buttons = (opts && opts.buttons) || 0;
                this.relatedTarget = (opts && opts.relatedTarget !== undefined) ? opts.relatedTarget : null;
                this.ctrlKey = !!(opts && opts.ctrlKey);
                this.shiftKey = !!(opts && opts.shiftKey);
                this.altKey = !!(opts && opts.altKey);
                this.metaKey = !!(opts && opts.metaKey);
            }
        };
        globalThis.KeyboardEvent = class KeyboardEvent extends UIEvent {
            constructor(type, opts) {
                super(type, opts);
                this.key = (opts && opts.key) || '';
                this.code = (opts && opts.code) || '';
                this.location = (opts && opts.location) || 0;
                this.repeat = !!(opts && opts.repeat);
                this.isComposing = !!(opts && opts.isComposing);
                this.charCode = (opts && opts.charCode) || 0;
                this.keyCode = (opts && opts.keyCode) || 0;
                this.which = (opts && opts.which) || 0;
                this.ctrlKey = !!(opts && opts.ctrlKey);
                this.shiftKey = !!(opts && opts.shiftKey);
                this.altKey = !!(opts && opts.altKey);
                this.metaKey = !!(opts && opts.metaKey);
            }
        };
        globalThis.InputEvent = class InputEvent extends UIEvent {
            constructor(type, opts) { super(type, opts); this.data = (opts && opts.data) || null; this.inputType = (opts && opts.inputType) || ''; }
        };
        globalThis.AnimationEvent = class AnimationEvent extends Event { constructor(t,o){super(t,o);} };
        globalThis.TransitionEvent = class TransitionEvent extends Event { constructor(t,o){super(t,o);} };
        globalThis.WheelEvent = class WheelEvent extends MouseEvent {
            constructor(type, opts) {
                super(type, opts);
                this.deltaX = (opts && opts.deltaX) || 0.0;
                this.deltaY = (opts && opts.deltaY) || 0.0;
                this.deltaZ = (opts && opts.deltaZ) || 0.0;
                this.deltaMode = (opts && opts.deltaMode) || 0;
            }
        };
        globalThis.CompositionEvent = class CompositionEvent extends UIEvent {
            constructor(type, opts) {
                super(type, opts);
                this.data = (opts && opts.data !== undefined) ? String(opts.data) : '';
            }
        };
        globalThis.ErrorEvent = class ErrorEvent extends Event { constructor(t,o){super(t,o);this.message=o&&o.message||'';this.filename=o&&o.filename||'';} };
        globalThis.GamepadEvent = class GamepadEvent extends Event {
            constructor(type, opts) { super(type, opts); this.gamepad = (opts && opts.gamepad) || null; }
        };
        globalThis.PointerEvent = class PointerEvent extends MouseEvent {
            constructor(t,o){super(t,o);this.pointerId=(o&&o.pointerId)||0;this.width=(o&&o.width)||1;this.height=(o&&o.height)||1;this.pressure=(o&&o.pressure)||0;this.tiltX=(o&&o.tiltX)||0;this.tiltY=(o&&o.tiltY)||0;this.pointerType=(o&&o.pointerType)||'mouse';this.isPrimary=(o&&o.isPrimary)!==undefined?o.isPrimary:true;}
        };
        globalThis.TouchEvent = class TouchEvent extends UIEvent {
            constructor(t,o){super(t,o);this.touches=(o&&o.touches)||[];this.targetTouches=(o&&o.targetTouches)||[];this.changedTouches=(o&&o.changedTouches)||[];}
        };
        globalThis.Touch = class Touch {
            constructor(o){this.identifier=(o&&o.identifier)||0;this.target=(o&&o.target)||null;this.clientX=(o&&o.clientX)||0;this.clientY=(o&&o.clientY)||0;this.pageX=(o&&o.pageX)||0;this.pageY=(o&&o.pageY)||0;}
        };
        globalThis.ClipboardEvent = class ClipboardEvent extends Event {
            constructor(t,o){super(t,o);this.clipboardData=(o&&o.clipboardData)||{getData:function(){return '';},setData:function(){},types:[]};}
        };
        globalThis.DragEvent = class DragEvent extends MouseEvent {
            constructor(t,o){super(t,o);this.dataTransfer=(o&&o.dataTransfer)||{getData:function(){return '';},setData:function(){},setDragImage:function(){},dropEffect:'none',effectAllowed:'all',types:[],files:[]};}
        };
        globalThis.PopStateEvent = class PopStateEvent extends Event {
            constructor(t,o){super(t,o);this.state=(o&&o.state)||null;}
        };
        globalThis.HashChangeEvent = class HashChangeEvent extends Event {
            constructor(t,o){super(t,o);this.oldURL=(o&&o.oldURL)||'';this.newURL=(o&&o.newURL)||'';}
        };
        globalThis.PromiseRejectionEvent = class PromiseRejectionEvent extends Event {
            constructor(t,o){super(t,o);this.promise=(o&&o.promise)||null;this.reason=(o&&o.reason)||undefined;}
        };
        globalThis.StorageEvent = class StorageEvent extends Event {
            constructor(t,o){super(t,o);this.key=(o&&o.key)||null;this.oldValue=(o&&o.oldValue)||null;this.newValue=(o&&o.newValue)||null;this.url=(o&&o.url)||'';this.storageArea=(o&&o.storageArea)||null;}
        };

        // Window dimensions
        window.innerWidth = 1280;
        window.innerHeight = 800;
        window.outerWidth = 1280;
        window.outerHeight = 900;
        window.devicePixelRatio = 1;
        window.__scrollX = 0;
        window.__scrollY = 0;
        Object.defineProperty(window, 'scrollX', {
            get: function() { return window.__scrollX; },
            set: function(v) { window.__scrollX = v|0; },
            configurable: true
        });
        Object.defineProperty(window, 'scrollY', {
            get: function() { return window.__scrollY; },
            set: function(v) { window.__scrollY = v|0; },
            configurable: true
        });
        Object.defineProperty(window, 'pageXOffset', {
            get: function() { return window.__scrollX; },
            configurable: true
        });
        Object.defineProperty(window, 'pageYOffset', {
            get: function() { return window.__scrollY; },
            configurable: true
        });
        window.scrollTo = function(xOrOpts, y) {
            var nx, ny;
            if (typeof xOrOpts === 'object' && xOrOpts !== null) {
                nx = ('left' in xOrOpts) ? xOrOpts.left|0 : window.__scrollX;
                ny = ('top' in xOrOpts) ? xOrOpts.top|0 : window.__scrollY;
            } else {
                nx = (xOrOpts|0);
                ny = (y|0);
            }
            if (nx < 0) nx = 0;
            if (ny < 0) ny = 0;
            var docEl = document.documentElement;
            if (docEl) {
                var maxX = docEl.scrollWidth - (window.innerWidth || 1280);
                var maxY = docEl.scrollHeight - (window.innerHeight || 800);
                if (maxX > 0 && nx > maxX) nx = maxX;
                if (maxY > 0 && ny > maxY) ny = maxY;
            }
            var changed = (nx !== window.__scrollX || ny !== window.__scrollY);
            window.__scrollX = nx;
            window.__scrollY = ny;
            if (changed) {
                window.dispatchEvent(new Event('scroll', {bubbles: false}));
                window.dispatchEvent(new Event('scrollend', {bubbles: false}));
            }
        };
        window.scroll = window.scrollTo;
        window.onscrollend = null;
        window.scrollBy = function(xOrOpts, y) {
            var dx, dy;
            if (typeof xOrOpts === 'object' && xOrOpts !== null) {
                dx = xOrOpts.left || 0;
                dy = xOrOpts.top || 0;
            } else {
                dx = xOrOpts || 0;
                dy = y || 0;
            }
            window.scrollTo(window.__scrollX + dx, window.__scrollY + dy);
        };
        window.screen = { width: 1280, height: 800, availWidth: 1280, availHeight: 800, colorDepth: 24, pixelDepth: 24, orientation: { type: 'landscape-primary', angle: 0, addEventListener: function(){}, removeEventListener: function(){} } };
        window.visualViewport = (function() {
            var vv = { width: 1280, height: 800, offsetLeft: 0, offsetTop: 0, pageLeft: 0, pageTop: 0, scale: 1, __listeners: {} };
            vv.addEventListener = function(type, cb, opts) {
                if (!vv.__listeners[type]) vv.__listeners[type] = [];
                var wrapped = cb;
                if (opts && (opts.once || opts === true)) {
                    wrapped = function __once(e) { vv.removeEventListener(type, wrapped); cb.call(vv, e); };
                    wrapped._orig = cb;
                }
                // Prevent duplicates
                for (var k = 0; k < vv.__listeners[type].length; k++) {
                    var ex = vv.__listeners[type][k];
                    if (ex === cb || ex._orig === cb) return;
                }
                vv.__listeners[type].push(wrapped);
            };
            vv.removeEventListener = function(type, cb) {
                var arr = vv.__listeners[type];
                if (arr) {
                    for (var k = arr.length - 1; k >= 0; k--) {
                        if (arr[k] === cb || arr[k]._orig === cb) { arr.splice(k, 1); break; }
                    }
                }
            };
            vv.dispatchEvent = function(event) {
                event.target = vv;
                event.currentTarget = vv;
                var cbs = vv.__listeners[event.type];
                if (cbs) { var snap = cbs.slice(); for (var i = 0; i < snap.length; i++) snap[i].call(vv, event); }
                var handler = vv['on' + event.type];
                if (typeof handler === 'function') handler.call(vv, event);
                return !event.defaultPrevented;
            };
            // IDL event handler properties
            vv.onresize = null; vv.onscroll = null; vv.onscrollend = null;
            return vv;
        })();

        // Navigator
        globalThis.navigator = {
            userAgent: 'Mozilla/5.0 (compatible; Braille/0.1)',
            language: 'en-US',
            languages: ['en-US'],
            platform: 'Linux',
            onLine: true,
            cookieEnabled: true,
            maxTouchPoints: 0,
            hardwareConcurrency: 1,
            vendor: 'Google Inc.',
            clipboard: (function() {
                var _buf = '';
                return {
                    writeText: function(text) { _buf = String(text); return Promise.resolve(); },
                    readText: function() { return Promise.resolve(_buf); },
                    read: function() { return Promise.resolve([]); },
                    write: function() { return Promise.resolve(); },
                };
            })(),
            mediaDevices: {},
            serviceWorker: { register: function() { return Promise.resolve(); } },
            permissions: { query: function() { return Promise.resolve({state:'granted'}); } },
            sendBeacon: function() { return true; },
        };

        // Location — setting href parses the URL and updates all components
        globalThis.location = (function() {
            var loc = {
                _href: 'about:blank', protocol: 'https:', hostname: 'localhost',
                pathname: '/', search: '', hash: '', origin: 'https://localhost',
                host: 'localhost', port: '',
                assign: function(url) { loc.href = url; },
                replace: function(url) { loc.href = url; },
                reload: function() {},
                toString: function() { return loc.href; },
            };
            Object.defineProperty(loc, 'href', {
                get: function() { return loc._href; },
                set: function(v) {
                    loc._href = v;
                    // Parse URL components
                    var m = String(v).match(/^(https?:)\/\/([^/:]+)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                    if (m) {
                        loc.protocol = m[1];
                        loc.hostname = m[2];
                        loc.port = m[3] || '';
                        loc.host = loc.port ? loc.hostname + ':' + loc.port : loc.hostname;
                        loc.pathname = m[4] || '/';
                        loc.search = m[5] || '';
                        loc.hash = m[6] || '';
                        loc.origin = loc.protocol + '//' + loc.host;
                    }
                    // Signal navigation to engine (unless suppressed by engine's own set_url)
                    if (!loc.__suppress_nav && typeof __braille_navigate === 'function') {
                        __braille_navigate(String(v));
                    }
                },
                configurable: true, enumerable: true,
            });
            return loc;
        })();

        // History — pushState/replaceState update URL components without triggering navigation
        globalThis.history = (function() {
            var stateStack = [null];
            var stateIndex = 0;
            function updateUrl(url) {
                if (!url) return;
                var u = String(url);
                // Resolve relative URLs
                if (u.charAt(0) === '/') u = location.origin + u;
                else if (!/^https?:\/\//.test(u)) u = location.origin + location.pathname.replace(/[^\/]*$/, '') + u;
                // Update location components directly (bypass the setter to avoid re-parse side effects)
                var m = u.match(/^(https?:)\/\/([^/:]+)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                if (m) {
                    location._href = u;
                    location.protocol = m[1];
                    location.hostname = m[2];
                    location.port = m[3] || '';
                    location.host = location.port ? location.hostname + ':' + location.port : location.hostname;
                    location.pathname = m[4] || '/';
                    location.search = m[5] || '';
                    location.hash = m[6] || '';
                    location.origin = location.protocol + '//' + location.host;
                }
            }
            return {
                pushState: function(s, t, u) {
                    stateStack.splice(stateIndex + 1);
                    stateStack.push(s);
                    stateIndex = stateStack.length - 1;
                    this.state = s;
                    this.length = stateStack.length;
                    updateUrl(u);
                },
                replaceState: function(s, t, u) {
                    stateStack[stateIndex] = s;
                    this.state = s;
                    updateUrl(u);
                },
                back: function() {
                    if (stateIndex > 0) { stateIndex--; this.state = stateStack[stateIndex]; }
                },
                forward: function() {
                    if (stateIndex < stateStack.length - 1) { stateIndex++; this.state = stateStack[stateIndex]; }
                },
                go: function(n) {
                    var idx = stateIndex + (n || 0);
                    if (idx >= 0 && idx < stateStack.length) { stateIndex = idx; this.state = stateStack[stateIndex]; }
                },
                state: null,
                length: 1,
            };
        })();

        // Storage
        function makeStorage() {
            var data = {};
            return {
                getItem: function(k) { return k in data ? data[k] : null; },
                setItem: function(k,v) { data[k] = String(v); },
                removeItem: function(k) { delete data[k]; },
                clear: function() { data = {}; },
                key: function(i) { var keys = Object.keys(data); return i < keys.length ? keys[i] : null; },
                get length() { return Object.keys(data).length; },
            };
        }
        globalThis.localStorage = makeStorage();
        globalThis.sessionStorage = makeStorage();

        // Geometry/display stubs
        globalThis.getComputedStyle = function(el) {
            if (!el || el.__nid === undefined) return new Proxy({}, { get: function(t,p) { return ''; } });
            var nid = el.__nid;
            function toKebab(cc) {
                if (cc === 'cssFloat') return 'float';
                return cc.replace(/[A-Z]/g, function(c) { return '-' + c.toLowerCase(); });
            }
            return new Proxy({
                getPropertyValue: function(prop) { return __n_getComputedStyle(nid, prop); },
                getPropertyPriority: function() { return ''; },
            }, {
                get: function(t, p) {
                    if (p in t) return t[p];
                    if (typeof p !== 'string') return undefined;
                    if (p === 'length') return 0;
                    if (p === 'cssText') return '';
                    return __n_getComputedStyle(nid, toKebab(p));
                }
            });
        };
        globalThis.matchMedia = function(q) {
            var matches = false;
            var m;
            if ((m = q.match(/\(\s*min-width\s*:\s*(\d+)px\s*\)/))) {
                matches = 1280 >= parseInt(m[1]);
            } else if ((m = q.match(/\(\s*max-width\s*:\s*(\d+)px\s*\)/))) {
                matches = 1280 <= parseInt(m[1]);
            } else if ((m = q.match(/\(\s*min-height\s*:\s*(\d+)px\s*\)/))) {
                matches = 800 >= parseInt(m[1]);
            } else if ((m = q.match(/\(\s*max-height\s*:\s*(\d+)px\s*\)/))) {
                matches = 800 <= parseInt(m[1]);
            } else if (/prefers-color-scheme\s*:\s*dark/.test(q)) {
                matches = false;
            } else if (/prefers-color-scheme\s*:\s*light/.test(q)) {
                matches = true;
            } else if (/prefers-reduced-motion\s*:\s*reduce/.test(q)) {
                matches = false;
            }
            return {
                matches: matches, media: q,
                onchange: null,
                addListener: function(cb) { /* deprecated, never fires */ },
                removeListener: function(cb) {},
                addEventListener: function(type, cb) {},
                removeEventListener: function(type, cb) {},
                dispatchEvent: function() { return true; },
            };
        };
        globalThis.requestAnimationFrame = function(cb) { return setTimeout(cb, 16); };
        globalThis.cancelAnimationFrame = function(id) { clearTimeout(id); };
        globalThis.requestIdleCallback = function(cb) { return setTimeout(cb, 0); };
        globalThis.cancelIdleCallback = function(id) { clearTimeout(id); };
        globalThis.getSelection = function() {
            var _ranges = [];
            return {
                get rangeCount() { return _ranges.length; },
                getRangeAt: function(i) { return _ranges[i] || null; },
                addRange: function(r) { _ranges.push(r); },
                removeAllRanges: function() { _ranges = []; },
                removeRange: function(r) { var i = _ranges.indexOf(r); if (i >= 0) _ranges.splice(i, 1); },
                collapse: function(node, offset) { _ranges = []; if (typeof Range !== 'undefined') { var r = new Range(); r.setStart(node, offset || 0); r.collapse(true); _ranges.push(r); } },
                collapseToStart: function() { if (_ranges.length) { _ranges[0].collapse(true); _ranges = [_ranges[0]]; } },
                collapseToEnd: function() { if (_ranges.length) { _ranges[0].collapse(false); _ranges = [_ranges[0]]; } },
                toString: function() { return _ranges.length ? _ranges[0].toString() : ''; },
                isCollapsed: true,
                anchorNode: null, anchorOffset: 0, focusNode: null, focusOffset: 0,
                type: 'None',
            };
        };

        // MessageChannel — React 18 scheduler uses this for async rendering
        globalThis.MessageChannel = class MessageChannel {
            constructor() {
                var self = this;
                this.port1 = {
                    onmessage: null,
                    postMessage: function(msg) {
                        if (self.port2.onmessage) setTimeout(function() { self.port2.onmessage({data: msg}); }, 0);
                    },
                    close: function() {},
                    addEventListener: function() {},
                    removeEventListener: function() {},
                };
                this.port2 = {
                    onmessage: null,
                    postMessage: function(msg) {
                        if (self.port1.onmessage) setTimeout(function() { self.port1.onmessage({data: msg}); }, 0);
                    },
                    close: function() {},
                    addEventListener: function() {},
                    removeEventListener: function() {},
                };
            }
        };

        // Observer stubs with initial callback firing
        globalThis.ResizeObserver = class {
            constructor(cb) { this._cb = cb; }
            observe(target) {
                var cb = this._cb;
                if (typeof cb === 'function' && target && typeof target.getBoundingClientRect === 'function') {
                    setTimeout(function() {
                        var rect = target.getBoundingClientRect();
                        var w = rect.width, h = rect.height;
                        cb([{
                            target: target,
                            contentRect: rect,
                            borderBoxSize: [{inlineSize: w, blockSize: h}],
                            contentBoxSize: [{inlineSize: w, blockSize: h}],
                            devicePixelContentBoxSize: [{inlineSize: w, blockSize: h}],
                        }], this);
                    }.bind(this), 0);
                }
            }
            unobserve() {}
            disconnect() {}
        };
        globalThis.IntersectionObserver = class {
            constructor(cb, opts) { this._cb = cb; this._opts = opts || {}; }
            observe(target) {
                var cb = this._cb;
                if (typeof cb === 'function' && target && typeof target.getBoundingClientRect === 'function') {
                    setTimeout(function() {
                        var rect = target.getBoundingClientRect();
                        cb([{
                            target: target,
                            isIntersecting: true,
                            intersectionRatio: 1.0,
                            boundingClientRect: rect,
                            intersectionRect: rect,
                            rootBounds: {top:0,left:0,width:1280,height:800,right:1280,bottom:800,x:0,y:0},
                            time: performance.now(),
                        }], this);
                    }.bind(this), 0);
                }
            }
            unobserve() {}
            disconnect() {}
            takeRecords() { return []; }
        };
        // MutationObserver — functional implementation
        (function() {
            var observers = [];
            var pendingDeliver = false;

            function MutationRecord(type, target) {
                this.type = type; this.target = target;
                this.addedNodes = []; this.removedNodes = [];
                this.attributeName = null; this.oldValue = null;
                this.previousSibling = null; this.nextSibling = null;
            }

            function queueRecord(record) {
                for (var i = 0; i < observers.length; i++) {
                    var obs = observers[i];
                    for (var j = 0; j < obs._targets.length; j++) {
                        var entry = obs._targets[j];
                        var target = record.target;
                        // Check if this observer watches this target (or subtree ancestor)
                        var match = false;
                        if (target === entry.target) match = true;
                        else if (entry.options.subtree) {
                            var cur = target;
                            while (cur) { if (cur === entry.target) { match = true; break; } cur = cur.parentNode; }
                        }
                        if (!match) continue;
                        if (record.type === 'attributes' && !entry.options.attributes) continue;
                        if (record.type === 'attributes' && Array.isArray(entry.options.attributeFilter) && entry.options.attributeFilter.indexOf(record.attributeName) < 0) continue;
                        if (record.type === 'childList' && !entry.options.childList) continue;
                        if (record.type === 'characterData' && !entry.options.characterData) continue;
                        obs._records.push(record);
                    }
                }
                if (!pendingDeliver) {
                    pendingDeliver = true;
                    queueMicrotask(function() {
                        pendingDeliver = false;
                        for (var i = 0; i < observers.length; i++) {
                            var obs = observers[i];
                            if (obs._records.length > 0) {
                                var recs = obs._records.splice(0);
                                obs._cb(recs, obs);
                            }
                        }
                    });
                }
            }

            globalThis.MutationObserver = function(cb) {
                this._cb = cb; this._records = []; this._targets = [];
            };
            MutationObserver.prototype.observe = function(target, options) {
                options = options || {};
                if (options.attributeFilter && options.attributes === undefined) options.attributes = true;
                this._targets.push({target: target, options: options});
                if (observers.indexOf(this) < 0) observers.push(this);
            };
            MutationObserver.prototype.disconnect = function() {
                this._targets = [];
                var idx = observers.indexOf(this);
                if (idx >= 0) observers.splice(idx, 1);
            };
            MutationObserver.prototype.takeRecords = function() { return this._records.splice(0); };

            globalThis.__mo_notify = function(type, target, extra) {
                var r = new MutationRecord(type, target);
                if (extra) {
                    if (extra.addedNodes) r.addedNodes = extra.addedNodes;
                    if (extra.removedNodes) r.removedNodes = extra.removedNodes;
                    if (extra.attributeName) r.attributeName = extra.attributeName;
                    if (extra.oldValue !== undefined) r.oldValue = extra.oldValue;
                }
                queueRecord(r);
            };
        })();

        // Performance — real monotonic timer anchored to engine start
        var __perf_start = Date.now();
        globalThis.performance = {
            now: function() { return Date.now() - __perf_start; },
            timeOrigin: Date.now(),
            mark: function(){}, measure: function(){},
            getEntriesByType: function(){return [];}, getEntriesByName: function(){return [];},
            timing: { navigationStart: __perf_start },
        };

        // URL
        globalThis.URL = class URL {
            constructor(u, base) {
                if (base && !u.match(/^https?:\/\//)) {
                    if (u.startsWith('/')) { var m = base.match(/^(https?:\/\/[^\/]+)/); u = (m?m[1]:'') + u; }
                    else { u = base.replace(/[^\/]*$/, '') + u; }
                }
                this.href = u;
                var m = u.match(/^(https?):\/\/([^\/\?#]+)(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                this.protocol = m ? m[1]+':' : '';
                this.host = m ? m[2] : '';
                this.hostname = this.host.replace(/:\d+$/, '');
                this.port = (this.host.match(/:(\d+)$/) || ['',''])[1];
                this.pathname = m ? (m[3]||'/') : '/';
                this._search = m ? (m[4]||'') : '';
                this.hash = m ? (m[5]||'') : '';
                this.origin = this.protocol + '//' + this.host;
                // searchParams is a live view — mutations sync back to the URL
                var self = this;
                this.searchParams = new URLSearchParams(this._search);
                this.searchParams._url = this;
            }
            get search() { return this._search; }
            set search(v) {
                this._search = v;
                this.searchParams = new URLSearchParams(v);
                this.searchParams._url = this;
                this._rebuildHref();
            }
            _rebuildHref() {
                var s = this.searchParams.toString();
                this._search = s ? '?' + s : '';
                this.href = this.origin + this.pathname + this._search + this.hash;
            }
            toString() { return this.href; }
            toJSON() { return this.href; }
        };
        // URLSearchParams — spec-compliant including 2-arg delete(name, value)
        globalThis.URLSearchParams = class URLSearchParams {
            constructor(init) {
                this._entries = [];
                if (init) {
                    var s = String(init).replace(/^\?/,'');
                    if (s) s.split('&').forEach(function(p) {
                        var eq = p.indexOf('=');
                        if (eq < 0) this._entries.push([decodeURIComponent(p), '']);
                        else this._entries.push([decodeURIComponent(p.substring(0,eq)), decodeURIComponent(p.substring(eq+1))]);
                    }.bind(this));
                }
            }
            _sync() { if (this._url) this._url._rebuildHref(); }
            get(n) { var e=this._entries.find(function(e){return e[0]===n;}); return e?e[1]:null; }
            getAll(n) { return this._entries.filter(function(e){return e[0]===n;}).map(function(e){return e[1];}); }
            has(n,v) { return arguments.length > 1 ? this._entries.some(function(e){return e[0]===n && e[1]===v;}) : this._entries.some(function(e){return e[0]===n;}); }
            set(n,v) { var found=false; this._entries=this._entries.filter(function(e){if(e[0]===n){if(!found){e[1]=String(v);found=true;return true;}return false;}return true;}); if(!found) this._entries.push([n,String(v)]); this._sync(); }
            append(n,v) { this._entries.push([n,String(v)]); this._sync(); }
            delete(n,v) { if (arguments.length > 1) { this._entries=this._entries.filter(function(e){return !(e[0]===n && e[1]===String(v));}); } else { this._entries=this._entries.filter(function(e){return e[0]!==n;}); } this._sync(); }
            sort() { this._entries.sort(function(a,b){return a[0]<b[0]?-1:a[0]>b[0]?1:0;}); this._sync(); }
            toString() { return this._entries.map(function(e){return encodeURIComponent(e[0])+'='+encodeURIComponent(e[1]);}).join('&'); }
            forEach(cb) { this._entries.forEach(function(e){cb(e[1],e[0]);}); }
            keys() { return this._entries.map(function(e){return e[0];})[Symbol.iterator](); }
            values() { return this._entries.map(function(e){return e[1];})[Symbol.iterator](); }
            entries() { return this._entries[Symbol.iterator](); }
            get size() { return this._entries.length; }
            [Symbol.iterator]() { return this.entries(); }
        };

        // Encoding — real UTF-8
        globalThis.TextEncoder = class TextEncoder {
            get encoding() { return 'utf-8'; }
            encode(s) {
                s = String(s || '');
                var bytes = [];
                for (var i = 0; i < s.length; i++) {
                    var cp = s.codePointAt(i);
                    if (cp < 0x80) {
                        bytes.push(cp);
                    } else if (cp < 0x800) {
                        bytes.push(0xC0 | (cp >> 6), 0x80 | (cp & 0x3F));
                    } else if (cp < 0x10000) {
                        bytes.push(0xE0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3F), 0x80 | (cp & 0x3F));
                    } else {
                        bytes.push(0xF0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3F), 0x80 | ((cp >> 6) & 0x3F), 0x80 | (cp & 0x3F));
                        i++; // skip surrogate pair second half
                    }
                }
                return new Uint8Array(bytes);
            }
            encodeInto(source, destination) {
                var encoded = this.encode(source);
                var written = Math.min(encoded.length, destination.length);
                for (var i = 0; i < written; i++) destination[i] = encoded[i];
                // Count how many source chars were consumed for 'written' bytes
                var read = 0, byteCount = 0;
                for (read = 0; read < source.length && byteCount < written; read++) {
                    var cp = source.codePointAt(read);
                    var size = cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
                    if (byteCount + size > written) break;
                    byteCount += size;
                    if (cp >= 0x10000) read++; // skip surrogate pair
                }
                return { read: read, written: byteCount };
            }
        };
        globalThis.TextDecoder = class TextDecoder {
            constructor(label) { this._label = (label || 'utf-8').toLowerCase(); }
            get encoding() { return this._label; }
            decode(buf) {
                if (!buf) return '';
                var bytes = new Uint8Array(buf instanceof ArrayBuffer ? buf : buf.buffer || buf);
                var result = '', i = 0;
                while (i < bytes.length) {
                    var b = bytes[i];
                    if (b < 0x80) { result += String.fromCodePoint(b); i++; }
                    else if ((b & 0xE0) === 0xC0) {
                        var cp = ((b & 0x1F) << 6) | (bytes[i+1] & 0x3F);
                        result += String.fromCodePoint(cp); i += 2;
                    } else if ((b & 0xF0) === 0xE0) {
                        var cp = ((b & 0x0F) << 12) | ((bytes[i+1] & 0x3F) << 6) | (bytes[i+2] & 0x3F);
                        result += String.fromCodePoint(cp); i += 3;
                    } else if ((b & 0xF8) === 0xF0) {
                        var cp = ((b & 0x07) << 18) | ((bytes[i+1] & 0x3F) << 12) | ((bytes[i+2] & 0x3F) << 6) | (bytes[i+3] & 0x3F);
                        result += String.fromCodePoint(cp); i += 4;
                    } else { result += '\uFFFD'; i++; }
                }
                return result;
            }
        };
        // Real base64 btoa/atob
        globalThis.btoa = function(s) {
            var T = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
            var str = String(s), out = '', i = 0;
            while (i < str.length) {
                var a = str.charCodeAt(i++), b = i < str.length ? str.charCodeAt(i++) : NaN, c = i < str.length ? str.charCodeAt(i++) : NaN;
                var n = (a << 16) | ((isNaN(b) ? 0 : b) << 8) | (isNaN(c) ? 0 : c);
                out += T[(n >> 18) & 63] + T[(n >> 12) & 63] + (isNaN(b) ? '=' : T[(n >> 6) & 63]) + (isNaN(c) ? '=' : T[n & 63]);
            }
            return out;
        };
        globalThis.atob = function(s) {
            var T = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=';
            var str = String(s).replace(/[\s]/g, ''), out = '', i = 0;
            while (i < str.length) {
                var a = T.indexOf(str.charAt(i++)), b = T.indexOf(str.charAt(i++));
                var c = T.indexOf(str.charAt(i++)), d = T.indexOf(str.charAt(i++));
                var n = (a << 18) | (b << 12) | ((c & 63) << 6) | (d & 63);
                out += String.fromCharCode((n >> 16) & 255);
                if (c !== 64) out += String.fromCharCode((n >> 8) & 255);
                if (d !== 64) out += String.fromCharCode(n & 255);
            }
            return out;
        };

        // Misc stubs
        // AbortController / AbortSignal with real event dispatch
        globalThis.AbortSignal = (function() {
            function AbortSignal() {}
            AbortSignal.prototype.aborted = false;
            AbortSignal.prototype.reason = undefined;
            AbortSignal.prototype.onabort = null;
            AbortSignal.prototype.addEventListener = function(type, cb) { if (type === 'abort') { if (!this._listeners) this._listeners = []; this._listeners.push(cb); } };
            AbortSignal.prototype.removeEventListener = function(type, cb) { if (type === 'abort' && this._listeners) this._listeners = this._listeners.filter(function(f){return f!==cb;}); };
            AbortSignal.prototype._fire = function() {
                var ev = new Event('abort', {bubbles: false, cancelable: false});
                Object.defineProperty(ev, 'target', {value: this, writable: false});
                ev._isTrusted = true;
                if (this.onabort) this.onabort(ev);
                if (this._listeners) for (var i = 0; i < this._listeners.length; i++) this._listeners[i](ev);
            };
            AbortSignal.prototype.throwIfAborted = function() { if (this.aborted) throw this.reason || new DOMException('The operation was aborted.', 'AbortError'); };
            function makeSignal() {
                var s = Object.create(AbortSignal.prototype);
                s.aborted = false;
                s.reason = undefined;
                s.onabort = null;
                s._listeners = [];
                s._dependents = [];
                return s;
            }
            // Two-phase abort: mark all dependents aborted first, then fire events
            function signalAbort(signal, reason) {
                if (signal.aborted) return;
                signal.aborted = true;
                signal.reason = reason;
                // Phase 1: mark all dependents (breadth-first)
                var toMark = signal._dependents.slice();
                var allSignals = [signal];
                while (toMark.length > 0) {
                    var dep = toMark.shift();
                    if (!dep.aborted) {
                        dep.aborted = true;
                        dep.reason = reason;
                        allSignals.push(dep);
                        if (dep._dependents) {
                            for (var i = 0; i < dep._dependents.length; i++) toMark.push(dep._dependents[i]);
                        }
                    }
                }
                // Phase 2: fire events in creation/registration order
                for (var i = 0; i < allSignals.length; i++) allSignals[i]._fire();
            }
            AbortSignal.abort = function(reason) { var s = makeSignal(); s.aborted = true; s.reason = reason !== undefined ? reason : new DOMException('The operation was aborted.', 'AbortError'); return s; };
            AbortSignal.timeout = function(ms) { var s = makeSignal(); setTimeout(function() { signalAbort(s, new DOMException('The operation timed out.', 'TimeoutError')); }, ms); return s; };
            AbortSignal.any = function(signals) {
                var s = makeSignal();
                for (var i = 0; i < signals.length; i++) {
                    if (signals[i].aborted) { s.aborted = true; s.reason = signals[i].reason; return s; }
                    if (signals[i]._dependents) signals[i]._dependents.push(s);
                }
                return s;
            };
            AbortSignal._makeSignal = makeSignal;
            AbortSignal._signalAbort = signalAbort;
            return AbortSignal;
        })();
        globalThis.AbortController = class AbortController {
            constructor() { this.signal = AbortSignal._makeSignal(); }
            abort(reason) { AbortSignal._signalAbort(this.signal, reason !== undefined ? reason : new DOMException('The operation was aborted.', 'AbortError')); }
        };
        // Worker class is registered by worker.rs with real delegation to the host.

        globalThis.XMLHttpRequest = (function() {
            function XMLHttpRequest() {
                this.readyState = 0;
                this.status = 0;
                this.statusText = '';
                this.responseText = '';
                this.response = '';
                this.responseURL = '';
                this.responseType = '';
                this.withCredentials = false;
                this.timeout = 0;
                this.upload = { addEventListener: function(){}, removeEventListener: function(){} };
                this.onreadystatechange = null;
                this.onload = null;
                this.onerror = null;
                this.onprogress = null;
                this.onloadend = null;
                this.onabort = null;
                this.onloadstart = null;
                this.ontimeout = null;
                this._method = 'GET';
                this._url = '';
                this._headers = {};
                this._responseHeaders = {};
                this._listeners = {};
                this._aborted = false;
            }
            XMLHttpRequest.UNSENT = 0;
            XMLHttpRequest.OPENED = 1;
            XMLHttpRequest.HEADERS_RECEIVED = 2;
            XMLHttpRequest.LOADING = 3;
            XMLHttpRequest.DONE = 4;
            XMLHttpRequest.prototype.UNSENT = 0;
            XMLHttpRequest.prototype.OPENED = 1;
            XMLHttpRequest.prototype.HEADERS_RECEIVED = 2;
            XMLHttpRequest.prototype.LOADING = 3;
            XMLHttpRequest.prototype.DONE = 4;

            XMLHttpRequest.prototype.open = function(method, url, async_) {
                this._method = method;
                this._url = url;
                this._headers = {};
                this._responseHeaders = {};
                this._aborted = false;
                this.readyState = 1;
                this.status = 0;
                this.statusText = '';
                this.responseText = '';
                this.response = '';
                this._fireReadyStateChange();
            };
            XMLHttpRequest.prototype.setRequestHeader = function(name, value) {
                this._headers[name] = value;
            };
            XMLHttpRequest.prototype.send = function(body) {
                if (this._aborted) return;
                var self = this;
                var opts = { method: self._method, headers: self._headers };
                if (body !== undefined && body !== null && self._method !== 'GET' && self._method !== 'HEAD') {
                    opts.body = body;
                }
                self.readyState = 1;

                fetch(self._url, opts).then(function(resp) {
                    if (self._aborted) return;
                    self.status = resp.status;
                    self.statusText = resp.statusText || '';
                    self.responseURL = resp.url || self._url;
                    // Store response headers
                    self._responseHeaders = {};
                    if (resp.headers && typeof resp.headers.forEach === 'function') {
                        resp.headers.forEach(function(val, key) {
                            self._responseHeaders[key.toLowerCase()] = val;
                        });
                    }
                    self.readyState = 2;
                    self._fireReadyStateChange();
                    return resp.text();
                }).then(function(text) {
                    if (self._aborted) return;
                    self.responseText = text || '';
                    self.response = self.responseType === 'json' ? JSON.parse(self.responseText) : self.responseText;
                    self.readyState = 4;
                    self._fireReadyStateChange();
                    self._fireEvent('load');
                    self._fireEvent('loadend');
                }).catch(function(err) {
                    if (self._aborted) return;
                    self.readyState = 4;
                    self.status = 0;
                    self._fireReadyStateChange();
                    self._fireEvent('error');
                    self._fireEvent('loadend');
                });
            };
            XMLHttpRequest.prototype.abort = function() {
                this._aborted = true;
                this.readyState = 0;
                this._fireEvent('abort');
            };
            XMLHttpRequest.prototype.getResponseHeader = function(name) {
                return this._responseHeaders[name.toLowerCase()] || null;
            };
            XMLHttpRequest.prototype.getAllResponseHeaders = function() {
                var result = '';
                for (var key in this._responseHeaders) {
                    result += key + ': ' + this._responseHeaders[key] + '\r\n';
                }
                return result;
            };
            XMLHttpRequest.prototype.overrideMimeType = function() {};
            XMLHttpRequest.prototype.addEventListener = function(type, cb) {
                if (!this._listeners[type]) this._listeners[type] = [];
                this._listeners[type].push(cb);
            };
            XMLHttpRequest.prototype.removeEventListener = function(type, cb) {
                if (this._listeners[type]) this._listeners[type] = this._listeners[type].filter(function(f){return f!==cb;});
            };
            // dispatchEvent will be wired up after EventTarget is defined (in dom_bridge IIFE)
            XMLHttpRequest.prototype._fireReadyStateChange = function() {
                if (typeof this.onreadystatechange === 'function') {
                    this.onreadystatechange({type: 'readystatechange', target: this});
                }
                this._fireEvent('readystatechange');
            };
            XMLHttpRequest.prototype._fireEvent = function(type) {
                var evt = {type: type, target: this, loaded: this.responseText ? this.responseText.length : 0, total: 0, lengthComputable: false};
                var handler = this['on' + type];
                if (typeof handler === 'function' && type !== 'readystatechange') handler.call(this, evt);
                var cbs = this._listeners[type];
                if (cbs) { for (var i = 0; i < cbs.length; i++) cbs[i].call(this, evt); }
            };
            return XMLHttpRequest;
        })();
        globalThis.DOMParser = class DOMParser {
            parseFromString(str, type) {
                var div = document.createElement('div');
                div.innerHTML = str;
                return {
                    documentElement: div,
                    body: div,
                    head: null,
                    title: '',
                    readyState: 'complete',
                    querySelector: function(sel) { return div.querySelector(sel); },
                    querySelectorAll: function(sel) { return div.querySelectorAll(sel); },
                    getElementById: function(id) {
                        var el = div.querySelector('#' + id);
                        return el || null;
                    },
                    getElementsByTagName: function(tag) { return div.getElementsByTagName(tag); },
                    getElementsByClassName: function(cls) { return div.getElementsByClassName(cls); },
                    createDocumentFragment: function() { return document.createDocumentFragment(); },
                    createElement: function(tag) { return document.createElement(tag); },
                    createTextNode: function(text) { return document.createTextNode(text); },
                };
            }
        };
        globalThis.Node = class Node {};
        globalThis.Element = class Element extends Node {};
        // HTMLElement constructor supports Custom Elements:
        // When called via `new MyElement()` where MyElement extends HTMLElement,
        // new.target is the CE constructor. We look up the tag from the registry
        // and create the backing DOM node.
        globalThis.HTMLElement = class HTMLElement extends Element {
            constructor() {
                super();
                if (typeof customElements !== 'undefined' && customElements._ctorToName) {
                    var name = customElements._ctorToName.get(new.target);
                    if (name) {
                        var nid = __n_createElement(name);
                        this.__nid = nid;
                        this.__props = {};
                        _cache[nid] = this;
                    }
                }
            }
        };
        globalThis.HTMLIFrameElement = class HTMLIFrameElement extends HTMLElement {};
        globalThis.HTMLInputElement = class HTMLInputElement extends HTMLElement {};
        globalThis.HTMLTextAreaElement = class HTMLTextAreaElement extends HTMLElement {};
        globalThis.HTMLSelectElement = class HTMLSelectElement extends HTMLElement {};
        globalThis.HTMLFormElement = class HTMLFormElement extends HTMLElement {};
        globalThis.HTMLAnchorElement = class HTMLAnchorElement extends HTMLElement {};
        globalThis.HTMLImageElement = class HTMLImageElement extends HTMLElement {};
        globalThis.HTMLButtonElement = class HTMLButtonElement extends HTMLElement {};
        globalThis.HTMLOptionElement = class HTMLOptionElement extends HTMLElement {};
        globalThis.HTMLBodyElement = class HTMLBodyElement extends HTMLElement {};
        globalThis.HTMLHeadElement = class HTMLHeadElement extends HTMLElement {};
        globalThis.HTMLFrameSetElement = class HTMLFrameSetElement extends HTMLElement {};
        globalThis.HTMLHtmlElement = class HTMLHtmlElement extends HTMLElement {};
        globalThis.HTMLDivElement = class HTMLDivElement extends HTMLElement {};
        globalThis.HTMLSpanElement = class HTMLSpanElement extends HTMLElement {};
        globalThis.HTMLParagraphElement = class HTMLParagraphElement extends HTMLElement {};
        globalThis.HTMLScriptElement = class HTMLScriptElement extends HTMLElement {};
        globalThis.HTMLStyleElement = class HTMLStyleElement extends HTMLElement {};
        globalThis.HTMLLinkElement = class HTMLLinkElement extends HTMLElement {};
        globalThis.HTMLMetaElement = class HTMLMetaElement extends HTMLElement {};
        globalThis.HTMLTableElement = class HTMLTableElement extends HTMLElement {};
        globalThis.HTMLTableRowElement = class HTMLTableRowElement extends HTMLElement {};
        globalThis.HTMLTableCellElement = class HTMLTableCellElement extends HTMLElement {};
        globalThis.HTMLUListElement = class HTMLUListElement extends HTMLElement {};
        globalThis.HTMLOListElement = class HTMLOListElement extends HTMLElement {};
        globalThis.HTMLLIElement = class HTMLLIElement extends HTMLElement {};
        globalThis.HTMLPreElement = class HTMLPreElement extends HTMLElement {};
        globalThis.HTMLCanvasElement = class HTMLCanvasElement extends HTMLElement {};
        globalThis.HTMLVideoElement = class HTMLVideoElement extends HTMLElement {};
        globalThis.HTMLAudioElement = class HTMLAudioElement extends HTMLElement {};
        globalThis.HTMLSourceElement = class HTMLSourceElement extends HTMLElement {};
        globalThis.HTMLLabelElement = class HTMLLabelElement extends HTMLElement {};
        globalThis.HTMLTemplateElement = class HTMLTemplateElement extends HTMLElement {};
        globalThis.SVGElement = class SVGElement extends Element {};
        globalThis.Window = class Window {};
        globalThis.Document = class Document extends Node {
            constructor() {
                super();
                this.nodeType = 9;
                this.nodeName = '#document';
                this.childNodes = [];
                this.__listeners = {};
                this.__captureListeners = {};
                this.__et_listeners = {};
            }
            get documentElement() {
                for (var i = 0; i < this.childNodes.length; i++) {
                    if (this.childNodes[i].nodeType === 1) return this.childNodes[i];
                }
                return null;
            }
            get body() {
                var de = this.documentElement;
                if (!de) return null;
                var kids = de.childNodes || de.children || [];
                for (var i = 0; i < kids.length; i++) {
                    if (kids[i].tagName === 'BODY' || kids[i].tagName === 'FRAMESET') return kids[i];
                }
                return null;
            }
            appendChild(child) {
                this.childNodes.push(child);
                if (child) child.parentNode = this;
                return child;
            }
            removeChild(child) {
                var idx = this.childNodes.indexOf(child);
                if (idx >= 0) this.childNodes.splice(idx, 1);
                if (child) child.parentNode = null;
                return child;
            }
            createElement(tag) { return document.createElement(tag); }
            createTextNode(t) { return document.createTextNode(t); }
            createEvent(type) { return document.createEvent(type); }
        };

        // Window-reflecting body/frameset event handlers (onblur, onerror, onfocus, onload, onscroll, onresize)
        // These forward to window when set on body or frameset elements.
        var _windowEventHandlers = ['onblur', 'onerror', 'onfocus', 'onload', 'onscroll', 'onresize',
            'onbeforeunload', 'onhashchange', 'onlanguagechange', 'onmessage', 'onmessageerror',
            'onoffline', 'ononline', 'onpagehide', 'onpageshow', 'onpopstate',
            'onrejectionhandled', 'onstorage', 'onunhandledrejection', 'onunload'];
        var _wehSet = new Set(_windowEventHandlers);
        [HTMLBodyElement, HTMLFrameSetElement].forEach(function(Ctor) {
            _windowEventHandlers.forEach(function(attr) {
                Object.defineProperty(Ctor.prototype, attr, {
                    get: function() {
                        return window['_weh_' + attr] || null;
                    },
                    set: function(v) {
                        window['_weh_' + attr] = typeof v === 'function' ? v : null;
                    },
                    enumerable: true,
                    configurable: true
                });
            });
            // Hook setAttribute to compile event handler content attributes
            var origSetAttr = Ctor.prototype.setAttribute;
            Ctor.prototype.setAttribute = function(name, value) {
                if (origSetAttr) origSetAttr.call(this, name, value);
                else if (this.__nid !== undefined) __n_setAttribute(this.__nid, name, String(value));
                if (_wehSet.has(name)) {
                    window['_weh_' + name] = new Function('event', String(value));
                }
            };
        });
        // Window on* getters/setters forwarding to stored handlers
        _windowEventHandlers.forEach(function(attr) {
            Object.defineProperty(window, attr, {
                get: function() { return window['_weh_' + attr] || null; },
                set: function(v) { window['_weh_' + attr] = typeof v === 'function' ? v : null; },
                enumerable: true,
                configurable: true
            });
        });

        // Standard event handler properties on HTMLElement.prototype
        var _elementEventHandlers = ['onclick', 'ondblclick', 'onmousedown', 'onmouseup',
            'onmouseover', 'onmouseout', 'onmousemove', 'onkeydown', 'onkeyup', 'onkeypress',
            'onchange', 'oninput', 'onsubmit', 'onreset', 'onselect',
            'ondrag', 'ondragstart', 'ondragend', 'ondragover', 'ondragenter', 'ondragleave', 'ondrop',
            'onscroll', 'onscrollend',
            'ontouchstart', 'ontouchmove', 'ontouchend', 'ontouchcancel',
            'onpointerdown', 'onpointerup', 'onpointermove', 'onpointerover', 'onpointerout',
            'onpointerenter', 'onpointerleave', 'onpointercancel', 'ongotpointercapture', 'onlostpointercapture',
            'oncontextmenu', 'onwheel', 'onanimationstart', 'onanimationend', 'onanimationiteration',
            'ontransitionend', 'ontransitionrun', 'ontransitionstart', 'ontransitioncancel',
            'onwebkitanimationstart', 'onwebkitanimationend', 'onwebkitanimationiteration',
            'onwebkittransitionend'];
        _elementEventHandlers.forEach(function(attr) {
            if (!(attr in HTMLElement.prototype)) {
                Object.defineProperty(HTMLElement.prototype, attr, {
                    get: function() { return this['_eh_' + attr] || null; },
                    set: function(v) { this['_eh_' + attr] = typeof v === 'function' ? v : null; },
                    enumerable: true,
                    configurable: true
                });
            }
        });

        // Also define element event handlers on window (animation events bubble to window)
        _elementEventHandlers.forEach(function(attr) {
            if (!(attr in window)) {
                Object.defineProperty(window, attr, {
                    get: function() { return this['_eh_' + attr] || null; },
                    set: function(v) { this['_eh_' + attr] = typeof v === 'function' ? v : null; },
                    enumerable: true,
                    configurable: true
                });
            }
        });

        // Value descriptors on HTML*Element prototypes for React's inputValueTracking.
        // React uses node.constructor.prototype to find native get/set for 'value'
        // and 'checked'. These must exist so React can set up change detection.
        var _valDesc = {
            get: function() {
                if (this.__props && this.__props._value !== undefined) return this.__props._value;
                if (this.tagName === 'TEXTAREA') return this.textContent || '';
                return (this.getAttribute && this.getAttribute('value')) || '';
            },
            set: function(v) {
                if (!this.__props) this.__props = {};
                var s = String(v);
                if (this.getAttribute) {
                    var ml = this.getAttribute('maxlength');
                    if (ml !== null) { var n = parseInt(ml, 10); if (!isNaN(n) && n >= 0 && s.length > n) s = s.substring(0, n); }
                }
                this.__props._value = s;
                if (this.tagName === 'TEXTAREA' && this.__nid !== undefined) {
                    __n_setTextContent(this.__nid, s);
                } else if (this.__nid !== undefined) {
                    __n_setAttribute(this.__nid, 'value', s);
                }
            },
            configurable: true,
        };
        Object.defineProperty(HTMLInputElement.prototype, 'value', _valDesc);
        Object.defineProperty(HTMLTextAreaElement.prototype, 'value', _valDesc);
        Object.defineProperty(HTMLSelectElement.prototype, 'value', _valDesc);
        Object.defineProperty(HTMLInputElement.prototype, 'checked', {
            get: function() {
                if (this.__props && this.__props._checked !== undefined) return this.__props._checked;
                return this.hasAttribute && this.hasAttribute('checked');
            },
            set: function(v) {
                if (!this.__props) this.__props = {};
                this.__props._checked = !!v;
            },
            configurable: true,
        });
        globalThis.DocumentFragment = class DocumentFragment {};
        globalThis.ShadowRoot = class ShadowRoot extends DocumentFragment {
            get mode() { return this._mode || 'open'; }
            get host() { return this._host || null; }
            get innerHTML() {
                if (this.__nid !== undefined) return __n_getInnerHTML(this.__nid);
                return '';
            }
            set innerHTML(v) {
                if (this.__nid !== undefined) {
                    __n_setInnerHTML(this.__nid, String(v));
                    // Upgrade custom elements in new content
                    if (typeof customElements !== 'undefined' && customElements._registry) {
                        __ceUpgradeTree(this);
                    }
                }
            }
        };

        // Custom Elements Registry
        globalThis.CustomElementRegistry = class CustomElementRegistry {
            constructor() {
                this._registry = new Map();      // name → {ctor, observedAttrs}
                this._ctorToName = new Map();     // ctor → name
                this._whenDefined = new Map();    // name → {promise, resolve}
            }
            define(name, ctor, options) {
                name = String(name).toLowerCase();
                if (!/^[a-z]/.test(name) || name.indexOf('-') === -1) {
                    throw new DOMException("'" + name + "' is not a valid custom element name", "SyntaxError");
                }
                if (this._registry.has(name)) {
                    throw new DOMException("The name '" + name + "' has already been used with this registry", "NotSupportedError");
                }
                if (this._ctorToName.has(ctor)) {
                    throw new DOMException("This constructor has already been used with this registry", "NotSupportedError");
                }
                var observedAttrs = [];
                if (ctor.observedAttributes && Array.isArray(ctor.observedAttributes)) {
                    observedAttrs = ctor.observedAttributes.slice();
                }
                this._registry.set(name, {ctor: ctor, observedAttrs: observedAttrs});
                this._ctorToName.set(ctor, name);
                // Upgrade existing elements in the DOM
                __ceUpgradeAll(name, ctor, observedAttrs);
                // Resolve whenDefined promise
                var wd = this._whenDefined.get(name);
                if (wd) {
                    wd.resolve(ctor);
                    this._whenDefined.delete(name);
                }
            }
            get(name) {
                var entry = this._registry.get(String(name).toLowerCase());
                return entry ? entry.ctor : undefined;
            }
            getName(ctor) {
                var name = this._ctorToName.get(ctor);
                return name !== undefined ? name : null;
            }
            whenDefined(name) {
                name = String(name).toLowerCase();
                if (!/^[a-z]/.test(name) || name.indexOf('-') === -1) {
                    return Promise.reject(new DOMException("'" + name + "' is not a valid custom element name", "SyntaxError"));
                }
                var entry = this._registry.get(name);
                if (entry) return Promise.resolve(entry.ctor);
                var wd = this._whenDefined.get(name);
                if (wd) return wd.promise;
                var resolve;
                var promise = new Promise(function(r) { resolve = r; });
                this._whenDefined.set(name, {promise: promise, resolve: resolve});
                return promise;
            }
        };
        globalThis.customElements = new CustomElementRegistry();

        // CE upgrade/lifecycle stubs — real implementations are installed by dom_bridge IIFE
        // which has access to _cache. These get overwritten.
        globalThis.__ceUpgradeAll = function() {};
        globalThis.__ceUpgradeTree = function() {};
        globalThis.__ceConnected = function() {};
        globalThis.__ceDisconnected = function() {};

        // Shared isConnected helper — walks parents and crosses shadow boundaries
        function __isConnected(nid) {
            var cur = nid;
            while (cur >= 0) {
                if (__n_getNodeType(cur) === 9) return true;
                var parent = __n_getParent(cur);
                if (parent < 0) {
                    // Check if current node is a shadow root — jump to host
                    if (typeof __n_isShadowRoot === 'function' && __n_isShadowRoot(cur)) {
                        cur = __n_getShadowHost(cur);
                        continue;
                    }
                }
                cur = parent;
            }
            return false;
        }
        globalThis.__isConnected = __isConnected;

        // JS retarget: retarget nodeA relative to nodeB (or null for non-node B)
        // Returns the nid that A should be retargeted to
        function __jsRetarget(aNid, bNid) {
            var a = aNid;
            while (true) {
                // Walk to root of a's tree
                var root = a;
                var p = __n_getParent(root);
                while (p >= 0) { root = p; p = __n_getParent(root); }
                // If root is not a shadow root, return a
                if (!__n_isShadowRoot(root)) return a;
                // If b is a node and b's root is the same shadow root, return a
                if (bNid >= 0) {
                    var bRoot = bNid;
                    var bp = __n_getParent(bRoot);
                    while (bp >= 0) { bRoot = bp; bp = __n_getParent(bRoot); }
                    if (bRoot === root) return a;
                }
                // Jump through shadow boundary
                a = __n_getShadowHost(root);
            }
        }
        globalThis.__jsRetarget = __jsRetarget;

        globalThis.CSSStyleSheet = class CSSStyleSheet {
            constructor() { this._rules = []; }
            insertRule(rule, index) {
                if (index === undefined) index = 0;
                this._rules.splice(index, 0, { cssText: rule });
                if (this.__ownerNode) this._syncToOwner();
                return index;
            }
            deleteRule(index) {
                this._rules.splice(index, 1);
                if (this.__ownerNode) this._syncToOwner();
            }
            get cssRules() { return this._rules; }
            _syncToOwner() {
                var text = '';
                for (var i = 0; i < this._rules.length; i++) {
                    text += this._rules[i].cssText + '\n';
                }
                this.__ownerNode.textContent = text;
            }
        };
        // ReadableStream (minimal — single-chunk body reader)
        globalThis.ReadableStream = class ReadableStream {
            constructor(src) { this._src = src; this.locked = false; }
            getReader() {
                this.locked = true;
                var data = this._src; var done = false;
                return {
                    read: function() { if (done) return Promise.resolve({done:true,value:undefined}); done = true; return Promise.resolve({done:false,value: typeof data === 'string' ? new TextEncoder().encode(data) : data}); },
                    releaseLock: function() {},
                    cancel: function() { return Promise.resolve(); },
                };
            }
            cancel() { return Promise.resolve(); }
            pipeTo() { return Promise.resolve(); }
            pipeThrough(t) { return t.readable || this; }
            tee() { return [new ReadableStream(this._src), new ReadableStream(this._src)]; }
        };
        globalThis.FormData = class FormData {
            constructor(form) {
                this._entries = [];
                if (form && form.querySelectorAll) {
                    var controls = form.querySelectorAll('input, textarea, select');
                    for (var i = 0; i < controls.length; i++) {
                        var c = controls[i];
                        var name = c.getAttribute('name');
                        if (!name) continue;
                        var tag = c.tagName;
                        if (tag === 'INPUT') {
                            var type = (c.getAttribute('type') || 'text').toLowerCase();
                            if (type === 'checkbox' || type === 'radio') {
                                if (c.checked) this._entries.push([name, c.value || 'on']);
                            } else if (type !== 'file' && type !== 'submit' && type !== 'button' && type !== 'reset' && type !== 'image') {
                                this._entries.push([name, c.value || '']);
                            }
                        } else if (tag === 'TEXTAREA' || tag === 'SELECT') {
                            this._entries.push([name, c.value || '']);
                        }
                    }
                }
            }
            append(n,v) { this._entries.push([n,String(v)]); }
            get(n) { var e=this._entries.find(function(e){return e[0]===n;}); return e?e[1]:null; }
            getAll(n) { return this._entries.filter(function(e){return e[0]===n;}).map(function(e){return e[1];}); }
            has(n) { return this._entries.some(function(e){return e[0]===n;}); }
            set(n,v) { this.delete(n); this.append(n,v); }
            delete(n) { this._entries=this._entries.filter(function(e){return e[0]!==n;}); }
            entries() { return this._entries[Symbol.iterator](); }
            keys() { return this._entries.map(function(e){return e[0];})[Symbol.iterator](); }
            values() { return this._entries.map(function(e){return e[1];})[Symbol.iterator](); }
            forEach(cb) { this._entries.forEach(function(e){cb(e[1],e[0]);}); }
            [Symbol.iterator]() { return this.entries(); }
        };
        // Blob / File / FileReader
        globalThis.Blob = class Blob {
            constructor(parts, options) {
                this._data = '';
                if (parts) for (var i = 0; i < parts.length; i++) {
                    var p = parts[i];
                    if (p instanceof Blob) this._data += p._data;
                    else if (p instanceof ArrayBuffer) this._data += new TextDecoder().decode(p);
                    else if (ArrayBuffer.isView(p)) this._data += new TextDecoder().decode(p);
                    else this._data += String(p);
                }
                this.type = (options && options.type) || '';
                this.size = this._data.length;
            }
            text() { return Promise.resolve(this._data); }
            arrayBuffer() { return Promise.resolve(new TextEncoder().encode(this._data).buffer); }
            slice(start, end, type) {
                var s = this._data.slice(start || 0, end);
                var b = new Blob([s], {type: type || this.type});
                return b;
            }
            stream() { return { getReader: function() { var d = this._d; var done = false; return { read: function() { if (done) return Promise.resolve({done:true}); done=true; return Promise.resolve({value: new TextEncoder().encode(d), done:false}); }, cancel: function() { return Promise.resolve(); } }; }.bind({_d: this._data}) }; }
        };
        globalThis.File = class File extends Blob {
            constructor(parts, name, options) {
                super(parts, options);
                this.name = name;
                this.lastModified = (options && options.lastModified) || Date.now();
            }
        };
        globalThis.FileReader = class FileReader {
            constructor() { this.result = null; this.readyState = 0; this.error = null; this.onload = null; this.onerror = null; this.onloadend = null; }
            _finish(result) {
                var self = this;
                self.readyState = 1;
                setTimeout(function() {
                    self.result = result;
                    self.readyState = 2;
                    if (self.onload) self.onload({target: self});
                    if (self.onloadend) self.onloadend({target: self});
                }, 0);
            }
            readAsText(blob) { this._finish(blob._data); }
            readAsArrayBuffer(blob) { this._finish(new TextEncoder().encode(blob._data).buffer); }
            readAsDataURL(blob) { this._finish('data:' + (blob.type || 'application/octet-stream') + ';base64,' + btoa(blob._data)); }
            abort() { this.readyState = 2; }
        };
        // URL.createObjectURL / revokeObjectURL
        (function() {
            var blobStore = {};
            URL.createObjectURL = function(blob) { var id = 'blob:' + crypto.randomUUID(); blobStore[id] = blob; return id; };
            URL.revokeObjectURL = function(url) { delete blobStore[url]; };
        })();

        globalThis.queueMicrotask = function(cb) { Promise.resolve().then(cb); };
        // Unhandled promise rejection support
        globalThis.__braille_pending_rejections = [];
        globalThis.__braille_drain_rejections = function() {
            var arr = __braille_pending_rejections.splice(0);
            for (var i = 0; i < arr.length; i++) {
                var evt = new PromiseRejectionEvent('unhandledrejection', {
                    cancelable: true, promise: null, reason: arr[i]
                });
                window.dispatchEvent(evt);
                if (typeof window.onunhandledrejection === 'function') {
                    window.onunhandledrejection(evt);
                }
            }
        };
        globalThis.structuredClone = globalThis.structuredClone || function(v) {
            var seen = new Map();
            function clone(val) {
                if (val === null || typeof val !== 'object' && typeof val !== 'function') return val;
                if (typeof val === 'function' || typeof val === 'symbol') throw new DOMException('could not be cloned', 'DataCloneError');
                if (seen.has(val)) return seen.get(val);
                if (val instanceof Date) { var d = new Date(val.getTime()); seen.set(val, d); return d; }
                if (val instanceof RegExp) { var r = new RegExp(val.source, val.flags); seen.set(val, r); return r; }
                if (val instanceof Map) { var m = new Map(); seen.set(val, m); val.forEach(function(v, k) { m.set(clone(k), clone(v)); }); return m; }
                if (val instanceof Set) { var s = new Set(); seen.set(val, s); val.forEach(function(v) { s.add(clone(v)); }); return s; }
                if (val instanceof ArrayBuffer) { var ab = val.slice(0); seen.set(val, ab); return ab; }
                if (ArrayBuffer.isView(val)) { var buf = val.buffer.slice(0); var c = new val.constructor(buf, val.byteOffset, val.length); seen.set(val, c); return c; }
                if (val instanceof Error) { var e = new Error(val.message); e.stack = val.stack; e.name = val.name; seen.set(val, e); return e; }
                if (Array.isArray(val)) { var a = []; seen.set(val, a); for (var i = 0; i < val.length; i++) a[i] = clone(val[i]); return a; }
                var o = {}; seen.set(val, o);
                var keys = Object.keys(val);
                for (var i = 0; i < keys.length; i++) o[keys[i]] = clone(val[keys[i]]);
                return o;
            }
            return clone(v);
        };
        globalThis.WeakRef = globalThis.WeakRef || class WeakRef { constructor(t){this._t=t;} deref(){return this._t;} };
        globalThis.FinalizationRegistry = globalThis.FinalizationRegistry || class FinalizationRegistry { register(){} };

        // NodeFilter constants (callback interface, not constructible)
        globalThis.NodeFilter = {
            FILTER_ACCEPT: 1,
            FILTER_REJECT: 2,
            FILTER_SKIP: 3,
            SHOW_ALL: 0xFFFFFFFF,
            SHOW_ELEMENT: 0x1,
            SHOW_ATTRIBUTE: 0x2,
            SHOW_TEXT: 0x4,
            SHOW_CDATA_SECTION: 0x8,
            SHOW_PROCESSING_INSTRUCTION: 0x40,
            SHOW_COMMENT: 0x80,
            SHOW_DOCUMENT: 0x100,
            SHOW_DOCUMENT_TYPE: 0x200,
            SHOW_DOCUMENT_FRAGMENT: 0x400
        };

        // Expose constructor globals for interface-objects conformance
        globalThis.NodeList = class NodeList {};
        globalThis.HTMLCollection = class HTMLCollection {};
        globalThis.DOMTokenList = class DOMTokenList {};
        DOMTokenList.prototype[Symbol.toStringTag] = 'DOMTokenList';
        DOMTokenList.prototype.keys = Array.prototype.keys;
        DOMTokenList.prototype.values = Array.prototype.values;
        DOMTokenList.prototype.entries = Array.prototype.entries;
        DOMTokenList.prototype.forEach = Array.prototype.forEach;
        DOMTokenList.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];
        globalThis.Attr = class Attr extends Node {};

        // TreeWalker — spec-compliant traversal with whatToShow/filter support
        globalThis.TreeWalker = class TreeWalker {
            constructor(root, whatToShow, filter) {
                this.root = root;
                this.whatToShow = whatToShow === undefined ? 0xFFFFFFFF : (whatToShow >>> 0);
                this.filter = filter || null;
                this.currentNode = root;
            }
            _acceptNode(node) {
                var nodeType = node.nodeType;
                // whatToShow bitmask: bit (nodeType - 1)
                if (!((1 << (nodeType - 1)) & this.whatToShow)) return 3; // FILTER_SKIP
                if (!this.filter) return 1; // FILTER_ACCEPT
                if (typeof this.filter === 'function') return this.filter(node);
                if (typeof this.filter.acceptNode === 'function') return this.filter.acceptNode(node);
                return 1;
            }
            parentNode() {
                var node = this.currentNode;
                while (node && node !== this.root) {
                    node = node.parentNode;
                    if (node) {
                        var r = this._acceptNode(node);
                        if (r === 1) { this.currentNode = node; return node; }
                    }
                }
                return null;
            }
            _traverseChildren(first) {
                var node = first ? this.currentNode.firstChild : this.currentNode.lastChild;
                while (node) {
                    var r = this._acceptNode(node);
                    if (r === 1) { this.currentNode = node; return node; }
                    if (r === 3) { // FILTER_SKIP — try children
                        var child = first ? node.firstChild : node.lastChild;
                        if (child) { node = child; continue; }
                    }
                    // FILTER_REJECT or SKIP with no children — try siblings
                    while (node) {
                        var sib = first ? node.nextSibling : node.previousSibling;
                        if (sib) { node = sib; break; }
                        var parent = node.parentNode;
                        if (!parent || parent === this.root || parent === this.currentNode) { return null; }
                        node = parent;
                    }
                }
                return null;
            }
            firstChild() { return this._traverseChildren(true); }
            lastChild() { return this._traverseChildren(false); }
            _traverseSiblings(next) {
                var node = this.currentNode;
                if (node === this.root) return null;
                while (true) {
                    var sib = next ? node.nextSibling : node.previousSibling;
                    while (sib) {
                        var r = this._acceptNode(sib);
                        if (r === 1) { this.currentNode = sib; return sib; }
                        if (r === 3) { // FILTER_SKIP — descend into children
                            var child = next ? sib.firstChild : sib.lastChild;
                            if (child) { sib = child; continue; }
                        }
                        sib = next ? sib.nextSibling : sib.previousSibling;
                    }
                    node = node.parentNode;
                    if (!node || node === this.root) return null;
                    var pr = this._acceptNode(node);
                    if (pr === 1) return null; // parent accepted = no more siblings to traverse
                }
            }
            nextSibling() { return this._traverseSiblings(true); }
            previousSibling() { return this._traverseSiblings(false); }
            nextNode() {
                var node = this.currentNode;
                // Try first child
                var child = node.firstChild;
                while (child) {
                    var r = this._acceptNode(child);
                    if (r === 1) { this.currentNode = child; return child; }
                    if (r === 3 && child.firstChild) { child = child.firstChild; continue; } // SKIP — descend
                    // REJECT or SKIP-no-children — try next sibling, then walk up
                    break;
                }
                // Depth-first: try siblings and ancestors' siblings
                var cur = child || node;
                while (cur && cur !== this.root) {
                    var sib = cur.nextSibling;
                    while (sib) {
                        var r = this._acceptNode(sib);
                        if (r === 1) { this.currentNode = sib; return sib; }
                        if (r === 3 && sib.firstChild) { sib = sib.firstChild; continue; }
                        sib = sib.nextSibling;
                    }
                    cur = cur.parentNode;
                }
                return null;
            }
            previousNode() {
                var node = this.currentNode;
                while (node !== this.root) {
                    var sib = node.previousSibling;
                    while (sib) {
                        // Descend to last accepted descendant
                        var r = this._acceptNode(sib);
                        var last = sib.lastChild;
                        while (last) {
                            var lr = this._acceptNode(last);
                            if (lr === 2) { // REJECT — try previous sibling of last
                                last = last.previousSibling;
                                continue;
                            }
                            var deeper = last.lastChild;
                            if (deeper) { last = deeper; continue; }
                            if (lr === 1) { this.currentNode = last; return last; }
                            last = last.previousSibling;
                        }
                        if (r === 1) { this.currentNode = sib; return sib; }
                        sib = sib.previousSibling;
                    }
                    var parent = node.parentNode;
                    if (!parent || parent === this.root) return null;
                    var pr = this._acceptNode(parent);
                    if (pr === 1) { this.currentNode = parent; return parent; }
                    node = parent;
                }
                return null;
            }
        };

        // NodeIterator — flat pre-order traversal with whatToShow/filter support
        globalThis.NodeIterator = class NodeIterator {
            constructor(root, whatToShow, filter) {
                this.root = root;
                this.whatToShow = whatToShow === undefined ? 0xFFFFFFFF : (whatToShow >>> 0);
                this.filter = filter || null;
                this.referenceNode = root;
                this.pointerBeforeReferenceNode = true;
            }
            _acceptNode(node) {
                if (!((1 << (node.nodeType - 1)) & this.whatToShow)) return 3; // FILTER_SKIP
                if (!this.filter) return 1;
                if (typeof this.filter === 'function') return this.filter(node);
                if (typeof this.filter.acceptNode === 'function') return this.filter.acceptNode(node);
                return 1;
            }
            _nextInPreOrder(node) {
                if (node.firstChild) return node.firstChild;
                var cur = node;
                while (cur && cur !== this.root) {
                    if (cur.nextSibling) return cur.nextSibling;
                    cur = cur.parentNode;
                }
                return null;
            }
            _prevInPreOrder(node) {
                if (node === this.root) return null;
                var sib = node.previousSibling;
                if (sib) {
                    // Descend to last descendant
                    while (sib.lastChild) sib = sib.lastChild;
                    return sib;
                }
                return node.parentNode;
            }
            nextNode() {
                var node = this.referenceNode;
                var beforeRef = this.pointerBeforeReferenceNode;
                if (beforeRef) {
                    // pointer is before reference — first candidate is reference itself
                    var r = this._acceptNode(node);
                    this.pointerBeforeReferenceNode = false;
                    if (r === 1) return node;
                    // If rejected/skipped, fall through to walk forward
                }
                while (true) {
                    node = this._nextInPreOrder(node);
                    if (!node) return null;
                    this.referenceNode = node;
                    this.pointerBeforeReferenceNode = false;
                    var r = this._acceptNode(node);
                    if (r === 1) return node;
                }
            }
            previousNode() {
                var node = this.referenceNode;
                var beforeRef = this.pointerBeforeReferenceNode;
                if (!beforeRef) {
                    // pointer is after reference — first candidate is reference itself
                    var r = this._acceptNode(node);
                    this.pointerBeforeReferenceNode = true;
                    if (r === 1) return node;
                }
                while (true) {
                    node = this._prevInPreOrder(node);
                    if (!node) return null;
                    this.referenceNode = node;
                    this.pointerBeforeReferenceNode = true;
                    var r = this._acceptNode(node);
                    if (r === 1) return node;
                }
            }
            detach() {} // legacy no-op
        };

        // DOMImplementation constructor
        globalThis.DOMImplementation = class DOMImplementation {
            createHTMLDocument(title) { return document; }
            hasFeature() { return true; }
        };

        // ProcessingInstruction — not commonly used but test checks it exists
        globalThis.ProcessingInstruction = class ProcessingInstruction extends Node {};
        globalThis.DocumentType = class DocumentType extends Node {};
        globalThis.CharacterData = class CharacterData extends Node {};
        globalThis.Text = class Text extends CharacterData {
            constructor(data) {
                super();
                this.data = data !== undefined ? String(data) : '';
            }
        };
        globalThis.Comment = class Comment extends CharacterData {
            constructor(data) {
                super();
                this.data = data !== undefined ? String(data) : '';
            }
        };

        // Make all interface objects non-enumerable on globalThis (per spec)
        ['Event', 'CustomEvent', 'UIEvent', 'FocusEvent', 'MouseEvent', 'KeyboardEvent',
         'InputEvent', 'AnimationEvent', 'TransitionEvent', 'WheelEvent', 'CompositionEvent',
         'ErrorEvent', 'GamepadEvent', 'PointerEvent', 'TouchEvent', 'Touch',
         'ClipboardEvent', 'DragEvent', 'PopStateEvent', 'HashChangeEvent',
         'PromiseRejectionEvent', 'StorageEvent', 'MessageChannel',
         'EventTarget', 'Node', 'Document', 'DocumentFragment', 'ShadowRoot',
         'DOMImplementation', 'ProcessingInstruction', 'DocumentType',
         'Element', 'Attr', 'CharacterData', 'Text', 'Comment',
         'HTMLElement', 'HTMLIFrameElement', 'HTMLInputElement', 'HTMLTextAreaElement',
         'HTMLSelectElement', 'HTMLFormElement', 'HTMLAnchorElement', 'HTMLImageElement',
         'HTMLButtonElement', 'HTMLOptionElement', 'HTMLBodyElement', 'HTMLHeadElement',
         'HTMLFrameSetElement', 'HTMLHtmlElement', 'HTMLDivElement', 'HTMLSpanElement',
         'HTMLParagraphElement', 'HTMLScriptElement', 'HTMLStyleElement', 'HTMLLinkElement',
         'HTMLMetaElement', 'HTMLTableElement', 'HTMLTableRowElement', 'HTMLTableCellElement',
         'HTMLUListElement', 'HTMLOListElement', 'HTMLLIElement', 'HTMLPreElement',
         'HTMLCanvasElement', 'HTMLVideoElement', 'HTMLAudioElement', 'HTMLSourceElement',
         'HTMLLabelElement', 'HTMLTemplateElement', 'SVGElement', 'Window',
         'NodeIterator', 'TreeWalker', 'NodeFilter', 'NodeList', 'HTMLCollection', 'DOMTokenList',
         'CustomElementRegistry', 'CSSStyleSheet',
         'MutationObserver', 'ResizeObserver', 'IntersectionObserver',
         'XMLHttpRequest', 'DOMParser', 'FormData',
         'URL', 'URLSearchParams', 'TextEncoder', 'TextDecoder',
         'Blob', 'File', 'FileReader', 'ReadableStream',
         'AbortController', 'AbortSignal'].forEach(function(name) {
            if (globalThis[name] !== undefined) {
                Object.defineProperty(globalThis, name, {
                    value: globalThis[name], writable: true, configurable: true, enumerable: false
                });
            }
        });

        // Analytics stubs
        globalThis.dataLayer = [];
        globalThis.ga = function(){};
        globalThis.gtag = function(){};
    "#).unwrap();
}

// register_dom_stubs already includes document stub; dom_bridge::install overrides with real bindings.
// This function is kept for reference but unused.
#[allow(dead_code)]
fn _register_document_stub(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(r#"
        globalThis.document = {
            createElement: function(tag) { return { nodeName: tag.toUpperCase(), nodeType: 1, tagName: tag.toUpperCase(), children: [], childNodes: [], parentNode: null, style: {}, className: '', classList: { add:function(){}, remove:function(){}, contains:function(){return false;}, toggle:function(){} }, dataset: {}, attributes: [], setAttribute: function(){}, getAttribute: function(){return null;}, removeAttribute: function(){}, hasAttribute: function(){return false;}, addEventListener: function(){}, removeEventListener: function(){}, appendChild: function(c){this.childNodes.push(c);this.children.push(c);c.parentNode=this;return c;}, removeChild: function(c){var i=this.childNodes.indexOf(c);if(i>=0)this.childNodes.splice(i,1);i=this.children.indexOf(c);if(i>=0)this.children.splice(i,1);c.parentNode=null;return c;}, insertBefore: function(n,r){var i=this.childNodes.indexOf(r);if(i>=0){this.childNodes.splice(i,0,n);this.children.splice(i,0,n);}else{this.childNodes.push(n);this.children.push(n);}n.parentNode=this;return n;}, cloneNode: function(){return document.createElement(this.tagName||'div');}, contains: function(){return false;}, querySelector: function(){return null;}, querySelectorAll: function(){return [];}, getElementsByTagName: function(){return [];}, getElementsByClassName: function(){return [];}, innerHTML: '', textContent: '', outerHTML: '', getBoundingClientRect: function(){return{top:0,left:0,width:0,height:0,right:0,bottom:0};}, dispatchEvent: function(){return true;}, ownerDocument: null, id: '', }; },
            createElementNS: function(ns, tag) { var el = document.createElement(tag); el.namespaceURI = ns; return el; },
            createTextNode: function(t) { return { nodeType: 3, textContent: t, nodeName: '#text', parentNode: null, data: t }; },
            createComment: function(t) { return { nodeType: 8, textContent: t, nodeName: '#comment', parentNode: null, data: t }; },
            createDocumentFragment: function() { return { nodeType: 11, childNodes: [], children: [], appendChild: function(c){this.childNodes.push(c);this.children.push(c);c.parentNode=this;return c;}, querySelector: function(){return null;}, querySelectorAll: function(){return [];} }; },
            createRange: function() { return { setStart:function(){}, setEnd:function(){}, commonAncestorContainer: null, collapsed: true, selectNodeContents: function(){} }; },
            createTreeWalker: function() { return { nextNode: function(){return null;}, currentNode: null }; },
            getElementById: function() { return null; },
            getElementsByTagName: function() { return []; },
            getElementsByClassName: function() { return []; },
            querySelector: function() { return null; },
            querySelectorAll: function() { return []; },
            addEventListener: function() {},
            removeEventListener: function() {},
            head: { appendChild: function(c){return c;}, children: [], querySelectorAll: function(){return [];}, style: {} },
            body: { appendChild: function(c){return c;}, children: [], classList: {add:function(){},remove:function(){},contains:function(){return false;}}, style: {}, setAttribute: function(){}, getAttribute: function(){return null;}, addEventListener: function(){}, removeEventListener: function(){} },
            documentElement: { appendChild: function(c){return c;}, style: {}, setAttribute: function(){}, getAttribute: function(){return null;}, classList: {add:function(){},remove:function(){},contains:function(){return false;}} },
            title: '',
            cookie: '',
            readyState: 'complete',
            location: location,
            defaultView: globalThis,
            implementation: { createHTMLDocument: function(t) { return document; } },
            createEvent: function(t) { return new Event(t); },
            nodeType: 9,
            nodeName: '#document',
        };
    "#).unwrap();
}
