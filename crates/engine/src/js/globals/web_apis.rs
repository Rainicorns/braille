//! Standalone Web API polyfills that do NOT interact with the DOM tree.
//! Event classes, URL, FormData, Blob, navigator, localStorage, observers, etc.
//! If it calls __n_* native functions or __w(), it belongs in dom_bridge instead.
//! If it's called by dom_bridge code, it belongs in dom_helpers.rs instead.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(WEB_APIS_JS).unwrap();
}

const WEB_APIS_JS: &str = r#"
        globalThis.window = globalThis;
        globalThis.self = globalThis;
        globalThis.isSecureContext = true;

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
        globalThis.BeforeUnloadEvent = class BeforeUnloadEvent extends Event {};
        globalThis.DeviceMotionEvent = class DeviceMotionEvent extends Event {};
        globalThis.DeviceOrientationEvent = class DeviceOrientationEvent extends Event {};

        globalThis.TextEvent = class TextEvent extends UIEvent {};

        // Window dimensions
        window.innerWidth = 1280;
        window.innerHeight = 800;
        window.outerWidth = 1280;
        window.outerHeight = 900;
        window.devicePixelRatio = 1;
        window.parent = window;
        window.top = window;
        window.self = window;
        Object.defineProperty(window, 'frames', {
            get: function() {
                var iframeIds = __braille_find_iframes();
                var result = [];
                for (var i = 0; i < iframeIds.length; i++) {
                    var el = __braille_get_element_wrapper(iframeIds[i]);
                    if (el && el.contentWindow) {
                        result.push(el.contentWindow);
                    }
                }
                return result;
            },
            configurable: true
        });
        Object.defineProperty(window, 'length', {
            get: function() {
                return window.frames.length;
            },
            configurable: true
        });
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
            clipboard: {
                writeText: function(text) {
                    __braille_clipboard_write(String(text));
                    return Promise.resolve();
                },
                readText: function() {
                    return Promise.resolve(__braille_clipboard_read());
                },
                read: function() { return Promise.resolve([]); },
                write: function() { return Promise.resolve(); },
            },
            mediaDevices: {
                getUserMedia: function() { return Promise.reject(new DOMException('Not allowed', 'NotAllowedError')); },
                enumerateDevices: function() { return Promise.resolve([]); },
                getDisplayMedia: function() { return Promise.reject(new DOMException('Not allowed', 'NotAllowedError')); },
            },
            serviceWorker: {
                register: function(url) { return Promise.resolve({ installing: null, waiting: null, active: null, scope: '/', unregister: function() { return Promise.resolve(true); } }); },
                ready: Promise.resolve({ active: null }),
                controller: null,
                addEventListener: function() {},
                removeEventListener: function() {},
            },
            permissions: {
                query: function(desc) {
                    var name = desc && desc.name || '';
                    // Default: granted for most things (permissive browser)
                    var state = 'granted';
                    return Promise.resolve({
                        state: state,
                        name: name,
                        addEventListener: function() {},
                        removeEventListener: function() {},
                        onchange: null,
                    });
                }
            },
            geolocation: {
                getCurrentPosition: function(success, error) {
                    if (typeof error === 'function') {
                        error({ code: 1, message: 'Permission denied', PERMISSION_DENIED: 1, POSITION_UNAVAILABLE: 2, TIMEOUT: 3 });
                    }
                },
                watchPosition: function(success, error) {
                    if (typeof error === 'function') {
                        error({ code: 1, message: 'Permission denied', PERMISSION_DENIED: 1, POSITION_UNAVAILABLE: 2, TIMEOUT: 3 });
                    }
                    return 0;
                },
                clearWatch: function() {},
            },
            sendBeacon: function() { return true; },
            bluetooth: { requestDevice: function() { return Promise.reject(new DOMException('Not found', 'NotFoundError')); } },
            serial: { requestPort: function() { return Promise.reject(new DOMException('Not found', 'NotFoundError')); } },
            usb: { requestDevice: function() { return Promise.reject(new DOMException('Not found', 'NotFoundError')); } },
            hid: { requestDevice: function() { return Promise.reject(new DOMException('Not found', 'NotFoundError')); } },
            locks: { request: function(name, cb) { return Promise.resolve(cb({ mode: 'exclusive', name: name })); } },
            credentials: { get: function() { return Promise.resolve(null); }, create: function() { return Promise.resolve(null); }, store: function() { return Promise.resolve(); } },
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
                    loc._href = String(v);
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
        // back/forward/go fire popstate events with the stored state
        globalThis.history = (function() {
            var stateStack = [{state: null, url: location.href}];
            var stateIndex = 0;
            function resolveUrl(url) {
                if (!url) return location.href;
                var u = String(url);
                if (u.charAt(0) === '/') u = location.origin + u;
                else if (!/^https?:\/\//.test(u)) u = location.origin + location.pathname.replace(/[^\/]*$/, '') + u;
                return u;
            }
            function updateUrl(url) {
                if (!url) return;
                var u = resolveUrl(url);
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
            function firePopState(state) {
                var evt = new PopStateEvent('popstate', {state: state});
                window.dispatchEvent(evt);
            }
            return {
                pushState: function(s, t, u) {
                    stateStack.splice(stateIndex + 1);
                    var resolved = resolveUrl(u);
                    stateStack.push({state: s, url: resolved});
                    stateIndex = stateStack.length - 1;
                    this.state = s;
                    this.length = stateStack.length;
                    updateUrl(u);
                },
                replaceState: function(s, t, u) {
                    var resolved = u ? resolveUrl(u) : stateStack[stateIndex].url;
                    stateStack[stateIndex] = {state: s, url: resolved};
                    this.state = s;
                    updateUrl(u);
                },
                back: function() {
                    if (stateIndex > 0) {
                        stateIndex--;
                        var entry = stateStack[stateIndex];
                        this.state = entry.state;
                        updateUrl(entry.url);
                        firePopState(entry.state);
                    }
                },
                forward: function() {
                    if (stateIndex < stateStack.length - 1) {
                        stateIndex++;
                        var entry = stateStack[stateIndex];
                        this.state = entry.state;
                        updateUrl(entry.url);
                        firePopState(entry.state);
                    }
                },
                go: function(n) {
                    var idx = stateIndex + (n || 0);
                    if (idx >= 0 && idx < stateStack.length && idx !== stateIndex) {
                        stateIndex = idx;
                        var entry = stateStack[stateIndex];
                        this.state = entry.state;
                        updateUrl(entry.url);
                        firePopState(entry.state);
                    }
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
            var matches = __n_matchMedia(q);
            var _listeners = [];
            var mql = {
                matches: matches, media: q,
                onchange: null,
                addListener: function(cb) { if (cb) _listeners.push(cb); },
                removeListener: function(cb) { var i = _listeners.indexOf(cb); if (i >= 0) _listeners.splice(i, 1); },
                addEventListener: function(type, cb) { if (type === 'change' && cb) _listeners.push(cb); },
                removeEventListener: function(type, cb) { var i = _listeners.indexOf(cb); if (i >= 0) _listeners.splice(i, 1); },
                dispatchEvent: function() { return true; },
            };
            return mql;
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

        // ResizeObserver — tracks dimension changes on observed elements
        (function() {
            var allROEntries = []; // [{observer, target, prevWidth, prevHeight}]

            globalThis.__ro_check = function() {
                var fired = false;
                // Group entries by observer
                var observerMap = new Map();
                for (var i = 0; i < allROEntries.length; i++) {
                    var entry = allROEntries[i];
                    var target = entry.target;
                    if (!target || typeof target.getBoundingClientRect !== 'function') continue;
                    var rect = target.getBoundingClientRect();
                    var w = rect.width, h = rect.height;
                    if (w !== entry.prevWidth || h !== entry.prevHeight) {
                        entry.prevWidth = w;
                        entry.prevHeight = h;
                        if (!observerMap.has(entry.observer)) observerMap.set(entry.observer, []);
                        observerMap.get(entry.observer).push({
                            target: target,
                            contentRect: rect,
                            borderBoxSize: [{inlineSize: w, blockSize: h}],
                            contentBoxSize: [{inlineSize: w, blockSize: h}],
                            devicePixelContentBoxSize: [{inlineSize: w, blockSize: h}],
                        });
                    }
                }
                observerMap.forEach(function(entries, obs) {
                    obs._cb(entries, obs);
                    fired = true;
                });
                return fired;
            };

            globalThis.ResizeObserver = class {
                constructor(cb) { this._cb = cb; this._id = Math.random(); }
                observe(target) {
                    // Avoid duplicate registration
                    for (var i = 0; i < allROEntries.length; i++) {
                        if (allROEntries[i].observer === this && allROEntries[i].target === target) return;
                    }
                    allROEntries.push({observer: this, target: target, prevWidth: -1, prevHeight: -1});
                }
                unobserve(target) {
                    allROEntries = allROEntries.filter(function(e) { return !(e.observer === this && e.target === target); }.bind(this));
                }
                disconnect() {
                    allROEntries = allROEntries.filter(function(e) { return e.observer !== this; }.bind(this));
                }
            };
        })();

        // IntersectionObserver — tracks visibility of elements in viewport
        (function() {
            var allIOEntries = []; // [{observer, target, prevIntersecting}]
            var viewportW = 1280, viewportH = 800;

            globalThis.__io_check = function() {
                var fired = false;
                var observerMap = new Map();
                for (var i = 0; i < allIOEntries.length; i++) {
                    var entry = allIOEntries[i];
                    var target = entry.target;
                    if (!target || typeof target.getBoundingClientRect !== 'function') continue;
                    var rect = target.getBoundingClientRect();
                    var thresholds = entry.observer._thresholds;
                    // Compute intersection with viewport
                    var intTop = Math.max(rect.top, 0);
                    var intLeft = Math.max(rect.left, 0);
                    var intRight = Math.min(rect.right, viewportW);
                    var intBottom = Math.min(rect.bottom, viewportH);
                    var intW = Math.max(0, intRight - intLeft);
                    var intH = Math.max(0, intBottom - intTop);
                    var targetArea = rect.width * rect.height;
                    var ratio = targetArea > 0 ? (intW * intH) / targetArea : 0;
                    var isIntersecting = ratio > 0;

                    // Check if any threshold was crossed
                    var prevRatio = entry.prevRatio === undefined ? -1 : entry.prevRatio;
                    var crossed = false;
                    for (var t = 0; t < thresholds.length; t++) {
                        var th = thresholds[t];
                        if ((prevRatio < th && ratio >= th) || (prevRatio >= th && ratio < th)) {
                            crossed = true;
                            break;
                        }
                    }
                    // Always fire on first check
                    if (entry.prevRatio === undefined) crossed = true;

                    if (crossed) {
                        entry.prevRatio = ratio;
                        if (!observerMap.has(entry.observer)) observerMap.set(entry.observer, []);
                        var intRect = {top: intTop, left: intLeft, right: intRight, bottom: intBottom, width: intW, height: intH, x: intLeft, y: intTop};
                        observerMap.get(entry.observer).push({
                            target: target,
                            isIntersecting: isIntersecting,
                            intersectionRatio: ratio,
                            boundingClientRect: rect,
                            intersectionRect: intRect,
                            rootBounds: {top:0, left:0, width:viewportW, height:viewportH, right:viewportW, bottom:viewportH, x:0, y:0},
                            time: performance.now(),
                        });
                    }
                }
                observerMap.forEach(function(entries, obs) {
                    obs._cb(entries, obs);
                    fired = true;
                });
                return fired;
            };

            globalThis.IntersectionObserver = class {
                constructor(cb, opts) {
                    this._cb = cb;
                    this._opts = opts || {};
                    // Normalize thresholds
                    var t = this._opts.threshold;
                    if (t === undefined || t === null) t = [0];
                    if (typeof t === 'number') t = [t];
                    this._thresholds = t;
                    this._pending = [];
                }
                observe(target) {
                    for (var i = 0; i < allIOEntries.length; i++) {
                        if (allIOEntries[i].observer === this && allIOEntries[i].target === target) return;
                    }
                    allIOEntries.push({observer: this, target: target, prevRatio: undefined});
                }
                unobserve(target) {
                    allIOEntries = allIOEntries.filter(function(e) { return !(e.observer === this && e.target === target); }.bind(this));
                }
                disconnect() {
                    allIOEntries = allIOEntries.filter(function(e) { return e.observer !== this; }.bind(this));
                }
                takeRecords() {
                    var records = this._pending;
                    this._pending = [];
                    return records;
                }
                get thresholds() { return this._thresholds; }
                get rootMargin() { return this._opts.rootMargin || '0px 0px 0px 0px'; }
                get root() { return this._opts.root || null; }
            };
        })();

        // Performance API — full implementation with marks, measures, and observers
        var __perf_start = Date.now();
        (function() {
            var _entries = [];
            var _observers = [];

            function PerformanceEntry(name, entryType, startTime, duration) {
                this.name = name;
                this.entryType = entryType;
                this.startTime = startTime;
                this.duration = duration;
            }
            PerformanceEntry.prototype.toJSON = function() {
                return { name: this.name, entryType: this.entryType, startTime: this.startTime, duration: this.duration };
            };

            function notifyObservers(entry) {
                for (var i = 0; i < _observers.length; i++) {
                    var obs = _observers[i];
                    if (obs._types && obs._types.indexOf(entry.entryType) >= 0) {
                        obs._buffer.push(entry);
                        if (obs._scheduled) continue;
                        obs._scheduled = true;
                        var o = obs;
                        setTimeout(function() {
                            o._scheduled = false;
                            if (o._buffer.length && o._cb) {
                                var list = { getEntries: function() { return o._buffer.slice(); } };
                                var buf = o._buffer;
                                o._buffer = [];
                                o._cb(list, o);
                            }
                        }, 0);
                    }
                }
            }

            globalThis.performance = {
                now: function() { return Date.now() - __perf_start; },
                timeOrigin: __perf_start,
                mark: function(name, opts) {
                    var startTime = (opts && opts.startTime !== undefined) ? opts.startTime : (Date.now() - __perf_start);
                    var entry = new PerformanceEntry(name, 'mark', startTime, 0);
                    if (opts && opts.detail !== undefined) entry.detail = opts.detail;
                    _entries.push(entry);
                    notifyObservers(entry);
                    return entry;
                },
                measure: function(name, startOrOpts, endMark) {
                    var startTime = 0, endTime = Date.now() - __perf_start;
                    if (typeof startOrOpts === 'string') {
                        for (var i = _entries.length - 1; i >= 0; i--) {
                            if (_entries[i].name === startOrOpts && _entries[i].entryType === 'mark') {
                                startTime = _entries[i].startTime;
                                break;
                            }
                        }
                        if (typeof endMark === 'string') {
                            for (var j = _entries.length - 1; j >= 0; j--) {
                                if (_entries[j].name === endMark && _entries[j].entryType === 'mark') {
                                    endTime = _entries[j].startTime;
                                    break;
                                }
                            }
                        }
                    } else if (startOrOpts && typeof startOrOpts === 'object') {
                        if (startOrOpts.start !== undefined) {
                            if (typeof startOrOpts.start === 'string') {
                                for (var k = _entries.length - 1; k >= 0; k--) {
                                    if (_entries[k].name === startOrOpts.start && _entries[k].entryType === 'mark') {
                                        startTime = _entries[k].startTime;
                                        break;
                                    }
                                }
                            } else {
                                startTime = startOrOpts.start;
                            }
                        }
                        if (startOrOpts.end !== undefined) {
                            if (typeof startOrOpts.end === 'string') {
                                for (var l = _entries.length - 1; l >= 0; l--) {
                                    if (_entries[l].name === startOrOpts.end && _entries[l].entryType === 'mark') {
                                        endTime = _entries[l].startTime;
                                        break;
                                    }
                                }
                            } else {
                                endTime = startOrOpts.end;
                            }
                        }
                        if (startOrOpts.duration !== undefined) {
                            endTime = startTime + startOrOpts.duration;
                        }
                    }
                    var entry = new PerformanceEntry(name, 'measure', startTime, endTime - startTime);
                    _entries.push(entry);
                    notifyObservers(entry);
                    return entry;
                },
                getEntries: function() { return _entries.slice(); },
                getEntriesByName: function(name, type) {
                    return _entries.filter(function(e) {
                        return e.name === name && (!type || e.entryType === type);
                    });
                },
                getEntriesByType: function(type) {
                    return _entries.filter(function(e) { return e.entryType === type; });
                },
                clearMarks: function(name) {
                    if (name) { _entries = _entries.filter(function(e) { return !(e.entryType === 'mark' && e.name === name); }); }
                    else { _entries = _entries.filter(function(e) { return e.entryType !== 'mark'; }); }
                },
                clearMeasures: function(name) {
                    if (name) { _entries = _entries.filter(function(e) { return !(e.entryType === 'measure' && e.name === name); }); }
                    else { _entries = _entries.filter(function(e) { return e.entryType !== 'measure'; }); }
                },
                clearResourceTimings: function() {
                    _entries = _entries.filter(function(e) { return e.entryType !== 'resource'; });
                },
                setResourceTimingBufferSize: function() {},
                timing: {
                    navigationStart: __perf_start, unloadEventStart: 0, unloadEventEnd: 0,
                    redirectStart: 0, redirectEnd: 0, fetchStart: __perf_start,
                    domainLookupStart: __perf_start, domainLookupEnd: __perf_start,
                    connectStart: __perf_start, connectEnd: __perf_start,
                    secureConnectionStart: 0, requestStart: __perf_start,
                    responseStart: __perf_start, responseEnd: __perf_start,
                    domLoading: __perf_start, domInteractive: __perf_start,
                    domContentLoadedEventStart: __perf_start, domContentLoadedEventEnd: __perf_start,
                    domComplete: __perf_start, loadEventStart: __perf_start, loadEventEnd: __perf_start,
                },
                navigation: { type: 0, redirectCount: 0 },
            };

            globalThis.PerformanceObserver = function PerformanceObserver(cb) {
                this._cb = cb;
                this._types = [];
                this._buffer = [];
                this._scheduled = false;
            };
            PerformanceObserver.prototype.observe = function(opts) {
                if (opts && opts.entryTypes) this._types = opts.entryTypes;
                else if (opts && opts.type) this._types = [opts.type];
                if (_observers.indexOf(this) < 0) _observers.push(this);
            };
            PerformanceObserver.prototype.disconnect = function() {
                var i = _observers.indexOf(this);
                if (i >= 0) _observers.splice(i, 1);
            };
            PerformanceObserver.prototype.takeRecords = function() {
                var buf = this._buffer;
                this._buffer = [];
                return buf;
            };
            PerformanceObserver.supportedEntryTypes = ['mark', 'measure'];
        })();

        // URL
        globalThis.URL = class URL {
            constructor(u, base) {
                u = String(u);
                if (base) base = String(base);
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

        // EventSource (Server-Sent Events)
        globalThis.EventSource = function EventSource(url, opts) {
            var self = this;
            this.url = url;
            this.withCredentials = (opts && opts.withCredentials) || false;
            this.readyState = 0; // CONNECTING
            this.onopen = null;
            this.onmessage = null;
            this.onerror = null;
            this._listeners = {};
            this.CONNECTING = 0;
            this.OPEN = 1;
            this.CLOSED = 2;
            this.addEventListener = function(type, cb) {
                if (!self._listeners[type]) self._listeners[type] = [];
                self._listeners[type].push(cb);
            };
            this.removeEventListener = function(type, cb) {
                if (!self._listeners[type]) return;
                var i = self._listeners[type].indexOf(cb);
                if (i >= 0) self._listeners[type].splice(i, 1);
            };
            this.dispatchEvent = function(event) {
                var type = event.type;
                if (self['on' + type]) self['on' + type](event);
                if (self._listeners[type]) {
                    for (var j = 0; j < self._listeners[type].length; j++) {
                        self._listeners[type][j](event);
                    }
                }
                return true;
            };
            this.close = function() {
                self.readyState = 2; // CLOSED
            };
        };
        EventSource.CONNECTING = 0;
        EventSource.OPEN = 1;
        EventSource.CLOSED = 2;

        // Analytics stubs
        globalThis.dataLayer = [];
        globalThis.ga = function(){};
        globalThis.gtag = function(){};

        // Window/Dialog stubs
        globalThis.open = function(url, target) { return null; };
        globalThis.close = function() {};
        globalThis.print = function() {};
        globalThis.stop = function() {};

        // Media elements — HTMLMediaElement prototype
        (function() {
            var MediaProto = {
                play: function() { return Promise.reject(new DOMException('Not allowed', 'NotAllowedError')); },
                pause: function() {},
                load: function() {},
                canPlayType: function() { return ''; },
                addTextTrack: function() { return { kind:'subtitles', label:'', language:'', mode:'disabled', cues:null, addCue:function(){}, removeCue:function(){} }; },
                paused: true, ended: false, currentTime: 0, duration: NaN,
                readyState: 0, networkState: 0, seeking: false,
                volume: 1, muted: false, defaultMuted: false,
                playbackRate: 1, defaultPlaybackRate: 1,
                buffered: { length: 0, start: function() { return 0; }, end: function() { return 0; } },
                played: { length: 0, start: function() { return 0; }, end: function() { return 0; } },
                seekable: { length: 0, start: function() { return 0; }, end: function() { return 0; } },
                textTracks: { length: 0, addEventListener: function() {}, removeEventListener: function() {} },
                videoWidth: 0, videoHeight: 0, poster: '',
                crossOrigin: null, preload: 'auto', autoplay: false, loop: false, controls: false,
                addEventListener: function() {}, removeEventListener: function() {},
                dispatchEvent: function() { return true; },
            };
            if (typeof HTMLVideoElement !== 'undefined') Object.assign(HTMLVideoElement.prototype, MediaProto);
            if (typeof HTMLAudioElement !== 'undefined') Object.assign(HTMLAudioElement.prototype, MediaProto);
        })();

        // ValidityState + File/Blob completeness
        if (typeof FileReader !== 'undefined') {
            FileReader.EMPTY = 0; FileReader.LOADING = 1; FileReader.DONE = 2;
            if (!FileReader.prototype.readAsBinaryString) {
                FileReader.prototype.readAsBinaryString = function(blob) {
                    var reader = this;
                    reader.readyState = 1;
                    blob.text().then(function(text) {
                        reader.readyState = 2;
                        reader.result = text;
                        if (reader.onload) reader.onload({target: reader});
                    });
                };
            }
        }

        // Service Worker lifecycle stubs
        if (navigator.serviceWorker) {
            navigator.serviceWorker.getRegistrations = function() { return Promise.resolve([]); };
            navigator.serviceWorker.getRegistration = function() { return Promise.resolve(undefined); };
        }

        // BroadcastChannel
        globalThis.BroadcastChannel = function BroadcastChannel(name) {
            this.name = name;
            this.onmessage = null;
            this.onmessageerror = null;
        };
        BroadcastChannel.prototype.postMessage = function() {};
        BroadcastChannel.prototype.close = function() {};
        BroadcastChannel.prototype.addEventListener = function() {};
        BroadcastChannel.prototype.removeEventListener = function() {};

        // screen.orientation
        if (typeof screen !== 'undefined') {
            screen.orientation = {
                type: 'landscape-primary', angle: 0,
                lock: function() { return Promise.resolve(); },
                unlock: function() {},
                addEventListener: function() {},
                removeEventListener: function() {},
                onchange: null,
            };
        }

        // OffscreenCanvas
        globalThis.OffscreenCanvas = function OffscreenCanvas(width, height) {
            this.width = width || 0;
            this.height = height || 0;
        };
        OffscreenCanvas.prototype.getContext = function() { return null; };
        OffscreenCanvas.prototype.convertToBlob = function() { return Promise.resolve(new Blob([])); };
        OffscreenCanvas.prototype.transferToImageBitmap = function() { return { width: this.width, height: this.height, close: function() {} }; };

        // ImageBitmap / createImageBitmap
        globalThis.ImageBitmap = function ImageBitmap() { this.width = 0; this.height = 0; };
        ImageBitmap.prototype.close = function() {};
        globalThis.createImageBitmap = function() {
            return Promise.resolve(new ImageBitmap());
        };

        // IntersectionObserverEntry / ResizeObserverEntry class prototypes
        if (typeof IntersectionObserverEntry === 'undefined') {
            globalThis.IntersectionObserverEntry = function IntersectionObserverEntry() {};
        }
        if (typeof ResizeObserverEntry === 'undefined') {
            globalThis.ResizeObserverEntry = function ResizeObserverEntry() {};
        }

        // Notification
        globalThis.Notification = function Notification(title, opts) {
            this.title = title;
            this.body = (opts && opts.body) || '';
            this.icon = (opts && opts.icon) || '';
            this.tag = (opts && opts.tag) || '';
            this.onclick = null;
            this.onclose = null;
            this.onerror = null;
            this.onshow = null;
        };
        Notification.permission = 'default';
        Notification.requestPermission = function(cb) {
            var result = 'denied';
            if (cb) cb(result);
            return Promise.resolve(result);
        };
        Notification.prototype.close = function() {};
        Notification.prototype.addEventListener = function() {};
        Notification.prototype.removeEventListener = function() {};

        // Canvas 2D
        (function() {
            var CanvasProto = {
                fillRect: function() {},
                strokeRect: function() {},
                clearRect: function() {},
                beginPath: function() {},
                closePath: function() {},
                moveTo: function() {},
                lineTo: function() {},
                bezierCurveTo: function() {},
                quadraticCurveTo: function() {},
                arc: function() {},
                arcTo: function() {},
                ellipse: function() {},
                rect: function() {},
                fill: function() {},
                stroke: function() {},
                clip: function() {},
                isPointInPath: function() { return false; },
                isPointInStroke: function() { return false; },
                fillText: function() {},
                strokeText: function() {},
                measureText: function(text) {
                    var fontSize = parseFloat(this.font) || 10;
                    var width = fontSize * 0.6 * (text || '').length;
                    return {
                        width: width,
                        actualBoundingBoxAscent: fontSize * 0.8,
                        actualBoundingBoxDescent: fontSize * 0.2,
                        fontBoundingBoxAscent: fontSize * 0.8,
                        fontBoundingBoxDescent: fontSize * 0.2,
                        actualBoundingBoxLeft: 0,
                        actualBoundingBoxRight: width,
                        emHeightAscent: fontSize * 0.8,
                        emHeightDescent: fontSize * 0.2,
                    };
                },
                drawImage: function() {},
                createImageData: function(w, h) {
                    if (typeof w === 'object') { h = w.height; w = w.width; }
                    return { width: w, height: h, data: new Uint8ClampedArray(w * h * 4) };
                },
                getImageData: function(x, y, w, h) {
                    return { width: w, height: h, data: new Uint8ClampedArray(w * h * 4) };
                },
                putImageData: function() {},
                createLinearGradient: function(x0, y0, x1, y1) {
                    return { addColorStop: function() {} };
                },
                createRadialGradient: function(x0, y0, r0, x1, y1, r1) {
                    return { addColorStop: function() {} };
                },
                createConicGradient: function(startAngle, cx, cy) {
                    return { addColorStop: function() {} };
                },
                createPattern: function() { return {}; },
                save: function() {},
                restore: function() {},
                scale: function() {},
                rotate: function() {},
                translate: function() {},
                transform: function() {},
                setTransform: function() {},
                resetTransform: function() {},
                getTransform: function() { return new DOMMatrix(); },
                setLineDash: function() {},
                getLineDash: function() { return []; },
                drawFocusIfNeeded: function() {},
                toDataURL: function() { return 'data:image/png;base64,'; },
                canvas: null,
                fillStyle: '#000000',
                strokeStyle: '#000000',
                lineWidth: 1,
                lineCap: 'butt',
                lineJoin: 'miter',
                miterLimit: 10,
                lineDashOffset: 0,
                font: '10px sans-serif',
                textAlign: 'start',
                textBaseline: 'alphabetic',
                direction: 'ltr',
                globalAlpha: 1,
                globalCompositeOperation: 'source-over',
                imageSmoothingEnabled: true,
                imageSmoothingQuality: 'low',
                shadowBlur: 0,
                shadowColor: 'rgba(0,0,0,0)',
                shadowOffsetX: 0,
                shadowOffsetY: 0,
                filter: 'none',
                letterSpacing: '0px',
                wordSpacing: '0px',
                fontKerning: 'auto',
                textRendering: 'auto',
            };
            globalThis.__CanvasRenderingContext2D = CanvasProto;
            globalThis.CanvasRenderingContext2D = function CanvasRenderingContext2D() {};
            Object.assign(CanvasRenderingContext2D.prototype, CanvasProto);
            globalThis.CanvasGradient = function CanvasGradient() {};
            CanvasGradient.prototype.addColorStop = function() {};
            globalThis.CanvasPattern = function CanvasPattern() {};
            CanvasPattern.prototype.setTransform = function() {};
            globalThis.Path2D = function Path2D() {};
            var p2d = Path2D.prototype;
            p2d.addPath = function() {}; p2d.closePath = function() {};
            p2d.moveTo = function() {}; p2d.lineTo = function() {};
            p2d.bezierCurveTo = function() {}; p2d.quadraticCurveTo = function() {};
            p2d.arc = function() {}; p2d.arcTo = function() {};
            p2d.ellipse = function() {}; p2d.rect = function() {};
            p2d.roundRect = function() {};
            globalThis.ImageData = function ImageData(w, h) {
                if (typeof w === 'object') { h = w.height; w = w.width; }
                this.width = w || 0; this.height = h || 0;
                this.data = new Uint8ClampedArray((this.width) * (this.height) * 4);
            };
        })();

        // IndexedDB — in-memory stub
        (function() {
            var _dbs = {};
            function IDBRequest() { this.result = undefined; this.error = null; this.readyState = 'pending'; this.onsuccess = null; this.onerror = null; this.source = null; this.transaction = null; }
            function fireSuccess(req, result) { req.result = result; req.readyState = 'done'; setTimeout(function() { if (req.onsuccess) req.onsuccess({target: req}); }, 0); }
            function fireError(req, error) { req.error = error; req.readyState = 'done'; setTimeout(function() { if (req.onerror) req.onerror({target: req}); }, 0); }
            function IDBObjectStore(name, keyPath, db, txn) {
                this.name = name; this.keyPath = keyPath; this.autoIncrement = true;
                this._data = new Map(); this._nextKey = 1; this._db = db; this.transaction = txn;
                this.indexNames = []; this._indexes = {};
            }
            IDBObjectStore.prototype.put = function(value, key) {
                var req = new IDBRequest();
                var k = key !== undefined ? key : (this.keyPath && value ? value[this.keyPath] : this._nextKey++);
                this._data.set(k, structuredClone(value));
                fireSuccess(req, k);
                return req;
            };
            IDBObjectStore.prototype.add = function(value, key) { return this.put(value, key); };
            IDBObjectStore.prototype.get = function(key) {
                var req = new IDBRequest();
                var val = this._data.has(key) ? structuredClone(this._data.get(key)) : undefined;
                fireSuccess(req, val);
                return req;
            };
            IDBObjectStore.prototype.getAll = function() {
                var req = new IDBRequest();
                var arr = []; this._data.forEach(function(v) { arr.push(structuredClone(v)); });
                fireSuccess(req, arr);
                return req;
            };
            IDBObjectStore.prototype.delete = function(key) {
                var req = new IDBRequest();
                this._data.delete(key);
                fireSuccess(req, undefined);
                return req;
            };
            IDBObjectStore.prototype.clear = function() {
                var req = new IDBRequest();
                this._data.clear();
                fireSuccess(req, undefined);
                return req;
            };
            IDBObjectStore.prototype.count = function() {
                var req = new IDBRequest();
                fireSuccess(req, this._data.size);
                return req;
            };
            IDBObjectStore.prototype.openCursor = function() {
                var req = new IDBRequest();
                var entries = []; this._data.forEach(function(v, k) { entries.push({key:k, value:structuredClone(v)}); });
                var idx = 0;
                function makeCursor() {
                    if (idx >= entries.length) { fireSuccess(req, null); return; }
                    var e = entries[idx];
                    var cursor = { key: e.key, primaryKey: e.key, value: e.value,
                        continue: function() { idx++; makeCursor(); },
                        advance: function(n) { idx += n; makeCursor(); },
                        delete: function() { return new IDBRequest(); },
                        update: function() { return new IDBRequest(); }
                    };
                    fireSuccess(req, cursor);
                }
                makeCursor();
                return req;
            };
            IDBObjectStore.prototype.createIndex = function(name) {
                this._indexes[name] = true;
                this.indexNames.push(name);
                return { get: function() { return new IDBRequest(); }, getAll: function() { var r = new IDBRequest(); fireSuccess(r,[]); return r; }, openCursor: function() { var r = new IDBRequest(); fireSuccess(r,null); return r; }, count: function() { var r = new IDBRequest(); fireSuccess(r,0); return r; } };
            };
            IDBObjectStore.prototype.index = function(name) { return this.createIndex(name); };
            IDBObjectStore.prototype.getAllKeys = function() { var req = new IDBRequest(); var arr = []; this._data.forEach(function(_,k) { arr.push(k); }); fireSuccess(req, arr); return req; };
            IDBObjectStore.prototype.getKey = function(key) { var req = new IDBRequest(); fireSuccess(req, this._data.has(key) ? key : undefined); return req; };

            function IDBTransaction(db, storeNames, mode) {
                this.db = db; this.mode = mode || 'readonly'; this.objectStoreNames = storeNames;
                this.oncomplete = null; this.onerror = null; this.onabort = null; this.error = null;
                var self = this;
                setTimeout(function() { if (self.oncomplete) self.oncomplete({target: self}); }, 0);
            }
            IDBTransaction.prototype.objectStore = function(name) {
                if (!this.db._stores[name]) this.db._stores[name] = new IDBObjectStore(name, null, this.db, this);
                return this.db._stores[name];
            };
            IDBTransaction.prototype.abort = function() { if (this.onabort) this.onabort({target: this}); };
            IDBTransaction.prototype.commit = function() { if (this.oncomplete) this.oncomplete({target: this}); };
            IDBTransaction.prototype.addEventListener = function(type, cb) { this['on'+type] = cb; };
            IDBTransaction.prototype.removeEventListener = function() {};

            function IDBDatabase(name, version) {
                this.name = name; this.version = version; this._stores = {};
                this.objectStoreNames = [];
                this.onclose = null; this.onerror = null; this.onversionchange = null;
            }
            IDBDatabase.prototype.createObjectStore = function(name, opts) {
                var store = new IDBObjectStore(name, opts && opts.keyPath, this, null);
                this._stores[name] = store;
                if (this.objectStoreNames.indexOf(name) < 0) this.objectStoreNames.push(name);
                return store;
            };
            IDBDatabase.prototype.deleteObjectStore = function(name) { delete this._stores[name]; var i = this.objectStoreNames.indexOf(name); if (i >= 0) this.objectStoreNames.splice(i,1); };
            IDBDatabase.prototype.transaction = function(storeNames, mode) { return new IDBTransaction(this, Array.isArray(storeNames) ? storeNames : [storeNames], mode); };
            IDBDatabase.prototype.close = function() {};
            IDBDatabase.prototype.addEventListener = function() {};
            IDBDatabase.prototype.removeEventListener = function() {};

            globalThis.indexedDB = {
                open: function(name, version) {
                    var req = new IDBRequest();
                    version = version || 1;
                    var isNew = !_dbs[name];
                    var needsUpgrade = isNew || (_dbs[name] && _dbs[name].version < version);
                    if (isNew) _dbs[name] = new IDBDatabase(name, version);
                    var db = _dbs[name];
                    db.version = version;
                    req.result = db;
                    if (needsUpgrade) {
                        req.transaction = new IDBTransaction(db, [], 'versionchange');
                        setTimeout(function() {
                            if (req.onupgradeneeded) req.onupgradeneeded({target: req, oldVersion: isNew ? 0 : db.version - 1, newVersion: version});
                            setTimeout(function() { fireSuccess(req, db); }, 0);
                        }, 0);
                    } else {
                        fireSuccess(req, db);
                    }
                    return req;
                },
                deleteDatabase: function(name) {
                    var req = new IDBRequest();
                    delete _dbs[name];
                    fireSuccess(req, undefined);
                    return req;
                },
                databases: function() { return Promise.resolve(Object.keys(_dbs).map(function(n) { return {name:n, version:_dbs[n].version}; })); },
                cmp: function(a, b) { return a < b ? -1 : a > b ? 1 : 0; },
            };
            globalThis.IDBKeyRange = {
                only: function(val) { return {lower:val,upper:val,lowerOpen:false,upperOpen:false,includes:function(v){return v===val;}}; },
                bound: function(l,u,lo,uo) { return {lower:l,upper:u,lowerOpen:!!lo,upperOpen:!!uo,includes:function(v){return (lo?v>l:v>=l)&&(uo?v<u:v<=u);}}; },
                lowerBound: function(l,o) { return {lower:l,upper:undefined,lowerOpen:!!o,upperOpen:true,includes:function(v){return o?v>l:v>=l;}}; },
                upperBound: function(u,o) { return {lower:undefined,upper:u,lowerOpen:true,upperOpen:!!o,includes:function(v){return o?v<u:v<=u;}}; },
            };
            globalThis.IDBRequest = IDBRequest;
            globalThis.IDBDatabase = IDBDatabase;
            globalThis.IDBTransaction = IDBTransaction;
            globalThis.IDBObjectStore = IDBObjectStore;
            globalThis.IDBCursor = function IDBCursor() {};
            globalThis.IDBIndex = function IDBIndex() {};
            globalThis.IDBOpenDBRequest = IDBRequest;
            globalThis.IDBVersionChangeEvent = Event;
        })();
"#;
