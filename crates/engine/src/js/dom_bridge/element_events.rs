/// Event methods: addEventListener, removeEventListener, dispatchEvent,
/// pointer capture, click(), dialog APIs (showModal/show/close).
pub(super) fn element_events_js() -> &'static str {
    r#"
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
    "#
}
