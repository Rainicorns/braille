/// Event dispatch engine: capture/bubble phases, click sequence, script loading,
/// and helper functions (__throwValidationError, __adoptSubtree).
pub(super) fn event_dispatch_js() -> &'static str {
    r#"
        // Event dispatch with capture + bubble phases
        // ownerDoc: optional non-global document that owns the target element
        function __dispatch(nodeId, event, ownerDoc) {
            // Build path: target -> parent -> ... -> root
            // For composed events, follow shadow host links across shadow boundaries
            // For iframe content, stop at the IFRAME boundary to prevent leaking
            var path = [];
            var _shadowNodes = {};  // nodeIds that are inside a shadow tree
            var _inShadow = false;
            var cur = nodeId;
            while (cur >= 0) {
                path.push(cur);
                if (_inShadow) _shadowNodes[cur] = true;
                var parent = __n_getParent(cur);
                // Stop at IFRAME boundary — events inside iframe don't propagate to outer document
                if (parent >= 0 && __n_getNodeType(parent) === 1 && __n_getTagName(parent) === 'IFRAME') {
                    break;
                }
                if (parent < 0 && event.composed) {
                    // Check if this is a shadow root with a host — use native functions
                    if (__n_isShadowRoot(cur)) {
                        _shadowNodes[cur] = true;
                        var hostId = __n_getShadowHost(cur);
                        if (hostId >= 0) {
                            _inShadow = false;
                            cur = hostId;
                            continue;
                        }
                    }
                }
                cur = parent;
            }
            // Mark nodes from target up to (but not including) the first shadow root
            // The target and its shadow-internal ancestors are in the shadow tree
            _inShadow = false;
            for (var si = 0; si < path.length; si++) {
                if (__n_isShadowRoot(path[si])) {
                    // Everything before this index (closer to target) is shadow-internal
                    for (var sj = 0; sj < si; sj++) _shadowNodes[path[sj]] = true;
                    _shadowNodes[path[si]] = true;
                    break;
                }
            }

            // Determine if we're dispatching in the global document tree or a standalone one
            var isGlobalDoc = !ownerDoc || ownerDoc === document;
            var theDoc = isGlobalDoc ? document : ownerDoc;

            event._dispatching = true;
            event.target = __w(nodeId);
            event.srcElement = event.target;
            event.eventPhase = 0;

            // window.event legacy: save previous for restore after dispatch
            var __prevEvent = __currentEvent;

            // Build composedPath: wrapped elements + document (+ window for global)
            var composedPath = [];
            for (var pi = 0; pi < path.length; pi++) composedPath.push(__w(path[pi]));
            composedPath.push(theDoc);
            if (isGlobalDoc) composedPath.push(window);
            event._path = composedPath;

            // Report an uncaught exception per WHATWG "report the exception" algorithm.
            // Calls window.onerror(message, filename, lineno, colno, error) and also
            // dispatches an ErrorEvent on window for addEventListener('error') listeners.
            function __reportListenerError(err) {
                var message = (err && err.message) ? String(err.message) : String(err);
                var filename = (err && err.fileName) ? String(err.fileName) : '';
                var lineno = (err && err.lineNumber) ? err.lineNumber : 0;
                var colno = (err && err.columnNumber) ? err.columnNumber : 0;

                // 0. Route to console so drain_console() captures it
                console.error('Uncaught ' + message + (err instanceof Error && err.stack ? '\n' + err.stack : ''));

                // 1. Fire window.onerror IDL handler (gets string args per spec)
                if (typeof window.onerror === 'function') {
                    window.onerror(message, filename, lineno, colno, err);
                }

                // 2. Dispatch ErrorEvent on window for addEventListener('error') listeners
                var errEvt = new Event('error', {bubbles: false, cancelable: true});
                errEvt.message = message;
                errEvt.filename = filename;
                errEvt.lineno = lineno;
                errEvt.colno = colno;
                errEvt.error = err;
                if (window.__et_listeners && window.__et_listeners['error_b']) {
                    var eCbs = window.__et_listeners['error_b'].slice();
                    for (var ei = 0; ei < eCbs.length; ei++) {
                        eCbs[ei].call(window, errEvt);
                    }
                }
            }

            // Helper to fire a list of callbacks.
            // Per WHATWG DOM spec "inner invoke": if a listener throws, report the
            // exception and continue to the next listener.
            // Supports both function listeners and object listeners with handleEvent.
            function fireCbs(cbs, thisObj) {
                if (!cbs || !cbs.length) return;
                var snapshot = cbs.slice();
                for (var j = 0; j < snapshot.length; j++) {
                    var cb = snapshot[j];
                    // Per spec: skip if listener was removed during dispatch
                    if (cbs.indexOf(cb) === -1) continue;
                    var wasPassive = event._inPassiveListener;
                    if (cb._passive) event._inPassiveListener = true;
                    try {
                        if (typeof cb === 'function') {
                            cb.call(thisObj, event);
                        } else if (cb && typeof cb === 'object') {
                            // Per spec: perform a fresh Get of handleEvent each time
                            var handler = cb.handleEvent;
                            if (typeof handler === 'function') {
                                handler.call(cb, event);
                            } else {
                                throw new TypeError("EventListener.handleEvent is not a function");
                            }
                        }
                    } catch (ex) {
                        __reportListenerError(ex);
                    }
                    event._inPassiveListener = wasPassive;
                    if (event._stopImmediate) return;
                }
            }
            // Fire IDL on<type> handler on an element if not already in listener list
            function fireOnHandler(el, bubbleCbs) {
                var handlerName = 'on' + event.type;
                var handler = el[handlerName];
                // Per HTML spec, webkit-prefixed event handler IDL attributes are lowercase
                // (e.g. onwebkitanimationend) but the event type is camelCase (webkitAnimationEnd)
                if (typeof handler !== 'function' && handlerName !== handlerName.toLowerCase()) {
                    handler = el[handlerName.toLowerCase()];
                }
                if (typeof handler !== 'function') return;
                // Don't double-fire if handler is already in the bubble listener list
                if (bubbleCbs) {
                    for (var k = 0; k < bubbleCbs.length; k++) {
                        if (bubbleCbs[k] === handler || bubbleCbs[k]._origCb === handler) return;
                    }
                }
                try {
                    var ret = handler.call(el, event);
                    if (ret === false && event.cancelable) event.preventDefault();
                } catch (ex) {
                    __reportListenerError(ex);
                }
            }
            // Fire __et_listeners on an element (for listeners added via EventTarget.prototype)
            function fireEt(obj, suffix) {
                if (obj && obj.__et_listeners) {
                    fireCbs(obj.__et_listeners[event.type + suffix], obj);
                }
            }

            // Helper: set window.event based on whether nodeId is shadow-internal
            function __setWinEvent(nid) {
                __currentEvent = (isGlobalDoc && !_shadowNodes[nid]) ? event : undefined;
            }

            // Run dispatch phases, then always clean up
            function runPhases() {
                // === CAPTURE PHASE (root → target) ===
                event.eventPhase = 1;

                if (isGlobalDoc) {
                    // Window capture
                    __currentEvent = event;
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
                    __setWinEvent(nid);
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
                __setWinEvent(targetNid);
                event.currentTarget = targetEl;

                // Inline event handler (e.g. onclick="...")
                var attrHandler = __n_getAttribute(targetNid, 'on' + event.type);
                if (attrHandler) {
                    try {
                        (new Function('event', attrHandler)).call(targetEl, event);
                    } catch (ex) {
                        __reportListenerError(ex);
                    }
                    if (event._stopImmediate) return;
                }

                // Fire both capture and bubble listeners at target (per spec)
                fireCbs(_captureKeys[targetNid + ':' + event.type], targetEl);
                if (event._stopImmediate) return;
                fireEt(targetEl, '_c');
                if (event._stopImmediate) return;
                var targetBubbleCbs = _bubbleKeys[targetNid + ':' + event.type];
                fireCbs(targetBubbleCbs, targetEl);
                if (event._stopImmediate) return;
                fireEt(targetEl, '_b');
                if (event._stopImmediate) return;
                fireOnHandler(targetEl, targetBubbleCbs);
                if (event._stopImmediate) return;

                if (!event.bubbles) return;

                // === BUBBLE PHASE (target+1 → root → document → window) ===
                event.eventPhase = 3;
                for (var i = 1; i < path.length; i++) {
                    if (event._stopPropagation) break;
                    var nid = path[i];
                    var el = __w(nid);
                    __setWinEvent(nid);
                    event.currentTarget = el;
                    var elBubbleCbs = _bubbleKeys[nid + ':' + event.type];
                    fireCbs(elBubbleCbs, el);
                    if (event._stopImmediate) return;
                    fireEt(el, '_b');
                    if (event._stopImmediate) return;
                    fireOnHandler(el, elBubbleCbs);
                    if (event._stopImmediate) return;
                }

                if (isGlobalDoc) {
                    // Document bubble
                    if (!event._stopPropagation) {
                        __currentEvent = event;
                        event.currentTarget = document;
                        fireCbs(doc.__listeners[event.type], document);
                        if (event._stopImmediate) return;
                    }

                    // Window bubble
                    if (!event._stopPropagation) {
                        __currentEvent = event;
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

            // Activation behavior pre-step: toggle checkbox/radio before listeners fire
            var _activationRevert = null;
            if (event.type === 'click') {
                var targetEl = __w(nodeId);
                if (targetEl.tagName === 'INPUT') {
                    var itype = (targetEl.getAttribute('type') || '').toLowerCase();
                    if (itype === 'checkbox') {
                        var oldChecked = targetEl.checked;
                        targetEl.checked = !oldChecked;
                        _activationRevert = function() { targetEl.checked = oldChecked; };
                    } else if (itype === 'radio') {
                        var oldChecked = targetEl.checked;
                        targetEl.checked = true;
                        _activationRevert = function() { targetEl.checked = oldChecked; };
                    }
                }
            }

            if (!event._stopPropagation) runPhases();

            // Reset relatedTarget-related targets after dispatch, before activation
            if (event._resetTargetsAfterDispatch) {
                event.target = null;
                event.relatedTarget = null;
                event._path = [];
            }

            // Per spec: clear dispatch flag before activation behavior runs,
            // so the event can be redispatched during post-click handling (e.g., in onchange).
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            __currentEvent = __prevEvent;

            // Activation behavior post-step: revert if event was canceled, else fire input/change
            if (_activationRevert && event.defaultPrevented) {
                _activationRevert();
            } else if (_activationRevert && !event.defaultPrevented) {
                // Checkbox/radio was toggled — fire input and change events (only if connected)
                var targetEl = __w(nodeId);
                if (__isConnected(nodeId)) {
                    targetEl.dispatchEvent(new Event('input', {bubbles: true, composed: true}));
                    targetEl.dispatchEvent(new Event('change', {bubbles: true}));
                }
            }

            // Post-dispatch activation for elements in the event path.
            // Per spec, activation behavior runs on the first element in the path that has one.
            // Only one activation behavior fires per dispatch.
            // If checkbox/radio already activated (pre-dispatch), skip — that was the activation.
            if (event.type === 'click' && !event.defaultPrevented && !_activationRevert) {
                // Build full path: target + ancestors
                var activPath = [nodeId];
                var ap = __n_getParent(nodeId);
                while (ap >= 0) { activPath.push(ap); ap = __n_getParent(ap); }

                for (var ai = 0; ai < activPath.length; ai++) {
                    var ael = __w(activPath[ai]);
                    var atag = ael.tagName;

                    // <a> / <area> activation: navigate to href (fragment-only for now)
                    if (atag === 'A' || atag === 'AREA') {
                        var href = ael.getAttribute('href');
                        if (href !== null) {
                            if (href.charAt(0) === '#') {
                                var oldURL = location.href;
                                location.hash = href;
                                location._href = location.origin + location.pathname + location.search + href;
                                var newURL = location.href;
                                var hevt = new Event('hashchange', {bubbles: false});
                                hevt.newURL = newURL;
                                hevt.oldURL = oldURL;
                                window.dispatchEvent(hevt);
                            }
                        }
                        break;
                    }

                    // <summary> activation: toggle parent <details>
                    if (atag === 'SUMMARY') {
                        var details = ael.parentNode;
                        if (details && details.tagName === 'DETAILS') {
                            if (details.hasAttribute('open')) details.removeAttribute('open');
                            else details.setAttribute('open', '');
                            details.dispatchEvent(new Event('toggle', {bubbles: false}));
                        }
                        break;
                    }

                    // <input type=submit/image> / <button type=submit> activation: form submit
                    if (atag === 'INPUT' || atag === 'BUTTON') {
                        var btype = (ael.getAttribute('type') || '').toLowerCase();
                        if ((atag === 'BUTTON' && (btype === 'submit' || btype === '')) ||
                            (atag === 'INPUT' && (btype === 'submit' || btype === 'image'))) {
                            // Disabled submit buttons must not activate (spec: activation behavior check)
                            if (ael.disabled) break;
                            var form = ael.form;
                            if (form && form.__nid !== undefined) {
                                var formConnected = false;
                                var cur = form.__nid;
                                while (cur >= 0) {
                                    if (__n_getNodeType(cur) === 9) { formConnected = true; break; }
                                    cur = __n_getParent(cur);
                                }
                                if (formConnected) {
                                    var submitEvt = new Event('submit', {bubbles: true, cancelable: true});
                                    submitEvt.submitter = ael;
                                    form.dispatchEvent(submitEvt);
                                }
                            }
                            break;
                        }
                        // <input type=reset> / <button type=reset> activation: form reset
                        if (btype === 'reset') {
                            var form = ael.form;
                            if (form && form.__nid !== undefined) {
                                var formConnected = false;
                                var cur = form.__nid;
                                while (cur >= 0) {
                                    if (__n_getNodeType(cur) === 9) { formConnected = true; break; }
                                    cur = __n_getParent(cur);
                                }
                                if (formConnected) {
                                    form.dispatchEvent(new Event('reset', {bubbles: true, cancelable: true}));
                                }
                            }
                            break;
                        }
                    }

                    // <label> activation: forward click to associated control
                    if (atag === 'LABEL') {
                        var controlId = __n_findLabelControl(ael.__nid);
                        if (controlId >= 0) {
                            var ctrl = __w(controlId);
                            // Don't forward if the click target is already the control
                            // or is interactive content inside the label
                            if (ctrl && ctrl.__nid !== undefined && ctrl.__nid !== nodeId) {
                                var isDescendant = false;
                                var chk = nodeId;
                                while (chk >= 0) {
                                    if (chk === ctrl.__nid) { isDescendant = true; break; }
                                    chk = __n_getParent(chk);
                                }
                                if (!isDescendant) {
                                    if (typeof ctrl.focus === 'function') ctrl.focus();
                                    ctrl.click();
                                }
                            }
                        }
                        break;
                    }
                }
            }

            // Note: dispatch flags already cleared before activation behavior above
        }

        // __braille_click(nodeId) — called from Rust with full pointer/mouse event sequence
        globalThis.__braille_click = function(nodeId) {
            var el = __w(nodeId);
            // Compute approximate center coordinates
            var rect = el.getBoundingClientRect ? el.getBoundingClientRect() : {left:0,top:0,width:0,height:0};
            var cx = Math.round(rect.left + rect.width / 2);
            var cy = Math.round(rect.top + rect.height / 2);
            var commonInit = {bubbles:true, cancelable:true, clientX:cx, clientY:cy, screenX:cx, screenY:cy, button:0, buttons:1};
            // Full event sequence: pointerdown → mousedown → pointerup → mouseup → click
            el.dispatchEvent(new PointerEvent('pointerdown', Object.assign({pointerId:1, pointerType:'mouse'}, commonInit)));
            el.dispatchEvent(new MouseEvent('mousedown', commonInit));
            commonInit.buttons = 0;
            el.dispatchEvent(new PointerEvent('pointerup', Object.assign({pointerId:1, pointerType:'mouse'}, commonInit)));
            el.dispatchEvent(new MouseEvent('mouseup', commonInit));
            el.dispatchEvent(new MouseEvent('click', commonInit));
        };

        // Unified link load event firing.  dispatchEvent already invokes on<type>
        // handlers via fireOnHandler, so we never call node.onload() manually.
        globalThis.__braille_fire_link_load = function(node) {
            if (!node || node.__linkLoadFired) return;
            node.__linkLoadFired = true;
            node.dispatchEvent(new Event('load'));
        };

        // Schedule a deferred link load for dynamically-inserted <link> elements.
        // If pre-fetched CSS is available, store it in the Rust DOM tree for style computation.
        globalThis.__braille_maybe_load_link = function(node) {
            if (!node || node.tagName !== 'LINK') return;
            var rel = node.rel || node.getAttribute('rel') || '';
            if (rel === 'stylesheet' || rel === 'prefetch' || rel === 'preload') {
                node.__linkLoadScheduled = true;
                // Try to load actual CSS content
                var href = node.getAttribute('href') || '';
                if (href && node.__nid !== undefined) {
                    __braille_load_link_css(node, href);
                }
                setTimeout(function() { __braille_fire_link_load(node); }, 0);
            }
        };

        // Load CSS content for a link element from pre-fetched CSS or data: URLs
        globalThis.__braille_load_link_css = function(node, href) {
            var cssText = null;
            // Check data: URL
            if (href.indexOf('data:text/css') === 0) {
                var commaIdx = href.indexOf(',');
                if (commaIdx >= 0) {
                    cssText = decodeURIComponent(href.substring(commaIdx + 1));
                }
            }
            // Check pre-fetched CSS (strip query params for lookup)
            if (!cssText && globalThis.__braille_fetched_css) {
                var cleanHref = href.split('?')[0];
                cssText = globalThis.__braille_fetched_css[href] || globalThis.__braille_fetched_css[cleanHref];
            }
            if (cssText && node.__nid !== undefined) {
                __n_setLinkCss(node.__nid, cssText);
            }
        };

        // Dynamic script loading: fetch and eval <script src="..."> on insertion
        globalThis.__braille_script_log = [];
        globalThis.__braille_maybe_load_scripts_in_subtree = function(node) {
            if (!node) return;
            if (node.tagName === 'SCRIPT') {
                __braille_maybe_load_script(node);
                return;
            }
            if (node.querySelectorAll) {
                var scripts = node.querySelectorAll('script');
                for (var si = 0; si < scripts.length; si++) {
                    __braille_maybe_load_script(scripts[si]);
                }
            }
        };
        var __validScriptTypes = {'': 1, 'text/javascript': 1, 'application/javascript': 1, 'application/x-javascript': 1, 'text/ecmascript': 1, 'application/ecmascript': 1, 'module': 1};
        globalThis.__braille_maybe_load_script = function(node) {
            if (!node || node.tagName !== 'SCRIPT') return;
            // Per HTML spec "already started" flag: once a script has been executed,
            // it must not execute again when re-inserted into the document.
            if (node.__scriptAlreadyStarted) return;
            // Per spec: don't execute scripts that were disconnected (e.g., removed by an earlier script in the same batch)
            if (node.__nid !== undefined && !node.isConnected) return;
            // Per spec: scripts with invalid type attributes don't execute
            var scriptType = node.getAttribute('type');
            if (scriptType !== null && !__validScriptTypes[scriptType.toLowerCase()]) return;
            node.__scriptAlreadyStarted = true;
            var src = node.getAttribute('src');
            if (src) {
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
                    node.dispatchEvent(new Event('load'));
                }).catch(function(err) {
                    document.currentScript = null;
                    __braille_script_log.push('ERR: ' + shortSrc + ' -> ' + String(err).substring(0, 100));
                    node.dispatchEvent(new Event('error'));
                });
            } else {
                var code = (node.__nid !== undefined) ? __n_getTextContent(node.__nid) : (node.textContent || '');
                if (code && code.trim()) {
                    var iframeRealm = __braille_find_owning_iframe_realm(node);
                    if (iframeRealm) {
                        setTimeout(function() {
                            __braille_exec_in_iframe(iframeRealm, code);
                            node.dispatchEvent(new Event('load'));
                        }, 0);
                    } else {
                        document.currentScript = node;
                        (0, eval)(code);
                        document.currentScript = null;
                        node.dispatchEvent(new Event('load'));
                    }
                }
            }
        };

        // Helper: throw DOMException from validation error string "ErrorName:message"
        function __throwValidationError(err) {
            var colonIdx = err.indexOf(':');
            var name = err.substring(0, colonIdx);
            var msg = err.substring(colonIdx + 1);
            throw new DOMException(msg, name);
        }

        // Adopt a subtree into a new document (update __ownerDoc recursively)
        function __adoptSubtree(node, newDoc) {
            if (!node || node.__nid === undefined) return;
            node.__ownerDoc = newDoc;
            var stack = __n_getAllChildIds(node.__nid).slice();
            while (stack.length > 0) {
                var cid = stack.pop();
                var child = _cache[cid];
                if (child) child.__ownerDoc = newDoc;
                var grandkids = __n_getAllChildIds(cid);
                for (var i = 0; i < grandkids.length; i++) stack.push(grandkids[i]);
            }
        }
    "#
}
