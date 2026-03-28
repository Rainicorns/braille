pub(crate) fn element_prototype_js() -> &'static str {
    r#"
        // ElemProto inherits from EP (Node prototype).
        // Element-specific methods go on ElemProto, Node methods stay on EP.
        var ElemProto = Object.create(EP);
        globalThis.__ElemProto = ElemProto;

        ElemProto.getAttribute = function(name) {
            name = String(name).toLowerCase();
            var v = __n_getAttribute(this.__nid, name);
            return __n_hasAttrValue(this.__nid, name) ? v : null;
        };
        ElemProto.setAttribute = function(name, value) {
            name = String(name).toLowerCase();
            var old = __n_hasAttrValue(this.__nid, name) ? __n_getAttribute(this.__nid, name) : null;
            __n_setAttribute(this.__nid, name, String(value));
            if (typeof __mo_notify === 'function') __mo_notify('attributes', this, {attributeName: name, oldValue: old});
        };
        ElemProto.removeAttribute = function(name) {
            name = String(name).toLowerCase();
            var old = __n_hasAttrValue(this.__nid, name) ? __n_getAttribute(this.__nid, name) : null;
            __n_removeAttribute(this.__nid, name);
            if (typeof __mo_notify === 'function') __mo_notify('attributes', this, {attributeName: name, oldValue: old});
        };
        ElemProto.hasAttribute = function(name) { return __n_hasAttribute(this.__nid, String(name).toLowerCase()); };
        ElemProto.hasAttributes = function() { return __n_hasAttributes(this.__nid); };

        EP.addEventListener = function(type, cb, opts) {
            if (typeof cb !== 'function') return;
            var capture = !!(opts === true || (opts && opts.capture));
            var once = !!(opts && typeof opts === 'object' && opts.once);
            var signal = (opts && typeof opts === 'object') ? opts.signal : undefined;
            if (signal !== undefined) {
                if (!signal || typeof signal !== 'object' || !('aborted' in signal)) throw new TypeError("Failed to execute 'addEventListener': member signal is not of type AbortSignal.");
                if (signal.aborted) return;
            }
            var key = this.__nid + ':' + type;
            var store = capture ? _captureKeys : _bubbleKeys;
            if (!store[key]) store[key] = [];
            if (once) {
                var el = this;
                var wrapper = function(e) { el.removeEventListener(type, wrapper, capture); cb.call(el, e); };
                wrapper._origCb = cb;
                store[key].push(wrapper);
            } else {
                store[key].push(cb);
            }
            if (signal) {
                var el = this;
                signal.addEventListener('abort', function() {
                    el.removeEventListener(type, cb, capture);
                });
            }
        };
        EP.removeEventListener = function(type, cb, opts) {
            var capture = !!(opts === true || (opts && opts.capture));
            var key = this.__nid + ':' + type;
            var store = capture ? _captureKeys : _bubbleKeys;
            if (store[key]) {
                store[key] = store[key].filter(function(f) { return f !== cb && f._origCb !== cb; });
            }
        };
        EP.dispatchEvent = function(event) {
            if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
            if (this.__nid === undefined) {
                // Standalone node with no DomTree backing — fire EventTarget listeners only
                event._dispatching = true;
                event.target = this;
                event.currentTarget = this;
                event._path = [this];
                event.eventPhase = 2;
                if (this.__et_listeners) {
                    var cbs = this.__et_listeners[event.type + '_b'];
                    if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(this, event); }
                }
                event._dispatching = false;
                event.currentTarget = null;
                event.eventPhase = 0;
                return !event.defaultPrevented;
            }
            // Find the owning document by walking up to the root element
            var ownerDoc = undefined;
            var rootNid = this.__nid;
            var p = __n_getParent(rootNid);
            while (p >= 0) { rootNid = p; p = __n_getParent(rootNid); }
            var rootEl = __w(rootNid);
            if (rootEl.__ownerDoc) ownerDoc = rootEl.__ownerDoc;
            __dispatch(this.__nid, event, ownerDoc);
            return !event.defaultPrevented;
        };
        // Pointer capture
        var __pointerCaptures = {};
        ElemProto.setPointerCapture = function(pointerId) { __pointerCaptures[pointerId] = this.__nid; };
        ElemProto.releasePointerCapture = function(pointerId) { if (__pointerCaptures[pointerId] === this.__nid) delete __pointerCaptures[pointerId]; };
        ElemProto.hasPointerCapture = function(pointerId) { return __pointerCaptures[pointerId] === this.__nid; };

        ElemProto.click = function() {
            var event = new MouseEvent('click', {bubbles: true, cancelable: true});
            event.target = this;
            event.currentTarget = this;
            __dispatch(this.__nid, event);

            // <details>/<summary> toggle
            if (this.tagName === 'SUMMARY') {
                var details = this.parentNode;
                if (details && details.tagName === 'DETAILS') {
                    if (details.hasAttribute('open')) details.removeAttribute('open');
                    else details.setAttribute('open', '');
                    details.dispatchEvent(new Event('toggle', {bubbles: false}));
                }
            }

            // Implicit form submission: <button type="submit"> or <input type="submit"> inside a <form>
            if (!event.defaultPrevented) {
                var tag = this.tagName;
                var btype = (this.getAttribute('type') || '').toLowerCase();
                if ((tag === 'BUTTON' && (btype === 'submit' || btype === '')) || (tag === 'INPUT' && btype === 'submit')) {
                    var form = this.form;
                    if (form) {
                        var submitEvt = new Event('submit', {bubbles: true, cancelable: true});
                        submitEvt.submitter = this;
                        form.dispatchEvent(submitEvt);
                    }
                }
            }

            // Label activation: clicking a label focuses/clicks its associated control
            if (!event.defaultPrevented && this.tagName === 'LABEL') {
                var controlId = __n_findLabelControl(this.__nid);
                if (controlId >= 0) {
                    var ctrl = __w(controlId);
                    if (ctrl && ctrl.__nid !== this.__nid) {
                        if (typeof ctrl.focus === 'function') ctrl.focus();
                        ctrl.click();
                    }
                }
            }
        };
        // <dialog> element APIs
        ElemProto.showModal = function() {
            if (this.tagName === 'DIALOG') { this.setAttribute('open', ''); if (!this.__props) this.__props = {}; this.__props._dialogModal = true; }
        };
        ElemProto.show = function() {
            if (this.tagName === 'DIALOG') this.setAttribute('open', '');
        };
        ElemProto.close = function(returnValue) {
            if (this.tagName === 'DIALOG') {
                this.removeAttribute('open');
                if (!this.__props) this.__props = {};
                if (returnValue !== undefined) this.__props._returnValue = String(returnValue);
                this.dispatchEvent(new Event('close', {bubbles: false}));
            }
        };

        ElemProto.querySelector = function(sel) {
            var id = __n_querySelector(this.__nid, sel);
            return id >= 0 ? __w(id) : null;
        };
        ElemProto.querySelectorAll = function(sel) {
            return __n_querySelectorAll(this.__nid, sel).map(__w);
        };
        ElemProto.getElementsByTagName = function(tag) {
            var self = this;
            return new Proxy([], {
                get: function(t, p) {
                    var live = self.querySelectorAll(tag);
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
        ElemProto.getElementsByClassName = function(cls) {
            var self = this;
            return new Proxy([], {
                get: function(t, p) {
                    var live = self.querySelectorAll('.' + cls);
                    if (p === 'length') return live.length;
                    if (p === 'item') return function(i) { return live[i] || null; };
                    if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                    if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
                    if (p === 'forEach') return function(cb) { for (var i = 0; i < live.length; i++) cb(live[i], i); };
                    return live[p];
                }
            });
        };
        Object.defineProperty(ElemProto, 'attributes', {
            get: function() {
                if (this.__nid === undefined) return undefined;
                var names = JSON.parse(__n_getAttributeNames(this.__nid));
                var result = [];
                for (var i = 0; i < names.length; i++) {
                    var val = __n_getAttribute(this.__nid, names[i]);
                    var attr = new Attr(names[i], val);
                    attr.ownerElement = this;
                    result.push(attr);
                }
                result.getNamedItem = function(n) { for (var i = 0; i < this.length; i++) if (this[i].name === n) return this[i]; return null; };
                result.setNamedItem = function(a) { if (a && a.ownerElement) a.ownerElement.setAttribute(a.name, a.value); };
                result.removeNamedItem = function(n) { /* no-op for now */ };
                result.item = function(i) { return this[i] || null; };
                return result;
            },
            enumerable: true, configurable: true
        });
        EP.contains = function(other) {
            if (!other || other.__nid === undefined) return false;
            return __n_contains(this.__nid, other.__nid);
        };
        EP.cloneNode = function(deep) {
            var nid = __n_cloneNode(this.__nid, !!deep);
            return __w(nid);
        };
        EP.replaceChild = function(newChild, oldChild) {
            if (newChild !== null && newChild !== undefined && typeof newChild === 'object' && newChild.nodeType === 2) {
                throw new DOMException("The new child element contains the parent.", "HierarchyRequestError");
            }
            if (newChild === null || newChild === undefined || (typeof newChild === 'object' && newChild.__nid === undefined)) {
                throw new TypeError("Failed to execute 'replaceChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (oldChild === null || oldChild === undefined || (typeof oldChild === 'object' && oldChild.__nid === undefined)) {
                throw new TypeError("Failed to execute 'replaceChild' on 'Node': parameter 2 is not of type 'Node'.");
            }
            if (newChild.__nid !== undefined && oldChild.__nid !== undefined && this.__nid !== undefined) {
                var err = __n_validatePreReplace(this.__nid, newChild.__nid, oldChild.__nid);
                if (err) {
                    var colonIdx = err.indexOf(':');
                    var name = err.substring(0, colonIdx);
                    var msg = err.substring(colonIdx + 1);
                    throw new DOMException(msg, name);
                }
                if (newChild.__nid === oldChild.__nid) {
                    return oldChild;
                }
                if (newChild.nodeType === 11) {
                    // DocumentFragment: insert all fragment children before oldChild, then remove oldChild
                    var kids = __n_getAllChildIds(newChild.__nid);
                    for (var i = 0; i < kids.length; i++) {
                        __n_insertBefore(this.__nid, kids[i], oldChild.__nid);
                    }
                    __n_removeChild(this.__nid, oldChild.__nid);
                } else {
                    __n_replaceChild(this.__nid, newChild.__nid, oldChild.__nid);
                }
            }
            return oldChild;
        };
        EP.hasChildNodes = function() { return __n_getFirstChild(this.__nid) >= 0; };

        // CharacterData methods
        EP.substringData = function(offset, count) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'substringData': 2 arguments required, but only " + arguments.length + " present.");
            var r = JSON.parse(__n_charDataSubstring(this.__nid, offset >>> 0, count >>> 0));
            if (r.err) throw new DOMException(r.err, r.err);
            return r.ok;
        };
        EP.appendData = function(data) {
            if (arguments.length < 1) throw new TypeError("Failed to execute 'appendData': 1 argument required, but only 0 present.");
            __n_charDataAppend(this.__nid, String(data));
        };
        EP.insertData = function(offset, data) {
            var err = __n_charDataInsert(this.__nid, offset >>> 0, String(data));
            if (err) throw new DOMException(err, err);
        };
        EP.deleteData = function(offset, count) {
            var err = __n_charDataDelete(this.__nid, offset >>> 0, count >>> 0);
            if (err) throw new DOMException(err, err);
        };
        EP.replaceData = function(offset, count, data) {
            var err = __n_charDataReplace(this.__nid, offset >>> 0, count >>> 0, String(data));
            if (err) throw new DOMException(err, err);
        };

        ElemProto.getBoundingClientRect = function() {
            // Return plausible non-zero defaults instead of all zeros
            var s = __n_getAttribute(this.__nid, 'style') || '';
            // display:none → all zeros
            if (/display\s*:\s*none/i.test(s)) return {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
            // Also check computed style for display:none
            var compDisplay = __n_getComputedStyle(this.__nid, 'display');
            if (compDisplay === 'none') return {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
            var w = 0, h = 0, found = false;
            // Try inline style first
            var wm = s.match(/(?:^|;)\s*width\s*:\s*(\d+)/);
            var hm = s.match(/(?:^|;)\s*height\s*:\s*(\d+)/);
            if (wm) { w = parseInt(wm[1]); found = true; }
            if (hm) { h = parseInt(hm[1]); found = true; }
            // Fall back to computed style if inline didn't have dimensions
            if (!wm) {
                var cw = __n_getComputedStyle(this.__nid, 'width');
                if (cw) { var pw = parseInt(cw); if (!isNaN(pw)) { w = pw; found = true; } }
            }
            if (!hm) {
                var ch = __n_getComputedStyle(this.__nid, 'height');
                if (ch) { var ph = parseInt(ch); if (!isNaN(ph)) { h = ph; found = true; } }
            }
            // If no explicit dimensions, use content-based defaults for visible elements
            if (!found) {
                var tag = this.tagName;
                if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || tag === 'BUTTON' || tag === 'IMG') { w = 100; h = 20; }
                else if (__n_getTextContent(this.__nid).trim()) { w = 100; h = 20; }
            }
            return {top:0,left:0,width:w,height:h,right:w,bottom:h,x:0,y:0};
        };
        ElemProto.getClientRects = function() { return [this.getBoundingClientRect()]; };
        // focus/blur defined later after defineProperties to track activeElement
        ElemProto.scrollIntoView = function() {};
        ElemProto.matches = function(sel) { return __n_matchesSelector(this.__nid, sel); };
        ElemProto.closest = function(sel) {
            var id = __n_closest(this.__nid, sel);
            return id >= 0 ? __w(id) : null;
        };
        ElemProto.getAttributeNames = function() {
            return JSON.parse(__n_getAttributeNames(this.__nid));
        };
        EP.append = function() {
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                this.appendChild(arg);
            }
        };
        EP.prepend = function() {
            var first = this.firstChild;
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                if (first) this.insertBefore(arg, first);
                else this.appendChild(arg);
            }
        };
        EP.replaceChildren = function() {
            while (this.firstChild) this.removeChild(this.firstChild);
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                this.appendChild(arg);
            }
        };
        EP.after = function() {
            var parent = this.parentNode;
            if (!parent) return;
            // Collect viableNextSibling BEFORE moving any nodes (spec step 3)
            var viable = this.nextSibling;
            var args = [];
            for (var i = 0; i < arguments.length; i++) args.push(arguments[i]);
            // If viable is one of the args, advance past it
            while (viable && args.indexOf(viable) !== -1) viable = viable.nextSibling;
            var frag = document.createDocumentFragment();
            for (var i = 0; i < args.length; i++) {
                var arg = args[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                frag.appendChild(arg);
            }
            if (viable) parent.insertBefore(frag, viable);
            else parent.appendChild(frag);
        };
        EP.before = function() {
            var parent = this.parentNode;
            if (!parent) return;
            var args = [];
            for (var i = 0; i < arguments.length; i++) args.push(arguments[i]);
            // Spec: viablePreviousSibling = this.previousSibling not in args, then insert after it
            // Simpler: find the reference node (this), but if this gets moved by frag.appendChild,
            // use its previousSibling's nextSibling (or parent.firstChild if no previousSibling)
            var prev = this.previousSibling;
            while (prev && args.indexOf(prev) !== -1) prev = prev.previousSibling;
            var frag = document.createDocumentFragment();
            for (var i = 0; i < args.length; i++) {
                var arg = args[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                frag.appendChild(arg);
            }
            var ref = prev ? prev.nextSibling : parent.firstChild;
            if (ref) parent.insertBefore(frag, ref);
            else parent.appendChild(frag);
        };
        EP.replaceWith = function() {
            var parent = this.parentNode;
            if (!parent) return;
            var next = this.nextSibling;
            var args = [];
            for (var i = 0; i < arguments.length; i++) args.push(arguments[i]);
            while (next && args.indexOf(next) !== -1) next = next.nextSibling;
            parent.removeChild(this);
            var frag = document.createDocumentFragment();
            for (var i = 0; i < args.length; i++) {
                var arg = args[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                frag.appendChild(arg);
            }
            if (next) parent.insertBefore(frag, next);
            else parent.appendChild(frag);
        };
        ElemProto.toggleAttribute = function(name, force) {
            if (force !== undefined) {
                if (force) { this.setAttribute(name, ''); return true; }
                else { this.removeAttribute(name); return false; }
            }
            if (this.hasAttribute(name)) { this.removeAttribute(name); return false; }
            this.setAttribute(name, ''); return true;
        };
        ElemProto.setAttributeNS = function(ns, qualifiedName, value) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            __n_setAttributeNS(this.__nid, ns, String(qualifiedName), String(value));
        };
        ElemProto.getAttributeNS = function(ns, localName) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            if (__n_hasAttributeNS(this.__nid, ns, String(localName))) {
                return __n_getAttributeNS(this.__nid, ns, String(localName));
            }
            return null;
        };
        ElemProto.removeAttributeNS = function(ns, localName) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            __n_removeAttributeNS(this.__nid, ns, String(localName));
        };
        ElemProto.hasAttributeNS = function(ns, localName) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            return __n_hasAttributeNS(this.__nid, ns, String(localName));
        };
        ElemProto.insertAdjacentHTML = function(position, html) {
            var temp = document.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            var frag = document.createDocumentFragment();
            while (temp.firstChild) frag.appendChild(temp.firstChild);
            if (position === 'beforebegin') this.before(frag);
            else if (position === 'afterbegin') this.prepend(frag);
            else if (position === 'beforeend') this.append(frag);
            else if (position === 'afterend') this.after(frag);
        };
        ElemProto.insertAdjacentElement = function(position, el) {
            if (position === 'beforebegin') this.before(el);
            else if (position === 'afterbegin') this.prepend(el);
            else if (position === 'beforeend') this.append(el);
            else if (position === 'afterend') this.after(el);
            return el;
        };
        ElemProto.getAnimations = function() { return []; };
        ElemProto.animate = function() {
            var anim = { finished: Promise.resolve(), cancel: function(){}, play: function(){}, pause: function(){}, onfinish: null };
            anim.finish = function() { if (typeof anim.onfinish === 'function') anim.onfinish(); };
            return anim;
        };
        ElemProto.attachShadow = function() { return document.createDocumentFragment(); };
        ElemProto.getAttributeNode = function(name) {
            if (!this.hasAttribute(name)) return null;
            return { name: name, value: this.getAttribute(name), specified: true };
        };
        EP.remove = function() {
            if (this.__nid !== undefined) {
                var pid = __n_getParent(this.__nid);
                if (pid >= 0) __n_removeChild(pid, this.__nid);
            }
        };
        EP.getRootNode = function() { return document; };
        EP.compareDocumentPosition = function(other) {
            if (!other || other.__nid === undefined || this.__nid === undefined) return 0;
            return __n_compareDocumentPosition(this.__nid, other.__nid);
        };

        // === Node-level properties (stay on EP) ===
        Object.defineProperties(EP, {
            textContent: {
                get: function() { if (this.__nid === undefined) return ''; return __n_getTextContent(this.__nid); },
                set: function(v) { if (this.__nid === undefined) return; __n_setTextContent(this.__nid, String(v)); },
                configurable: true
            },
            nodeName: { get: function() {
                if (this.__nid === undefined) return '';
                var nt = __n_getNodeType(this.__nid);
                if (nt === 3) return '#text';
                if (nt === 8) return '#comment';
                if (nt === 9) return '#document';
                if (nt === 11) return '#document-fragment';
                return __n_getTagName(this.__nid) || '#node';
            }, configurable: true },
            nodeType: { get: function() { if (this.__nid === undefined) return undefined; return __n_getNodeType(this.__nid); }, configurable: true },
            parentNode: {
                get: function() { if (this.__nid === undefined) return null; var p = __n_getParent(this.__nid); return p >= 0 ? __w(p) : null; },
                configurable: true
            },
            parentElement: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var p = __n_getParent(this.__nid);
                    if (p < 0) return null;
                    var nt = __n_getNodeType(p);
                    return nt === 1 ? __w(p) : null;
                },
                configurable: true
            },
            children: {
                get: function() { if (this.__nid === undefined) return []; return __n_getChildElementIds(this.__nid).map(__w); },
                configurable: true
            },
            childNodes: {
                get: function() { if (this.__nid === undefined) return []; return __n_getAllChildIds(this.__nid).map(__w); },
                configurable: true
            },
            firstChild: {
                get: function() { if (this.__nid === undefined) return null; var id = __n_getFirstChild(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            lastChild: {
                get: function() { if (this.__nid === undefined) return null; var id = __n_getLastChild(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            nextSibling: {
                get: function() { if (this.__nid === undefined) return null; var id = __n_getNextSibling(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            previousSibling: {
                get: function() { if (this.__nid === undefined) return null; var id = __n_getPrevSibling(this.__nid); return id >= 0 ? __w(id) : null; },
                configurable: true
            },
            nodeValue: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8) return __n_getNodeValue(this.__nid);
                    return null;
                },
                set: function(v) {
                    if (this.__nid === undefined) return;
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8) __n_setCharData(this.__nid, String(v));
                },
                configurable: true
            },
            ownerDocument: { get: function() { return document; }, configurable: true },
            isConnected: {
                get: function() {
                    if (this.__nid === undefined) return false;
                    var cur = this.__nid;
                    while (cur >= 0) {
                        if (__n_getNodeType(cur) === 9) return true;
                        cur = __n_getParent(cur);
                    }
                    return false;
                },
                configurable: true
            },
        });

        // === Element-specific properties (on ElemProto) ===
        Object.defineProperties(ElemProto, {
            tagName: { get: function() { return __n_getTagName(this.__nid); }, configurable: true },
            id: {
                get: function() { return this.getAttribute('id') || ''; },
                set: function(v) { this.setAttribute('id', v); },
                configurable: true
            },
            className: {
                get: function() { return this.getAttribute('class') || ''; },
                set: function(v) { this.setAttribute('class', v); },
                configurable: true
            },
            value: {
                get: function() {
                    if (this.__props && this.__props._value !== undefined) return this.__props._value;
                    if (this.tagName === 'SELECT') {
                        var opts = this.querySelectorAll('option');
                        for (var i = 0; i < opts.length; i++) {
                            if ((opts[i].__props && opts[i].__props._selected) || opts[i].hasAttribute('selected')) {
                                return opts[i].getAttribute('value') || opts[i].textContent || '';
                            }
                        }
                        return opts.length > 0 ? (opts[0].getAttribute('value') || opts[0].textContent || '') : '';
                    }
                    return this.getAttribute('value') || '';
                },
                set: function(v) {
                    if (!this.__props) this.__props = {};
                    var s = String(v);
                    var ml = this.getAttribute('maxlength');
                    if (ml !== null) { var n = parseInt(ml, 10); if (!isNaN(n) && n >= 0 && s.length > n) s = s.substring(0, n); }
                    this.__props._value = s;
                    if (this.tagName === 'SELECT') {
                        var opts = this.querySelectorAll('option');
                        for (var i = 0; i < opts.length; i++) {
                            if (!opts[i].__props) opts[i].__props = {};
                            opts[i].__props._selected = ((opts[i].getAttribute('value') || opts[i].textContent || '') === String(v));
                        }
                    }
                    if (this.tagName === 'TEXTAREA') __n_setTextContent(this.__nid, String(v));
                },
                configurable: true
            },
            defaultValue: {
                get: function() {
                    if (this.tagName === 'TEXTAREA') return __n_getTextContent(this.__nid);
                    return this.getAttribute('value') || '';
                },
                set: function(v) {
                    if (this.tagName === 'TEXTAREA') {
                        __n_setTextContent(this.__nid, String(v));
                    } else {
                        this.setAttribute('value', String(v));
                    }
                },
                configurable: true
            },
            maxLength: {
                get: function() {
                    var v = this.getAttribute('maxlength');
                    if (v === null) return -1;
                    var n = parseInt(v, 10);
                    return isNaN(n) || n < 0 ? -1 : n;
                },
                set: function(v) {
                    var n = parseInt(v, 10);
                    if (isNaN(n) || n < 0) { this.removeAttribute('maxlength'); return; }
                    this.setAttribute('maxlength', String(n));
                },
                configurable: true
            },
            minLength: {
                get: function() {
                    var v = this.getAttribute('minlength');
                    if (v === null) return -1;
                    var n = parseInt(v, 10);
                    return isNaN(n) || n < 0 ? -1 : n;
                },
                set: function(v) {
                    var n = parseInt(v, 10);
                    if (isNaN(n) || n < 0) { this.removeAttribute('minlength'); return; }
                    this.setAttribute('minlength', String(n));
                },
                configurable: true
            },
            cols: {
                get: function() {
                    var v = this.getAttribute('cols');
                    if (v === null) return 20;
                    var n = parseInt(v, 10);
                    return isNaN(n) || n <= 0 ? 20 : n;
                },
                set: function(v) {
                    var n = parseInt(v, 10);
                    if (isNaN(n) || n <= 0) n = 20;
                    this.setAttribute('cols', String(n));
                },
                configurable: true
            },
            rows: {
                get: function() {
                    var v = this.getAttribute('rows');
                    if (v === null) return 2;
                    var n = parseInt(v, 10);
                    return isNaN(n) || n <= 0 ? 2 : n;
                },
                set: function(v) {
                    var n = parseInt(v, 10);
                    if (isNaN(n) || n <= 0) n = 2;
                    this.setAttribute('rows', String(n));
                },
                configurable: true
            },
            wrap: {
                get: function() { return this.getAttribute('wrap') || 'soft'; },
                set: function(v) { this.setAttribute('wrap', String(v)); },
                configurable: true
            },
            textLength: {
                get: function() {
                    var val = '';
                    if (this.__props && this.__props._value !== undefined) val = this.__props._value;
                    else if (this.tagName === 'TEXTAREA') val = __n_getTextContent(this.__nid);
                    else val = this.getAttribute('value') || '';
                    return val.length;
                },
                configurable: true
            },
            checked: {
                get: function() {
                    if (this.__props && this.__props._checked !== undefined) return this.__props._checked;
                    return this.hasAttribute('checked');
                },
                set: function(v) { if (!this.__props) this.__props = {}; this.__props._checked = !!v; },
                configurable: true
            },
            defaultChecked: {
                get: function() { return this.hasAttribute('checked'); },
                set: function(v) { if(v) this.setAttribute('checked',''); else this.removeAttribute('checked'); },
                configurable: true
            },
            selected: {
                get: function() {
                    if (this.__props && this.__props._selected !== undefined) return this.__props._selected;
                    return this.hasAttribute('selected');
                },
                set: function(v) { if (!this.__props) this.__props = {}; this.__props._selected = !!v; },
                configurable: true
            },
            disabled: {
                get: function() { return this.hasAttribute('disabled'); },
                set: function(v) { if (v) this.setAttribute('disabled', ''); else this.removeAttribute('disabled'); },
                configurable: true
            },
            noModule: {
                get: function() { return this.hasAttribute('nomodule'); },
                set: function(v) { if(v) this.setAttribute('nomodule',''); else this.removeAttribute('nomodule'); },
                configurable: true
            },
            async: {
                get: function() { return this.hasAttribute('async'); },
                set: function(v) { if(v) this.setAttribute('async',''); else this.removeAttribute('async'); },
                configurable: true
            },
            defer: {
                get: function() { return this.hasAttribute('defer'); },
                set: function(v) { if(v) this.setAttribute('defer',''); else this.removeAttribute('defer'); },
                configurable: true
            },
            reversed: {
                get: function() { return this.hasAttribute('reversed'); },
                set: function(v) { if(v) this.setAttribute('reversed',''); else this.removeAttribute('reversed'); },
                configurable: true
            },
            type: {
                get: function() {
                    if (this.tagName === 'INPUT') return (this.getAttribute('type') || 'text').toLowerCase();
                    if (this.tagName === 'BUTTON') return (this.getAttribute('type') || 'submit').toLowerCase();
                    return this.getAttribute('type') || '';
                },
                set: function(v) { this.setAttribute('type', String(v)); },
                configurable: true
            },
            href: {
                get: function() { return this.getAttribute('href') || ''; },
                set: function(v) { this.setAttribute('href', String(v)); },
                configurable: true
            },
            src: {
                get: function() { return this.getAttribute('src') || ''; },
                set: function(v) { this.setAttribute('src', String(v)); },
                configurable: true
            },
            innerHTML: {
                get: function() { return __n_getInnerHTML(this.__nid); },
                set: function(v) { __n_setInnerHTML(this.__nid, String(v)); },
                configurable: true
            },
            style: {
                get: function() {
                    if (!this._s) {
                        var nid = this.__nid;
                        function parseStyle() {
                            var s = __n_getAttribute(nid, 'style');
                            var arr = [];
                            if (!s) return arr;
                            var parts = s.split(';');
                            for (var i = 0; i < parts.length; i++) {
                                var p = parts[i].trim();
                                if (!p) continue;
                                var ci = p.indexOf(':');
                                if (ci < 0) continue;
                                arr.push([p.substring(0, ci).trim(), p.substring(ci + 1).trim()]);
                            }
                            return arr;
                        }
                        function serializeStyle(arr) {
                            return arr.map(function(e) { return e[0] + ': ' + e[1]; }).join('; ');
                        }
                        function writeStyle(arr) {
                            var s = serializeStyle(arr);
                            if (s) __n_setAttribute(nid, 'style', s);
                            else __n_removeAttribute(nid, 'style');
                        }
                        function toKebab(cc) {
                            if (cc === 'cssFloat') return 'float';
                            return cc.replace(/[A-Z]/g, function(c) { return '-' + c.toLowerCase(); });
                        }
                        var store = {
                            setProperty: function(prop, val) {
                                var arr = parseStyle();
                                var found = false;
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === prop) { arr[i][1] = val; found = true; break; }
                                }
                                if (!found) arr.push([prop, val]);
                                writeStyle(arr);
                            },
                            removeProperty: function(prop) {
                                var arr = parseStyle();
                                var old = '';
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === prop) { old = arr[i][1]; arr.splice(i, 1); break; }
                                }
                                writeStyle(arr);
                                return old;
                            },
                            getPropertyValue: function(prop) {
                                var arr = parseStyle();
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === prop) return arr[i][1];
                                }
                                return '';
                            },
                            getPropertyPriority: function() { return ''; },
                        };
                        this._s = new Proxy(store, {
                            set: function(t, p, v) {
                                if (typeof p !== 'string') return true;
                                if (p === 'cssText') {
                                    if (v && String(v).trim()) __n_setAttribute(nid, 'style', String(v));
                                    else __n_removeAttribute(nid, 'style');
                                    return true;
                                }
                                var kebab = toKebab(p);
                                var arr = parseStyle();
                                if (v === '' || v === null || v === undefined) {
                                    for (var i = 0; i < arr.length; i++) {
                                        if (arr[i][0] === kebab) { arr.splice(i, 1); break; }
                                    }
                                } else {
                                    var found = false;
                                    for (var i = 0; i < arr.length; i++) {
                                        if (arr[i][0] === kebab) { arr[i][1] = String(v); found = true; break; }
                                    }
                                    if (!found) arr.push([kebab, String(v)]);
                                }
                                writeStyle(arr);
                                return true;
                            },
                            get: function(t, p) {
                                if (p in t) return t[p];
                                if (typeof p !== 'string') return undefined;
                                if (p === 'cssText') {
                                    return __n_getAttribute(nid, 'style') || '';
                                }
                                if (p === 'length') {
                                    return parseStyle().length;
                                }
                                if (p === 'item') {
                                    return function(idx) {
                                        var arr = parseStyle();
                                        return idx < arr.length ? arr[idx][0] : '';
                                    };
                                }
                                var kebab = toKebab(p);
                                var arr = parseStyle();
                                for (var i = 0; i < arr.length; i++) {
                                    if (arr[i][0] === kebab) return arr[i][1];
                                }
                                return '';
                            }
                        });
                    }
                    return this._s;
                },
                configurable: true
            },
            classList: {
                get: function() {
                    var el = this;
                    return {
                        add: function() { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); for(var i=0;i<arguments.length;i++) if(c.indexOf(arguments[i])<0) c.push(arguments[i]); el.setAttribute('class',c.join(' ')); },
                        remove: function() { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); for(var i=0;i<arguments.length;i++){var idx=c.indexOf(arguments[i]);if(idx>=0)c.splice(idx,1);} el.setAttribute('class',c.join(' ')); },
                        contains: function(cls) { return (el.getAttribute('class')||'').split(/\s+/).indexOf(cls)>=0; },
                        toggle: function(cls,force) { if(force!==undefined){if(force)this.add(cls);else this.remove(cls);return force;} if(this.contains(cls)){this.remove(cls);return false;} this.add(cls);return true; },
                        forEach: function(cb) { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); for(var i=0;i<c.length;i++) cb(c[i],i,c); },
                        get length() { return (el.getAttribute('class')||'').split(/\s+/).filter(Boolean).length; },
                        item: function(i) { var c=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean); return i<c.length?c[i]:null; },
                        toString: function() { return el.getAttribute('class')||''; },
                        get value() { return el.getAttribute('class')||''; },
                        set value(v) { el.setAttribute('class', v); },
                    };
                },
                configurable: true
            },
            dataset: {
                get: function() {
                    var el = this;
                    return new Proxy({}, {
                        get: function(t, prop) {
                            if (typeof prop !== 'string') return undefined;
                            return __n_getDataAttr(el.__nid, prop) || undefined;
                        },
                        set: function(t, prop, val) {
                            var name = 'data-' + prop.replace(/[A-Z]/g, function(c){return '-'+c.toLowerCase();});
                            __n_setAttribute(el.__nid, name, String(val));
                            return true;
                        }
                    });
                },
                configurable: true
            },
            scrollTop: { get: function() { return 0; }, set: function(){}, configurable: true },
            scrollLeft: { get: function() { return 0; }, set: function(){}, configurable: true },
            scrollWidth: { get: function() { return this.getBoundingClientRect().width; }, configurable: true },
            scrollHeight: { get: function() { return this.getBoundingClientRect().height; }, configurable: true },
            offsetTop: { get: function() { return 0; }, configurable: true },
            offsetLeft: { get: function() { return 0; }, configurable: true },
            offsetWidth: { get: function() { return this.getBoundingClientRect().width; }, configurable: true },
            offsetHeight: { get: function() { return this.getBoundingClientRect().height; }, configurable: true },
            clientWidth: { get: function() { if (this.tagName === 'HTML') return 1280; return this.getBoundingClientRect().width; }, configurable: true },
            clientHeight: { get: function() { if (this.tagName === 'HTML') return 800; return this.getBoundingClientRect().height; }, configurable: true },
            clientTop: { get: function() { return 0; }, configurable: true },
            clientLeft: { get: function() { return 0; }, configurable: true },
            offsetParent: { get: function() { return this.parentNode; }, configurable: true },
            innerText: {
                get: function() {
                    function walk(nid) {
                        var nt = __n_getNodeType(nid);
                        if (nt === 3) return __n_getCharData(nid);
                        if (nt !== 1) return '';
                        var disp = __n_getComputedStyle(nid, 'display');
                        if (disp === 'none') return '';
                        var vis = __n_getComputedStyle(nid, 'visibility');
                        if (vis === 'hidden') return '';
                        var kids = __n_getAllChildIds(nid);
                        var parts = [];
                        for (var i = 0; i < kids.length; i++) parts.push(walk(kids[i]));
                        return parts.join('');
                    }
                    return walk(this.__nid);
                },
                set: function(v) { this.textContent = v; },
                configurable: true
            },
            outerHTML: {
                get: function() {
                    var tag = (this.tagName || 'div').toLowerCase();
                    var attrs = this.getAttributeNames();
                    var s = '<' + tag;
                    for (var i = 0; i < attrs.length; i++) {
                        s += ' ' + attrs[i] + '="' + (this.getAttribute(attrs[i]) || '').replace(/"/g, '&quot;') + '"';
                    }
                    s += '>' + (this.innerHTML || '') + '</' + tag + '>';
                    return s;
                },
                set: function(v) {
                    var parent = this.parentNode;
                    if (!parent) return;
                    var temp = document.createElement('div');
                    temp.innerHTML = String(v);
                    var frag = document.createDocumentFragment();
                    while (temp.firstChild) {
                        frag.appendChild(temp.firstChild);
                    }
                    parent.replaceChild(frag, this);
                },
                configurable: true
            },
            tabIndex: {
                get: function() {
                    var v = this.getAttribute('tabindex');
                    if (v !== null) return parseInt(v) || 0;
                    var tag = this.tagName;
                    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || tag === 'BUTTON' || tag === 'A' || tag === 'AREA') return 0;
                    return -1;
                },
                set: function(v) { this.setAttribute('tabindex', String(v)); },
                configurable: true
            },
            title: {
                get: function() { return this.getAttribute('title') || ''; },
                set: function(v) { this.setAttribute('title', String(v)); },
                configurable: true
            },
            lang: {
                get: function() { return this.getAttribute('lang') || ''; },
                set: function(v) { this.setAttribute('lang', String(v)); },
                configurable: true
            },
            dir: {
                get: function() { return this.getAttribute('dir') || ''; },
                set: function(v) { this.setAttribute('dir', String(v)); },
                configurable: true
            },
            hidden: {
                get: function() { return this.hasAttribute('hidden'); },
                set: function(v) { if (v) this.setAttribute('hidden', ''); else this.removeAttribute('hidden'); },
                configurable: true
            },
            name: {
                get: function() { return this.getAttribute('name') || ''; },
                set: function(v) { this.setAttribute('name', String(v)); },
                configurable: true
            },
            placeholder: {
                get: function() { return this.getAttribute('placeholder') || ''; },
                set: function(v) { this.setAttribute('placeholder', String(v)); },
                configurable: true
            },
            rel: {
                get: function() { return this.getAttribute('rel') || ''; },
                set: function(v) { this.setAttribute('rel', String(v)); },
                configurable: true
            },
            validity: {
                get: function() {
                    var el = this;
                    var val = el.value || '';
                    var tag = el.tagName;
                    if (tag !== 'INPUT' && tag !== 'TEXTAREA' && tag !== 'SELECT') {
                        return { valid: true, valueMissing: false, typeMismatch: false, patternMismatch: false,
                            tooLong: false, tooShort: false, rangeUnderflow: false, rangeOverflow: false,
                            stepMismatch: false, badInput: false, customError: false };
                    }
                    var customMsg = (el.__props && el.__props._customValidity) || '';
                    var customError = customMsg.length > 0;
                    var valueMissing = !!(el.hasAttribute('required') && val === '');
                    var typeMismatch = false;
                    var inputType = (el.getAttribute('type') || '').toLowerCase();
                    if (val && inputType === 'email') typeMismatch = !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(val);
                    if (val && inputType === 'url') typeMismatch = !/^https?:\/\/.+/.test(val);
                    var patternMismatch = false;
                    var pat = el.getAttribute('pattern');
                    if (pat && val) { try { patternMismatch = !new RegExp('^(?:' + pat + ')$').test(val); } catch(e) {} }
                    var tooLong = false, tooShort = false;
                    var maxl = el.getAttribute('maxlength'); if (maxl !== null && val.length > parseInt(maxl)) tooLong = true;
                    var minl = el.getAttribute('minlength'); if (minl !== null && val.length > 0 && val.length < parseInt(minl)) tooShort = true;
                    var rangeUnderflow = false, rangeOverflow = false, stepMismatch = false, badInput = false;
                    var mn = el.getAttribute('min');
                    var mx = el.getAttribute('max');
                    var stepAttr = el.getAttribute('step');
                    var numericTypes = { number: 1, range: 1 };
                    var dateTimeTypes = { date: 1, time: 1, 'datetime-local': 1, month: 1, week: 1 };
                    if (tag === 'INPUT' && inputType in numericTypes) {
                        var isRange = inputType === 'range';
                        var defMin = isRange ? 0 : null;
                        var defMax = isRange ? 100 : null;
                        var defStep = 1;
                        if (val !== '') {
                            var nv = parseFloat(val);
                            if (isNaN(nv) || !isFinite(nv)) {
                                if (!isRange) badInput = true;
                            } else {
                                var minVal = mn !== null ? parseFloat(mn) : defMin;
                                var maxVal = mx !== null ? parseFloat(mx) : defMax;
                                if (minVal !== null && nv < minVal) rangeUnderflow = true;
                                if (maxVal !== null && nv > maxVal) rangeOverflow = true;
                                var stepVal = stepAttr !== null ? parseFloat(stepAttr) : defStep;
                                if (stepVal !== null && stepAttr !== 'any' && !isNaN(stepVal) && stepVal > 0) {
                                    var base = minVal !== null ? minVal : 0;
                                    var diff = Math.abs((nv - base) % stepVal);
                                    if (diff > 1e-10 && Math.abs(diff - stepVal) > 1e-10) stepMismatch = true;
                                }
                            }
                        }
                    } else if (tag === 'INPUT' && inputType in dateTimeTypes) {
                        if (val !== '') {
                            var dtValid = true;
                            var dtVal = 0, dtMin = null, dtMax = null;
                            if (inputType === 'date') {
                                if (!/^\d{4}-\d{2}-\d{2}$/.test(val)) { badInput = true; dtValid = false; }
                                else { dtVal = new Date(val + 'T00:00:00Z').getTime(); if (isNaN(dtVal)) { badInput = true; dtValid = false; } }
                                if (dtValid && mn !== null) { dtMin = new Date(mn + 'T00:00:00Z').getTime(); }
                                if (dtValid && mx !== null) { dtMax = new Date(mx + 'T00:00:00Z').getTime(); }
                            } else if (inputType === 'time') {
                                if (!/^\d{2}:\d{2}(:\d{2})?$/.test(val)) { badInput = true; dtValid = false; }
                                else {
                                    var tp = val.split(':'); dtVal = parseInt(tp[0]) * 3600 + parseInt(tp[1]) * 60 + (tp[2] ? parseInt(tp[2]) : 0);
                                    if (parseInt(tp[0]) > 23 || parseInt(tp[1]) > 59 || (tp[2] && parseInt(tp[2]) > 59)) { badInput = true; dtValid = false; }
                                }
                                if (dtValid && mn !== null) { var mp = mn.split(':'); dtMin = parseInt(mp[0]) * 3600 + parseInt(mp[1]) * 60 + (mp[2] ? parseInt(mp[2]) : 0); }
                                if (dtValid && mx !== null) { var xp = mx.split(':'); dtMax = parseInt(xp[0]) * 3600 + parseInt(xp[1]) * 60 + (xp[2] ? parseInt(xp[2]) : 0); }
                            } else if (inputType === 'datetime-local') {
                                if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2})?$/.test(val)) { badInput = true; dtValid = false; }
                                else { dtVal = new Date(val + 'Z').getTime(); if (isNaN(dtVal)) { badInput = true; dtValid = false; } }
                                if (dtValid && mn !== null) { dtMin = new Date(mn + 'Z').getTime(); }
                                if (dtValid && mx !== null) { dtMax = new Date(mx + 'Z').getTime(); }
                            } else if (inputType === 'month') {
                                if (!/^\d{4}-\d{2}$/.test(val)) { badInput = true; dtValid = false; }
                                else {
                                    var mParts = val.split('-'); dtVal = parseInt(mParts[0]) * 12 + parseInt(mParts[1]);
                                    if (parseInt(mParts[1]) < 1 || parseInt(mParts[1]) > 12) { badInput = true; dtValid = false; }
                                }
                                if (dtValid && mn !== null) { var mnP = mn.split('-'); dtMin = parseInt(mnP[0]) * 12 + parseInt(mnP[1]); }
                                if (dtValid && mx !== null) { var mxP = mx.split('-'); dtMax = parseInt(mxP[0]) * 12 + parseInt(mxP[1]); }
                            } else if (inputType === 'week') {
                                if (!/^\d{4}-W\d{2}$/.test(val)) { badInput = true; dtValid = false; }
                                else {
                                    var wParts = val.split('-W'); dtVal = parseInt(wParts[0]) * 53 + parseInt(wParts[1]);
                                    if (parseInt(wParts[1]) < 1 || parseInt(wParts[1]) > 53) { badInput = true; dtValid = false; }
                                }
                                if (dtValid && mn !== null) { var wnP = mn.split('-W'); dtMin = parseInt(wnP[0]) * 53 + parseInt(wnP[1]); }
                                if (dtValid && mx !== null) { var wxP = mx.split('-W'); dtMax = parseInt(wxP[0]) * 53 + parseInt(wxP[1]); }
                            }
                            if (dtValid) {
                                if (dtMin !== null && !isNaN(dtMin) && dtVal < dtMin) rangeUnderflow = true;
                                if (dtMax !== null && !isNaN(dtMax) && dtVal > dtMax) rangeOverflow = true;
                            }
                        }
                    } else if (tag === 'INPUT' && inputType === 'color') {
                        if (val !== '' && !/^#[0-9a-fA-F]{6}$/.test(val)) badInput = true;
                    } else {
                        if (mn !== null && val !== '' && parseFloat(val) < parseFloat(mn)) rangeUnderflow = true;
                        if (mx !== null && val !== '' && parseFloat(val) > parseFloat(mx)) rangeOverflow = true;
                    }
                    var valid = !valueMissing && !typeMismatch && !patternMismatch && !tooLong && !tooShort && !rangeUnderflow && !rangeOverflow && !stepMismatch && !badInput && !customError;
                    return { valid: valid, valueMissing: valueMissing, typeMismatch: typeMismatch,
                        patternMismatch: patternMismatch, tooLong: tooLong, tooShort: tooShort,
                        rangeUnderflow: rangeUnderflow, rangeOverflow: rangeOverflow,
                        stepMismatch: stepMismatch, badInput: badInput, customError: customError };
                },
                configurable: true
            },
            validationMessage: {
                get: function() {
                    var v = this.validity;
                    if (v.valid) return '';
                    if (v.customError) return (this.__props && this.__props._customValidity) || '';
                    if (v.valueMissing) return 'Please fill out this field.';
                    if (v.typeMismatch) return 'Please enter a valid value.';
                    if (v.patternMismatch) return 'Please match the requested format.';
                    if (v.tooShort) return 'Please use at least ' + this.getAttribute('minlength') + ' characters.';
                    if (v.tooLong) return 'Please use no more than ' + this.getAttribute('maxlength') + ' characters.';
                    if (v.rangeUnderflow) return 'Value must be greater than or equal to ' + this.getAttribute('min') + '.';
                    if (v.rangeOverflow) return 'Value must be less than or equal to ' + this.getAttribute('max') + '.';
                    if (v.stepMismatch) return 'Please enter a valid value. The nearest valid values are those aligned with the step.';
                    if (v.badInput) return 'Please enter a valid value.';
                    return '';
                },
                configurable: true
            },
        });

        // open property for DIALOG and DETAILS
        Object.defineProperty(ElemProto, 'open', {
            get: function() {
                if (this.tagName === 'DIALOG' || this.tagName === 'DETAILS') return this.hasAttribute('open');
                return undefined;
            },
            set: function(v) {
                if (this.tagName === 'DIALOG' || this.tagName === 'DETAILS') {
                    if (v) this.setAttribute('open', '');
                    else this.removeAttribute('open');
                }
            },
            configurable: true
        });
        Object.defineProperty(ElemProto, 'returnValue', {
            get: function() {
                if (this.tagName !== 'DIALOG') return undefined;
                return (this.__props && this.__props._returnValue) || '';
            },
            set: function(v) {
                if (this.tagName === 'DIALOG') { if (!this.__props) this.__props = {}; this.__props._returnValue = String(v); }
            },
            configurable: true
        });
    "#
}
