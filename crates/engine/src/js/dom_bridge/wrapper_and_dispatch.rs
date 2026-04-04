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
            IFRAME: HTMLIFrameElement, BODY: HTMLBodyElement,
            HEAD: HTMLHeadElement, HTML: HTMLHtmlElement, TITLE: HTMLTitleElement,
            FRAMESET: HTMLFrameSetElement,
            DIV: HTMLDivElement, SPAN: HTMLSpanElement,
            P: HTMLParagraphElement, SCRIPT: HTMLScriptElement,
            STYLE: HTMLStyleElement, LINK: HTMLLinkElement,
            META: HTMLMetaElement, TABLE: HTMLTableElement,
            TR: HTMLTableRowElement, TD: HTMLTableCellElement,
            TH: HTMLTableCellElement, UL: HTMLUListElement,
            OL: HTMLOListElement, LI: HTMLLIElement,
            PRE: HTMLPreElement, CANVAS: HTMLCanvasElement,
            VIDEO: HTMLVideoElement, AUDIO: HTMLAudioElement,
            SOURCE: HTMLSourceElement, LABEL: HTMLLabelElement,
            TEMPLATE: HTMLTemplateElement,
            AREA: HTMLAreaElement, BASE: HTMLBaseElement,
            BR: HTMLBRElement, DATA: HTMLDataElement,
            DATALIST: HTMLDataListElement, DETAILS: HTMLDetailsElement,
            DIALOG: HTMLDialogElement, DIR: HTMLDirectoryElement,
            DL: HTMLDListElement, EMBED: HTMLEmbedElement,
            FIELDSET: HTMLFieldSetElement, FONT: HTMLFontElement,
            FRAME: HTMLFrameElement,
            H1: HTMLHeadingElement, H2: HTMLHeadingElement,
            H3: HTMLHeadingElement, H4: HTMLHeadingElement,
            H5: HTMLHeadingElement, H6: HTMLHeadingElement,
            HR: HTMLHRElement, LEGEND: HTMLLegendElement,
            MAP: HTMLMapElement, MARQUEE: HTMLMarqueeElement,
            MENU: HTMLMenuElement, METER: HTMLMeterElement,
            INS: HTMLModElement, DEL: HTMLModElement,
            OBJECT: HTMLObjectElement, OPTGROUP: HTMLOptGroupElement,
            OUTPUT: HTMLOutputElement, PARAM: HTMLParamElement,
            PICTURE: HTMLPictureElement, PROGRESS: HTMLProgressElement,
            BLOCKQUOTE: HTMLQuoteElement, Q: HTMLQuoteElement,
            CAPTION: HTMLTableCaptionElement,
            COL: HTMLTableColElement, COLGROUP: HTMLTableColElement,
            THEAD: HTMLTableSectionElement, TBODY: HTMLTableSectionElement,
            TFOOT: HTMLTableSectionElement,
            TIME: HTMLTimeElement, TRACK: HTMLTrackElement,
            // Generic HTML elements that use HTMLElement (not a specialized subclass)
            ABBR: HTMLElement, ADDRESS: HTMLElement, ARTICLE: HTMLElement,
            ASIDE: HTMLElement, B: HTMLElement, BDI: HTMLElement, BDO: HTMLElement,
            CITE: HTMLElement, CODE: HTMLElement, DD: HTMLElement,
            DFN: HTMLElement, DT: HTMLElement, EM: HTMLElement,
            FIGCAPTION: HTMLElement, FIGURE: HTMLElement, FOOTER: HTMLElement,
            HEADER: HTMLElement, HGROUP: HTMLElement, I: HTMLElement,
            KBD: HTMLElement, MAIN: HTMLElement, MARK: HTMLElement,
            NAV: HTMLElement, NOSCRIPT: HTMLElement, RP: HTMLElement,
            RT: HTMLElement, RUBY: HTMLElement, S: HTMLElement,
            SAMP: HTMLElement, SEARCH: HTMLElement, SECTION: HTMLElement,
            SMALL: HTMLElement, STRONG: HTMLElement, SUB: HTMLElement,
            SUMMARY: HTMLElement, SUP: HTMLElement, U: HTMLElement,
            VAR: HTMLElement, WBR: HTMLElement,
            // Deprecated/obsolete tags that are also HTMLElement
            ACRONYM: HTMLElement, BIG: HTMLElement, CENTER: HTMLElement,
            NOBR: HTMLElement, NOFRAMES: HTMLElement, NOEMBED: HTMLElement,
            PLAINTEXT: HTMLElement, RB: HTMLElement, RTC: HTMLElement,
            SPACER: HTMLElement, STRIKE: HTMLElement, TT: HTMLElement, XMP: HTMLElement,
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
            var obj;
            if (nt === 1) {
                var tag = __n_getTagName(nodeId);
                var ctor = _ctorMap[tag];
                // Check custom elements registry for hyphenated tags
                if (!ctor && typeof customElements !== 'undefined' && customElements._registry) {
                    var ceEntry = customElements._registry.get(tag.toLowerCase());
                    if (ceEntry) ctor = ceEntry.ctor;
                }
                if (ctor) {
                    obj = Object.create(ctor.prototype);
                    obj.constructor = ctor;
                } else {
                    var ns = __n_getNamespace(nodeId);
                    if (!ns || ns === 'http://www.w3.org/1999/xhtml') {
                        obj = Object.create(HTMLUnknownElement.prototype);
                        obj.constructor = HTMLUnknownElement;
                    } else {
                        obj = Object.create(Element.prototype);
                    }
                }
            } else if (nt === 11 && __n_isShadowRoot(nodeId)) {
                // ShadowRoot — use ShadowRoot.prototype
                obj = Object.create(ShadowRoot.prototype);
                obj._mode = __n_getShadowRootMode(nodeId);
                var hostId = __n_getShadowHost(nodeId);
                if (hostId >= 0) {
                    var hostEl = __w(hostId);
                    obj._host = hostEl;
                    obj._shadowHost = hostEl;
                }
            } else {
                obj = Object.create(proto);
            }
            obj.__nid = nodeId;
            obj.__props = {};
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

        // window.event legacy getter — tracks the currently dispatching event
        var __currentEvent = undefined;
        Object.defineProperty(window, 'event', {
            get: function() { return __currentEvent; },
            set: function(v) { __currentEvent = v; },
            configurable: true,
            enumerable: true
        });

        // Event dispatch with capture + bubble phases
        // ownerDoc: optional non-global document that owns the target element
        function __dispatch(nodeId, event, ownerDoc) {
            // Build path: target -> parent -> ... -> root
            // For composed events, follow shadow host links across shadow boundaries
            var path = [];
            var _shadowNodes = {};  // nodeIds that are inside a shadow tree
            var _inShadow = false;
            var cur = nodeId;
            while (cur >= 0) {
                path.push(cur);
                if (_inShadow) _shadowNodes[cur] = true;
                var parent = __n_getParent(cur);
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
                                if (typeof window.onhashchange === 'function') {
                                    window.onhashchange({type:'hashchange', newURL: newURL, oldURL: oldURL});
                                }
                                var hevt = new Event('hashchange', {bubbles: false});
                                hevt.newURL = newURL;
                                hevt.oldURL = oldURL;
                                if (window.__et_listeners && window.__et_listeners['hashchange_b']) {
                                    var hcbs = window.__et_listeners['hashchange_b'];
                                    for (var hi = 0; hi < hcbs.length; hi++) hcbs[hi].call(window, hevt);
                                }
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
                                    if (typeof form.onsubmit === 'function') {
                                        var ret = form.onsubmit(submitEvt);
                                        if (ret === false) submitEvt.preventDefault();
                                    }
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
                                    var resetEvt = new Event('reset', {bubbles: true, cancelable: true});
                                    if (typeof form.onreset === 'function') form.onreset(resetEvt);
                                    form.dispatchEvent(resetEvt);
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

            // Per spec step 14: unset dispatching, stop propagation, and stop immediate flags
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            // Restore previous window.event
            __currentEvent = __prevEvent;
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

        // Element mutation methods that operate on the real DomTree
        EP.appendChild = function(child) {
            if (child === null || child === undefined || (typeof child === 'object' && child.__nid === undefined && child.nodeType === undefined)) {
                throw new TypeError("Failed to execute 'appendChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            // CharacterData nodes (Text=3, PI=7, Comment=8) cannot have children
            var pnt = this.nodeType;
            if (pnt === 3 || pnt === 7 || pnt === 8) {
                throw new DOMException("CharacterData type " + this.nodeName + " must not have children", "HierarchyRequestError");
            }
            if (this.__nid === undefined) return child;
            // Capture ownerDocument BEFORE mutation (tree walk changes after append)
            var parentDoc = this.ownerDocument || (this.nodeType === 9 ? this : document);
            var childDoc = (child && child.__nid !== undefined) ? (child.ownerDocument || document) : null;
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
            // Adopt: update ownerDocument for child (and descendants) if parent is in a different document
            if (childDoc && parentDoc !== childDoc) {
                __adoptSubtree(child, parentDoc);
            }
            // CE lifecycle: connectedCallback for inserted nodes
            if (typeof __ceConnected === 'function' && __isConnected(this.__nid)) {
                __ceConnected(child);
            }
            // Upgrade custom elements in inserted subtree
            if (typeof __ceUpgradeTree === 'function' && child && child.__nid !== undefined) {
                __ceUpgradeTree(child);
            }
            __braille_maybe_load_script(child);
            __braille_maybe_load_link(child);
            if (typeof __braille_maybe_init_iframe === 'function') __braille_maybe_init_iframe(child);
            __ceFlushReactions();
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
                // CE lifecycle: disconnectedCallback before removal
                if (typeof __ceDisconnected === 'function' && __isConnected(this.__nid)) {
                    __ceDisconnected(child);
                }
                __n_removeChild(this.__nid, child.__nid);
                if (typeof __mo_notify === 'function') __mo_notify('childList', this, {removedNodes: [child]});
            }
            __ceFlushReactions();
            return child;
        };
        EP.insertBefore = function(newChild, refChild) {
            if (newChild === null || newChild === undefined || (typeof newChild === 'object' && newChild.__nid === undefined && newChild.nodeType === undefined)) {
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
            // CE lifecycle: connectedCallback for inserted nodes
            if (typeof __ceConnected === 'function' && __isConnected(this.__nid)) {
                __ceConnected(newChild);
            }
            if (typeof __ceUpgradeTree === 'function' && newChild && newChild.__nid !== undefined) {
                __ceUpgradeTree(newChild);
            }
            __braille_maybe_load_script(newChild);
            __braille_maybe_load_link(newChild);
            if (typeof __braille_maybe_init_iframe === 'function') __braille_maybe_init_iframe(newChild);
            __ceFlushReactions();
            return newChild;
        };

        // Fullscreen tracking
        var __fullscreenElement = null;
        EP.requestFullscreen = function() { __fullscreenElement = this; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };

        // Helper: create a standalone document-like wrapper around a root element.
        // Used by createHTMLDocument(), createDocument(), and document.cloneNode().
        // Returns a proper Document node (inherits from Document.prototype → EP → Node constants).
        function __isInvalidAttrName(name) {
            if (name.length === 0) return true;
            for (var i = 0; i < name.length; i++) {
                var c = name.charCodeAt(i);
                if (c === 0 || c === 9 || c === 10 || c === 12 || c === 13 || c === 32 || c === 47 || c === 62 || c === 61) return true;
            }
            return false;
        }

        function __makeDocumentLike(rootEl) {
            // Create a Rust-backed Document node so all EP methods (appendChild, insertBefore, etc.) work
            var docNid = __n_createDocumentNode();
            var newDoc = __w(docNid);
            // If rootEl is provided and Rust-backed, parent it under the document node
            if (rootEl && rootEl.__nid !== undefined) {
                __n_appendChild(docNid, rootEl.__nid);
            }
            // Set own data properties — use defineProperty for getter-only EP properties
            var ownProps = {
                readyState: 'complete',
                ownerDocument: null,
                isConnected: false,
                location: null,
                title: '',
                contentType: 'application/xml',
                URL: 'about:blank',
                documentURI: 'about:blank',
                compatMode: 'CSS1Compat',
                characterSet: 'UTF-8',
                charset: 'UTF-8',
                inputEncoding: 'UTF-8'
            };
            for (var k in ownProps) Object.defineProperty(newDoc, k, { value: ownProps[k], writable: true, enumerable: true, configurable: true });
            newDoc.__listeners = {};
            newDoc.__captureListeners = {};
            Object.defineProperty(newDoc, 'documentElement', { get: function() {
                var kids = __n_getAllChildIds(docNid);
                for (var i = 0; i < kids.length; i++) {
                    if (__n_getNodeType(kids[i]) === 1) return __w(kids[i]);
                }
                return null;
            }, configurable: true });
            Object.defineProperty(newDoc, 'body', { get: function() {
                if (this._body) return this._body;
                var de = this.documentElement;
                if (!de) return null;
                var kids = de.childNodes;
                for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'BODY') return kids[i];
                return null;
            }, set: function(v) { this._body = v; }, configurable: true });
            Object.defineProperty(newDoc, 'head', { get: function() {
                if (this._head) return this._head;
                var de = this.documentElement;
                if (!de) return null;
                var kids = de.childNodes;
                for (var i = 0; i < kids.length; i++) if (kids[i].tagName === 'HEAD') return kids[i];
                return null;
            }, set: function(v) { this._head = v; }, configurable: true });
            // Each doc gets its own implementation that knows its owning document
            var impl = Object.create(DOMImplementation.prototype);
            impl.__ownerDocument = newDoc;
            impl.createHTMLDocument = function(title) { return document.implementation.createHTMLDocument(title); };
            impl.createDocument = function(ns, qn, dt) { return document.implementation.createDocument(ns, qn, dt); };
            impl.createDocumentType = function(qn, pub_, sys_) {
                var dt = document.implementation.createDocumentType(qn, pub_, sys_);
                dt.__ownerDoc = this.__ownerDocument;
                return dt;
            };
            impl.hasFeature = function() { return true; };
            Object.defineProperty(newDoc, 'implementation', { value: impl, writable: true, configurable: true });
            Object.defineProperty(newDoc, 'doctype', { get: function() {
                var kids = __n_getAllChildIds(docNid);
                for (var i = 0; i < kids.length; i++) {
                    if (__n_getNodeType(kids[i]) === 10) return __w(kids[i]);
                }
                return null;
            }, configurable: true });
            newDoc.querySelector = function(sel) { var de = this.documentElement; return de ? de.querySelector(sel) : null; };
            newDoc.querySelectorAll = function(sel) { var de = this.documentElement; return de ? de.querySelectorAll(sel) : __makeStaticNodeList([]); };
            newDoc.getElementById = function(id) { var de = this.documentElement; return de ? (de.querySelector('#' + id) || null) : null; };
            newDoc.getElementsByTagName = function(tag) { var de = this.documentElement; return de ? de.querySelectorAll(tag) : []; };
            newDoc.getElementsByClassName = function(cls) { var de = this.documentElement; return de ? de.querySelectorAll('.' + cls) : []; };
            newDoc.createElement = function(tag) { var el = document.createElement(tag); el.__ownerDoc = newDoc; return el; };
            newDoc.createElementNS = function(ns, tag) { var el = document.createElementNS(ns, tag); el.__ownerDoc = newDoc; return el; };
            newDoc.createTextNode = function(text) { var n = document.createTextNode(text); n.__ownerDoc = newDoc; return n; };
            newDoc.createComment = function(text) { var n = document.createComment(text); n.__ownerDoc = newDoc; return n; };
            newDoc.createDocumentFragment = function() { var n = document.createDocumentFragment(); n.__ownerDoc = newDoc; return n; };
            newDoc.createProcessingInstruction = function(t, d) { var n = document.createProcessingInstruction(t, d); n.__ownerDoc = newDoc; return n; };
            newDoc.createCDATASection = function(data) { var n = document.createCDATASection(data); n.__ownerDoc = newDoc; return n; };
            newDoc.createAttribute = function(n) { return document.createAttribute(n); };
            newDoc.createAttributeNS = function(ns, qn) { return document.createAttributeNS(ns, qn); };
            newDoc.createEvent = function(type) { var e = new Event(''); e._initialized = false; e.type = ''; return e; };
            // Event handling — own-property versions for standalone docs
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
            Object.defineProperty(newDoc, 'scrollingElement', { get: function() { return this.documentElement; }, configurable: true });
            newDoc.elementFromPoint = function(x, y) { return this.documentElement || null; };
            newDoc.elementsFromPoint = function(x, y) { var de = this.documentElement; return de ? [de] : []; };
            newDoc.write = function() {
                var html = Array.prototype.join.call(arguments, '');
                if (!html) return;
                var body = newDoc.body;
                if (!body) return;
                var temp = document.createElement('div');
                __n_setInnerHTML(temp.__nid, html);
                while (temp.firstChild) body.appendChild(temp.firstChild);
            };
            newDoc.writeln = function() {
                newDoc.write.apply(newDoc, arguments);
                newDoc.write('\n');
            };
            // Tag the root element and all descendants so EP.ownerDocument works
            if (rootEl) {
                rootEl.__ownerDoc = newDoc;
                if (rootEl.__nid !== undefined) __adoptSubtree(rootEl, newDoc);
            }
            return newDoc;
        }
        globalThis.__makeDocumentLike = __makeDocumentLike;
        globalThis.__adoptSubtree = __adoptSubtree;
        globalThis.__w = __w;
        globalThis.__cache = _cache;

        // Override document methods
        var doc = globalThis.document;
        doc.__listeners = {};
        doc.parentNode = null;
        doc.parentElement = null;
        doc.title = '';
        doc.compatMode = 'CSS1Compat';
        doc.characterSet = 'UTF-8';
        doc.charset = 'UTF-8';
        doc.inputEncoding = 'UTF-8';
        doc.contentType = 'text/html';
        Object.defineProperty(doc, 'ownerDocument', { value: null, writable: true, configurable: true });
        Object.defineProperty(doc, 'URL', {
            get: function() { return (typeof location !== 'undefined' && location.href) || 'about:blank'; },
            configurable: true
        });
        Object.defineProperty(doc, 'documentURI', {
            get: function() { return doc.URL; },
            configurable: true
        });
        doc.getElementById = function(id) {
            var nid = __n_getElementById(String(id));
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelector = function(sel) {
            var nid = __n_querySelector(0, sel, 0);
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelectorAll = function(sel) {
            return __makeStaticNodeList(__n_querySelectorAll(0, sel, 0).map(__w));
        };
        doc.createElement = function(tag) {
            var nid = __n_createElement(tag);
            var el = __w(nid);
            el.namespaceURI = 'http://www.w3.org/1999/xhtml';
            return el;
        };
        doc.createElementNS = function(ns, tag) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'createElementNS' on 'Document': 2 arguments required.");
            var qn = String(tag);
            var nsStr = (ns === null || ns === undefined) ? '' : String(ns);
            var result = JSON.parse(__n_validateAndExtract(nsStr, qn));
            if (result.err) {
                var eName = result.err;
                throw new DOMException("Failed to execute 'createElementNS' on 'Document': The qualified name provided ('" + qn + "') " + (eName === 'InvalidCharacterError' ? 'contains the invalid character' : 'has a namespace error') + ".", eName);
            }
            var localName = result.ok.localName;
            var pfx = result.ok.prefix || '';
            var nsNorm = (ns === null || ns === undefined || ns === '') ? null : String(ns);
            var nsForNative = nsNorm || '';
            var nid = __n_createElementNS(localName, nsForNative, pfx);
            var el = __w(nid);
            el.namespaceURI = nsNorm;
            el.__localName = localName;
            el.prefix = pfx || null;
            // Fix prototype based on namespace
            if (nsNorm !== 'http://www.w3.org/1999/xhtml') {
                // Non-HTML namespace → plain Element
                Object.setPrototypeOf(el, Element.prototype);
            } else if (localName !== localName.toLowerCase() || !_ctorMap[localName.toUpperCase()]) {
                // HTML namespace but uppercase or unknown tag → HTMLUnknownElement
                Object.setPrototypeOf(el, HTMLUnknownElement.prototype);
            }
            return el;
        };
        doc.createTextNode = function(text) {
            var nid = __n_createTextNode(String(text));
            var node = __w(nid);
            return node;
        };
        doc.createComment = function(text) {
            var nid = __n_createComment(String(text));
            return __w(nid);
        };
        doc.createDocumentFragment = function() {
            var nid = __n_createDocFragment();
            return __w(nid);
        };
        doc.getElementsByTagName = function(tag) {
            return __makeHTMLCollection(function() { return __n_getElementsByTagName(0, tag, true).map(__w); });
        };
        doc.getElementsByTagNameNS = function(ns, localName) {
            var nsStr = (ns === null || ns === undefined) ? '' : String(ns);
            var lnStr = String(localName);
            return __makeHTMLCollection(function() { return __n_getElementsByTagNameNS(0, nsStr, lnStr).map(__w); });
        };
        doc.getElementsByClassName = function(cls) {
            return __makeHTMLCollection(function() { return doc.querySelectorAll('.' + cls); });
        };
        doc.addEventListener = function(type, cb, opts) {
            var capture, once, passive, passiveExplicit;
            if (opts && typeof opts === 'object' && opts !== null) {
                capture = !!opts.capture;
                once = !!opts.once;
                passiveExplicit = ('passive' in opts) && opts.passive !== undefined;
                passive = passiveExplicit ? !!opts.passive : false;
            } else {
                capture = !!opts;
                once = false;
                passiveExplicit = false;
                passive = false;
            }
            // Passive-by-default for touch/wheel on document
            if (!passiveExplicit && __passiveDefaultTypes[type]) passive = true;
            if (passive) {
                if (!document.__passiveTypes) document.__passiveTypes = {};
                document.__passiveTypes[type] = true;
            }
            if (typeof cb !== 'function' && !(cb && typeof cb === 'object')) return;
            var store = capture ? _docCapture : doc.__listeners;
            if (!store[type]) store[type] = [];
            if (once) {
                var wrapper = function(e) { doc.removeEventListener(type, wrapper, capture); cb.call(document, e); };
                wrapper._origCb = cb;
                if (passive) wrapper._passive = true;
                store[type].push(wrapper);
            } else {
                if (passive && typeof cb === 'function') cb._passive = true;
                store[type].push(cb);
            }
        };
        doc.removeEventListener = function(type, cb, opts) {
            var capture = (opts && typeof opts === 'object' && opts !== null) ? !!opts.capture : !!opts;
            var store = capture ? _docCapture : doc.__listeners;
            if (store[type]) store[type] = store[type].filter(function(f){return f!==cb && f._origCb!==cb;});
        };

        doc.createComment = function(text) {
            var nid = __n_createComment(String(text));
            return __w(nid);
        };

        doc.createAttribute = function(localName) {
            if (arguments.length === 0) throw new TypeError("Failed to execute 'createAttribute' on 'Document': 1 argument required, but only 0 present.");
            var name = String(localName);
            if (__isInvalidAttrName(name)) throw new DOMException("Failed to execute 'createAttribute' on 'Document': The string contains invalid characters.", "InvalidCharacterError");
            return new Attr(name.toLowerCase());
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
            if (event._initialized === false) throw new DOMException("The event is not initialized.", "InvalidStateError");
            var __prevEvent = __currentEvent;
            __currentEvent = event;
            event._dispatching = true;
            event.target = document;
            event.srcElement = document;
            event._path = [document, window];
            event.eventPhase = 2;
            event.currentTarget = document;
            var cbs = doc.__listeners[event.type];
            if (cbs) {
                var snapshot = cbs.slice();
                for (var i = 0; i < snapshot.length; i++) {
                    var wasPassive = event._inPassiveListener;
                    if (snapshot[i]._passive) event._inPassiveListener = true;
                    snapshot[i].call(document, event);
                    event._inPassiveListener = wasPassive;
                    if (event._stopImmediate) break;
                }
            }
            // Call document IDL on-handler (e.g. document.onscrollend)
            if (!event._stopImmediate) {
                var docHandler = document['on' + event.type];
                if (typeof docHandler === 'function') docHandler.call(document, event);
            }
            // Bubble to window
            if (event.bubbles && !event._stopPropagation && !event._stopImmediate) {
                event.eventPhase = 3;
                event.currentTarget = window;
                var winCbs = window.__et_listeners && window.__et_listeners[event.type + '_b'];
                if (winCbs) {
                    var ws = winCbs.slice();
                    for (var i = 0; i < ws.length; i++) {
                        ws[i].call(window, event);
                        if (event._stopImmediate) break;
                    }
                }
            }
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            __currentEvent = __prevEvent;
            return !event.defaultPrevented;
        };
        doc.elementFromPoint = function(x, y) {
            // Walk all elements depth-first, find deepest one containing (x,y)
            // Stop at IFRAME boundaries — iframe content is accessed separately
            var best = doc.documentElement || null;
            function walk(el) {
                if (!el || el.nodeType !== 1) return;
                var r = el.getBoundingClientRect();
                var display = __n_getComputedStyle(el.__nid, 'display');
                if (display === 'none') return;
                if (display !== 'contents' && r && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
                    best = el;
                }
                // Don't descend into iframe children (they're in a separate document)
                if (el.tagName === 'IFRAME') return;
                var ch = el.children;
                if (ch) { for (var i = 0; i < ch.length; i++) walk(ch[i]); }
            }
            if (best) walk(best);
            return best;
        };
        doc.elementsFromPoint = function(x, y) { var el = doc.elementFromPoint(x, y); return el ? [el] : []; };
        var _createEventAliases = {
            'beforeunloadevent': 'BeforeUnloadEvent',
            'compositionevent': 'CompositionEvent',
            'customevent': 'CustomEvent',
            'devicemotionevent': 'DeviceMotionEvent',
            'deviceorientationevent': 'DeviceOrientationEvent',
            'dragevent': 'DragEvent',
            'event': 'Event',
            'events': 'Event',
            'focusevent': 'FocusEvent',
            'hashchangeevent': 'HashChangeEvent',
            'htmlevents': 'Event',
            'keyboardevent': 'KeyboardEvent',
            'messageevent': 'MessageEvent',
            'mouseevent': 'MouseEvent',
            'mouseevents': 'MouseEvent',
            'storageevent': 'StorageEvent',
            'svgevents': 'Event',
            'textevent': 'TextEvent',
            'uievent': 'UIEvent',
            'uievents': 'UIEvent',
        };
        doc.createEvent = function(type) {
            var key = String(type).toLowerCase();
            if (key === 'touchevent' && !('ontouchstart' in document)) {
                throw new DOMException("Failed to execute 'createEvent' on 'Document': The provided event type ('" + type + "') is invalid.", 'NotSupportedError');
            }
            var ctorName = _createEventAliases[key];
            if (!ctorName) {
                throw new DOMException("Failed to execute 'createEvent' on 'Document': The provided event type ('" + type + "') is invalid.", 'NotSupportedError');
            }
            var Ctor = globalThis[ctorName];
            var e = new Ctor('');
            e._initialized = false;
            e.type = '';
            return e;
        };
        doc.createTreeWalker = function(root, whatToShow, filter) {
            if (arguments.length === 0) throw new TypeError("Failed to execute 'createTreeWalker' on 'Document': 1 argument required, but only 0 present.");
            return new TreeWalker(root, whatToShow, filter);
        };
        doc.createNodeIterator = function(root, whatToShow, filter) {
            return new NodeIterator(root, whatToShow, filter);
        };
        doc.importNode = function(node, deep) {
            if (!node) return node;
            if (node.nodeType === 2) {
                var attr = new Attr(node.name, node.value, node.namespaceURI, node.prefix);
                attr.localName = node.localName;
                return attr;
            }
            if (node.__nid !== undefined) {
                var clone = node.cloneNode(!!deep);
                __adoptSubtree(clone, this);
                return clone;
            }
            return node;
        };
        doc.adoptNode = function(node) {
            if (!node || typeof node !== 'object') throw new TypeError("Failed to execute 'adoptNode' on 'Document': parameter 1 is not of type 'Node'.");
            if (node.nodeType === 9) throw new DOMException("Failed to execute 'adoptNode' on 'Document': A Document node cannot be adopted.", "NotSupportedError");
            if (node.nodeType === 2) throw new DOMException("Cannot adopt an Attr node", "NotSupportedError");
            // Remove from old parent
            if (node.parentNode) {
                if (node.nodeType === 10) {
                    // DocumentType: just detach by clearing parentNode
                    // (doctype nodes may not be real DOM nodes with __nid)
                    node.parentNode = null;
                } else {
                    node.parentNode.removeChild(node);
                }
            }
            // Recursively set ownerDocument
            function setOwnerDoc(n, doc) {
                n.__ownerDoc = doc;
                // Also set own ownerDocument property if it exists (e.g. DocumentType nodes)
                if (n.hasOwnProperty && n.hasOwnProperty('ownerDocument')) {
                    n.ownerDocument = doc;
                }
                var kids = n.childNodes;
                if (kids) { for (var i = 0; i < kids.length; i++) setOwnerDoc(kids[i], doc); }
            }
            setOwnerDoc(node, this);
            return node;
        };
        doc.cloneNode = function(deep) {
            return Document.prototype.cloneNode.call(doc, deep);
        };
        doc.exitFullscreen = function() { __fullscreenElement = null; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };
        doc.getAnimations = function() { return []; };

        doc.createProcessingInstruction = function(target, data) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'createProcessingInstruction' on 'Document': 2 arguments required.");
            var t = String(target), d = String(data);
            if (!__n_isValidXmlName(t)) throw new DOMException("The target provided ('" + t + "') is not a valid XML name.", "InvalidCharacterError");
            if (d.indexOf('?>') !== -1) throw new DOMException("The data provided ('..?>..') contains '?>'.", "InvalidCharacterError");
            var nid = __n_createPI(t, d);
            return __w(nid);
        };

        doc.createCDATASection = function(data) {
            if (arguments.length < 1) throw new TypeError("Failed to execute 'createCDATASection' on 'Document': 1 argument required.");
            var nid = __n_createCDATASection(String(data));
            return __w(nid);
        };

        doc.write = function() {
            var html = Array.prototype.join.call(arguments, '');
            if (!html) return;
            var body = doc.body;
            if (!body) return;
            var temp = doc.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            while (temp.firstChild) body.appendChild(temp.firstChild);
        };
        doc.writeln = function() {
            doc.write.apply(doc, arguments);
            doc.write('\n');
        };
        Document.prototype.write = function() {
            var html = Array.prototype.join.call(arguments, '');
            if (!html) return;
            var body = this.body;
            if (!body) return;
            var temp = document.createElement('div');
            __n_setInnerHTML(temp.__nid, html);
            while (temp.firstChild) body.appendChild(temp.firstChild);
        };
        Document.prototype.writeln = function() {
            this.write.apply(this, arguments);
            this.write('\n');
        };

        // window.dispatchEvent assigned after EventTarget is defined (below)

        // Track focused element for document.activeElement
        var __focusedElement = null;
        EP.focus = function() {
            var prev = __focusedElement;
            if (prev === this) return;
            __focusedElement = this;
            if (prev) {
                prev.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: this }));
            }
            this.dispatchEvent(new FocusEvent('focusin', { bubbles: true, relatedTarget: prev }));
            if (prev) {
                prev.dispatchEvent(new FocusEvent('blur', { bubbles: false, relatedTarget: this }));
            }
            this.dispatchEvent(new FocusEvent('focus', { bubbles: false, relatedTarget: prev }));
        };
        EP.blur = function() {
            if (__focusedElement !== this) return;
            __focusedElement = null;
            this.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: null }));
            this.dispatchEvent(new FocusEvent('blur', { bubbles: false, relatedTarget: null }));
        };

        // document.cookie implementation (JS-side cookie jar)
        var _cookieJar = {};
        Object.defineProperties(doc, {
            body: { get: function() { return doc.querySelector('body'); }, configurable: true },
            head: { get: function() { return doc.querySelector('head'); }, configurable: true },
            documentElement: { get: function() { return doc.querySelector('html'); }, configurable: true },
            scrollingElement: { get: function() { return doc.documentElement; }, configurable: true },
            activeElement: { get: function() { return __focusedElement || doc.querySelector('body'); }, configurable: true },
            styleSheets: { get: function() {
                var sheets = [];
                var styles = doc.querySelectorAll('style');
                for (var i = 0; i < styles.length; i++) sheets.push(styles[i].sheet);
                var links = doc.querySelectorAll('link[rel="stylesheet"]');
                for (var i = 0; i < links.length; i++) sheets.push(links[i].sheet);
                return sheets;
            }, configurable: true },
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
                    var nid = __n_getDoctypeNodeId();
                    if (nid === -1) return null;
                    return __w(nid);
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
                    newDoc.contentType = 'text/html';
                    newDoc.title = title !== undefined ? String(title) : '';
                    // Set ownerDocument on child elements
                    headEl.__ownerDoc = newDoc;
                    bodyEl.__ownerDoc = newDoc;
                    if (title !== undefined) titleEl.__ownerDoc = newDoc;
                    // Create DOCTYPE node and insert before htmlEl in the Rust tree
                    var dt = document.implementation.createDocumentType('html', '', '');
                    __n_insertBefore(newDoc.__nid, dt.__nid, htmlEl.__nid);
                    return newDoc;
                },
                createDocument: function(ns, qualifiedName, doctype) {
                    if (arguments.length < 2) throw new TypeError("Failed to execute 'createDocument' on 'DOMImplementation': 2 arguments required.");
                    // Type-check doctype: must be null, undefined, or a DocumentType node
                    if (doctype !== null && doctype !== undefined && (!doctype || doctype.nodeType !== 10)) {
                        throw new TypeError("Failed to execute 'createDocument' on 'DOMImplementation': parameter 3 is not of type 'DocumentType'.");
                    }
                    var nsVal = (ns === undefined) ? null : ns;
                    var qnVal = (qualifiedName === null) ? '' : String(qualifiedName);
                    // Validate qualifiedName if non-empty
                    if (qnVal !== '') {
                        var nsStr = (nsVal === null || nsVal === undefined) ? '' : String(nsVal);
                        var result = JSON.parse(__n_validateAndExtract(nsStr, qnVal));
                        if (result.err) {
                            var eName = result.err;
                            throw new DOMException("Failed to execute 'createDocument' on 'DOMImplementation': The qualified name provided ('" + qnVal + "') " + (eName === 'InvalidCharacterError' ? 'is not a valid name' : 'has a namespace error') + ".", eName);
                        }
                    } else if (nsVal === null || nsVal === undefined || nsVal === '') {
                        // Empty qname with null namespace is fine (creates doc with no element)
                    } else {
                        // Non-null namespace with empty qname → NamespaceError per spec? No, spec allows it.
                    }
                    var rootEl = null;
                    if (qnVal !== '') {
                        rootEl = document.createElementNS(nsVal, qnVal);
                    }
                    var newDoc = __makeDocumentLike(rootEl);
                    // Set contentType based on namespace
                    if (nsVal === 'http://www.w3.org/1999/xhtml') {
                        newDoc.contentType = 'application/xhtml+xml';
                    } else if (nsVal === 'http://www.w3.org/2000/svg') {
                        newDoc.contentType = 'image/svg+xml';
                    } else {
                        newDoc.contentType = 'application/xml';
                    }
                    // Set prototype to XMLDocument
                    Object.setPrototypeOf(newDoc, XMLDocument.prototype);
                    // Handle doctype parameter — insert into Rust tree
                    if (doctype) {
                        doctype.__ownerDoc = newDoc;
                        if (rootEl && rootEl.__nid !== undefined) {
                            __n_insertBefore(newDoc.__nid, doctype.__nid, rootEl.__nid);
                        } else {
                            __n_appendChild(newDoc.__nid, doctype.__nid);
                        }
                    }
                    // Set ownerDocument on root element
                    if (rootEl) rootEl.__ownerDoc = newDoc;
                    // XML documents preserve case in createElement
                    newDoc.createElement = function(tag) {
                        var nid = __n_createElement(tag);
                        var el = __w(nid);
                        el.__localName = String(tag);
                        el.__ownerDoc = newDoc;
                        var ct = newDoc.contentType;
                        if (ct === 'text/html' || ct === 'application/xhtml+xml') {
                            el.namespaceURI = 'http://www.w3.org/1999/xhtml';
                        } else {
                            el.namespaceURI = null;
                        }
                        return el;
                    };
                    // XML documents preserve case in createAttribute
                    newDoc.createAttribute = function(localName) {
                        if (arguments.length === 0) throw new TypeError("Failed to execute 'createAttribute' on 'Document': 1 argument required, but only 0 present.");
                        var name = String(localName);
                        if (__isInvalidAttrName(name)) throw new DOMException("Failed to execute 'createAttribute' on 'Document': The string contains invalid characters.", "InvalidCharacterError");
                        return new Attr(name);
                    };
                    return newDoc;
                },
                createDocumentType: function(qualifiedName, publicId, systemId) {
                    if (arguments.length < 3) throw new TypeError("Failed to execute 'createDocumentType' on 'DOMImplementation': 3 arguments required.");
                    var qn = String(qualifiedName);
                    // DOCTYPE names only reject > and whitespace
                    if (/[>\s]/.test(qn)) {
                        throw new DOMException("Failed to execute 'createDocumentType' on 'DOMImplementation': The qualified name provided is not a valid name.", "InvalidCharacterError");
                    }
                    var nid = __n_createDoctype(qn, String(publicId), String(systemId));
                    return __w(nid);
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
            var newDoc = __makeDocumentLike(null);
            // XML document: createElement preserves case and returns Element (not HTMLElement)
            newDoc.createElement = function(tag) {
                var nid = __n_createElement(String(tag));
                var el = Object.create(__ElemProto);
                el.__nid = nid;
                el.__props = {};
                el.__localName = String(tag);
                el.__ownerDoc = newDoc;
                el.namespaceURI = null;
                el.constructor = Element;
                _cache[nid] = el;
                return el;
            };
            // XML document: createAttribute preserves case
            newDoc.createAttribute = function(localName) {
                if (arguments.length === 0) throw new TypeError("Failed to execute 'createAttribute' on 'Document': 1 argument required, but only 0 present.");
                var name = String(localName);
                if (__isInvalidAttrName(name)) throw new DOMException("Failed to execute 'createAttribute' on 'Document': The string contains invalid characters.", "InvalidCharacterError");
                return new Attr(name);
            };
            return newDoc;
        };

        // EventTarget constructor — standalone event targets (not backed by DOM nodes)
        function EventTarget() {
            this.__et_listeners = {};
        }
        EventTarget.prototype.addEventListener = function(type, cb, opts) {
            var self = (this == null) ? window : this;
            if (!self.__et_listeners) self.__et_listeners = {};
            // Read all options first (spec requires Get even if cb is null)
            var capture, once, passive, passiveExplicit, signal;
            if (opts && typeof opts === 'object' && opts !== null) {
                capture = !!opts.capture;
                once = !!opts.once;
                passiveExplicit = ('passive' in opts) && opts.passive !== undefined;
                passive = passiveExplicit ? !!opts.passive : false;
                signal = opts.signal;
            } else {
                capture = !!opts;
                once = false;
                passiveExplicit = false;
                passive = false;
                signal = undefined;
            }
            // Passive-by-default for touch/wheel on window
            if (!passiveExplicit && __passiveDefaultTypes[type] && (self === window)) {
                passive = true;
            }
            if (passive) {
                if (!self.__passiveTypes) self.__passiveTypes = {};
                self.__passiveTypes[type] = true;
            }
            if (signal !== undefined) {
                if (!signal || typeof signal !== 'object' || !('aborted' in signal)) throw new TypeError("Failed to execute 'addEventListener': member signal is not of type AbortSignal.");
                if (signal.aborted) return;
            }
            if (typeof cb !== 'function' && !(cb && typeof cb === 'object')) return;
            var key = type + (capture ? '_c' : '_b');
            if (!self.__et_listeners[key]) self.__et_listeners[key] = [];
            for (var i = 0; i < self.__et_listeners[key].length; i++) {
                if (self.__et_listeners[key][i] === cb || self.__et_listeners[key][i]._origCb === cb) return;
            }
            var entry;
            if (once) {
                var wrapper = function(e) {
                    self.removeEventListener(type, cb, capture);
                    if (typeof cb === 'function') cb.call(self, e);
                    else cb.handleEvent(e);
                };
                wrapper._origCb = cb;
                wrapper._passive = passive;
                entry = wrapper;
            } else if (passive) {
                var wrapper = function(e) {
                    if (typeof cb === 'function') cb.call(self, e);
                    else cb.handleEvent(e);
                };
                wrapper._origCb = cb;
                wrapper._passive = true;
                entry = wrapper;
            } else {
                entry = cb;
            }
            self.__et_listeners[key].push(entry);
            if (signal) {
                signal.addEventListener('abort', function() {
                    self.removeEventListener(type, cb, capture);
                });
            }
        };
        EventTarget.prototype.removeEventListener = function(type, cb, opts) {
            if (!this.__et_listeners) return;
            var capture = (opts && typeof opts === 'object' && opts !== null) ? !!opts.capture : !!opts;
            var key = type + (capture ? '_c' : '_b');
            if (this.__et_listeners[key]) {
                this.__et_listeners[key] = this.__et_listeners[key].filter(function(f) { return f !== cb && f._origCb !== cb; });
            }
        };
        EventTarget.prototype.dispatchEvent = function(event) {
            var self = (this == null || this === undefined) ? window : this;
            if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
            if (event._initialized === false) throw new DOMException("The event is not initialized.", "InvalidStateError");
            // relatedTarget retargeting for non-DOM targets
            var origRelatedTarget = event.relatedTarget;
            if (origRelatedTarget !== null && origRelatedTarget !== undefined && origRelatedTarget.__nid !== undefined) {
                var retargetedNid = __jsRetarget(origRelatedTarget.__nid, -1);
                if (retargetedNid !== origRelatedTarget.__nid) {
                    event.relatedTarget = __w(retargetedNid);
                }
            }
            var __prevEvent = __currentEvent;
            __currentEvent = event;
            event._dispatching = true;
            event.target = self;
            event.srcElement = self;
            event.currentTarget = self;
            event._path = [self];
            event.eventPhase = 2;
            // At AT_TARGET, fire both capture and bubble listeners in registration order
            var phases = [event.type + '_c', event.type + '_b'];
            for (var ph = 0; ph < phases.length; ph++) {
                var key = phases[ph];
                var cbs = self.__et_listeners ? self.__et_listeners[key] : undefined;
                if (cbs) {
                    var snapshot = cbs.slice();
                    for (var i = 0; i < snapshot.length; i++) {
                        var fn = snapshot[i];
                        // Check if listener was removed (e.g. by abort signal) during dispatch
                        var live = self.__et_listeners[key];
                        if (live.indexOf(fn) === -1) continue;
                        var wasPassive = event._inPassiveListener;
                        if (fn._passive) event._inPassiveListener = true;
                        if (typeof fn === 'function') fn.call(self, event);
                        else if (fn && typeof fn.handleEvent === 'function') fn.handleEvent(event);
                        event._inPassiveListener = wasPassive;
                        if (event._stopImmediate) break;
                    }
                }
                if (event._stopImmediate || event._stopPropagation) break;
            }
            event._dispatching = false;
            event._stopPropagation = false;
            event._stopImmediate = false;
            event.currentTarget = null;
            event.eventPhase = 0;
            __currentEvent = __prevEvent;
            return !event.defaultPrevented;
        };
        Object.defineProperty(globalThis, 'EventTarget', {
            value: EventTarget, writable: true, configurable: true, enumerable: false
        });

        // Fix prototype chains: Node -> EventTarget, so Document/Element get addEventListener etc.
        if (typeof Node !== 'undefined') Object.setPrototypeOf(Node.prototype, EventTarget.prototype);
        if (typeof Window !== 'undefined') Object.setPrototypeOf(Window.prototype, EventTarget.prototype);

        // Wire XMLHttpRequest.dispatchEvent with proper window.event support
        if (typeof XMLHttpRequest !== 'undefined') {
            var _origXhrAddListener = XMLHttpRequest.prototype.addEventListener;
            var _origXhrRemoveListener = XMLHttpRequest.prototype.removeEventListener;
            XMLHttpRequest.prototype.dispatchEvent = function(event) {
                if (event._dispatching) throw new DOMException("The event is already being dispatched.", "InvalidStateError");
                if (event._initialized === false) throw new DOMException("The event is not initialized.", "InvalidStateError");
                // relatedTarget retargeting for non-DOM targets
                var origRelatedTarget = event.relatedTarget;
                if (origRelatedTarget !== null && origRelatedTarget !== undefined && origRelatedTarget.__nid !== undefined) {
                    var retargetedNid = __jsRetarget(origRelatedTarget.__nid, -1);
                    if (retargetedNid !== origRelatedTarget.__nid) {
                        event.relatedTarget = __w(retargetedNid);
                    }
                }
                var __prevEvent = __currentEvent;
                __currentEvent = event;
                event._dispatching = true;
                event.target = this;
                event.currentTarget = this;
                event._path = [this];
                event.eventPhase = 2;
                var cbs = this._listeners && this._listeners[event.type];
                if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(this, event); }
                // Fire on* handler (e.g. onload, onerror)
                var handler = this['on' + event.type];
                if (typeof handler === 'function') handler.call(this, event);
                event._dispatching = false;
                event.currentTarget = null;
                event.eventPhase = 0;
                __currentEvent = __prevEvent;
                return !event.defaultPrevented;
            };
        }

        // CharacterData prototype — between Node.prototype and Text/Comment
        // JS-side cache for character data: preserves lone surrogates that can't
        // round-trip through Rust String (UTF-8). Keyed by __nid.
        var __cdCache = new Map();
        var CharacterData = function CharacterData() {};
        CharacterData.prototype = Object.create(EP);
        CharacterData.prototype.constructor = CharacterData;
        Object.defineProperties(CharacterData.prototype, {
            data: {
                get: function() {
                    if (__cdCache.has(this.__nid)) return __cdCache.get(this.__nid);
                    return __n_getCharData(this.__nid);
                },
                set: function(v) {
                    var old = this.data;
                    var s = v === null ? '' : String(v);
                    __cdCache.set(this.__nid, s);
                    __n_setCharData(this.__nid, s);
                    if (typeof __mo_notify === 'function') __mo_notify('characterData', this, {oldValue: old});
                },
                configurable: true
            },
            length: {
                get: function() {
                    if (__cdCache.has(this.__nid)) return __cdCache.get(this.__nid).length;
                    return __n_charDataLength(this.__nid);
                },
                configurable: true
            },
        });
        CharacterData.prototype.substringData = function(offset, count) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'substringData' on 'CharacterData': 2 arguments required, but only " + arguments.length + " present.");
            offset = offset >>> 0; count = count >>> 0;
            var d = this.data;
            if (offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            return d.substring(offset, offset + count);
        };
        CharacterData.prototype.appendData = function(data) {
            if (arguments.length < 1) throw new TypeError("Failed to execute 'appendData' on 'CharacterData': 1 argument required, but only 0 present.");
            this.data = this.data + String(data);
        };
        CharacterData.prototype.insertData = function(offset, data) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'insertData' on 'CharacterData': 2 arguments required, but only " + arguments.length + " present.");
            offset = offset >>> 0;
            var d = this.data;
            if (offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            this.data = d.substring(0, offset) + String(data) + d.substring(offset);
        };
        CharacterData.prototype.deleteData = function(offset, count) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'deleteData' on 'CharacterData': 2 arguments required, but only " + arguments.length + " present.");
            offset = offset >>> 0; count = count >>> 0;
            var d = this.data;
            if (offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            var end = offset + count;
            if (end > d.length) end = d.length;
            this.data = d.substring(0, offset) + d.substring(end);
        };
        CharacterData.prototype.replaceData = function(offset, count, data) {
            if (arguments.length < 3) throw new TypeError("Failed to execute 'replaceData' on 'CharacterData': 3 arguments required, but only " + arguments.length + " present.");
            offset = offset >>> 0; count = count >>> 0;
            var d = this.data;
            if (offset > d.length) throw new DOMException('Index or size is negative, or greater than the allowed value', 'IndexSizeError');
            var end = offset + count;
            if (end > d.length) end = d.length;
            this.data = d.substring(0, offset) + String(data) + d.substring(end);
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
            get: function() {
                var result = '';
                var n = this;
                while (n.previousSibling && n.previousSibling.nodeType === 3) {
                    n = n.previousSibling;
                }
                while (n && n.nodeType === 3) {
                    result += n.data;
                    n = n.nextSibling;
                }
                return result;
            },
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
            this._value = value || '';
            var qn = name || '';
            var colonIdx = qn.indexOf(':');
            var ln = colonIdx >= 0 ? qn.substring(colonIdx + 1) : qn;
            var pfx = prefix !== undefined ? prefix : (colonIdx >= 0 ? qn.substring(0, colonIdx) : null);
            var props = {
                nodeType: 2,
                name: qn,
                localName: ln,
                namespaceURI: ns || null,
                prefix: pfx || null,
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
            var self = this;
            var valueDef = {
                get: function() { return self._value; },
                set: function(v) {
                    var s = String(v);
                    self._value = s;
                    if (self.ownerElement && self.ownerElement.setAttribute) {
                        self.ownerElement.setAttribute(self.name, s);
                    }
                },
                enumerable: true, configurable: true
            };
            Object.defineProperty(this, 'value', valueDef);
            Object.defineProperty(this, 'nodeValue', valueDef);
            Object.defineProperty(this, 'textContent', valueDef);
        }
        Attr.prototype = Object.create(EP);
        Attr.prototype.constructor = Attr;
        Attr.prototype.cloneNode = function() {
            var clone = new Attr(this.name, this.value, this.namespaceURI, this.prefix);
            clone.localName = this.localName;
            return clone;
        };
        globalThis.Attr = Attr;

        // Document constructor is defined earlier (line ~898) as a factory function.
        // Set Document.prototype to inherit from EP so wrapped document nodes get element methods.
        var DocCtor = globalThis.Document;
        DocCtor.prototype = Object.create(EP);
        DocCtor.prototype.constructor = DocCtor;

        // DOMImplementation constructor (for instanceof checks)
        function DOMImplementation() {}
        DOMImplementation.prototype = Object.create(Object.getPrototypeOf(document.implementation) || {});
        DOMImplementation.prototype.constructor = DOMImplementation;
        Object.setPrototypeOf(document.implementation, DOMImplementation.prototype);
        globalThis.DOMImplementation = DOMImplementation;

        // DocumentType constructor (for instanceof checks)
        function DocumentType() {}
        DocumentType.prototype = Object.create(EP);
        DocumentType.prototype.constructor = DocumentType;
        Object.defineProperties(DocumentType.prototype, {
            name: { get: function() { return __n_getDoctypeName(this.__nid); }, configurable: true },
            publicId: { get: function() { return __n_getDoctypePublicId(this.__nid); }, configurable: true },
            systemId: { get: function() { return __n_getDoctypeSystemId(this.__nid); }, configurable: true },
            nodeName: { get: function() { return __n_getDoctypeName(this.__nid); }, configurable: true }
        });
        globalThis.DocumentType = DocumentType;

        // XMLDocument constructor (type marker per spec — no additional methods)
        function XMLDocument() {}
        XMLDocument.prototype = Object.create(Document.prototype);
        XMLDocument.prototype.constructor = XMLDocument;
        globalThis.XMLDocument = XMLDocument;

        function DocumentFragment() {
            var nid = __n_createDocFragment();
            var w = __w(nid);
            Object.setPrototypeOf(w, DocumentFragment.prototype);
            return w;
        }
        DocumentFragment.prototype = Object.create(EP);
        DocumentFragment.prototype.constructor = DocumentFragment;
        globalThis.DocumentFragment = DocumentFragment;

        // Re-wire ShadowRoot prototype chain since DocumentFragment was replaced
        Object.setPrototypeOf(ShadowRoot.prototype, DocumentFragment.prototype);
        ShadowRoot.prototype.constructor = ShadowRoot;

        function ProcessingInstruction() {}
        ProcessingInstruction.prototype = Object.create(CharacterData.prototype);
        ProcessingInstruction.prototype.constructor = ProcessingInstruction;
        Object.defineProperty(ProcessingInstruction.prototype, 'target', {
            get: function() { return __n_getPITarget(this.__nid); },
            configurable: true
        });
        globalThis.ProcessingInstruction = ProcessingInstruction;

        // Wire global document to Document.prototype
        // nodeId 0 is always the Document node (DomTree::new() allocates it first)
        document.__nid = 0;
        document.__props = document.__props || {};
        _cache[0] = document;
        Object.setPrototypeOf(document, Document.prototype);

        // Add Document-specific methods to Document.prototype
        // (Global doc's own-property methods shadow these, but standalone documents inherit them)
        Document.prototype.createElement = function(tag) { return document.createElement(tag); };
        Document.prototype.createElementNS = function(ns, tag) { return document.createElementNS(ns, tag); };
        Document.prototype.createTextNode = function(text) { return document.createTextNode(text); };
        Document.prototype.createComment = function(text) { return document.createComment(text); };
        Document.prototype.createDocumentFragment = function() { return document.createDocumentFragment(); };
        Document.prototype.createProcessingInstruction = function(t, d) { return document.createProcessingInstruction(t, d); };
        Document.prototype.createCDATASection = function(data) { return document.createCDATASection(data); };
        Document.prototype.createAttribute = function(n) { return document.createAttribute(n); };
        Document.prototype.createAttributeNS = function(ns, qn) { return document.createAttributeNS(ns, qn); };
        Document.prototype.createEvent = function(type) {
            var key = String(type).toLowerCase();
            if (key === 'touchevent' && !('ontouchstart' in document)) {
                throw new DOMException("Failed to execute 'createEvent' on 'Document': The provided event type ('" + type + "') is invalid.", 'NotSupportedError');
            }
            var ctorName = _createEventAliases[key];
            if (!ctorName) {
                throw new DOMException("Failed to execute 'createEvent' on 'Document': The provided event type ('" + type + "') is invalid.", 'NotSupportedError');
            }
            var Ctor = globalThis[ctorName];
            var e = new Ctor('');
            e._initialized = false;
            e.type = '';
            return e;
        };
        Document.prototype.getElementById = function(id) {
            var sid = String(id);
            var de = this.documentElement;
            if (!de || !de.querySelector) return null;
            return de.querySelector('[id="' + sid.replace(/"/g, '\\"') + '"]');
        };
        Document.prototype.querySelector = function(sel) {
            var id = __n_querySelector(0, sel, 0);
            return id >= 0 ? __w(id) : null;
        };
        Document.prototype.querySelectorAll = function(sel) {
            return __makeStaticNodeList(__n_querySelectorAll(0, sel, 0).map(__w));
        };
        Document.prototype.getElementsByTagName = function(tag) {
            var de = this.documentElement;
            if (!de || !de.querySelectorAll) return __makeHTMLCollection(function() { return []; });
            return __makeHTMLCollection(function() { return de.querySelectorAll(tag); });
        };
        Document.prototype.getElementsByClassName = function(cls) {
            var de = this.documentElement;
            if (!de || !de.querySelectorAll) return __makeHTMLCollection(function() { return []; });
            return __makeHTMLCollection(function() { return de.querySelectorAll('.' + cls); });
        };

        Document.prototype.adoptNode = function(node) { return doc.adoptNode.call(this, node); };
        Document.prototype.importNode = function(node, deep) { return doc.importNode.call(this, node, deep); };
        Document.prototype.cloneNode = function(deep) {
            var de = this.documentElement;
            var clonedDE = de ? de.cloneNode(!!deep) : null;
            var newDoc = __makeDocumentLike(clonedDE);
            Object.setPrototypeOf(newDoc, Object.getPrototypeOf(this));
            // Clone doctype if present — insert into Rust tree before documentElement
            var dt = this.doctype;
            if (dt && deep) {
                var clonedDT = dt.cloneNode(false);
                if (clonedDE && clonedDE.__nid !== undefined) {
                    __n_insertBefore(newDoc.__nid, clonedDT.__nid, clonedDE.__nid);
                } else {
                    __n_appendChild(newDoc.__nid, clonedDT.__nid);
                }
            }
            if (this.contentType) newDoc.contentType = this.contentType;
            return newDoc;
        };

        // HTMLStyleElement.sheet → lazily creates a CSSStyleSheet
        Object.defineProperty(HTMLStyleElement.prototype, 'sheet', {
            get: function() {
                if (!this.__sheet) {
                    this.__sheet = new CSSStyleSheet();
                    this.__sheet.__ownerNode = this;
                }
                return this.__sheet;
            },
            configurable: true
        });

        // HTMLLinkElement.sheet → empty CSSStyleSheet (many sites check link.sheet)
        Object.defineProperty(HTMLLinkElement.prototype, 'sheet', {
            get: function() {
                if (!this.__sheet) {
                    this.__sheet = new CSSStyleSheet();
                    this.__sheet.__ownerNode = this;
                }
                return this.__sheet;
            },
            configurable: true
        });

        // === Unify DOM prototype chains ===
        // Copy Node methods (EP) onto the real Node.prototype from dom_stubs
        Object.defineProperties(globalThis.Node.prototype, Object.getOwnPropertyDescriptors(EP));
        // Wire: Element.prototype → __ElemProto → Node.prototype
        Object.setPrototypeOf(__ElemProto, globalThis.Node.prototype);
        Object.setPrototypeOf(globalThis.Element.prototype, __ElemProto);
        Object.setPrototypeOf(Document.prototype, globalThis.Node.prototype);
        Object.setPrototypeOf(DocumentType.prototype, globalThis.Node.prototype);
        Object.setPrototypeOf(DocumentFragment.prototype, globalThis.Node.prototype);

        // DocumentFragment also gets querySelector/querySelectorAll
        DocumentFragment.prototype.querySelector = function(sel) {
            if (this.__nid === undefined) return null;
            var nid = __n_querySelector(this.__nid, sel, this.__nid);
            return nid >= 0 ? __w(nid) : null;
        };
        DocumentFragment.prototype.querySelectorAll = function(sel) {
            if (this.__nid === undefined) return __makeStaticNodeList([]);
            return __makeStaticNodeList(__n_querySelectorAll(this.__nid, sel, this.__nid).map(__w));
        };
        DocumentFragment.prototype.getElementById = function(id) {
            if (this.__nid === undefined || !id) return null;
            var nid = __n_querySelector(this.__nid, '[id="' + id.replace(/"/g, '\\"') + '"]', this.__nid);
            return nid >= 0 ? __w(nid) : null;
        };

        // CE upgrade/lifecycle helpers — these have access to _cache inside the IIFE
        globalThis.__ceUpgradeAll = function(name, ctor, observedAttrs) {
            var els = document.querySelectorAll(name);
            for (var i = 0; i < els.length; i++) {
                __ceUpgradeElement(els[i], ctor, observedAttrs);
            }
        };
        function __ceUpgradeElement(el, ctor, observedAttrs) {
            if (el.__ce_upgraded) return;
            el.__ce_upgraded = true;
            // Re-wrap with correct prototype
            delete _cache[el.__nid];
            Object.setPrototypeOf(el, ctor.prototype);
            el.constructor = ctor;
            _cache[el.__nid] = el;
            // Fire attributeChangedCallback for existing attributes
            if (typeof el.attributeChangedCallback === 'function' && observedAttrs.length > 0) {
                for (var j = 0; j < observedAttrs.length; j++) {
                    var aname = observedAttrs[j];
                    if (el.hasAttribute(aname)) {
                        el.attributeChangedCallback(aname, null, el.getAttribute(aname));
                    }
                }
            }
            // Fire connectedCallback if connected
            if (typeof el.connectedCallback === 'function' && __isConnected(el.__nid)) {
                el.connectedCallback();
            }
        }
        globalThis.__ceUpgradeTree = function(root) {
            if (typeof customElements === 'undefined' || !customElements._registry || !customElements._registry.size) return;
            customElements._registry.forEach(function(entry, name) {
                if (root.querySelectorAll) {
                    var els = root.querySelectorAll(name);
                    for (var i = 0; i < els.length; i++) {
                        __ceUpgradeElement(els[i], entry.ctor, entry.observedAttrs);
                    }
                }
            });
        };
        globalThis.__ceConnected = function(el) {
            if (el && el.__ce_upgraded) {
                __cePushReaction('connected', el);
            }
            if (el && el.__nid !== undefined) {
                var kids = __n_getAllChildIds(el.__nid);
                for (var i = 0; i < kids.length; i++) {
                    var child = __cache[kids[i]];
                    if (child) __ceConnected(child);
                }
            }
        };
        globalThis.__ceDisconnected = function(el) {
            if (el && el.__ce_upgraded) {
                __cePushReaction('disconnected', el);
            }
            if (el && el.__nid !== undefined) {
                var kids = __n_getAllChildIds(el.__nid);
                for (var i = 0; i < kids.length; i++) {
                    var child = __cache[kids[i]];
                    if (child) __ceDisconnected(child);
                }
            }
        };

        // Wire window event methods to EventTarget.prototype (spec: Window extends EventTarget)
        window.addEventListener = EventTarget.prototype.addEventListener;
        window.removeEventListener = EventTarget.prototype.removeEventListener;
        window.dispatchEvent = EventTarget.prototype.dispatchEvent;
    "#
}
