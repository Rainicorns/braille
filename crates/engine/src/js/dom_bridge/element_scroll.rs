/// Scroll, geometry, and DOM convenience methods:
/// getBoundingClientRect, getClientRects, scrollIntoView, scrollTo/scroll/scrollBy,
/// setSelectionRange, select, matches, closest, getAttributeNames,
/// append/prepend/replaceChildren, after/before/replaceWith, toggleAttribute,
/// setAttributeNS/getAttributeNS/removeAttributeNS/hasAttributeNS,
/// insertAdjacentHTML/Text/Element, getAnimations, animate, attachShadow.
pub(super) fn element_scroll_js() -> &'static str {
    r#"
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
        // animate: better version with onfinish/finish defined later (~line 964)
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
        // Expose for Rust-side fire_scroll_snap_events
        globalThis.__computeSnapOffset = __computeSnapOffset;

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
            var frag = document.createDocumentFragment();
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                frag.appendChild(arg);
            }
            var err = __n_validatePreInsert(this.__nid, frag.__nid, -1);
            if (err) {
                var parts = err.split(':');
                throw new DOMException(parts.slice(1).join(':'), parts[0]);
            }
            __ceBatchDepth++;
            this.appendChild(frag);
            __ceBatchDepth--;
            __ceFlushReactions();
        };
        EP.prepend = function() {
            var frag = document.createDocumentFragment();
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                frag.appendChild(arg);
            }
            var first = this.firstChild;
            var err = __n_validatePreInsert(this.__nid, frag.__nid, first ? first.__nid : -1);
            if (err) {
                var parts = err.split(':');
                throw new DOMException(parts.slice(1).join(':'), parts[0]);
            }
            __ceBatchDepth++;
            if (first) this.insertBefore(frag, first);
            else this.appendChild(frag);
            __ceBatchDepth--;
            __ceFlushReactions();
        };
        EP.replaceChildren = function() {
            var frag = document.createDocumentFragment();
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (arg === null || arg === undefined || typeof arg !== 'object' || arg.__nid === undefined) arg = document.createTextNode(String(arg));
                frag.appendChild(arg);
            }
            var err = __n_validatePreInsert(this.__nid, frag.__nid, -1);
            if (err) {
                var parts = err.split(':');
                throw new DOMException(parts.slice(1).join(':'), parts[0]);
            }
            __ceBatchDepth++;
            while (this.firstChild) this.removeChild(this.firstChild);
            this.appendChild(frag);
            __ceBatchDepth--;
            __ceFlushReactions();
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
            name = String(name);
            if (name === '') throw new DOMException("The string contains invalid characters.", "InvalidCharacterError");
            name = __attrName(this, name);
            if (force !== undefined) {
                if (force) { this.setAttribute(name, ''); return true; }
                else { this.removeAttribute(name); return false; }
            }
            if (this.hasAttribute(name)) { this.removeAttribute(name); return false; }
            this.setAttribute(name, ''); return true;
        };
        ElemProto.setAttributeNS = function(ns, qualifiedName, value) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            qualifiedName = String(qualifiedName);
            var result = JSON.parse(__n_validateAndExtract(ns, qualifiedName));
            if (result.err) {
                var eName = result.err;
                throw new DOMException("Failed to execute 'setAttributeNS' on 'Element': " + (eName === 'InvalidCharacterError' ? "'" + qualifiedName + "' is not a valid attribute name." : "The namespace provided has an error."), eName);
            }
            __n_setAttributeNS(this.__nid, ns, qualifiedName, String(value));
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
            if (p === 'beforebegin') {
                if (!this.parentNode) throw new DOMException("The element has no parent.", "NoModificationAllowedError");
                this.before(frag);
            } else if (p === 'afterbegin') this.prepend(frag);
            else if (p === 'beforeend') this.append(frag);
            else if (p === 'afterend') {
                if (!this.parentNode) throw new DOMException("The element has no parent.", "NoModificationAllowedError");
                this.after(frag);
            }
            else throw new DOMException("Failed to execute 'insertAdjacentHTML' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.", "SyntaxError");
        };
        ElemProto.insertAdjacentText = function(position, text) {
            var p = String(position).toLowerCase();
            var node = document.createTextNode(text);
            if (p === 'beforebegin') {
                if (!this.parentNode) throw new DOMException("The element has no parent.", "HierarchyRequestError");
                this.before(node);
            } else if (p === 'afterbegin') this.prepend(node);
            else if (p === 'beforeend') this.append(node);
            else if (p === 'afterend') {
                if (!this.parentNode) throw new DOMException("The element has no parent.", "HierarchyRequestError");
                this.after(node);
            }
            else throw new DOMException("Failed to execute 'insertAdjacentText' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.", "SyntaxError");
        };
        ElemProto.insertAdjacentElement = function(position, el) {
            var p = String(position).toLowerCase();
            if (p === 'beforebegin') {
                if (!this.parentNode) return null;
                this.before(el);
            } else if (p === 'afterbegin') this.prepend(el);
            else if (p === 'beforeend') this.append(el);
            else if (p === 'afterend') {
                if (!this.parentNode) return null;
                this.after(el);
            }
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
            var tag = this.localName;
            // Valid shadow hosts: custom elements (hyphen in name) or specific built-in elements
            var validHosts = ['article','aside','blockquote','body','div','footer','h1','h2','h3','h4','h5','h6','header','main','nav','p','section','span'];
            if (tag.indexOf('-') === -1 && validHosts.indexOf(tag) === -1) {
                throw new DOMException("Failed to execute 'attachShadow' on 'Element': This element does not support attachShadow", "NotSupportedError");
            }
            var shadowId = __n_createShadowRoot(this.__nid, opts.mode);
            var shadow = __w(shadowId);
            shadow._shadowHost = this;
            shadow.__ownerDoc = this.ownerDocument || (this.nodeType === 9 ? this : document);
            return shadow;
        };
    "#
}
