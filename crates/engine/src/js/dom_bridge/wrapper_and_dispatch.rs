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
            var nt = __n_getNodeType(nodeId);
            var proto;
            switch (nt) {
                case 1:  proto = __ElemProto; break;
                case 3:  proto = Text.prototype; break;
                case 7:  proto = ProcessingInstruction.prototype; break;
                case 8:  proto = Comment.prototype; break;
                case 9:  proto = Document.prototype; break;
                case 10: proto = DocumentType.prototype; break;
                case 11: proto = DocumentFragment.prototype; break;
                default: proto = EP; break;
            }
            var obj = Object.create(proto);
            obj.__nid = nodeId;
            obj.__props = {};
            if (nt === 1) {
                var tag = __n_getTagName(nodeId);
                var ctor = _ctorMap[tag];
                if (ctor) obj.constructor = ctor;
            }
            _cache[nodeId] = obj;
            return obj;
        }
        globalThis.__braille_get_element_wrapper = __w;
        globalThis.__braille_reset_dom_cache = function() {
            for (var k in _cache) delete _cache[k];
            for (var k in _listeners) delete _listeners[k];
            for (var k in _captureKeys) delete _captureKeys[k];
            for (var k in _bubbleKeys) delete _bubbleKeys[k];
            for (var k in _winListeners) delete _winListeners[k];
            for (var k in _winCapture) delete _winCapture[k];
            for (var k in _docCapture) delete _docCapture[k];
        };

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
            // Fire __et_listeners on an element (for listeners added via EventTarget.prototype)
            function fireEt(obj, suffix) {
                if (obj && obj.__et_listeners) {
                    fireCbs(obj.__et_listeners[event.type + suffix], obj);
                }
            }

            // Run dispatch phases, then always clean up
            function runPhases() {
                // === CAPTURE PHASE (root → target) ===
                event.eventPhase = 1;

                if (isGlobalDoc) {
                    // Window capture
                    event.currentTarget = window;
                    fireCbs(window.__et_listeners[event.type + '_c'], window);
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
                    var el = __w(nid);
                    event.currentTarget = el;
                    fireCbs(_captureKeys[nid + ':' + event.type], el);
                    if (event._stopImmediate || event._stopPropagation) return;
                    fireEt(el, '_c');
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
                fireEt(targetEl, '_c');
                if (event._stopImmediate) return;
                fireCbs(_bubbleKeys[targetNid + ':' + event.type], targetEl);
                if (event._stopImmediate) return;
                fireEt(targetEl, '_b');
                if (event._stopImmediate) return;

                if (!event.bubbles) return;

                // === BUBBLE PHASE (target+1 → root → document → window) ===
                event.eventPhase = 3;
                for (var i = 1; i < path.length; i++) {
                    if (event._stopPropagation) break;
                    var nid = path[i];
                    var el = __w(nid);
                    event.currentTarget = el;
                    fireCbs(_bubbleKeys[nid + ':' + event.type], el);
                    if (event._stopImmediate) return;
                    fireEt(el, '_b');
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
                        fireCbs(window.__et_listeners[event.type + '_b'], window);
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
            if (child !== null && child !== undefined && typeof child === 'object' && child.nodeType === 2) {
                throw new DOMException("The new child element contains the parent.", "HierarchyRequestError");
            }
            if (child === null || child === undefined || (typeof child === 'object' && child.__nid === undefined && child.nodeType === undefined)) {
                throw new TypeError("Failed to execute 'appendChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            // CharacterData nodes (Text=3, PI=7, Comment=8) cannot have children
            var pnt = this.nodeType;
            if (pnt === 3 || pnt === 7 || pnt === 8) {
                throw new DOMException("CharacterData type " + this.nodeName + " must not have children", "HierarchyRequestError");
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
            if (newChild !== null && newChild !== undefined && typeof newChild === 'object' && newChild.nodeType === 2) {
                throw new DOMException("The new child element contains the parent.", "HierarchyRequestError");
            }
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
        // Used by createHTMLDocument(), createDocument(), and document.cloneNode().
        // Returns a proper Document node (inherits from Document.prototype → EP → Node constants).
        function __makeDocumentLike(rootEl) {
            // Document.prototype is defined later via function hoisting
            var newDoc = Object.create(Document.prototype);
            // Override getter-based properties from EP with own data properties
            var ownProps = {
                nodeType: 9, nodeName: '#document', readyState: 'complete',
                parentNode: null, parentElement: null,
                childNodes: rootEl ? [rootEl] : [],
                firstChild: rootEl || null, lastChild: rootEl || null,
                previousSibling: null, nextSibling: null,
                ownerDocument: null, isConnected: false,
                title: '', contentType: 'application/xml',
                URL: 'about:blank', documentURI: 'about:blank',
                compatMode: 'CSS1Compat', characterSet: 'UTF-8',
                charset: 'UTF-8', inputEncoding: 'UTF-8'
            };
            for (var k in ownProps) Object.defineProperty(newDoc, k, { value: ownProps[k], writable: true, enumerable: true, configurable: true });
            newDoc.__listeners = {};
            newDoc.__captureListeners = {};
            Object.defineProperty(newDoc, 'documentElement', { get: function() { return rootEl; }, configurable: true });
            Object.defineProperty(newDoc, 'body', { get: function() {
                if (!rootEl) return null;
                var kids = rootEl.childNodes;
                for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'BODY') return kids[i];
                return null;
            }, configurable: true });
            Object.defineProperty(newDoc, 'head', { get: function() {
                if (!rootEl) return null;
                var kids = rootEl.childNodes;
                for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'HEAD') return kids[i];
                return null;
            }, configurable: true });
            Object.defineProperty(newDoc, 'implementation', { get: function() { return document.implementation; }, configurable: true });
            Object.defineProperty(newDoc, 'doctype', { get: function() { return null; }, configurable: true });
            newDoc.querySelector = function(sel) { return rootEl ? rootEl.querySelector(sel) : null; };
            newDoc.querySelectorAll = function(sel) { return rootEl ? rootEl.querySelectorAll(sel) : []; };
            newDoc.getElementById = function(id) { return rootEl ? (rootEl.querySelector('#' + id) || null) : null; };
            newDoc.getElementsByTagName = function(tag) { return rootEl ? rootEl.querySelectorAll(tag) : []; };
            newDoc.getElementsByClassName = function(cls) { return rootEl ? rootEl.querySelectorAll('.' + cls) : []; };
            newDoc.createElement = function(tag) { return document.createElement(tag); };
            newDoc.createElementNS = function(ns, tag) { return document.createElementNS(ns, tag); };
            newDoc.createTextNode = function(text) { return document.createTextNode(text); };
            newDoc.createComment = function(text) { return document.createComment(text); };
            newDoc.createDocumentFragment = function() { return document.createDocumentFragment(); };
            newDoc.createProcessingInstruction = function(t, d) { return document.createProcessingInstruction(t, d); };
            newDoc.createAttribute = function(n) { return document.createAttribute(n); };
            newDoc.createAttributeNS = function(ns, qn) { return document.createAttributeNS(ns, qn); };
            newDoc.createEvent = function(type) { var e = new Event(''); e._initialized = false; e.type = ''; return e; };
            newDoc.appendChild = function(child) { if (rootEl) return rootEl.appendChild(child); return child; };
            newDoc.addEventListener = function(type, cb, opts) {
                if (typeof cb !== 'function') return;
                var capture = !!(opts === true || (opts && opts.capture));
                var store = capture ? newDoc.__captureListeners : newDoc.__listeners;
                if (!store[type]) store[type] = [];
                store[type].push(cb);
            };
            newDoc.removeEventListener = function(type, cb, opts) {
                var capture = !!(opts === true || (opts && opts.capture));
                var store = capture ? newDoc.__captureListeners : newDoc.__listeners;
                if (store[type]) store[type] = store[type].filter(function(f){return f!==cb;});
            };
            newDoc.dispatchEvent = function(event) {
                if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
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
            };
            // Tag the root element so EP.dispatchEvent can find the owning document
            if (rootEl) rootEl.__ownerDoc = newDoc;
            return newDoc;
        }

        // Override document methods
        var doc = globalThis.document;
        doc.__listeners = {};
        doc.parentNode = null;
        doc.parentElement = null;
        doc.title = '';
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
                var wrapper = function(e) { doc.removeEventListener(type, wrapper, capture); cb.call(document, e); };
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

        doc.createAttribute = function(localName) {
            if (arguments.length === 0) throw new TypeError("Failed to execute 'createAttribute' on 'Document': 1 argument required, but only 0 present.");
            var name = String(localName).toLowerCase();
            return new Attr(name);
        };

        doc.createAttributeNS = function(ns, qualifiedName) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'createAttributeNS' on 'Document': 2 arguments required.");
            var prefix = null;
            var localName = String(qualifiedName);
            var idx = localName.indexOf(':');
            if (idx >= 0) { prefix = localName.substring(0, idx); localName = localName.substring(idx + 1); }
            var attr = new Attr(qualifiedName, '', ns === null ? null : String(ns), prefix);
            attr.localName = localName;
            return attr;
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

        // window.__et_listeners initialized here; methods assigned after EventTarget is defined (below)
        window.__et_listeners = {};

        doc.dispatchEvent = function(event) {
            if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
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

        doc.createProcessingInstruction = function(target, data) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'createProcessingInstruction' on 'Document': 2 arguments required.");
            var nid = __n_createPI(String(target), String(data));
            return __w(nid);
        };

        // window.dispatchEvent assigned after EventTarget is defined (below)

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
                createDocument: function(ns, qualifiedName, doctype) {
                    var rootEl = null;
                    if (qualifiedName) {
                        rootEl = document.createElementNS(ns, qualifiedName);
                    }
                    var newDoc = rootEl ? __makeDocumentLike(rootEl) : __makeDocumentLike(null);
                    newDoc.contentType = ns === 'http://www.w3.org/1999/xhtml' ? 'application/xhtml+xml' : 'application/xml';
                    return newDoc;
                },
                createDocumentType: function(qualifiedName, publicId, systemId) {
                    // DocumentType is defined later in this IIFE but is available
                    // via closure by the time any user code calls this function.
                    var dt = Object.create(DocumentType.prototype);
                    var props = {
                        nodeType: 10, nodeName: String(qualifiedName),
                        name: String(qualifiedName),
                        publicId: String(publicId || ''),
                        systemId: String(systemId || ''),
                        parentNode: null, parentElement: null,
                        childNodes: [], firstChild: null, lastChild: null,
                        previousSibling: null, nextSibling: null,
                        ownerDocument: null
                    };
                    for (var k in props) Object.defineProperty(dt, k, { value: props[k], writable: true, enumerable: true, configurable: true });
                    return dt;
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
                    if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
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
            var self = (this == null) ? window : this;
            if (!self.__et_listeners) self.__et_listeners = {};
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var signal = (opts && typeof opts === 'object') ? opts.signal : undefined;
            if (signal !== undefined) {
                if (!signal || typeof signal !== 'object' || !('aborted' in signal)) throw new TypeError("Failed to execute 'addEventListener': member signal is not of type AbortSignal.");
                if (signal.aborted) return;
            }
            var key = type + (capture ? '_c' : '_b');
            if (!self.__et_listeners[key]) self.__et_listeners[key] = [];
            for (var i = 0; i < self.__et_listeners[key].length; i++) {
                if (self.__et_listeners[key][i] === cb || self.__et_listeners[key][i]._origCb === cb) return;
            }
            if (once) {
                var wrapper = function(e) {
                    self.removeEventListener(type, cb, capture);
                    if (typeof cb === 'function') cb.call(self, e);
                    else cb.handleEvent(e);
                };
                wrapper._origCb = cb;
                self.__et_listeners[key].push(wrapper);
            } else {
                self.__et_listeners[key].push(cb);
            }
            if (signal) {
                signal.addEventListener('abort', function() {
                    self.removeEventListener(type, cb, capture);
                });
            }
        };
        EventTarget.prototype.removeEventListener = function(type, cb, opts) {
            if (!this.__et_listeners) return;
            var capture = !!(opts === true || (opts && opts.capture));
            var key = type + (capture ? '_c' : '_b');
            if (this.__et_listeners[key]) {
                this.__et_listeners[key] = this.__et_listeners[key].filter(function(f) { return f !== cb && f._origCb !== cb; });
            }
        };
        EventTarget.prototype.dispatchEvent = function(event) {
            if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
            event._dispatching = true;
            event.target = this;
            event.currentTarget = this;
            event._path = [this];
            event.eventPhase = 2;
            var key = event.type + '_b';
            var cbs = this.__et_listeners ? this.__et_listeners[key] : undefined;
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
        Object.defineProperties(CharacterData.prototype, {
            data: {
                get: function() { return __n_getCharData(this.__nid); },
                set: function(v) { __n_setCharData(this.__nid, v === null ? '' : String(v)); },
                configurable: true
            },
            length: {
                get: function() { return __n_charDataLength(this.__nid); },
                configurable: true
            },
        });
        CharacterData.prototype.substringData = function(offset, count) {
            var d = this.data;
            if (offset < 0 || offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            return d.substring(offset, offset + count);
        };
        CharacterData.prototype.appendData = function(data) { this.data = this.data + String(data); };
        CharacterData.prototype.insertData = function(offset, data) {
            var d = this.data;
            if (offset < 0 || offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            this.data = d.substring(0, offset) + String(data) + d.substring(offset);
        };
        CharacterData.prototype.deleteData = function(offset, count) {
            var d = this.data;
            if (offset < 0 || offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            this.data = d.substring(0, offset) + d.substring(offset + count);
        };
        CharacterData.prototype.replaceData = function(offset, count, data) {
            var d = this.data;
            if (offset < 0 || offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            this.data = d.substring(0, offset) + String(data) + d.substring(offset + count);
        };
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

        // Attr constructor — attribute nodes (nodeType 2)
        // Attr.prototype inherits from Node (EP) for instanceof, but we
        // override getter-based properties with own data properties via defineProperty.
        function Attr(name, value, ns, prefix) {
            var props = {
                nodeType: 2,
                name: name || '',
                localName: name || '',
                value: value || '',
                nodeValue: value || '',
                textContent: value || '',
                namespaceURI: ns || null,
                prefix: prefix || null,
                ownerElement: null,
                specified: true,
                nodeName: name || '',
                childNodes: [],
                parentNode: null,
                parentElement: null,
                firstChild: null,
                lastChild: null,
                previousSibling: null,
                nextSibling: null,
                isConnected: false
            };
            for (var k in props) {
                Object.defineProperty(this, k, { value: props[k], writable: true, enumerable: true, configurable: true });
            }
        }
        Attr.prototype = Object.create(EP);
        Attr.prototype.constructor = Attr;
        globalThis.Attr = Attr;

        // Document constructor
        function Document() {}
        Document.prototype = Object.create(EP);
        Document.prototype.constructor = Document;
        globalThis.Document = Document;

        // DOMImplementation constructor (for instanceof checks)
        function DOMImplementation() {}
        DOMImplementation.prototype = Object.getPrototypeOf(document.implementation) || {};
        DOMImplementation.prototype.constructor = DOMImplementation;
        Object.setPrototypeOf(document.implementation, DOMImplementation.prototype);
        globalThis.DOMImplementation = DOMImplementation;

        // DocumentType constructor (for instanceof checks)
        function DocumentType() {}
        DocumentType.prototype = Object.create(EP);
        DocumentType.prototype.constructor = DocumentType;
        globalThis.DocumentType = DocumentType;

        function DocumentFragment() {}
        DocumentFragment.prototype = Object.create(EP);
        DocumentFragment.prototype.constructor = DocumentFragment;
        globalThis.DocumentFragment = DocumentFragment;

        function ProcessingInstruction() {}
        ProcessingInstruction.prototype = Object.create(CharacterData.prototype);
        ProcessingInstruction.prototype.constructor = ProcessingInstruction;
        globalThis.ProcessingInstruction = ProcessingInstruction;

        // Wire global document to Document.prototype
        // nodeId 0 is always the Document node (DomTree::new() allocates it first)
        document.__nid = 0;
        Object.setPrototypeOf(document, Document.prototype);

        // Add Document-specific methods to Document.prototype
        // (Global doc's own-property methods shadow these, but standalone documents inherit them)
        Document.prototype.createElement = function(tag) { return document.createElement(tag); };
        Document.prototype.createElementNS = function(ns, tag) { return document.createElementNS(ns, tag); };
        Document.prototype.createTextNode = function(text) { return document.createTextNode(text); };
        Document.prototype.createComment = function(text) { return document.createComment(text); };
        Document.prototype.createDocumentFragment = function() { return document.createDocumentFragment(); };
        Document.prototype.createProcessingInstruction = function(t, d) { return document.createProcessingInstruction(t, d); };
        Document.prototype.createAttribute = function(n) { return document.createAttribute(n); };
        Document.prototype.createAttributeNS = function(ns, qn) { return document.createAttributeNS(ns, qn); };
        Document.prototype.createEvent = function(type) { var e = new Event(''); e._initialized = false; e.type = ''; return e; };
        Document.prototype.getElementById = function(id) { return null; };
        Document.prototype.querySelector = function(sel) { return null; };
        Document.prototype.querySelectorAll = function(sel) { return []; };
        Document.prototype.getElementsByTagName = function(tag) { return []; };
        Document.prototype.getElementsByClassName = function(cls) { return []; };

        // DocumentFragment also gets querySelector/querySelectorAll
        DocumentFragment.prototype.querySelector = function(sel) {
            if (this.__nid === undefined) return null;
            var nid = __n_querySelector(this.__nid, sel);
            return nid >= 0 ? __w(nid) : null;
        };
        DocumentFragment.prototype.querySelectorAll = function(sel) {
            if (this.__nid === undefined) return [];
            return __n_querySelectorAll(this.__nid, sel).map(__w);
        };

        // Wire window event methods to EventTarget.prototype (spec: Window extends EventTarget)
        window.addEventListener = EventTarget.prototype.addEventListener;
        window.removeEventListener = EventTarget.prototype.removeEventListener;
        window.dispatchEvent = EventTarget.prototype.dispatchEvent;
    "#
}
