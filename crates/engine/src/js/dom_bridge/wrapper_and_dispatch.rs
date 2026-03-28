/// Wrapper factory, event dispatch with capture+bubble phases, and Node constants
/// (compareDocumentPosition, DOCUMENT_POSITION_*).
pub(super) fn wrapper_and_dispatch_js() -> &'static str {
    r#"
        // Tag → constructor map for React's node.constructor.prototype lookup
        var _ctorMap = {
            INPUT: HTMLInputElement, TEXTAREA: HTMLTextAreaElement,
            SELECT: HTMLSelectElement, FORM: HTMLFormElement,
            A: HTMLAnchorElement, IMG: HTMLImageElement,
            BUTTON: HTMLButtonElement, OPTION: HTMLOptionElement,
            IFRAME: HTMLIFrameElement,
        };

        // Wrapper factory
        function __w(nodeId) {
            if (_cache[nodeId]) return _cache[nodeId];
            var obj = Object.create(EP);
            obj.__nid = nodeId;
            obj.__props = {}; // per-element property store (dirty value/checked/selected)
            // Set constructor so React's inputValueTracking can find
            // the native value descriptor via node.constructor.prototype
            var tag = __n_getTagName(nodeId);
            var ctor = _ctorMap[tag];
            if (ctor) obj.constructor = ctor;
            _cache[nodeId] = obj;
            return obj;
        }
        globalThis.__braille_get_element_wrapper = __w;

        // Collect all dirty property values from cached wrappers.
        // Returns a JSON string: [[nodeId, value], ...]
        globalThis.__braille_collect_dirty_values = function() {
            var result = [];
            for (var nid in _cache) {
                var el = _cache[nid];
                if (el.__props && el.__props._value !== undefined) {
                    result.push([parseInt(nid), String(el.__props._value)]);
                }
            }
            return JSON.stringify(result);
        };

        // Event dispatch with capture + bubble phases
        // ownerDoc: optional non-global document that owns the target element
        function __dispatch(nodeId, event, ownerDoc) {
            // Build path: target -> parent -> ... -> root
            var path = [];
            var cur = nodeId;
            while (cur >= 0) {
                path.push(cur);
                cur = __n_getParent(cur);
            }

            // Determine if we're dispatching in the global document tree or a standalone one
            var isGlobalDoc = !ownerDoc || ownerDoc === document;
            var theDoc = isGlobalDoc ? document : ownerDoc;

            event._dispatching = true;
            event.target = __w(nodeId);
            event.eventPhase = 0;

            // Build composedPath: wrapped elements + document (+ window for global)
            var composedPath = [];
            for (var pi = 0; pi < path.length; pi++) composedPath.push(__w(path[pi]));
            composedPath.push(theDoc);
            if (isGlobalDoc) composedPath.push(window);
            event._path = composedPath;

            // Helper to fire a list of callbacks
            function fireCbs(cbs, thisObj) {
                if (!cbs || !cbs.length) return;
                var snapshot = cbs.slice();
                for (var j = 0; j < snapshot.length; j++) {
                    snapshot[j].call(thisObj, event);
                    if (event._stopImmediate) return;
                }
            }

            // Run dispatch phases, then always clean up
            function runPhases() {
                // === CAPTURE PHASE (root → target) ===
                event.eventPhase = 1;

                if (isGlobalDoc) {
                    // Window capture
                    event.currentTarget = window;
                    fireCbs(_winCapture[event.type], window);
                    if (event._stopImmediate || event._stopPropagation) return;

                    // Document capture
                    event.currentTarget = document;
                    fireCbs(_docCapture[event.type], document);
                    if (event._stopImmediate || event._stopPropagation) return;
                } else {
                    // Non-global document capture
                    event.currentTarget = theDoc;
                    fireCbs(theDoc.__captureListeners && theDoc.__captureListeners[event.type], theDoc);
                    if (event._stopImmediate || event._stopPropagation) return;
                }

                // DOM elements capture: from root down to (but not including) target
                for (var i = path.length - 1; i > 0; i--) {
                    var nid = path[i];
                    event.currentTarget = __w(nid);
                    fireCbs(_captureKeys[nid + ':' + event.type], event.currentTarget);
                    if (event._stopImmediate || event._stopPropagation) return;
                }

                // === AT-TARGET PHASE ===
                event.eventPhase = 2;
                var targetNid = path[0];
                var targetEl = __w(targetNid);
                event.currentTarget = targetEl;

                // Inline event handler (e.g. onclick="...")
                var attrHandler = __n_getAttribute(targetNid, 'on' + event.type);
                if (attrHandler) {
                    (new Function('event', attrHandler)).call(targetEl, event);
                    if (event._stopImmediate) return;
                }

                // Fire both capture and bubble listeners at target (per spec)
                fireCbs(_captureKeys[targetNid + ':' + event.type], targetEl);
                if (event._stopImmediate) return;
                fireCbs(_bubbleKeys[targetNid + ':' + event.type], targetEl);
                if (event._stopImmediate) return;

                if (!event.bubbles) return;

                // === BUBBLE PHASE (target+1 → root → document → window) ===
                event.eventPhase = 3;
                for (var i = 1; i < path.length; i++) {
                    if (event._stopPropagation) break;
                    var nid = path[i];
                    event.currentTarget = __w(nid);
                    fireCbs(_bubbleKeys[nid + ':' + event.type], event.currentTarget);
                    if (event._stopImmediate) return;
                }

                if (isGlobalDoc) {
                    // Document bubble
                    if (!event._stopPropagation) {
                        event.currentTarget = document;
                        fireCbs(doc.__listeners[event.type], document);
                        if (event._stopImmediate) return;
                    }

                    // Window bubble
                    if (!event._stopPropagation) {
                        event.currentTarget = window;
                        fireCbs(_winListeners[event.type], window);
                    }
                } else {
                    // Non-global document bubble
                    if (!event._stopPropagation) {
                        event.currentTarget = theDoc;
                        fireCbs(theDoc.__listeners && theDoc.__listeners[event.type], theDoc);
                    }
                }
            }

            runPhases();

            // Per spec step 14: unset dispatching, stop propagation, and stop immediate flags
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
        }

        // __braille_click(nodeId) — called from Rust
        globalThis.__braille_click = function(nodeId) {
            var el = __w(nodeId);
            el.click();
        };

        // Fire load event on <link> elements (CSS, prefetch, etc.)
        // We don't actually load CSS, but frameworks need the onload to resolve promises.
        globalThis.__braille_maybe_load_link = function(node) {
            if (!node || node.tagName !== 'LINK') return;
            var rel = node.rel || node.getAttribute('rel') || '';
            if (rel === 'stylesheet' || rel === 'prefetch' || rel === 'preload') {
                setTimeout(function() {
                    if (typeof node.onload === 'function') {
                        node.onload({type: 'load', target: node});
                    }
                    node.dispatchEvent(new Event('load'));
                }, 0);
            }
        };

        // Dynamic script loading: fetch and eval <script src="..."> on insertion
        globalThis.__braille_script_log = [];
        globalThis.__braille_maybe_load_script = function(node) {
            if (!node || node.tagName !== 'SCRIPT') return;
            var src = node.getAttribute('src');
            if (!src) return;
            var shortSrc = src.substring(src.lastIndexOf('/') + 1).substring(0, 40);
            __braille_script_log.push('FETCH: ' + shortSrc);
            fetch(src).then(function(resp) {
                __braille_script_log.push('RESP: ' + shortSrc + ' ok=' + resp.ok + ' status=' + resp.status);
                if (!resp.ok) throw new Error('HTTP ' + resp.status);
                return resp.text();
            }).then(function(code) {
                __braille_script_log.push('EVAL: ' + shortSrc + ' len=' + code.length);
                document.currentScript = node;
                (0, eval)(code);
                document.currentScript = null;
                __braille_script_log.push('OK: ' + shortSrc);
                if (typeof node.onload === 'function') {
                    node.onload({type: 'load', target: node});
                }
                node.dispatchEvent(new Event('load'));
            }).catch(function(err) {
                document.currentScript = null;
                __braille_script_log.push('ERR: ' + shortSrc + ' -> ' + String(err).substring(0, 100));
                if (typeof node.onerror === 'function') {
                    node.onerror({type: 'error', target: node, message: String(err)});
                }
                node.dispatchEvent(new Event('error'));
            });
        };

        // Helper: throw DOMException from validation error string "ErrorName:message"
        function __throwValidationError(err) {
            var colonIdx = err.indexOf(':');
            var name = err.substring(0, colonIdx);
            var msg = err.substring(colonIdx + 1);
            throw new DOMException(msg, name);
        }

        // Element mutation methods that operate on the real DomTree
        EP.appendChild = function(child) {
            if (child === null || child === undefined || (typeof child === 'object' && child.__nid === undefined)) {
                throw new TypeError("Failed to execute 'appendChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (this.__nid === undefined) return child;
            if (child && child.__nid !== undefined) {
                var err = __n_validatePreInsert(this.__nid, child.__nid, -1);
                if (err) __throwValidationError(err);
                if (child.nodeType === 11) {
                    var kids = __n_getAllChildIds(child.__nid);
                    var added = [];
                    for (var i = 0; i < kids.length; i++) {
                        __n_appendChild(this.__nid, kids[i]);
                        added.push(__w(kids[i]));
                    }
                    if (typeof __mo_notify === 'function' && added.length) __mo_notify('childList', this, {addedNodes: added});
                } else {
                    __n_appendChild(this.__nid, child.__nid);
                    if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [child]});
                }
            }
            __braille_maybe_load_script(child);
            __braille_maybe_load_link(child);
            return child;
        };
        EP.removeChild = function(child) {
            if (child === null || child === undefined || (typeof child === 'object' && child.__nid === undefined)) {
                throw new TypeError("Failed to execute 'removeChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (child && child.__nid !== undefined && this.__nid !== undefined) {
                if (__n_getParent(child.__nid) !== this.__nid) {
                    throw new DOMException("The node to be removed is not a child of this node.", "NotFoundError");
                }
                __n_removeChild(this.__nid, child.__nid);
                if (typeof __mo_notify === 'function') __mo_notify('childList', this, {removedNodes: [child]});
            }
            return child;
        };
        EP.insertBefore = function(newChild, refChild) {
            if (newChild === null || newChild === undefined || (typeof newChild === 'object' && newChild.__nid === undefined)) {
                throw new TypeError("Failed to execute 'insertBefore' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (arguments.length < 2) {
                throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required, but only 1 present.");
            }
            if (refChild !== null && refChild !== undefined && (typeof refChild !== 'object' || refChild.__nid === undefined)) {
                throw new TypeError("Failed to execute 'insertBefore' on 'Node': parameter 2 is not of type 'Node'.");
            }
            if (this.__nid === undefined) return newChild;
            if (newChild && newChild.__nid !== undefined) {
                var refId = (refChild && refChild.__nid !== undefined) ? refChild.__nid : -1;
                var err = __n_validatePreInsert(this.__nid, newChild.__nid, refId);
                if (err) __throwValidationError(err);
                if (newChild.nodeType === 11) {
                    var kids = __n_getAllChildIds(newChild.__nid);
                    var added = [];
                    for (var i = 0; i < kids.length; i++) {
                        __n_insertBefore(this.__nid, kids[i], refId);
                        added.push(__w(kids[i]));
                    }
                    if (typeof __mo_notify === 'function' && added.length) __mo_notify('childList', this, {addedNodes: added});
                } else {
                    if (refId >= 0 && newChild.__nid === refId) {
                        return newChild;
                    }
                    __n_insertBefore(this.__nid, newChild.__nid, refId);
                    if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [newChild]});
                }
            }
            __braille_maybe_load_script(newChild);
            __braille_maybe_load_link(newChild);
            return newChild;
        };

        // Fullscreen tracking
        var __fullscreenElement = null;
        EP.requestFullscreen = function() { __fullscreenElement = this; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };

        // Helper: create a standalone document-like wrapper around a root element.
        // Used by createHTMLDocument() and document.cloneNode().
        function __makeDocumentLike(rootEl) {
            var newDoc = {
                nodeType: 9, nodeName: '#document', readyState: 'complete',
                __listeners: {}, __captureListeners: {},
                get documentElement() { return rootEl; },
                get body() {
                    var kids = rootEl.childNodes;
                    for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'BODY') return kids[i];
                    return null;
                },
                get head() {
                    var kids = rootEl.childNodes;
                    for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'HEAD') return kids[i];
                    return null;
                },
                querySelector: function(sel) { return rootEl.querySelector(sel); },
                querySelectorAll: function(sel) { return rootEl.querySelectorAll(sel); },
                getElementById: function(id) { return rootEl.querySelector('#' + id) || null; },
                getElementsByTagName: function(tag) { return rootEl.querySelectorAll(tag); },
                getElementsByClassName: function(cls) { return rootEl.querySelectorAll('.' + cls); },
                createElement: function(tag) { return document.createElement(tag); },
                createTextNode: function(text) { return document.createTextNode(text); },
                createDocumentFragment: function() { return document.createDocumentFragment(); },
                createEvent: function(type) { var e = new Event(''); e._initialized = false; e.type = ''; return e; },
                appendChild: function(child) { return rootEl.appendChild(child); },
                addEventListener: function(type, cb, opts) {
                    if (typeof cb !== 'function') return;
                    var capture = !!(opts === true || (opts && opts.capture));
                    var store = capture ? newDoc.__captureListeners : newDoc.__listeners;
                    if (!store[type]) store[type] = [];
                    store[type].push(cb);
                },
                removeEventListener: function(type, cb, opts) {
                    var capture = !!(opts === true || (opts && opts.capture));
                    var store = capture ? newDoc.__captureListeners : newDoc.__listeners;
                    if (store[type]) store[type] = store[type].filter(function(f){return f!==cb;});
                },
                dispatchEvent: function(event) {
                    event._dispatching = true;
                    event.target = newDoc;
                    event.currentTarget = newDoc;
                    var cbs = newDoc.__listeners[event.type];
                    if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(newDoc, event); }
                    event._dispatching = false;
                    event._stopPropagation = false;
                    event._stopImmediate = false;
                    event.currentTarget = null;
                    event.eventPhase = 0;
                    return !event.defaultPrevented;
                },
            };
            // Tag the root element so EP.dispatchEvent can find the owning document
            rootEl.__ownerDoc = newDoc;
            return newDoc;
        }

        // Override document methods
        var doc = globalThis.document;
        doc.__listeners = {};
        doc.parentNode = null;
        doc.parentElement = null;
        doc.getElementById = function(id) {
            var nid = __n_getElementById(String(id));
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelector = function(sel) {
            var nid = __n_querySelector(0, sel);
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelectorAll = function(sel) {
            return __n_querySelectorAll(0, sel).map(__w);
        };
        doc.createElement = function(tag) {
            var nid = __n_createElement(tag);
            return __w(nid);
        };
        doc.createElementNS = function(ns, tag) {
            var nid = __n_createElement(tag);
            var el = __w(nid);
            el.namespaceURI = ns;
            return el;
        };
        doc.createTextNode = function(text) {
            var nid = __n_createTextNode(text);
            var node = __w(nid);
            return node;
        };
        doc.createComment = function(text) { return { nodeType: 8, textContent: text }; };
        doc.createDocumentFragment = function() {
            var nid = __n_createDocFragment();
            return __w(nid);
        };
        doc.getElementsByTagName = function(tag) {
            return new Proxy([], {
                get: function(t, p) {
                    var live = doc.querySelectorAll(tag);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === 'namedItem') return function(name) {
                        for (var i = 0; i < live.length; i++) {
                            if (live[i].getAttribute('name') === name || live[i].getAttribute('id') === name) return live[i];
                        }
                        return null;
                    };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        doc.getElementsByClassName = function(cls) {
            return new Proxy([], {
                get: function(t, p) {
                    var live = doc.querySelectorAll('.' + cls);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        doc.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var store = capture ? _docCapture : doc.__listeners;
            if (!store[type]) store[type] = [];
            if (once) {
                var wrapper = function(e) { cb.call(document, e); doc.removeEventListener(type, wrapper, capture); };
                wrapper._origCb = cb;
                store[type].push(wrapper);
            } else {
                store[type].push(cb);
            }
        };
        doc.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var store = capture ? _docCapture : doc.__listeners;
            if (store[type]) store[type] = store[type].filter(function(f){return f!==cb && f._origCb!==cb;});
        };

        doc.createComment = function(text) {
            var nid = __n_createComment(text || '');
            return __w(nid);
        };

        function BrailleRange() {
            this.startContainer = null; this.startOffset = 0;
            this.endContainer = null; this.endOffset = 0;
            this.collapsed = true; this.commonAncestorContainer = null;
        }
        BrailleRange.START_TO_START = 0; BrailleRange.START_TO_END = 1;
        BrailleRange.END_TO_END = 2; BrailleRange.END_TO_START = 3;
        BrailleRange.prototype.setStart = function(node, offset) { this.startContainer = node; this.startOffset = offset; this._update(); };
        BrailleRange.prototype.setEnd = function(node, offset) { this.endContainer = node; this.endOffset = offset; this._update(); };
        BrailleRange.prototype.setStartBefore = function(node) { this.startContainer = node.parentNode; this.startOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) : 0; this._update(); };
        BrailleRange.prototype.setStartAfter = function(node) { this.startContainer = node.parentNode; this.startOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) + 1 : 0; this._update(); };
        BrailleRange.prototype.setEndBefore = function(node) { this.endContainer = node.parentNode; this.endOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) : 0; this._update(); };
        BrailleRange.prototype.setEndAfter = function(node) { this.endContainer = node.parentNode; this.endOffset = node.parentNode ? Array.prototype.indexOf.call(node.parentNode.childNodes, node) + 1 : 0; this._update(); };
        BrailleRange.prototype.selectNode = function(node) { this.setStartBefore(node); this.setEndAfter(node); };
        BrailleRange.prototype.selectNodeContents = function(node) { this.startContainer = node; this.startOffset = 0; this.endContainer = node; this.endOffset = node.childNodes ? node.childNodes.length : 0; this._update(); };
        BrailleRange.prototype.collapse = function(toStart) { if (toStart || toStart === undefined) { this.endContainer = this.startContainer; this.endOffset = this.startOffset; } else { this.startContainer = this.endContainer; this.startOffset = this.endOffset; } this.collapsed = true; };
        BrailleRange.prototype.cloneRange = function() { var r = new BrailleRange(); r.startContainer = this.startContainer; r.startOffset = this.startOffset; r.endContainer = this.endContainer; r.endOffset = this.endOffset; r._update(); return r; };
        BrailleRange.prototype.detach = function() {};
        BrailleRange.prototype.getBoundingClientRect = function() {
            var el = this.startContainer;
            if (el && el.nodeType === 3) el = el.parentNode;
            return el && el.getBoundingClientRect ? el.getBoundingClientRect() : {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
        };
        BrailleRange.prototype.getClientRects = function() { return [this.getBoundingClientRect()]; };
        BrailleRange.prototype.toString = function() {
            if (this.startContainer && this.endContainer && this.startContainer === this.endContainer && this.startContainer.nodeType === 3) {
                return (this.startContainer.textContent || '').substring(this.startOffset, this.endOffset);
            }
            return this.startContainer ? (this.startContainer.textContent || '') : '';
        };
        BrailleRange.prototype.createContextualFragment = function(html) {
            var temp = document.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            var frag = document.createDocumentFragment();
            while (temp.firstChild) frag.appendChild(temp.firstChild);
            return frag;
        };
        BrailleRange.prototype._update = function() {
            this.collapsed = (this.startContainer === this.endContainer && this.startOffset === this.endOffset);
            // Walk ancestors of startContainer and endContainer to find common ancestor
            if (this.startContainer && this.endContainer) {
                var ancestors = [];
                var cur = this.startContainer;
                while (cur) { ancestors.push(cur); cur = cur.parentNode; }
                cur = this.endContainer;
                while (cur) { if (ancestors.indexOf(cur) >= 0) { this.commonAncestorContainer = cur; return; } cur = cur.parentNode; }
            }
            this.commonAncestorContainer = null;
        };
        globalThis.Range = BrailleRange;
        doc.createRange = function() { return new BrailleRange(); };

        // window.addEventListener / removeEventListener
        window.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var store = capture ? _winCapture : _winListeners;
            if (!store[type]) store[type] = [];
            if (once) {
                var wrapper = function(e) { cb.call(window, e); window.removeEventListener(type, wrapper, capture); };
                wrapper._origCb = cb;
                store[type].push(wrapper);
            } else {
                store[type].push(cb);
            }
        };
        window.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var store = capture ? _winCapture : _winListeners;
            if (store[type]) {
                store[type] = store[type].filter(function(f){return f!==cb && f._origCb!==cb;});
            }
        };

        doc.dispatchEvent = function(event) {
            event._dispatching = true;
            event.target = document;
            event.currentTarget = document;
            var cbs = doc.__listeners[event.type];
            if (cbs) {
                var snapshot = cbs.slice();
                for (var i = 0; i < snapshot.length; i++) snapshot[i].call(document, event);
            }
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            return !event.defaultPrevented;
        };
        doc.createEvent = function(type) { var e = new Event(''); e._initialized = false; e.type = ''; return e; };
        doc.createTreeWalker = function(root, whatToShow, filter) {
            // Minimal TreeWalker: pre-order traversal of element nodes
            var current = root;
            return {
                currentNode: root,
                nextNode: function() {
                    // depth-first walk
                    if (current.firstChild) { current = current.firstChild; this.currentNode = current; return current; }
                    while (current) {
                        if (current.nextSibling) { current = current.nextSibling; this.currentNode = current; return current; }
                        current = current.parentNode;
                        if (current === root) { current = null; this.currentNode = null; return null; }
                    }
                    return null;
                },
                previousNode: function() { return null; },
                firstChild: function() { var c = current.firstChild; if (c) { current = c; this.currentNode = c; } return c; },
                lastChild: function() { var c = current.lastChild; if (c) { current = c; this.currentNode = c; } return c; },
                nextSibling: function() { var s = current.nextSibling; if (s) { current = s; this.currentNode = s; } return s; },
                previousSibling: function() { var s = current.previousSibling; if (s) { current = s; this.currentNode = s; } return s; },
                parentNode: function() { var p = current.parentNode; if (p && p !== root) { current = p; this.currentNode = p; return p; } return null; },
            };
        };
        doc.createNodeIterator = function(root) { return doc.createTreeWalker(root); };
        doc.importNode = function(node, deep) {
            if (!node) return node;
            if (node.__nid !== undefined) return node.cloneNode(!!deep);
            return node;
        };
        doc.adoptNode = function(node) { return node; };
        doc.cloneNode = function(deep) {
            var docEl = doc.documentElement;
            if (!docEl) return __makeDocumentLike(document.createElement('html'));
            var cloned = docEl.cloneNode(!!deep);
            return __makeDocumentLike(cloned);
        };
        doc.exitFullscreen = function() { __fullscreenElement = null; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };
        doc.getAnimations = function() { return []; };

        window.dispatchEvent = function(event) {
            event._dispatching = true;
            event.target = window;
            event.currentTarget = window;
            var cbs = _winListeners[event.type];
            if (cbs) {
                var snapshot = cbs.slice();
                for (var i = 0; i < snapshot.length; i++) snapshot[i].call(window, event);
            }
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            return !event.defaultPrevented;
        };

        // Track focused element for document.activeElement
        var __focusedElement = null;
        EP.focus = function() { __focusedElement = this; };
        EP.blur = function() { if (__focusedElement === this) __focusedElement = null; };

        // document.cookie implementation (JS-side cookie jar)
        var _cookieJar = {};
        Object.defineProperties(doc, {
            body: { get: function() { return doc.querySelector('body'); }, configurable: true },
            head: { get: function() { return doc.querySelector('head'); }, configurable: true },
            documentElement: { get: function() { return doc.querySelector('html'); }, configurable: true },
            activeElement: { get: function() { return __focusedElement || doc.querySelector('body'); }, configurable: true },
            cookie: {
                get: function() {
                    var now = Date.now();
                    var parts = [];
                    for (var name in _cookieJar) {
                        var c = _cookieJar[name];
                        if (c.expires && c.expires <= now) { delete _cookieJar[name]; continue; }
                        parts.push(name + '=' + c.value);
                    }
                    return parts.join('; ');
                },
                set: function(s) {
                    if (typeof s !== 'string') return;
                    var parts = s.split(';');
                    var nv = parts[0].trim().split('=');
                    if (nv.length < 2) return;
                    var name = nv[0].trim();
                    var value = nv.slice(1).join('=').trim();
                    var expires = null;
                    for (var i = 1; i < parts.length; i++) {
                        var p = parts[i].trim().toLowerCase();
                        if (p.indexOf('expires=') === 0) {
                            expires = Date.parse(parts[i].trim().substring(8));
                        } else if (p.indexOf('max-age=') === 0) {
                            var sec = parseInt(parts[i].trim().substring(8));
                            if (!isNaN(sec)) expires = Date.now() + sec * 1000;
                        }
                    }
                    if (expires !== null && expires < Date.now()) {
                        delete _cookieJar[name];
                    } else {
                        _cookieJar[name] = { value: value, expires: expires };
                    }
                },
                configurable: true
            },
            title: {
                get: function() {
                    var t = doc.querySelector('title');
                    return t ? t.textContent : '';
                },
                set: function(v) {
                    var t = doc.querySelector('title');
                    if (t) t.textContent = String(v);
                },
                configurable: true
            },
            currentScript: { value: null, writable: true, configurable: true },
            doctype: {
                get: function() {
                    var json = __n_getDoctypeInfo();
                    if (!json) return null;
                    var info = JSON.parse(json);
                    return { name: info.name, publicId: info.publicId, systemId: info.systemId, nodeType: 10, nodeName: info.name };
                },
                configurable: true
            },
            domain: {
                get: function() { return doc.__domain || location.hostname; },
                set: function(v) {
                    var cur = location.hostname;
                    if (cur === v || cur.endsWith('.' + v)) doc.__domain = v;
                },
                configurable: true
            },
            fullscreenElement: { get: function() { return __fullscreenElement; }, configurable: true },
            fullscreenEnabled: { value: true, configurable: true },
            referrer: { value: '', writable: true, configurable: true },
            characterSet: { value: 'UTF-8', configurable: true },
            contentType: { value: 'text/html', configurable: true },
            hidden: { value: false, configurable: true },
            visibilityState: { value: 'visible', configurable: true },
            forms: { get: function() {
                return new Proxy([], {
                    get: function(t, p) {
                        var live = doc.querySelectorAll('form');
                        if (p === 'length') return live.length;
                        if (p === 'item') return function(i) { return live[i] || null; };
                        if (p === 'namedItem') return function(name) {
                            for (var i = 0; i < live.length; i++) {
                                if (live[i].getAttribute('name') === name || live[i].getAttribute('id') === name) return live[i];
                            }
                            return null;
                        };
                        if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                        if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                        if (typeof p === 'string') {
                            for (var i = 0; i < live.length; i++) {
                                if (live[i].getAttribute('id') === p || live[i].getAttribute('name') === p) return live[i];
                            }
                        }
                        if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                        return live[p];
                    }
                });
            }, configurable: true },
            implementation: { value: {
                createHTMLDocument: function(title) {
                    var htmlEl = document.createElement('html');
                    var headEl = document.createElement('head');
                    var bodyEl = document.createElement('body');
                    htmlEl.appendChild(headEl);
                    htmlEl.appendChild(bodyEl);
                    if (title !== undefined) {
                        var titleEl = document.createElement('title');
                        titleEl.textContent = String(title);
                        headEl.appendChild(titleEl);
                    }
                    var newDoc = __makeDocumentLike(htmlEl);
                    newDoc.title = title !== undefined ? String(title) : '';
                    return newDoc;
                },
                hasFeature: function() { return true; },
            }, configurable: true },
        });
        // Node constructor with constants (used by React, etc.)
        var Node = function Node() {};
        Node.prototype = EP;
        // nodeType constants
        Node.ELEMENT_NODE = 1;
        Node.ATTRIBUTE_NODE = 2;
        Node.TEXT_NODE = 3;
        Node.CDATA_SECTION_NODE = 4;
        Node.ENTITY_REFERENCE_NODE = 5;
        Node.ENTITY_NODE = 6;
        Node.PROCESSING_INSTRUCTION_NODE = 7;
        Node.COMMENT_NODE = 8;
        Node.DOCUMENT_NODE = 9;
        Node.DOCUMENT_TYPE_NODE = 10;
        Node.DOCUMENT_FRAGMENT_NODE = 11;
        Node.NOTATION_NODE = 12;
        // document position constants
        Node.DOCUMENT_POSITION_DISCONNECTED = 1;
        Node.DOCUMENT_POSITION_PRECEDING = 2;
        Node.DOCUMENT_POSITION_FOLLOWING = 4;
        Node.DOCUMENT_POSITION_CONTAINS = 8;
        Node.DOCUMENT_POSITION_CONTAINED_BY = 16;
        Node.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = 32;
        // Constants must also be on the prototype so instances inherit them
        EP.ELEMENT_NODE = 1;
        EP.ATTRIBUTE_NODE = 2;
        EP.TEXT_NODE = 3;
        EP.CDATA_SECTION_NODE = 4;
        EP.ENTITY_REFERENCE_NODE = 5;
        EP.ENTITY_NODE = 6;
        EP.PROCESSING_INSTRUCTION_NODE = 7;
        EP.COMMENT_NODE = 8;
        EP.DOCUMENT_NODE = 9;
        EP.DOCUMENT_TYPE_NODE = 10;
        EP.DOCUMENT_FRAGMENT_NODE = 11;
        EP.NOTATION_NODE = 12;
        EP.DOCUMENT_POSITION_DISCONNECTED = 1;
        EP.DOCUMENT_POSITION_PRECEDING = 2;
        EP.DOCUMENT_POSITION_FOLLOWING = 4;
        EP.DOCUMENT_POSITION_CONTAINS = 8;
        EP.DOCUMENT_POSITION_CONTAINED_BY = 16;
        EP.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = 32;
        globalThis.Node = Node;

        // Document constructor — creates a standalone XML document (initially empty)
        globalThis.Document = function Document() {
            var rootEl = null;
            var newDoc = {
                nodeType: 9, nodeName: '#document', readyState: 'complete',
                __listeners: {}, __captureListeners: {},
                get documentElement() { return rootEl; },
                get body() {
                    if (!rootEl) return null;
                    var kids = rootEl.childNodes;
                    for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'BODY') return kids[i];
                    return null;
                },
                get head() {
                    if (!rootEl) return null;
                    var kids = rootEl.childNodes;
                    for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'HEAD') return kids[i];
                    return null;
                },
                querySelector: function(sel) { return rootEl ? rootEl.querySelector(sel) : null; },
                querySelectorAll: function(sel) { return rootEl ? rootEl.querySelectorAll(sel) : []; },
                getElementById: function(id) { return rootEl ? rootEl.querySelector('#' + id) || null : null; },
                getElementsByTagName: function(tag) { return rootEl ? rootEl.querySelectorAll(tag) : []; },
                getElementsByClassName: function(cls) { return rootEl ? rootEl.querySelectorAll('.' + cls) : []; },
                createElement: function(tag) { return document.createElement(tag); },
                createTextNode: function(text) { return document.createTextNode(text); },
                createDocumentFragment: function() { return document.createDocumentFragment(); },
                createEvent: function(type) { var e = new Event(''); e._initialized = false; e.type = ''; return e; },
                appendChild: function(child) {
                    if (!rootEl && child && child.__nid !== undefined) {
                        rootEl = child;
                        rootEl.__ownerDoc = newDoc;
                    } else if (rootEl) {
                        rootEl.appendChild(child);
                    }
                    return child;
                },
                addEventListener: function(type, cb, opts) {
                    if (typeof cb !== 'function') return;
                    var capture = !!(opts === true || (opts && opts.capture));
                    var store = capture ? newDoc.__captureListeners : newDoc.__listeners;
                    if (!store[type]) store[type] = [];
                    store[type].push(cb);
                },
                removeEventListener: function(type, cb, opts) {
                    var capture = !!(opts === true || (opts && opts.capture));
                    var store = capture ? newDoc.__captureListeners : newDoc.__listeners;
                    if (store[type]) store[type] = store[type].filter(function(f){return f!==cb;});
                },
                dispatchEvent: function(event) {
                    event._dispatching = true;
                    event.target = newDoc;
                    event.currentTarget = newDoc;
                    var cbs = newDoc.__listeners[event.type];
                    if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(newDoc, event); }
                    event._dispatching = false;
                    event._stopPropagation = false;
                    event._stopImmediate = false;
                    event.currentTarget = null;
                    event.eventPhase = 0;
                    return !event.defaultPrevented;
                },
            };
            return newDoc;
        };

        // EventTarget constructor — standalone event targets (not backed by DOM nodes)
        function EventTarget() {
            this.__et_listeners = {};
        }
        EventTarget.prototype.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function' && !(cb && typeof cb.handleEvent === 'function')) return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var key = type + (capture ? '_c' : '_b');
            if (!this.__et_listeners[key]) this.__et_listeners[key] = [];
            for (var i = 0; i < this.__et_listeners[key].length; i++) {
                if (this.__et_listeners[key][i] === cb || this.__et_listeners[key][i]._origCb === cb) return;
            }
            if (once) {
                var self = this;
                var wrapper = function(e) {
                    if (typeof cb === 'function') cb.call(self, e);
                    else cb.handleEvent(e);
                    self.removeEventListener(type, cb, capture);
                };
                wrapper._origCb = cb;
                this.__et_listeners[key].push(wrapper);
            } else {
                this.__et_listeners[key].push(cb);
            }
        };
        EventTarget.prototype.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var key = type + (capture ? '_c' : '_b');
            if (this.__et_listeners[key]) {
                this.__et_listeners[key] = this.__et_listeners[key].filter(function(f) { return f !== cb && f._origCb !== cb; });
            }
        };
        EventTarget.prototype.dispatchEvent = function(event) {
            event._dispatching = true;
            event.target = this;
            event.currentTarget = this;
            event._path = [this];
            event.eventPhase = 2;
            var key = event.type + '_b';
            var cbs = this.__et_listeners[key];
            if (cbs) {
                var snapshot = cbs.slice();
                for (var i = 0; i < snapshot.length; i++) {
                    var fn = snapshot[i];
                    if (typeof fn === 'function') fn.call(this, event);
                    else if (fn && typeof fn.handleEvent === 'function') fn.handleEvent(event);
                    if (event._stopImmediate) break;
                }
            }
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            return !event.defaultPrevented;
        };
        globalThis.EventTarget = EventTarget;

        // CharacterData prototype — between Node.prototype and Text/Comment
        var CharacterData = function CharacterData() {};
        CharacterData.prototype = Object.create(EP);
        CharacterData.prototype.constructor = CharacterData;
        globalThis.CharacterData = CharacterData;

        // Text constructor — creates a real text node in the DomTree
        function Text(data) {
            var str = arguments.length === 0 ? '' : String(data === undefined ? '' : data);
            var nid = __n_createTextNode(str);
            var obj = __w(nid);
            Object.setPrototypeOf(obj, Text.prototype);
            return obj;
        }
        Text.prototype = Object.create(CharacterData.prototype);
        Text.prototype.constructor = Text;
        Object.defineProperty(Text.prototype, 'wholeText', {
            get: function() { return this.data; },
            configurable: true
        });
        Text.prototype.splitText = function(offset) {
            var d = this.data;
            if (offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            var newData = d.substring(offset);
            this.data = d.substring(0, offset);
            var newNode = new Text(newData);
            if (this.parentNode) {
                this.parentNode.insertBefore(newNode, this.nextSibling);
            }
            return newNode;
        };
        globalThis.Text = Text;

        // Comment constructor — creates a real comment node in the DomTree
        function Comment(data) {
            var str = arguments.length === 0 ? '' : String(data === undefined ? '' : data);
            var nid = __n_createComment(str);
            var obj = __w(nid);
            Object.setPrototypeOf(obj, Comment.prototype);
            return obj;
        }
        Comment.prototype = Object.create(CharacterData.prototype);
        Comment.prototype.constructor = Comment;
        globalThis.Comment = Comment;

        // Document constructor
        function Document() {}
        Document.prototype = Object.create(EP);
        Document.prototype.constructor = Document;
        globalThis.Document = Document;
    "#
}
