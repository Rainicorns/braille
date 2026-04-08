//! Shared DOM helpers used by both dom_bridge (inside the IIFE) and user-facing
//! APIs. These are registered as globals BEFORE dom_bridge runs.
//! Includes: __makeHTMLCollection, __isConnected, __mo_notify, CE reaction queue,
//! __ceFlushReactions, __jsRetarget, DOMParser.
//! Rule: if dom_bridge calls it AND it must be accessible outside the IIFE, it goes here.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(DOM_HELPERS_JS).unwrap();
}

/// Run after dom_bridge to make all interface objects non-enumerable (per spec).
/// Must run late because dom_bridge defines Document, Text, Comment, etc.
pub(super) fn finalize(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(FINALIZE_JS).unwrap();
}

const DOM_HELPERS_JS: &str = r#"
        // Stub document so early code doesn't crash (overwritten by dom_bridge)
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
                if (ns === 'http://www.w3.org/1999/xhtml' && el.getAttribute('name') === s) return el;
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
                    // Expandos on target shadow everything (including item/namedItem)
                    if (typeof p === 'string' && Object.prototype.hasOwnProperty.call(t, p)) return t[p];
                    // item/namedItem: return prototype methods (same reference identity)
                    if (p === 'item') return HTMLCollection.prototype.item;
                    if (p === 'namedItem') return HTMLCollection.prototype.namedItem;
                    // Named item access (skip empty string and JS internals)
                    if (typeof p === 'string' && p !== '' && p !== 'then' && p !== 'toJSON' && p !== 'constructor' && p !== '__proto__') {
                        var found = __findNamed(live, p);
                        if (found) return found;
                    }
                    // Fall through to prototype chain (HTMLCollection.prototype → Object.prototype)
                    var proto = HTMLCollection.prototype;
                    if (p in proto) return proto[p];
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
                        if (ns === 'http://www.w3.org/1999/xhtml') {
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
                set: function(t, p, value, receiver) {
                    // Derived objects (Object.create(collection)) can set own properties freely
                    if (receiver !== proxy) {
                        Object.defineProperty(receiver, p, {value: value, writable: true, enumerable: true, configurable: true});
                        return true;
                    }
                    // Array indices: always reject on the collection itself
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
                },
                getPrototypeOf: function() { return HTMLCollection.prototype; }
            });
            return proxy;
        };

        globalThis.__getElemsByClassName = function(root, classNames) {
            var s = String(classNames);
            var tokens = s.split(/[\t\n\f\r ]+/);
            var filtered = [];
            for (var i = 0; i < tokens.length; i++) {
                if (tokens[i] !== '') filtered.push(tokens[i]);
            }
            if (filtered.length === 0) return [];
            var all = root.querySelectorAll('*');
            var result = [];
            for (var i = 0; i < all.length; i++) {
                var el = all[i];
                var cls = el.getAttribute('class');
                if (!cls) continue;
                var elTokens = cls.split(/[\t\n\f\r ]+/);
                var match = true;
                for (var j = 0; j < filtered.length; j++) {
                    if (elTokens.indexOf(filtered[j]) === -1) { match = false; break; }
                }
                if (match) result.push(el);
            }
            return result;
        };

        // MutationObserver — functional implementation
        (function() {
            var observers = [];
            var pendingDeliver = false;

            function MutationRecord(type, target) {
                this.type = type; this.target = target;
                this.addedNodes = []; this.removedNodes = [];
                this.attributeName = null; this.attributeNamespace = null;
                this.oldValue = null;
                this.previousSibling = null; this.nextSibling = null;
            }

            function queueRecord(record) {
                for (var i = 0; i < observers.length; i++) {
                    var obs = observers[i];
                    for (var j = 0; j < obs._targets.length; j++) {
                        var entry = obs._targets[j];
                        var target = record.target;
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
                        var rec = new MutationRecord(record.type, record.target);
                        rec.addedNodes = record.addedNodes;
                        rec.removedNodes = record.removedNodes;
                        rec.attributeName = record.attributeName;
                        rec.attributeNamespace = record.attributeNamespace;
                        rec.previousSibling = record.previousSibling;
                        rec.nextSibling = record.nextSibling;
                        if (record.type === 'attributes' && entry.options.attributeOldValue) {
                            rec.oldValue = record.oldValue;
                        } else if (record.type === 'characterData' && entry.options.characterDataOldValue) {
                            rec.oldValue = record.oldValue;
                        } else {
                            rec.oldValue = null;
                        }
                        obs._records.push(rec);
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
                                obs._cb.call(obs, recs, obs);
                            }
                        }
                    });
                }
            }

            globalThis.MutationObserver = function(cb) {
                this._cb = cb; this._records = []; this._targets = [];
            };
            MutationObserver.prototype.observe = function(target, options) {
                if (!target || typeof target !== 'object') throw new TypeError("Failed to execute 'observe' on 'MutationObserver': parameter 1 is not of type 'Node'.");
                options = options || {};
                if (options.attributeOldValue || options.attributeFilter) {
                    if (options.attributes === undefined) options.attributes = true;
                }
                if (options.characterDataOldValue) {
                    if (options.characterData === undefined) options.characterData = true;
                }
                if (!options.childList && !options.attributes && !options.characterData) {
                    throw new TypeError("Failed to execute 'observe' on 'MutationObserver': The options object must set at least one of 'attributes', 'characterData', or 'childList' to true.");
                }
                if (options.attributeOldValue && options.attributes === false) {
                    throw new TypeError("Failed to execute 'observe' on 'MutationObserver': The options object may not set both 'attributeOldValue' to true and 'attributes' to false.");
                }
                if (options.attributeFilter && options.attributes === false) {
                    throw new TypeError("Failed to execute 'observe' on 'MutationObserver': The options object may not set 'attributeFilter' when 'attributes' is false.");
                }
                if (options.characterDataOldValue && options.characterData === false) {
                    throw new TypeError("Failed to execute 'observe' on 'MutationObserver': The options object may not set both 'characterDataOldValue' to true and 'characterData' to false.");
                }
                for (var i = 0; i < this._targets.length; i++) {
                    if (this._targets[i].target === target) {
                        this._targets[i].options = options;
                        if (observers.indexOf(this) < 0) observers.push(this);
                        return;
                    }
                }
                this._targets.push({target: target, options: options});
                if (observers.indexOf(this) < 0) observers.push(this);
            };
            MutationObserver.prototype.disconnect = function() {
                this._targets = [];
                this._records = [];
                var idx = observers.indexOf(this);
                if (idx >= 0) observers.splice(idx, 1);
            };
            MutationObserver.prototype.takeRecords = function() { return this._records.splice(0); };

            globalThis.__mo_notify = function(type, target, extra) {
                var r = new MutationRecord(type, target);
                if (extra) {
                    if (extra.addedNodes) r.addedNodes = extra.addedNodes;
                    if (extra.removedNodes) r.removedNodes = extra.removedNodes;
                    if (extra.attributeName !== undefined) r.attributeName = extra.attributeName;
                    if (extra.attributeNamespace !== undefined) r.attributeNamespace = extra.attributeNamespace;
                    if (extra.oldValue !== undefined) r.oldValue = extra.oldValue;
                    if (extra.previousSibling !== undefined) r.previousSibling = extra.previousSibling;
                    if (extra.nextSibling !== undefined) r.nextSibling = extra.nextSibling;
                }
                queueRecord(r);
            };
            globalThis.MutationRecord = MutationRecord;
        })();

        // Shared isConnected helper — walks parents and crosses shadow boundaries
        function __isConnected(nid) {
            var cur = nid;
            while (cur >= 0) {
                if (__n_getNodeType(cur) === 9) return true;
                var parent = __n_getParent(cur);
                if (parent < 0) {
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
        globalThis.__isConnectedToMainDoc = function(nid) {
            var cur = nid;
            while (cur >= 0) {
                if (cur === 0) return true;
                var parent = __n_getParent(cur);
                if (parent < 0) {
                    if (typeof __n_isShadowRoot === 'function' && __n_isShadowRoot(cur)) {
                        cur = __n_getShadowHost(cur);
                        continue;
                    }
                }
                cur = parent;
            }
            return false;
        };

        // CE reaction queue — batches connectedCallback/disconnectedCallback
        globalThis.__ceReactionQueue = [];
        globalThis.__ceBatchDepth = 0;
        globalThis.__cePushReaction = function(type, el) {
            __ceReactionQueue.push({type: type, el: el});
        };
        globalThis.__ceFlushReactions = function() {
            if (__ceBatchDepth > 0) return;
            while (__ceReactionQueue.length > 0) {
                var queue = __ceReactionQueue;
                __ceReactionQueue = [];
                for (var i = 0; i < queue.length; i++) {
                    var r = queue[i];
                    if (r.type === 'connected' && typeof r.el.connectedCallback === 'function') {
                        r.el.connectedCallback();
                    } else if (r.type === 'disconnected' && typeof r.el.disconnectedCallback === 'function') {
                        r.el.disconnectedCallback();
                    }
                }
            }
        };

        // JS retarget: retarget nodeA relative to nodeB (or null for non-node B)
        function __jsRetarget(aNid, bNid) {
            var a = aNid;
            while (true) {
                var root = a;
                var p = __n_getParent(root);
                while (p >= 0) { root = p; p = __n_getParent(root); }
                if (!__n_isShadowRoot(root)) return a;
                if (bNid >= 0) {
                    var bRoot = bNid;
                    var bp = __n_getParent(bRoot);
                    while (bp >= 0) { bRoot = bp; bp = __n_getParent(bRoot); }
                    if (bRoot === root) return a;
                }
                a = __n_getShadowHost(root);
            }
        }
        globalThis.__jsRetarget = __jsRetarget;

        // DOMParser — class definition (body references dom_bridge globals at call-time)
        globalThis.DOMParser = class DOMParser {
            parseFromString(str, type) {
                var ct = type || 'text/html';
                if (ct === 'text/html') {
                    var nodeIds = JSON.parse(__n_parseHTMLDocument(str));
                    var htmlEl = null;
                    var dtNode = null;
                    for (var i = 0; i < nodeIds.length; i++) {
                        var w = __w(nodeIds[i]);
                        if (w.nodeType === 10) dtNode = w;
                        else if (w.nodeType === 1 && w.tagName === 'HTML') htmlEl = w;
                    }
                    if (!htmlEl) {
                        htmlEl = document.createElement('html');
                        htmlEl.appendChild(document.createElement('head'));
                        htmlEl.appendChild(document.createElement('body'));
                    }
                    var newDoc = __makeDocumentLike(htmlEl);
                    newDoc.contentType = 'text/html';
                    if (dtNode) {
                        dtNode.__ownerDoc = newDoc;
                        __n_insertBefore(newDoc.__nid, dtNode.__nid, htmlEl.__nid);
                    }
                    __adoptSubtree(htmlEl, newDoc);
                    return newDoc;
                } else {
                    var div = document.createElement('div');
                    __n_setInnerHTML(div.__nid, str);
                    var rootEl = null;
                    var children = div.childNodes;
                    for (var i = 0; i < children.length; i++) {
                        if (children[i].nodeType === 1) { rootEl = children[i]; break; }
                    }
                    if (!rootEl) rootEl = div.firstChild;
                    if (rootEl && rootEl.parentNode) rootEl.parentNode.removeChild(rootEl);
                    var newDoc = __makeDocumentLike(rootEl);
                    newDoc.contentType = ct;
                    newDoc.createElement = function(tag) {
                        var nid = __n_createElement(tag);
                        var el = __w(nid);
                        el.__localName = String(tag);
                        el.__ownerDoc = newDoc;
                        if (ct === 'text/html' || ct === 'application/xhtml+xml') {
                            el.namespaceURI = 'http://www.w3.org/1999/xhtml';
                        } else {
                            el.namespaceURI = null;
                        }
                        return el;
                    };
                    if (rootEl) __adoptSubtree(rootEl, newDoc);
                    return newDoc;
                }
            }
        };

        // Named element access: walk subtree, register globalThis[id] for each element with an id.
        // Per HTML spec, elements with id are accessible as window properties.
        function __registerNamedElements(root) {
            if (!root || root.__nid === undefined) return;
            function walk(nid) {
                var w = __w(nid);
                if (w && w.nodeType === 1) {
                    var id = __n_getAttribute(nid, 'id');
                    if (id && !(id in globalThis)) globalThis[id] = w;
                }
                var kids = __n_getAllChildIds(nid);
                for (var i = 0; i < kids.length; i++) walk(kids[i]);
            }
            var kids = __n_getAllChildIds(root.__nid);
            for (var i = 0; i < kids.length; i++) walk(kids[i]);
        }
"#;

const FINALIZE_JS: &str = r#"
        // Make all interface objects non-enumerable on globalThis (per spec)
        ['Event', 'CustomEvent', 'UIEvent', 'FocusEvent', 'MouseEvent', 'KeyboardEvent',
         'InputEvent', 'AnimationEvent', 'TransitionEvent', 'WheelEvent', 'CompositionEvent',
         'ErrorEvent', 'GamepadEvent', 'PointerEvent', 'TouchEvent', 'Touch',
         'ClipboardEvent', 'DragEvent', 'PopStateEvent', 'HashChangeEvent',
         'PromiseRejectionEvent', 'StorageEvent', 'MessageChannel',
         'EventTarget', 'Node', 'Document', 'DocumentFragment', 'ShadowRoot',
         'DOMImplementation', 'XMLDocument', 'ProcessingInstruction', 'DocumentType',
         'Element', 'Attr', 'CharacterData', 'Text', 'Comment',
         'HTMLElement', 'HTMLIFrameElement', 'HTMLInputElement', 'HTMLTextAreaElement',
         'HTMLSelectElement', 'HTMLFormElement', 'HTMLAnchorElement', 'HTMLImageElement',
         'HTMLButtonElement', 'HTMLOptionElement', 'HTMLBodyElement', 'HTMLHeadElement',
         'HTMLFrameSetElement', 'HTMLHtmlElement', 'HTMLTitleElement', 'HTMLDivElement', 'HTMLSpanElement',
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
         'AbortController', 'AbortSignal',
         'DOMRect', 'DOMRectReadOnly', 'DOMPoint', 'DOMPointReadOnly',
         'DOMMatrix', 'DOMMatrixReadOnly',
         'BroadcastChannel', 'Notification', 'OffscreenCanvas',
         'ImageBitmap', 'ImageData', 'CanvasRenderingContext2D', 'CanvasGradient', 'CanvasPattern', 'Path2D',
         'IDBRequest', 'IDBDatabase', 'IDBTransaction', 'IDBObjectStore', 'IDBCursor', 'IDBIndex',
         'IDBOpenDBRequest', 'IDBVersionChangeEvent', 'IDBKeyRange'].forEach(function(name) {
            if (globalThis[name] !== undefined) {
                Object.defineProperty(globalThis, name, {
                    value: globalThis[name], writable: true, configurable: true, enumerable: false
                });
            }
        });
"#;
