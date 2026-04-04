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
            if (name === 'id' && value && !(value in globalThis)) globalThis[value] = this;
            if (typeof __mo_notify === 'function') __mo_notify('attributes', this, {attributeName: name, oldValue: old});
            // CE attributeChangedCallback
            if (this.__ce_upgraded && typeof this.attributeChangedCallback === 'function') {
                var ce = customElements._registry.get(__n_getTagName(this.__nid).toLowerCase());
                if (ce && ce.observedAttrs.indexOf(name) !== -1) {
                    this.attributeChangedCallback(name, old, String(value));
                }
            }
        };
        ElemProto.removeAttribute = function(name) {
            name = String(name).toLowerCase();
            var old = __n_hasAttrValue(this.__nid, name) ? __n_getAttribute(this.__nid, name) : null;
            __n_removeAttribute(this.__nid, name);
            if (typeof __mo_notify === 'function') __mo_notify('attributes', this, {attributeName: name, oldValue: old});
            // CE attributeChangedCallback
            if (this.__ce_upgraded && typeof this.attributeChangedCallback === 'function') {
                var ce = customElements._registry.get(__n_getTagName(this.__nid).toLowerCase());
                if (ce && ce.observedAttrs.indexOf(name) !== -1) {
                    this.attributeChangedCallback(name, old, null);
                }
            }
        };
        ElemProto.hasAttribute = function(name) { return __n_hasAttribute(this.__nid, String(name).toLowerCase()); };
        ElemProto.hasAttributes = function() { return __n_hasAttributes(this.__nid); };

        // Event types that are passive by default on scroll-blocking targets
        var __passiveDefaultTypes = {touchstart:1,touchmove:1,wheel:1,mousewheel:1};
        function __isScrollBlockingTarget(el) {
            if (el === window || el === document) return true;
            if (el.__nid === undefined) return false;
            var tag = __n_getTagName(el.__nid);
            return tag === 'HTML' || tag === 'BODY';
        }

        EP.addEventListener = function(type, cb, opts) {
            var capture, once, signal, passive, passiveExplicit;
            if (opts && typeof opts === 'object' && opts !== null) {
                capture = !!opts.capture;
                once = !!opts.once;
                signal = opts.signal;
                // passive is explicitly set only if the key exists AND the value is not undefined
                passiveExplicit = ('passive' in opts) && opts.passive !== undefined;
                passive = passiveExplicit ? !!opts.passive : false;
            } else {
                capture = !!opts;
                once = false;
                signal = undefined;
                passiveExplicit = false;
                passive = false;
            }
            // Passive-by-default: touch/wheel on window/document/html/body
            if (!passiveExplicit && __passiveDefaultTypes[type] && __isScrollBlockingTarget(this)) {
                passive = true;
            }
            // Track passive listeners for synthetic event cancelability
            if (passive) {
                if (!this.__passiveTypes) this.__passiveTypes = {};
                this.__passiveTypes[type] = true;
            }
            if (typeof cb !== 'function' && !(cb && typeof cb === 'object')) return;
            if (signal !== undefined) {
                if (!signal || typeof signal !== 'object' || !('aborted' in signal)) throw new TypeError("Failed to execute 'addEventListener': member signal is not of type AbortSignal.");
                if (signal.aborted) return;
            }
            var key = this.__nid + ':' + type;
            var store = capture ? _captureKeys : _bubbleKeys;
            if (!store[key]) store[key] = [];
            if (once) {
                var el = this;
                var wrapper = function(e) {
                    el.removeEventListener(type, wrapper, capture);
                    if (typeof cb === 'function') cb.call(el, e);
                    else if (cb && typeof cb.handleEvent === 'function') cb.handleEvent(e);
                };
                wrapper._origCb = cb;
                if (passive) wrapper._passive = true;
                store[key].push(wrapper);
            } else {
                if (passive && typeof cb === 'function') cb._passive = true;
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
            var capture = (opts && typeof opts === 'object' && opts !== null) ? !!opts.capture : !!opts;
            var key = this.__nid + ':' + type;
            var store = capture ? _captureKeys : _bubbleKeys;
            var arr = store[key];
            if (arr) {
                for (var i = arr.length - 1; i >= 0; i--) {
                    if (arr[i] === cb || arr[i]._origCb === cb) {
                        arr.splice(i, 1);
                    }
                }
            }
        };
        EP.dispatchEvent = function(event) {
            if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
            if (event._initialized === false) throw new DOMException("The event is not initialized.", "InvalidStateError");
            if (this.__nid === undefined) {
                // Standalone node with no DomTree backing — fire EventTarget listeners only
                var __prevEvent = __currentEvent;
                __currentEvent = event;
                event._dispatching = true;
                event.target = this;
                event.srcElement = this;
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
                __currentEvent = __prevEvent;
                return !event.defaultPrevented;
            }
            // relatedTarget retargeting per DOM spec §2.9
            var origRelatedTarget = event.relatedTarget;
            var _hasRelatedTarget = (origRelatedTarget !== null && origRelatedTarget !== undefined);
            if (_hasRelatedTarget) {
                var rtNid = origRelatedTarget.__nid;
                var targetNid = this.__nid;
                // Retarget relatedTarget against target
                if (rtNid !== undefined) {
                    var retargetedNid = __jsRetarget(rtNid, targetNid);
                    if (retargetedNid !== rtNid) {
                        event.relatedTarget = __w(retargetedNid);
                    }
                }
            }

            // clearTargets: set if target is in a shadow tree, or origRelatedTarget IS a shadow root
            var _clearTargets = __n_isShadowRoot(__n_rootOf(this.__nid));
            if (!_clearTargets && _hasRelatedTarget && origRelatedTarget.__nid !== undefined) {
                _clearTargets = __n_isShadowRoot(origRelatedTarget.__nid);
            }

            // Skip dispatch when retargetedRelatedTarget === target but origRelatedTarget !== target
            if (_hasRelatedTarget && origRelatedTarget.__nid !== undefined) {
                var retargetedNid = __jsRetarget(origRelatedTarget.__nid, this.__nid);
                if (retargetedNid === this.__nid && origRelatedTarget !== this) {
                    // Early return: set target/relatedTarget but don't fire listeners
                    event.target = this;
                    event.relatedTarget = __w(retargetedNid);
                    if (_clearTargets && event.defaultPrevented) {
                        event.target = null;
                        event.relatedTarget = null;
                        event._path = [];
                    }
                    return !event.defaultPrevented;
                }
            }

            if (_clearTargets) {
                event._resetTargetsAfterDispatch = true;
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
            // Per spec: click() on disabled form controls is a no-op for event dispatch
            // but activation behavior (checkbox toggle) still runs
            var isDisabled = this.disabled;
            var tag = this.tagName;
            var isFormControl = (tag === 'INPUT' || tag === 'BUTTON' || tag === 'SELECT' || tag === 'TEXTAREA');
            var event = new MouseEvent('click', {bubbles: true, cancelable: true});
            event.target = this;
            event.currentTarget = this;
            if (isFormControl && isDisabled) {
                // Disabled: no event dispatch, no activation behavior from click()
                return;
            }
            __dispatch(this.__nid, event);
            // All activation behaviors (summary toggle, form submit/reset, label, anchor)
            // are handled in __dispatch post-step.
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
            var nid = this.__nid;
            // Per spec, capture the document's HTML-ness at creation time —
            // the live collection remembers whether to lowercase even if the
            // node later moves to a different document type.
            var od = this.ownerDocument;
            var isHTML = !od || !od.__isXML;
            return __makeHTMLCollection(function() {
                return __n_getElementsByTagName(nid, tag, isHTML).map(__w);
            });
        };
        ElemProto.getElementsByTagNameNS = function(ns, localName) {
            var nid = this.__nid;
            var nsStr = (ns === null || ns === undefined) ? '' : String(ns);
            var lnStr = String(localName);
            return __makeHTMLCollection(function() { return __n_getElementsByTagNameNS(nid, nsStr, lnStr).map(__w); });
        };
        ElemProto.getElementsByClassName = function(cls) {
            var self = this;
            return __makeHTMLCollection(function() { return self.querySelectorAll('.' + cls); });
        };
        Object.defineProperty(ElemProto, 'attributes', {
            get: function() {
                if (this.__nid === undefined) return undefined;
                var el = this;
                function getAttrs() {
                    var full = JSON.parse(__n_getAttributesFull(el.__nid));
                    var attrs = [];
                    for (var i = 0; i < full.length; i++) {
                        var a = full[i];
                        var attr = new Attr(a.name, a.value, a.ns, a.prefix);
                        attr.ownerElement = el;
                        attrs.push(attr);
                    }
                    return attrs;
                }
                return new Proxy(Object.create(null), {
                    get: function(t, p) {
                        var attrs = getAttrs();
                        if (p === 'length') return attrs.length;
                        if (p === 'item') return function(i) { return attrs[i] || null; };
                        if (p === 'getNamedItem') return function(n) {
                            for (var i = 0; i < attrs.length; i++) if (attrs[i].name === n) return attrs[i];
                            return null;
                        };
                        if (p === 'getNamedItemNS') return function(ns, n) {
                            for (var i = 0; i < attrs.length; i++) if (attrs[i].localName === n) return attrs[i];
                            return null;
                        };
                        if (p === 'setNamedItem') return function(a) { if (a && a.ownerElement) a.ownerElement.setAttribute(a.name, a.value); };
                        if (p === 'removeNamedItem') return function(n) { el.removeAttribute(n); };
                        if (p === Symbol.iterator) return function() { return attrs[Symbol.iterator](); };
                        if (__isArrayIndex(p)) return attrs[p >>> 0];
                        // Named access
                        if (typeof p === 'string') {
                            for (var i = 0; i < attrs.length; i++) if (attrs[i].name === p) return attrs[i];
                        }
                        return undefined;
                    },
                    ownKeys: function() {
                        var attrs = getAttrs();
                        var keys = [];
                        for (var i = 0; i < attrs.length; i++) keys.push(String(i));
                        for (var i = 0; i < attrs.length; i++) keys.push(attrs[i].name);
                        return keys;
                    },
                    getOwnPropertyDescriptor: function(t, p) {
                        var attrs = getAttrs();
                        if (__isArrayIndex(p)) {
                            var idx = p >>> 0;
                            if (idx < attrs.length) return { value: attrs[idx], writable: false, enumerable: true, configurable: true };
                            return undefined;
                        }
                        if (typeof p === 'string') {
                            for (var i = 0; i < attrs.length; i++) {
                                if (attrs[i].name === p) return { value: attrs[i], writable: false, enumerable: false, configurable: true };
                            }
                        }
                        return undefined;
                    }
                });
            },
            enumerable: true, configurable: true
        });
        EP.contains = function(other) {
            if (!other || other.__nid === undefined) return false;
            return __n_contains(this.__nid, other.__nid);
        };
        EP.cloneNode = function(deep) {
            var nid = __n_cloneNode(this.__nid, !!deep);
            var clone = __w(nid);
            // Copy namespace metadata that lives on JS wrappers
            if (this.__localName !== undefined) clone.__localName = this.__localName;
            if (this.__prefix !== undefined) clone.__prefix = this.__prefix;
            if (this.__namespaceURI !== undefined) clone.__namespaceURI = this.__namespaceURI;
            if (this.__ownerDoc !== undefined) clone.__ownerDoc = this.__ownerDoc;
            return clone;
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
                // CE lifecycle: disconnect old child
                var wasConnected = typeof __ceDisconnected === 'function' && __isConnected(this.__nid);
                if (wasConnected) __ceDisconnected(oldChild);
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
                // CE lifecycle: connect new child + upgrade
                if (wasConnected && typeof __ceConnected === 'function') __ceConnected(newChild);
                if (typeof __ceUpgradeTree === 'function') __ceUpgradeTree(newChild);
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
            if (this.__nid === undefined) return {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
            var json = __n_getLayout(this.__nid);
            if (!json) return {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
            var l = JSON.parse(json);
            return {x:l.x, y:l.y, width:l.width, height:l.height,
                    top:l.y, left:l.x, right:l.x+l.width, bottom:l.y+l.height};
        };
        ElemProto.getClientRects = function() { return [this.getBoundingClientRect()]; };
        // focus/blur defined later after defineProperties to track activeElement
        ElemProto.scrollIntoView = function() {};
        ElemProto.animate = function(keyframes, options) {
            return { finished: Promise.resolve(), cancel: function(){}, play: function(){}, pause: function(){} };
        };
        function __computeSnapNearest(arr, target) {
            var best = arr[0], bestDist = Math.abs(arr[0] - target);
            for (var i = 1; i < arr.length; i++) {
                var d = Math.abs(arr[i] - target);
                if (d < bestDist) { best = arr[i]; bestDist = d; }
            }
            return best;
        }
        function __computeSnapOffset(scroller, targetX, targetY) {
            var snapType = __n_getComputedStyle(scroller.__nid, 'scroll-snap-type');
            if (!snapType || snapType === 'none') return { x: targetX, y: targetY };

            var parts = snapType.split(/\s+/);
            var axis = parts[0];
            var strictness = parts[1] || 'proximity';
            var snapX = (axis === 'x' || axis === 'both' || axis === 'inline');
            var snapY = (axis === 'y' || axis === 'both' || axis === 'block');

            var scrollerRect = scroller.getBoundingClientRect();
            var snapPointsX = [], snapPointsY = [];
            var children = scroller.children;
            for (var i = 0; i < children.length; i++) {
                var child = children[i];
                var align = __n_getComputedStyle(child.__nid, 'scroll-snap-align');
                if (!align || align === 'none') continue;

                var childRect = child.getBoundingClientRect();
                // Our layout engine reports content-space positions (not
                // viewport-adjusted for scroll), so no scroll offset needed.
                var relLeft = childRect.left - scrollerRect.left;
                var relTop = childRect.top - scrollerRect.top;

                var alignParts = align.split(/\s+/);
                var alignX = alignParts.length > 1 ? alignParts[1] : alignParts[0];
                var alignY = alignParts[0];

                if (snapX) {
                    if (alignX === 'start') snapPointsX.push(relLeft);
                    else if (alignX === 'center') snapPointsX.push(relLeft + childRect.width/2 - scrollerRect.width/2);
                    else if (alignX === 'end') snapPointsX.push(relLeft + childRect.width - scrollerRect.width);
                }
                if (snapY) {
                    if (alignY === 'start') snapPointsY.push(relTop);
                    else if (alignY === 'center') snapPointsY.push(relTop + childRect.height/2 - scrollerRect.height/2);
                    else if (alignY === 'end') snapPointsY.push(relTop + childRect.height - scrollerRect.height);
                }
            }

            var resultX = targetX, resultY = targetY;
            if (snapX && snapPointsX.length > 0) {
                resultX = __computeSnapNearest(snapPointsX, targetX);
                if (strictness === 'proximity') {
                    if (Math.abs(resultX - targetX) > scrollerRect.width / 2) resultX = targetX;
                }
            }
            if (snapY && snapPointsY.length > 0) {
                resultY = __computeSnapNearest(snapPointsY, targetY);
                if (strictness === 'proximity') {
                    if (Math.abs(resultY - targetY) > scrollerRect.height / 2) resultY = targetY;
                }
            }
            return { x: resultX, y: resultY };
        }

        ElemProto.scrollTo = function(xOrOpts, y) {
            var nx, ny, behavior;
            if (typeof xOrOpts === 'object' && xOrOpts !== null) {
                nx = ('left' in xOrOpts) ? Number(xOrOpts.left) : this.scrollLeft;
                ny = ('top' in xOrOpts) ? Number(xOrOpts.top) : this.scrollTop;
                behavior = xOrOpts.behavior || 'auto';
            } else {
                nx = Number(xOrOpts) || 0;
                ny = Number(y) || 0;
                behavior = 'auto';
            }
            var snapped = __computeSnapOffset(this, nx, ny);
            nx = snapped.x;
            ny = snapped.y;
            if (behavior === 'smooth') {
                // Smooth scroll: animate toward target, cancel if element removed
                var el = this;
                var startLeft = el.scrollLeft;
                var startTop = el.scrollTop;
                var targetLeft = nx;
                var targetTop = ny;
                // Cancel any in-flight smooth scroll
                if (el.__smoothScrollRaf) { cancelAnimationFrame(el.__smoothScrollRaf); el.__smoothScrollRaf = null; }
                var startTime = performance.now();
                var duration = 100; // short duration for test environments
                function step() {
                    if (!el.isConnected && el !== document.documentElement && el !== document.scrollingElement) {
                        // Element removed from DOM — cancel, no scrollend
                        el.__smoothScrollRaf = null;
                        return;
                    }
                    var elapsed = performance.now() - startTime;
                    var t = Math.min(elapsed / duration, 1);
                    // ease-out
                    var ease = 1 - (1 - t) * (1 - t);
                    var curLeft = startLeft + (targetLeft - startLeft) * ease;
                    var curTop = startTop + (targetTop - startTop) * ease;
                    // Set without re-triggering smooth scroll; use direct prop set
                    if (!el.__props) el.__props = {};
                    var oldLeft = el.__props._scrollLeft || 0;
                    var oldTop = el.__props._scrollTop || 0;
                    // Clamp
                    var cl = Math.max(0, curLeft);
                    var ct = Math.max(0, curTop);
                    var maxL = el.scrollWidth - el.clientWidth;
                    if (maxL > 0 && cl > maxL) cl = maxL;
                    var maxT = el.scrollHeight - el.clientHeight;
                    if (maxT > 0 && ct > maxT) ct = maxT;
                    el.__props._scrollLeft = cl;
                    el.__props._scrollTop = ct;
                    if (oldLeft !== cl || oldTop !== ct) {
                        var isRoot = (el === document.scrollingElement);
                        var evTarget = isRoot ? document : el;
                        evTarget.dispatchEvent(new Event('scroll', {bubbles: isRoot}));
                    }
                    if (t < 1) {
                        el.__smoothScrollRaf = requestAnimationFrame(step);
                    } else {
                        el.__smoothScrollRaf = null;
                        var isRoot = (el === document.scrollingElement);
                        var evTarget = isRoot ? document : el;
                        evTarget.dispatchEvent(new Event('scrollend', {bubbles: isRoot}));
                    }
                }
                el.__smoothScrollRaf = requestAnimationFrame(step);
            } else {
                // Auto scroll: bypass setters to avoid per-axis scrollend
                if (!this.__props) this.__props = {};
                var oldL = this.__props._scrollLeft || 0;
                var oldT = this.__props._scrollTop || 0;
                var newL = Number(nx) || 0;
                var newT = Number(ny) || 0;
                if (newL < 0) newL = 0;
                if (newT < 0) newT = 0;
                var maxL = this.scrollWidth - this.clientWidth;
                var maxT = this.scrollHeight - this.clientHeight;
                if (maxL > 0 && newL > maxL) newL = maxL;
                if (maxT > 0 && newT > maxT) newT = maxT;
                this.__props._scrollLeft = newL;
                this.__props._scrollTop = newT;
                if (newL !== oldL || newT !== oldT) {
                    var isRoot = (this === document.scrollingElement);
                    var evTarget = isRoot ? document : this;
                    evTarget.dispatchEvent(new Event('scroll', {bubbles: isRoot}));
                    evTarget.dispatchEvent(new Event('scrollend', {bubbles: isRoot}));
                }
            }
        };
        ElemProto.scroll = ElemProto.scrollTo;
        ElemProto.scrollBy = function(xOrOpts, y) {
            var dx, dy;
            if (typeof xOrOpts === 'object' && xOrOpts !== null) {
                dx = xOrOpts.left || 0;
                dy = xOrOpts.top || 0;
            } else {
                dx = xOrOpts || 0;
                dy = y || 0;
            }
            this.scrollTo(this.scrollLeft + dx, this.scrollTop + dy);
        };
        ElemProto.setSelectionRange = function(start, end, direction) {
            if (!this.__props) this.__props = {};
            var len = (this.value || '').length;
            this.__props._selStart = Math.max(0, Math.min(start|0, len));
            this.__props._selEnd = Math.max(this.__props._selStart, Math.min(end|0, len));
        };
        ElemProto.select = function() {
            if (this.tagName === 'INPUT' || this.tagName === 'TEXTAREA') {
                this.setSelectionRange(0, (this.value || '').length);
            }
        };
        ElemProto.matches = function(sel) { return __n_matchesSelector(this.__nid, sel); };
        ElemProto.webkitMatchesSelector = ElemProto.matches;
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
            var p = String(position).toLowerCase();
            var temp = document.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            var frag = document.createDocumentFragment();
            while (temp.firstChild) frag.appendChild(temp.firstChild);
            if (p === 'beforebegin') this.before(frag);
            else if (p === 'afterbegin') this.prepend(frag);
            else if (p === 'beforeend') this.append(frag);
            else if (p === 'afterend') this.after(frag);
            else throw new DOMException("Failed to execute 'insertAdjacentHTML' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.", "SyntaxError");
        };
        ElemProto.insertAdjacentText = function(position, text) {
            var p = String(position).toLowerCase();
            var node = document.createTextNode(text);
            if (p === 'beforebegin') this.before(node);
            else if (p === 'afterbegin') this.prepend(node);
            else if (p === 'beforeend') this.append(node);
            else if (p === 'afterend') this.after(node);
            else throw new DOMException("Failed to execute 'insertAdjacentText' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.", "SyntaxError");
        };
        ElemProto.insertAdjacentElement = function(position, el) {
            var p = String(position).toLowerCase();
            if (p === 'beforebegin') this.before(el);
            else if (p === 'afterbegin') this.prepend(el);
            else if (p === 'beforeend') this.append(el);
            else if (p === 'afterend') this.after(el);
            else throw new DOMException("Failed to execute 'insertAdjacentElement' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.", "SyntaxError");
            return el;
        };
        ElemProto.getAnimations = function() { return []; };
        ElemProto.animate = function() {
            var anim = { finished: Promise.resolve(), cancel: function(){}, play: function(){}, pause: function(){}, onfinish: null };
            anim.finish = function() { if (typeof anim.onfinish === 'function') anim.onfinish(); };
            return anim;
        };
        ElemProto.attachShadow = function(opts) {
            if (!opts || (opts.mode !== 'open' && opts.mode !== 'closed')) {
                throw new TypeError("Failed to execute 'attachShadow' on 'Element': The provided value '" + (opts && opts.mode) + "' is not a valid enum value of type ShadowRootMode.");
            }
            if (__n_hasShadowRoot(this.__nid)) {
                throw new DOMException("Failed to execute 'attachShadow' on 'Element': Shadow root cannot be created on a host which already hosts a shadow tree.", "NotSupportedError");
            }
            var tag = this.tagName;
            // Valid shadow hosts: custom elements (hyphen in name) or specific built-in elements
            var validHosts = ['ARTICLE','ASIDE','BLOCKQUOTE','BODY','DIV','FOOTER','H1','H2','H3','H4','H5','H6','HEADER','MAIN','NAV','P','SECTION','SPAN'];
            if (tag.indexOf('-') === -1 && validHosts.indexOf(tag) === -1) {
                throw new DOMException("Failed to execute 'attachShadow' on 'Element': This element does not support attachShadow", "NotSupportedError");
            }
            var shadowId = __n_createShadowRoot(this.__nid, opts.mode);
            var shadow = __w(shadowId);
            shadow._shadowHost = this;
            if (opts.mode === 'open') {
                this.shadowRoot = shadow;
            }
            return shadow;
        };
        ElemProto.getAttributeNode = function(name) {
            if (!this.hasAttribute(name)) return null;
            var val = this.getAttribute(name);
            var attr = new Attr(name, val);
            attr.ownerElement = this;
            return attr;
        };
        ElemProto.getAttributeNodeNS = function(ns, localName) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            var json = __n_getAttributeNodeNS(this.__nid, ns, String(localName));
            if (!json) return null;
            var info = JSON.parse(json);
            var qualName = info.prefix ? (info.prefix + ':' + info.localName) : info.localName;
            var attr = new Attr(qualName, info.value, info.namespace || null, info.prefix || null);
            attr.localName = info.localName;
            attr.ownerElement = this;
            return attr;
        };
        EP.remove = function() {
            if (this.__nid !== undefined) {
                var pid = __n_getParent(this.__nid);
                if (pid >= 0) __n_removeChild(pid, this.__nid);
            } else if (this.parentNode && this.parentNode.removeChild) {
                this.parentNode.removeChild(this);
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
                set: function(v) {
                    if (this.__nid === undefined) return;
                    // For element nodes, capture children for MO notification
                    var removedNodes = [];
                    var isElement = (this.nodeType === 1);
                    if (isElement && typeof __mo_notify === 'function') {
                        var kids = this.childNodes;
                        for (var i = 0; i < kids.length; i++) removedNodes.push(kids[i]);
                    }
                    __n_setTextContent(this.__nid, String(v));
                    if (isElement && typeof __mo_notify === 'function') {
                        var addedNodes = [];
                        var newKids = this.childNodes;
                        for (var i = 0; i < newKids.length; i++) addedNodes.push(newKids[i]);
                        if (removedNodes.length > 0 || addedNodes.length > 0) {
                            __mo_notify('childList', this, {removedNodes: removedNodes, addedNodes: addedNodes});
                        }
                    }
                },
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
                get: function() {
                    if (this.__nid === undefined) return [];
                    var self = this;
                    return __makeHTMLCollection(function() { return __n_getChildElementIds(self.__nid).map(__w); });
                },
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
            firstElementChild: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var kids = __n_getChildElementIds(this.__nid);
                    return kids.length > 0 ? __w(kids[0]) : null;
                },
                configurable: true
            },
            lastElementChild: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var kids = __n_getChildElementIds(this.__nid);
                    return kids.length > 0 ? __w(kids[kids.length - 1]) : null;
                },
                configurable: true
            },
            childElementCount: {
                get: function() {
                    if (this.__nid === undefined) return 0;
                    return __n_getChildElementIds(this.__nid).length;
                },
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
            nextElementSibling: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var id = __n_getNextSibling(this.__nid);
                    while (id >= 0) {
                        if (__n_getNodeType(id) === 1) return __w(id);
                        id = __n_getNextSibling(id);
                    }
                    return null;
                },
                configurable: true
            },
            previousElementSibling: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var id = __n_getPrevSibling(this.__nid);
                    while (id >= 0) {
                        if (__n_getNodeType(id) === 1) return __w(id);
                        id = __n_getPrevSibling(id);
                    }
                    return null;
                },
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
            ownerDocument: { get: function() { return this.__ownerDoc || document; }, configurable: true },
            isConnected: {
                get: function() {
                    if (this.__nid === undefined) return false;
                    return __isConnected(this.__nid);
                },
                configurable: true
            },
        });

        // === Element-specific properties (on ElemProto) ===
        Object.defineProperties(ElemProto, {
            tagName: { get: function() {
                var prefix = this.prefix;
                var ln = this.localName;
                if (ln === undefined || ln === null) return __n_getTagName(this.__nid);
                var tn = prefix ? prefix + ':' + ln : ln;
                // HTML elements in HTML documents get uppercased tagName
                // In XML documents (contentType !== 'text/html'), preserve case
                if (this.namespaceURI === 'http://www.w3.org/1999/xhtml' || (this.__namespaceURI === undefined && !prefix)) {
                    var od = this.__ownerDoc || (typeof document !== 'undefined' ? document : null);
                    if (od && od.contentType && od.contentType !== 'text/html') return tn;
                    return tn.toUpperCase();
                }
                return tn;
            }, configurable: true },
            localName: { get: function() {
                if (this.__localName !== undefined) return this.__localName;
                if (this.__nid !== undefined) {
                    var ln = __n_getLocalName(this.__nid);
                    if (ln) return ln;
                }
                return null;
            }, set: function(v) { this.__localName = v; }, configurable: true },
            prefix: { get: function() {
                return (this.__prefix !== undefined) ? this.__prefix : null;
            }, set: function(v) { this.__prefix = v; }, configurable: true },
            namespaceURI: { get: function() {
                return (this.__namespaceURI !== undefined) ? this.__namespaceURI : 'http://www.w3.org/1999/xhtml';
            }, set: function(v) { this.__namespaceURI = v; }, configurable: true },
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
                    if (this.__props && this.__props._value !== undefined) {
                        return this.__props._value;
                    }
                    if (this.tagName === 'SELECT') {
                        var opts = this.querySelectorAll('option');
                        for (var i = 0; i < opts.length; i++) {
                            if ((opts[i].__props && opts[i].__props._selected) || opts[i].hasAttribute('selected')) {
                                return opts[i].getAttribute('value') || opts[i].textContent || '';
                            }
                        }
                        return opts.length > 0 ? (opts[0].getAttribute('value') || opts[0].textContent || '') : '';
                    }
                    if (this.tagName === 'TEXTAREA') {
                        var tc = this.textContent;
                        return tc || '';
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
            type: {
                get: function() { return this.getAttribute('type') || ''; },
                set: function(v) { this.setAttribute('type', String(v)); },
                configurable: true
            },
            disabled: {
                get: function() { return this.hasAttribute('disabled'); },
                set: function(v) { if (v) this.setAttribute('disabled', ''); else this.removeAttribute('disabled'); },
                configurable: true
            },
            form: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var cur = __n_getParent(this.__nid);
                    while (cur >= 0) {
                        var w = __w(cur);
                        if (w.tagName === 'FORM') return w;
                        cur = __n_getParent(cur);
                    }
                    return null;
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
                get: function() {
                    var raw = this.getAttribute('href');
                    if (raw === null) return '';
                    // <a> and <area> resolve href to absolute URL per spec
                    if (this.tagName === 'A' || this.tagName === 'AREA') {
                        var resolved;
                        if (/^https?:\/\//.test(raw)) resolved = raw;
                        else if (raw.charAt(0) === '#') resolved = location.origin + location.pathname + location.search + raw;
                        else if (raw.charAt(0) === '?') resolved = location.origin + location.pathname + raw;
                        else if (raw.charAt(0) === '/') resolved = location.origin + raw;
                        else resolved = location.origin + location.pathname.replace(/[^\/]*$/, '') + raw;
                        // Percent-encode non-ASCII characters (UTF-8)
                        return resolved.replace(/[\u0080-\uFFFF]/g, function(ch) {
                            var code = ch.charCodeAt(0);
                            var bytes = [];
                            if (code < 0x800) {
                                bytes.push(0xC0 | (code >> 6), 0x80 | (code & 0x3F));
                            } else {
                                bytes.push(0xE0 | (code >> 12), 0x80 | ((code >> 6) & 0x3F), 0x80 | (code & 0x3F));
                            }
                            return bytes.map(function(b) { return '%' + b.toString(16).toUpperCase(); }).join('');
                        });
                    }
                    return raw;
                },
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
                set: function(v) {
                    // Capture existing children for MO notification
                    var removedNodes = [];
                    if (typeof __mo_notify === 'function') {
                        var kids = this.childNodes;
                        for (var i = 0; i < kids.length; i++) removedNodes.push(kids[i]);
                    }
                    __n_setInnerHTML(this.__nid, String(v));
                    // Upgrade custom elements in new content
                    if (typeof __ceUpgradeTree === 'function') __ceUpgradeTree(this);
                    // Fire MO childList notification
                    if (typeof __mo_notify === 'function') {
                        var addedNodes = [];
                        var newKids = this.childNodes;
                        for (var i = 0; i < newKids.length; i++) addedNodes.push(newKids[i]);
                        if (removedNodes.length > 0 || addedNodes.length > 0) {
                            __mo_notify('childList', this, {removedNodes: removedNodes, addedNodes: addedNodes});
                        }
                    }
                },
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
                set: function(v) {
                    var s = String(v);
                    if (s) __n_setAttribute(this.__nid, 'style', s);
                    else __n_removeAttribute(this.__nid, 'style');
                    this._s = undefined;
                },
                configurable: true
            },
            classList: {
                get: function() {
                    var el = this;
                    function _tokens() { var raw=(el.getAttribute('class')||'').split(/\s+/).filter(Boolean),seen={},out=[]; for(var i=0;i<raw.length;i++){if(!seen[raw[i]]){seen[raw[i]]=true;out.push(raw[i]);}} return out; }
                    if (!this.__classList) {
                        var obj = Object.create(DOMTokenList.prototype);
                        function _update(c) { if(el.hasAttribute('class')||c.length>0) el.setAttribute('class',c.join(' ')); obj._sync(); }
                        obj.add = function() { for(var i=0;i<arguments.length;i++) _validateToken(String(arguments[i])); var c=_tokens(); for(var i=0;i<arguments.length;i++){var s=String(arguments[i]);if(c.indexOf(s)<0) c.push(s);} _update(c); };
                        obj.remove = function() { for(var i=0;i<arguments.length;i++) _validateToken(String(arguments[i])); var c=_tokens(); for(var i=0;i<arguments.length;i++){var s=String(arguments[i]);var idx=c.indexOf(s);if(idx>=0)c.splice(idx,1);} _update(c); };
                        obj.contains = function(cls) { return _tokens().indexOf(String(cls))>=0; };
                        obj.toggle = function(cls,force) { _validateToken(String(cls)); if(force!==undefined){if(force){var c=_tokens();if(c.indexOf(String(cls))<0){c.push(String(cls));_update(c);}return true;}else{var c=_tokens();var idx=c.indexOf(String(cls));if(idx>=0){c.splice(idx,1);_update(c);}return false;}} var c=_tokens();var idx=c.indexOf(String(cls));if(idx>=0){c.splice(idx,1);_update(c);return false;}c.push(String(cls));_update(c);return true; };
                        function _validateToken(t) { if(t==='') throw new DOMException("The token provided must not be empty.","SyntaxError"); if(/\s/.test(t)) throw new DOMException("The token provided ('"+t+"') contains HTML space characters, which are not valid in tokens.","InvalidCharacterError"); }
                        obj.replace = function(oldToken, newToken) { var o=String(oldToken),n=String(newToken); _validateToken(o); _validateToken(n); var c=_tokens(); if(c.indexOf(o)<0) return false; var first=-1; for(var i=0;i<c.length;i++){if(c[i]===o||c[i]===n){first=i;break;}} c[first]=n; for(var i=c.length-1;i>=0;i--){if(i!==first&&(c[i]===o||c[i]===n))c.splice(i,1);} _update(c); return true; };
                        obj.item = function(i) { var c=_tokens(); return (i>=0&&i<c.length)?c[i]:null; };
                        obj.toString = function() { return el.getAttribute('class')||''; };
                        Object.defineProperty(obj, 'value', { get: function() { return el.getAttribute('class')||''; }, set: function(v) { el.setAttribute('class', v); obj._sync(); }, configurable: true });
                        obj._sync = function() {
                            var c = _tokens();
                            // Remove old indexed properties beyond new length
                            for (var i = c.length; i < (obj.length || 0); i++) delete obj[i];
                            obj.length = c.length;
                            for (var i = 0; i < c.length; i++) obj[i] = c[i];
                        };
                        obj._sync();
                        this.__classList = obj;
                    } else {
                        this.__classList._sync();
                    }
                    return this.__classList;
                },
                set: function(v) {
                    var cl = this.classList;
                    cl.value = String(v);
                },
                configurable: true
            },
            dataset: {
                get: function() {
                    var el = this;
                    function dataKeys() {
                        var names = JSON.parse(__n_getAttributeNames(el.__nid));
                        var keys = [];
                        for (var i = 0; i < names.length; i++) {
                            if (names[i].indexOf('data-') === 0) {
                                var rest = names[i].substring(5);
                                var camel = rest.replace(/-([a-z])/g, function(m, c) { return c.toUpperCase(); });
                                keys.push(camel);
                            }
                        }
                        return keys;
                    }
                    return new Proxy({}, {
                        get: function(t, prop) {
                            if (typeof prop !== 'string') return undefined;
                            return __n_getDataAttr(el.__nid, prop) || undefined;
                        },
                        set: function(t, prop, val) {
                            var name = 'data-' + prop.replace(/[A-Z]/g, function(c){return '-'+c.toLowerCase();});
                            __n_setAttribute(el.__nid, name, String(val));
                            return true;
                        },
                        ownKeys: function() {
                            return dataKeys();
                        },
                        getOwnPropertyDescriptor: function(t, prop) {
                            var val = __n_getDataAttr(el.__nid, prop);
                            if (val !== '') return { value: val, writable: true, enumerable: true, configurable: true };
                            var keys = dataKeys();
                            if (keys.indexOf(prop) !== -1) return { value: val, writable: true, enumerable: true, configurable: true };
                            return undefined;
                        }
                    });
                },
                configurable: true
            },
            selectionStart: {
                get: function() {
                    var t = this.tagName;
                    if (t !== 'INPUT' && t !== 'TEXTAREA') return undefined;
                    if (this.__props && this.__props._selStart !== undefined) return this.__props._selStart;
                    return 0;
                },
                set: function(val) {
                    if (!this.__props) this.__props = {};
                    this.__props._selStart = Math.max(0, Math.min(val|0, (this.value||'').length));
                },
                configurable: true
            },
            selectionEnd: {
                get: function() {
                    var t = this.tagName;
                    if (t !== 'INPUT' && t !== 'TEXTAREA') return undefined;
                    if (this.__props && this.__props._selEnd !== undefined) return this.__props._selEnd;
                    return 0;
                },
                set: function(val) {
                    if (!this.__props) this.__props = {};
                    this.__props._selEnd = Math.max(0, Math.min(val|0, (this.value||'').length));
                },
                configurable: true
            },
            scrollTop: {
                get: function() {
                    return (this.__props && this.__props._scrollTop) || 0;
                },
                set: function(val) {
                    if (!this.__props) this.__props = {};
                    var v = Number(val) || 0;
                    if (v < 0) v = 0;
                    var maxScroll = this.scrollHeight - this.clientHeight;
                    if (maxScroll > 0 && v > maxScroll) v = maxScroll;
                    var old = this.__props._scrollTop || 0;
                    this.__props._scrollTop = v;
                    if (old !== v) {
                        var isRoot = (this === document.scrollingElement);
                        var target = isRoot ? document : this;
                        target.dispatchEvent(new Event('scroll', {bubbles: isRoot}));
                        // Fire scrollend asynchronously (matches browser behavior)
                        setTimeout(function() {
                            target.dispatchEvent(new Event('scrollend', {bubbles: isRoot}));
                        }, 0);
                    }
                },
                configurable: true
            },
            scrollLeft: {
                get: function() {
                    return (this.__props && this.__props._scrollLeft) || 0;
                },
                set: function(val) {
                    if (!this.__props) this.__props = {};
                    var v = Number(val) || 0;
                    if (v < 0) v = 0;
                    var maxScroll = this.scrollWidth - this.clientWidth;
                    if (maxScroll > 0 && v > maxScroll) v = maxScroll;
                    var old = this.__props._scrollLeft || 0;
                    this.__props._scrollLeft = v;
                    if (old !== v) {
                        var isRoot = (this === document.scrollingElement);
                        var target = isRoot ? document : this;
                        target.dispatchEvent(new Event('scroll', {bubbles: isRoot}));
                        // Fire scrollend asynchronously (matches browser behavior)
                        setTimeout(function() {
                            target.dispatchEvent(new Event('scrollend', {bubbles: isRoot}));
                        }, 0);
                    }
                },
                configurable: true
            },
            scrollWidth: {
                get: function() {
                    var myRect = this.getBoundingClientRect();
                    var w = myRect.width;
                    // For input/textarea, text content may be wider than element
                    var tag = this.tagName;
                    if (tag === 'INPUT' || tag === 'TEXTAREA') {
                        var textWidth = ((this.value || '').length) * 8;
                        if (textWidth > w) w = textWidth;
                    }
                    var children = this.children;
                    if (children) {
                        for (var i = 0; i < children.length; i++) {
                            var cr = children[i].getBoundingClientRect();
                            var right = cr.left - myRect.left + cr.width;
                            if (right > w) w = right;
                        }
                    }
                    return w;
                },
                configurable: true
            },
            scrollHeight: {
                get: function() {
                    var myRect = this.getBoundingClientRect();
                    var h = myRect.height;
                    var children = this.children;
                    if (children) {
                        for (var i = 0; i < children.length; i++) {
                            var cr = children[i].getBoundingClientRect();
                            var bottom = cr.top - myRect.top + cr.height;
                            if (bottom > h) h = bottom;
                        }
                    }
                    return h;
                },
                configurable: true
            },
            offsetTop: { get: function() { return this.getBoundingClientRect().top; }, configurable: true },
            offsetLeft: { get: function() { return this.getBoundingClientRect().left; }, configurable: true },
            offsetWidth: { get: function() { return this.getBoundingClientRect().width; }, configurable: true },
            offsetHeight: { get: function() { return this.getBoundingClientRect().height; }, configurable: true },
            clientWidth: { get: function() { if (this.tagName === 'HTML') return 1280; return this.getBoundingClientRect().width; }, configurable: true },
            clientHeight: { get: function() { if (this.tagName === 'HTML') return 800; return this.getBoundingClientRect().height; }, configurable: true },
            clientTop: { get: function() { return 0; }, configurable: true },
            clientLeft: { get: function() { return 0; }, configurable: true },
            offsetParent: { get: function() { return this.parentNode; }, configurable: true },
            content: { get: function() {
                if (this.tagName !== 'TEMPLATE') return undefined;
                var cid = __n_getTemplateContent(this.__nid);
                if (cid < 0) return undefined;
                return __w(cid);
            }, configurable: true },
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
