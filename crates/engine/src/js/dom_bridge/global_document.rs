/// Document methods, properties, focus tracking, cookie jar, Range, DOMRect/Point/Matrix,
/// and DOMImplementation factory methods.
pub(super) fn global_document_js() -> &'static str {
    r#"
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

        // GlobalEventHandlers list (spec: Document and Window implement this mixin).
        // Defined here so both __makeDocumentLike and the main document setup can use it.
        var _globalEventHandlers = ['onclick', 'ondblclick', 'onmousedown', 'onmouseup',
            'onmouseover', 'onmouseout', 'onmousemove', 'onkeydown', 'onkeyup', 'onkeypress',
            'onchange', 'oninput', 'onbeforeinput', 'onsubmit', 'onreset', 'onselect',
            'onselectstart', 'onselectionchange',
            'onfocus', 'onblur', 'onfocusin', 'onfocusout',
            'onload', 'onerror', 'onabort', 'onresize',
            'oncopy', 'oncut', 'onpaste',
            'ondrag', 'ondragstart', 'ondragend', 'ondragover', 'ondragenter', 'ondragleave', 'ondrop',
            'onscroll', 'onscrollend',
            'ontouchstart', 'ontouchmove', 'ontouchend', 'ontouchcancel',
            'onpointerdown', 'onpointerup', 'onpointermove', 'onpointerover', 'onpointerout',
            'onpointerenter', 'onpointerleave', 'onpointercancel', 'ongotpointercapture', 'onlostpointercapture',
            'oncontextmenu', 'onwheel', 'onanimationstart', 'onanimationend', 'onanimationiteration',
            'ontransitionend', 'ontransitionrun', 'ontransitionstart', 'ontransitioncancel',
            'onwebkitanimationstart', 'onwebkitanimationend', 'onwebkitanimationiteration',
            'onwebkittransitionend'];

        function __installGlobalEventHandlers(obj) {
            _globalEventHandlers.forEach(function(attr) {
                if (!(attr in obj)) {
                    Object.defineProperty(obj, attr, {
                        get: function() { return this['_eh_' + attr] || null; },
                        set: function(v) { this['_eh_' + attr] = typeof v === 'function' ? v : null; },
                        enumerable: true,
                        configurable: true
                    });
                }
            });
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
                isConnected: true,
                location: null,
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
            __installGlobalEventHandlers(newDoc);
            Object.defineProperty(newDoc, 'title', {
                get: function() {
                    var t = newDoc.querySelector ? newDoc.querySelector('title') : null;
                    return t ? t.textContent : '';
                },
                set: function(v) {
                    var t = newDoc.querySelector ? newDoc.querySelector('title') : null;
                    if (!t) {
                        var head = newDoc.head;
                        if (head) {
                            var titleNid = __n_createElement('title');
                            __n_appendChild(head.__nid, titleNid);
                            t = __w(titleNid);
                        }
                    }
                    if (t) t.textContent = String(v);
                },
                configurable: true
            });
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
            newDoc.getElementsByClassName = function(cls) { var de = this.documentElement; return de ? __makeHTMLCollection(function() { return __getElemsByClassName(de, cls); }) : __makeHTMLCollection(function() { return []; }); };
            newDoc.createElement = function(tag) {
                var ct = newDoc.contentType;
                if (ct && ct !== 'text/html') {
                    // Non-HTML documents preserve case
                    var nid = __n_createElement(tag);
                    var el = __w(nid);
                    el.__localName = String(tag);
                    el.__ownerDoc = newDoc;
                    if (ct === 'application/xhtml+xml') el.namespaceURI = 'http://www.w3.org/1999/xhtml';
                    else el.namespaceURI = null;
                    return el;
                }
                var el = document.createElement(tag);
                el.__ownerDoc = newDoc;
                return el;
            };
            newDoc.createElementNS = function(ns, tag) { var el = document.createElementNS(ns, tag); el.__ownerDoc = newDoc; return el; };
            newDoc.createTextNode = function(text) { var n = document.createTextNode(text); n.__ownerDoc = newDoc; return n; };
            newDoc.createComment = function(text) { var n = document.createComment(text); n.__ownerDoc = newDoc; return n; };
            newDoc.createDocumentFragment = function() { var n = document.createDocumentFragment(); n.__ownerDoc = newDoc; return n; };
            newDoc.createProcessingInstruction = function(t, d) { var n = document.createProcessingInstruction(t, d); n.__ownerDoc = newDoc; return n; };
            newDoc.createCDATASection = function(data) {
                if (arguments.length < 1) throw new TypeError("Failed to execute 'createCDATASection' on 'Document': 1 argument required.");
                var ct = newDoc.contentType || 'application/xml';
                if (ct === 'text/html') throw new DOMException("Failed to execute 'createCDATASection' on 'Document': This document is an HTML document.", "NotSupportedError");
                var nid = __n_createCDATASection(String(data));
                var n = __w(nid);
                n.__ownerDoc = newDoc;
                return n;
            };
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
                if (this.__iframeRealm) {
                    if (!this.__writeBuffer) this.__writeBuffer = '';
                    this.__writeBuffer += html;
                    return;
                }
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
            newDoc.open = function() {
                this.__writeBuffer = '';
                return this;
            };
            newDoc.close = function() {
                if (this.__writeBuffer && this.__iframeRealm) {
                    var html = this.__writeBuffer;
                    this.__writeBuffer = null;
                    __braille_iframe_write_close(this.__iframeRealm, html, this.__iframeNodeId);
                }
            };
            newDoc.createRange = function() {
                var r = new BrailleRange();
                r.startContainer = newDoc; r.endContainer = newDoc;
                r.commonAncestorContainer = newDoc;
                return r;
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

        // Install GlobalEventHandlers on document (spec: Document implements GlobalEventHandlers).
        // Without this, feature detection like `"oninput" in document` returns false,
        // causing frameworks (React 18) to take IE fallback paths (attachEvent).
        __installGlobalEventHandlers(doc);

        Object.defineProperty(doc, 'ownerDocument', { value: null, writable: true, configurable: true });
        Object.defineProperty(doc, 'location', {
            get: function() { return (typeof window !== 'undefined') ? window.location : undefined; },
            set: function(v) { if (typeof window !== 'undefined') window.location = v; },
            configurable: true
        });
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
            if (sel === '') throw new DOMException("Document.querySelector: '' is not a valid selector", "SyntaxError");
            var nid = __n_querySelector(0, sel, 0);
            return nid >= 0 ? __w(nid) : null;
        };
        doc.querySelectorAll = function(sel) {
            if (sel === '') throw new DOMException("Document.querySelectorAll: '' is not a valid selector", "SyntaxError");
            return __makeStaticNodeList(__n_querySelectorAll(0, sel, 0).map(__w));
        };
        // Element name validation regex per WPT name-validation spec:
        // ASCII alpha start: subsequent chars must not be \0, \t, \n, \f, \r, space, /, >
        // :, _, or >= U+0080 start: subsequent chars only A-Za-z0-9, -, ., :, _, >= U+0080
        var __validElemNameRe = /^(?:[A-Za-z][^\0\t\n\f\r\u0020\u002f\u003e]*|[:\u005f\u0080-\u{10FFFF}][A-Za-z0-9\u002d\u002e\u003a\u005f\u0080-\u{10FFFF}]*)$/u;
        doc.createElement = function(tag) {
            tag = String(tag);
            if (!__validElemNameRe.test(tag)) {
                throw new DOMException("Failed to execute 'createElement' on 'Document': The tag name provided ('" + tag + "') is not a valid name.", "InvalidCharacterError");
            }
            var nid = __n_createElement(tag);
            var el = __w(nid);
            el.namespaceURI = 'http://www.w3.org/1999/xhtml';
            if (tag.toLowerCase() === 'template') {
                var contentId = __n_createTemplateContent(nid);
                var contentFrag = __w(contentId);
                // Per spec, template content belongs to an "associated inert document", not the template's own document
                if (!doc.__templateDoc) {
                    doc.__templateDoc = new Document();
                }
                contentFrag.__ownerDoc = doc.__templateDoc;
                contentFrag.__host = el;  // Mark as hosted fragment (per spec: "host" concept)
            }
            // Per spec, synchronously upgrade if tag matches a registered custom element
            if (typeof customElements !== 'undefined' && customElements._registry) {
                var entry = customElements._registry.get(tag.toLowerCase());
                if (entry) {
                    __ceUpgradeElement(el, entry.ctor, entry.observedAttrs);
                }
            }
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
            // Validate local name against element name rules
            if (!__validElemNameRe.test(localName)) {
                throw new DOMException("Failed to execute 'createElementNS' on 'Document': The qualified name provided ('" + qn + "') contains the invalid character.", "InvalidCharacterError");
            }
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
            return __makeHTMLCollection(function() { return __getElemsByClassName(doc.documentElement, cls); });
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
            var ln = name.toLowerCase();
            var attr = new Attr(ln, '', null, null);
            // createAttribute does NOT split on colon — the full name is the localName
            attr.localName = ln;
            attr.prefix = null;
            return attr;
        };

        doc.createAttributeNS = function(ns, qualifiedName) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'createAttributeNS' on 'Document': 2 arguments required.");
            var qn = String(qualifiedName);
            var nsStr = (ns === null || ns === undefined) ? '' : String(ns);
            var result = JSON.parse(__n_validateAndExtract(nsStr, qn));
            if (result.err) {
                var eName = result.err;
                throw new DOMException("Failed to execute 'createAttributeNS' on 'Document': " + (eName === 'InvalidCharacterError' ? "'" + qn + "' is not a valid attribute name." : "The namespace provided has an error."), eName);
            }
            var localName = result.ok.localName;
            // Validate local name as attribute name
            if (__isInvalidAttrName(localName)) {
                throw new DOMException("Failed to execute 'createAttributeNS' on 'Document': '" + qn + "' is not a valid attribute name.", "InvalidCharacterError");
            }
            var prefix = result.ok.prefix || null;
            var attr = new Attr(qn, '', ns === null ? null : String(ns), prefix);
            attr.localName = localName;
            return attr;
        };

        // Shared boundary-point comparator used by Range methods.
        // Returns -1 if (c1,o1) < (c2,o2), 0 if equal, 1 if greater.
        function __compareBP(c1, o1, c2, o2) {
            if (c1 === c2) return o1 < o2 ? -1 : o1 > o2 ? 1 : 0;
            var pos = c1.compareDocumentPosition(c2);
            if (pos & 16) {
                // c1 contains c2: find c2's ancestor that is a direct child of c1
                var cur = c2;
                while (cur.parentNode && cur.parentNode !== c1) cur = cur.parentNode;
                var idx = 0;
                var kids = c1.childNodes;
                for (var i = 0; i < kids.length; i++) { if (kids[i] === cur) { idx = i; break; } }
                return o1 <= idx ? -1 : 1;
            }
            if (pos & 8) {
                // c2 contains c1: find c1's ancestor that is a direct child of c2
                var cur = c1;
                while (cur.parentNode && cur.parentNode !== c2) cur = cur.parentNode;
                var idx = 0;
                var kids = c2.childNodes;
                for (var i = 0; i < kids.length; i++) { if (kids[i] === cur) { idx = i; break; } }
                return idx < o2 ? -1 : 1;
            }
            if (pos & 4) return -1; // c2 follows c1
            if (pos & 2) return 1;  // c2 precedes c1
            return 0;
        }

        // Get the root node of a node (walk parentNode to the top)
        function __getRootNode(node) {
            var cur = node;
            while (cur.parentNode) cur = cur.parentNode;
            return cur;
        }

        // Get the length of a node per spec (child count for elements, text length for text/comment/PI)
        function __nodeLength(node) {
            var t = node.nodeType;
            if (t === 3 || t === 4 || t === 7 || t === 8) return (node.data || '').length;
            return node.childNodes ? node.childNodes.length : 0;
        }

        function BrailleRange() {
            this.startContainer = doc; this.startOffset = 0;
            this.endContainer = doc; this.endOffset = 0;
            this.collapsed = true; this.commonAncestorContainer = doc;
        }
        BrailleRange.START_TO_START = 0; BrailleRange.START_TO_END = 1;
        BrailleRange.END_TO_END = 2; BrailleRange.END_TO_START = 3;
        BrailleRange.prototype.setStart = function(node, offset) {
            if (node.nodeType === 10) throw new DOMException("The supplied node is a doctype.", "InvalidNodeTypeError");
            offset = offset >>> 0;
            if (offset > __nodeLength(node)) throw new DOMException("The offset is larger than the node's length.", "IndexSizeError");
            this.startContainer = node; this.startOffset = offset;
            // If new start is after end, or in a different tree, collapse to start
            if (__getRootNode(node) !== __getRootNode(this.endContainer) ||
                __compareBP(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
                this.endContainer = this.startContainer; this.endOffset = this.startOffset;
            }
            this._update();
        };
        BrailleRange.prototype.setEnd = function(node, offset) {
            if (node.nodeType === 10) throw new DOMException("The supplied node is a doctype.", "InvalidNodeTypeError");
            offset = offset >>> 0;
            if (offset > __nodeLength(node)) throw new DOMException("The offset is larger than the node's length.", "IndexSizeError");
            this.endContainer = node; this.endOffset = offset;
            // If new end is before start, or in a different tree, collapse to end
            if (__getRootNode(node) !== __getRootNode(this.startContainer) ||
                __compareBP(this.endContainer, this.endOffset, this.startContainer, this.startOffset) < 0) {
                this.startContainer = this.endContainer; this.startOffset = this.endOffset;
            }
            this._update();
        };
        BrailleRange.prototype.setStartBefore = function(node) {
            if (!node.parentNode) throw new DOMException("The supplied node has no parent.", "InvalidNodeTypeError");
            this.setStart(node.parentNode, Array.prototype.indexOf.call(node.parentNode.childNodes, node));
        };
        BrailleRange.prototype.setStartAfter = function(node) {
            if (!node.parentNode) throw new DOMException("The supplied node has no parent.", "InvalidNodeTypeError");
            this.setStart(node.parentNode, Array.prototype.indexOf.call(node.parentNode.childNodes, node) + 1);
        };
        BrailleRange.prototype.setEndBefore = function(node) {
            if (!node.parentNode) throw new DOMException("The supplied node has no parent.", "InvalidNodeTypeError");
            this.setEnd(node.parentNode, Array.prototype.indexOf.call(node.parentNode.childNodes, node));
        };
        BrailleRange.prototype.setEndAfter = function(node) {
            if (!node.parentNode) throw new DOMException("The supplied node has no parent.", "InvalidNodeTypeError");
            this.setEnd(node.parentNode, Array.prototype.indexOf.call(node.parentNode.childNodes, node) + 1);
        };
        BrailleRange.prototype.selectNode = function(node) {
            if (!node.parentNode) throw new DOMException("The supplied node has no parent.", "InvalidNodeTypeError");
            this.setStartBefore(node); this.setEndAfter(node);
        };
        BrailleRange.prototype.selectNodeContents = function(node) {
            if (node.nodeType === 10) throw new DOMException("The supplied node is a doctype.", "InvalidNodeTypeError");
            this.startContainer = node; this.startOffset = 0;
            this.endContainer = node; this.endOffset = __nodeLength(node); this._update();
        };
        BrailleRange.prototype.collapse = function(toStart) { if (toStart) { this.endContainer = this.startContainer; this.endOffset = this.startOffset; } else { this.startContainer = this.endContainer; this.startOffset = this.endOffset; } this.collapsed = true; this.commonAncestorContainer = this.startContainer; };
        BrailleRange.prototype.cloneRange = function() { var r = new BrailleRange(); r.startContainer = this.startContainer; r.startOffset = this.startOffset; r.endContainer = this.endContainer; r.endOffset = this.endOffset; r._update(); return r; };
        BrailleRange.prototype.detach = function() {};
        BrailleRange.prototype.getBoundingClientRect = function() {
            var el = this.startContainer;
            if (el && el.nodeType === 3) el = el.parentNode;
            return el && el.getBoundingClientRect ? el.getBoundingClientRect() : {top:0,left:0,width:0,height:0,right:0,bottom:0,x:0,y:0};
        };
        BrailleRange.prototype.getClientRects = function() { return [this.getBoundingClientRect()]; };
        BrailleRange.prototype.toString = function() {
            var s = this.startContainer, so = this.startOffset;
            var e = this.endContainer, eo = this.endOffset;
            if (!s || !e) return '';
            if (s === e && s.nodeType === 3) return (s.data || '').substring(so, eo);
            // Walk DOM tree in order, collecting text from text nodes within the range
            var result = '';
            // Helper: next node in tree order
            function nextNode(n) {
                if (n.firstChild) return n.firstChild;
                while (n) { if (n.nextSibling) return n.nextSibling; n = n.parentNode; }
                return null;
            }
            // Find the first text node in/after the start boundary
            var cur;
            if (s.nodeType === 3) {
                result += (s.data || '').substring(so);
                cur = nextNode(s);
            } else {
                cur = s.childNodes[so] || nextNode(s);
            }
            // Determine the end boundary node for comparison
            var endNode;
            if (e.nodeType === 3) { endNode = e; }
            else { endNode = e.childNodes[eo] || null; }
            // Walk until we hit the end boundary
            while (cur) {
                if (cur === endNode) break;
                if (e.nodeType !== 3 && endNode === null) {
                    // endNode is null = end is after all children of e; stop if we pass e
                    var anc = cur;
                    var pastEnd = false;
                    while (anc) { if (anc === e) break; anc = anc.parentNode; }
                    if (!anc) pastEnd = true;
                    if (pastEnd) break;
                }
                if (cur.nodeType === 3) result += cur.data || '';
                cur = nextNode(cur);
            }
            // Partial end text node
            if (e.nodeType === 3 && e !== s) result += (e.data || '').substring(0, eo);
            return result;
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
        BrailleRange.prototype.deleteContents = function() {
            if (!this.startContainer) return;
            if (this.startContainer === this.endContainer && this.startContainer.nodeType === 3) {
                var t = this.startContainer.textContent || '';
                this.startContainer.textContent = t.substring(0, this.startOffset) + t.substring(this.endOffset);
            }
        };
        BrailleRange.prototype.extractContents = function() {
            var frag = document.createDocumentFragment();
            if (this.startContainer && this.startContainer.nodeType === 3) {
                var t = this.startContainer.textContent || '';
                var extracted = t.substring(this.startOffset, this.endOffset);
                this.startContainer.textContent = t.substring(0, this.startOffset) + t.substring(this.endOffset);
                frag.appendChild(document.createTextNode(extracted));
            }
            return frag;
        };
        BrailleRange.prototype.cloneContents = function() {
            var frag = document.createDocumentFragment();
            if (this.startContainer && this.startContainer.nodeType === 3) {
                var t = this.startContainer.textContent || '';
                frag.appendChild(document.createTextNode(t.substring(this.startOffset, this.endOffset)));
            }
            return frag;
        };
        BrailleRange.prototype.insertNode = function(node) {
            if (!this.startContainer) return;
            if (this.startContainer.nodeType === 3) {
                var parent = this.startContainer.parentNode;
                if (parent) {
                    var t = this.startContainer.textContent || '';
                    var before = document.createTextNode(t.substring(0, this.startOffset));
                    var after = document.createTextNode(t.substring(this.startOffset));
                    parent.insertBefore(before, this.startContainer);
                    parent.insertBefore(node, this.startContainer);
                    parent.insertBefore(after, this.startContainer);
                    parent.removeChild(this.startContainer);
                }
            } else if (this.startContainer.childNodes) {
                var ref = this.startContainer.childNodes[this.startOffset] || null;
                this.startContainer.insertBefore(node, ref);
            }
        };
        BrailleRange.prototype.surroundContents = function(newParent) {
            var contents = this.extractContents();
            this.insertNode(newParent);
            newParent.appendChild(contents);
            this.selectNode(newParent);
        };
        BrailleRange.prototype.compareBoundaryPoints = function(how, sourceRange) {
            // WebIDL unsigned short (ToUint16) conversion
            how = ((+how) >>> 0) % 65536;
            if (how > 3) throw new DOMException("Failed to execute 'compareBoundaryPoints' on 'Range': The comparison method provided must be one of 'START_TO_START', 'START_TO_END', 'END_TO_END', or 'END_TO_START'.", "NotSupportedError");
            if (!sourceRange || !(sourceRange instanceof BrailleRange)) throw new TypeError("Failed to execute 'compareBoundaryPoints' on 'Range': parameter 2 is not of type 'Range'.");
            // Check same root
            var thisRoot = __getRootNode(this.startContainer);
            var srcRoot = __getRootNode(sourceRange.startContainer);
            if (thisRoot !== srcRoot) throw new DOMException("The two ranges are not in the same tree.", "WrongDocumentError");
            var ac, ao, bc, bo;
            if (how === 0) { ac = this.startContainer; ao = this.startOffset; bc = sourceRange.startContainer; bo = sourceRange.startOffset; }
            else if (how === 1) { ac = this.endContainer; ao = this.endOffset; bc = sourceRange.startContainer; bo = sourceRange.startOffset; }
            else if (how === 2) { ac = this.endContainer; ao = this.endOffset; bc = sourceRange.endContainer; bo = sourceRange.endOffset; }
            else { ac = this.startContainer; ao = this.startOffset; bc = sourceRange.endContainer; bo = sourceRange.endOffset; }
            return __compareBP(ac, ao, bc, bo);
        };
        BrailleRange.prototype.isPointInRange = function(node, offset) {
            offset = offset >>> 0; // WebIDL unsigned long
            var nodeRoot = __getRootNode(node);
            var rangeRoot = __getRootNode(this.startContainer);
            if (nodeRoot !== rangeRoot) return false;
            if (node.nodeType === 10) throw new DOMException("The supplied node is a doctype.", "InvalidNodeTypeError");
            if (offset > __nodeLength(node)) throw new DOMException("The offset is larger than the node's length.", "IndexSizeError");
            if (__compareBP(node, offset, this.startContainer, this.startOffset) < 0) return false;
            if (__compareBP(node, offset, this.endContainer, this.endOffset) > 0) return false;
            return true;
        };
        BrailleRange.prototype.comparePoint = function(node, offset) {
            offset = offset >>> 0; // WebIDL unsigned long
            var nodeRoot = __getRootNode(node);
            var rangeRoot = __getRootNode(this.startContainer);
            if (nodeRoot !== rangeRoot) throw new DOMException("The two ranges are not in the same tree.", "WrongDocumentError");
            if (node.nodeType === 10) throw new DOMException("The supplied node is a doctype.", "InvalidNodeTypeError");
            if (offset > __nodeLength(node)) throw new DOMException("The offset is larger than the node's length.", "IndexSizeError");
            if (__compareBP(node, offset, this.startContainer, this.startOffset) < 0) return -1;
            if (__compareBP(node, offset, this.endContainer, this.endOffset) > 0) return 1;
            return 0;
        };
        BrailleRange.prototype.intersectsNode = function(node) {
            if (arguments.length < 1) throw new TypeError("Failed to execute 'intersectsNode' on 'Range': 1 argument required, but only 0 present.");
            if (!node || typeof node !== 'object' || node.nodeType === undefined) throw new TypeError("Failed to execute 'intersectsNode' on 'Range': parameter 1 is not of type 'Node'.");
            if (!this.startContainer || !this.endContainer) return false;
            // Per spec: if node's root is not the same as range's root, return false
            if (__getRootNode(node) !== __getRootNode(this.startContainer)) return false;
            var parent = node.parentNode;
            if (!parent) return true;
            var siblings = parent.childNodes;
            var offset = 0;
            for (var i = 0; i < siblings.length; i++) {
                if (siblings[i] === node) { offset = i; break; }
            }
            // (parent, offset) before range end AND (parent, offset+1) after range start
            return __compareBP(parent, offset, this.endContainer, this.endOffset) < 0 &&
                   __compareBP(this.startContainer, this.startOffset, parent, offset + 1) < 0;
        };
        // Track all live ranges for boundary adjustment during DOM mutations
        var __liveRanges = [];
        var __origSetStart = BrailleRange.prototype.setStart;
        var __origSetEnd = BrailleRange.prototype.setEnd;
        function __trackRange(r) { if (__liveRanges.indexOf(r) < 0) __liveRanges.push(r); }
        BrailleRange.prototype.setStart = function(node, offset) {
            __origSetStart.call(this, node, offset);
            __trackRange(this);
        };
        BrailleRange.prototype.setEnd = function(node, offset) {
            __origSetEnd.call(this, node, offset);
            __trackRange(this);
        };
        var __origSetStartBefore = BrailleRange.prototype.setStartBefore;
        BrailleRange.prototype.setStartBefore = function(node) {
            __origSetStartBefore.call(this, node);
            __trackRange(this);
        };
        var __origSetStartAfter = BrailleRange.prototype.setStartAfter;
        BrailleRange.prototype.setStartAfter = function(node) {
            __origSetStartAfter.call(this, node);
            __trackRange(this);
        };
        var __origSetEndBefore = BrailleRange.prototype.setEndBefore;
        BrailleRange.prototype.setEndBefore = function(node) {
            __origSetEndBefore.call(this, node);
            __trackRange(this);
        };
        var __origSetEndAfter = BrailleRange.prototype.setEndAfter;
        BrailleRange.prototype.setEndAfter = function(node) {
            __origSetEndAfter.call(this, node);
            __trackRange(this);
        };
        var __origSelectNode = BrailleRange.prototype.selectNode;
        BrailleRange.prototype.selectNode = function(node) {
            __origSelectNode.call(this, node);
            __trackRange(this);
        };
        var __origSelectNodeContents = BrailleRange.prototype.selectNodeContents;
        BrailleRange.prototype.selectNodeContents = function(node) {
            __origSelectNodeContents.call(this, node);
            __trackRange(this);
        };
        // Called by moveBefore/insertBefore/removeChild to adjust range boundaries
        // when a node is removed from its parent, per DOM spec §4.2.3 step 14.
        globalThis.__adjustRangesForRemoval = function(node, oldParent) {
            if (!__liveRanges.length) return;
            // Compute index of the node being removed (called before actual removal)
            var oldIndex = 0;
            if (oldParent && oldParent.childNodes) {
                var kids = oldParent.childNodes;
                for (var i = 0; i < kids.length; i++) {
                    if (kids[i] === node) { oldIndex = i; break; }
                }
            }
            for (var ri = 0; ri < __liveRanges.length; ri++) {
                var r = __liveRanges[ri];
                var changed = false;
                // Per spec §4.2.3 step 14: if start/end node is an inclusive
                // descendant of the removed node, set boundary to (parent, index).
                var sc = r.startContainer;
                while (sc) {
                    if (sc === node) {
                        r.startContainer = oldParent;
                        r.startOffset = oldIndex;
                        changed = true;
                        break;
                    }
                    sc = sc.parentNode;
                }
                var ec = r.endContainer;
                while (ec) {
                    if (ec === node) {
                        r.endContainer = oldParent;
                        r.endOffset = oldIndex;
                        changed = true;
                        break;
                    }
                    ec = ec.parentNode;
                }
                // Per spec: if start node is parent and start offset > index, decrement
                if (r.startContainer === oldParent && r.startOffset > oldIndex) {
                    r.startOffset--;
                    changed = true;
                }
                // Per spec: if end node is parent and end offset > index, decrement
                if (r.endContainer === oldParent && r.endOffset > oldIndex) {
                    r.endOffset--;
                    changed = true;
                }
                if (changed) r._update();
            }
        };
        // Per DOM spec: adjust live range boundaries when nodes are inserted.
        // `parent` is the container, `index` is the insertion index, `count` is the number of nodes inserted.
        globalThis.__adjustRangesForInsertion = function(parent, index, count) {
            if (!__liveRanges.length) return;
            for (var ri = 0; ri < __liveRanges.length; ri++) {
                var r = __liveRanges[ri];
                var changed = false;
                if (r.startContainer === parent && r.startOffset > index) {
                    r.startOffset += count;
                    changed = true;
                }
                if (r.endContainer === parent && r.endOffset > index) {
                    r.endOffset += count;
                    changed = true;
                }
                if (changed) r._update();
            }
        };
        // Per DOM spec "split" steps 8-9: adjust live range boundaries for splitText.
        globalThis.__adjustRangesForSplitText = function(node, offset, newNode) {
            if (!__liveRanges.length) return;
            var parent = node.parentNode;
            var nodeIdx = -1;
            if (parent) {
                var ch = parent.childNodes;
                for (var i = 0; i < ch.length; i++) {
                    if (ch[i] === node) { nodeIdx = i; break; }
                }
            }
            for (var ri = 0; ri < __liveRanges.length; ri++) {
                var r = __liveRanges[ri];
                var changed = false;
                // Step 8: if start node is node and start offset > offset,
                // set start node to newNode and startOffset -= offset
                if (r.startContainer === node && r.startOffset > offset) {
                    r.startContainer = newNode;
                    r.startOffset -= offset;
                    changed = true;
                }
                // Same for end
                if (r.endContainer === node && r.endOffset > offset) {
                    r.endContainer = newNode;
                    r.endOffset -= offset;
                    changed = true;
                }
                // Step 9: if start node is parent and start offset == nodeIdx + 1, increment
                if (parent && nodeIdx >= 0) {
                    if (r.startContainer === parent && r.startOffset === nodeIdx + 1) {
                        r.startOffset++;
                        changed = true;
                    }
                    if (r.endContainer === parent && r.endOffset === nodeIdx + 1) {
                        r.endOffset++;
                        changed = true;
                    }
                }
                if (changed) r._update();
            }
        };
        // Per DOM spec "replace data" step 2: adjust live range boundaries
        // when character data in `node` is replaced at (offset, count) with data of length addedCount.
        globalThis.__adjustRangesForCharData = function(node, offset, count, addedCount) {
            if (!__liveRanges.length) return;
            for (var ri = 0; ri < __liveRanges.length; ri++) {
                var r = __liveRanges[ri];
                var changed = false;
                if (r.startContainer === node) {
                    if (r.startOffset > offset + count) {
                        r.startOffset += addedCount - count;
                        changed = true;
                    } else if (r.startOffset > offset) {
                        r.startOffset = offset;
                        changed = true;
                    }
                }
                if (r.endContainer === node) {
                    if (r.endOffset > offset + count) {
                        r.endOffset += addedCount - count;
                        changed = true;
                    } else if (r.endOffset > offset) {
                        r.endOffset = offset;
                        changed = true;
                    }
                }
                if (changed) r._update();
            }
        };
        globalThis.Range = BrailleRange;
        doc.createRange = function() { return new BrailleRange(); };

        // StaticRange — non-live range (simple data holder)
        function StaticRange(init) {
            if (!init || typeof init !== 'object') throw new TypeError("Failed to construct 'StaticRange': 1 argument required.");
            if (init.startContainer === undefined || init.startContainer === null) throw new TypeError("Failed to construct 'StaticRange': startContainer is required.");
            if (init.startOffset === undefined) throw new TypeError("Failed to construct 'StaticRange': startOffset is required.");
            if (init.endContainer === undefined || init.endContainer === null) throw new TypeError("Failed to construct 'StaticRange': endContainer is required.");
            if (init.endOffset === undefined) throw new TypeError("Failed to construct 'StaticRange': endOffset is required.");
            var sc = init.startContainer, ec = init.endContainer;
            if (sc.nodeType === 10 || sc.nodeType === 2) throw new DOMException("The supplied node is a doctype or attribute.", "InvalidNodeTypeError");
            if (ec.nodeType === 10 || ec.nodeType === 2) throw new DOMException("The supplied node is a doctype or attribute.", "InvalidNodeTypeError");
            this.startContainer = sc;
            this.startOffset = init.startOffset >>> 0;
            this.endContainer = ec;
            this.endOffset = init.endOffset >>> 0;
            this.collapsed = (sc === ec && this.startOffset === this.endOffset);
        }
        globalThis.StaticRange = StaticRange;

        // DOMRect / DOMRectReadOnly
        function DOMRectReadOnly(x, y, width, height) {
            this.x = x || 0; this.y = y || 0; this.width = width || 0; this.height = height || 0;
        }
        Object.defineProperties(DOMRectReadOnly.prototype, {
            top: { get: function() { return Math.min(this.y, this.y + this.height); } },
            bottom: { get: function() { return Math.max(this.y, this.y + this.height); } },
            left: { get: function() { return Math.min(this.x, this.x + this.width); } },
            right: { get: function() { return Math.max(this.x, this.x + this.width); } },
        });
        DOMRectReadOnly.prototype.toJSON = function() {
            return {x:this.x,y:this.y,width:this.width,height:this.height,top:this.top,bottom:this.bottom,left:this.left,right:this.right};
        };
        DOMRectReadOnly.fromRect = function(o) { o = o || {}; return new DOMRectReadOnly(o.x, o.y, o.width, o.height); };
        function DOMRect(x, y, width, height) { DOMRectReadOnly.call(this, x, y, width, height); }
        DOMRect.prototype = Object.create(DOMRectReadOnly.prototype);
        DOMRect.prototype.constructor = DOMRect;
        DOMRect.fromRect = function(o) { o = o || {}; return new DOMRect(o.x, o.y, o.width, o.height); };
        globalThis.DOMRect = DOMRect;
        globalThis.DOMRectReadOnly = DOMRectReadOnly;

        // DOMPoint / DOMPointReadOnly
        function DOMPointReadOnly(x, y, z, w) { this.x = x || 0; this.y = y || 0; this.z = z || 0; this.w = w === undefined ? 1 : w; }
        DOMPointReadOnly.fromPoint = function(o) { o = o || {}; return new DOMPointReadOnly(o.x, o.y, o.z, o.w); };
        DOMPointReadOnly.prototype.toJSON = function() { return {x:this.x,y:this.y,z:this.z,w:this.w}; };
        function DOMPoint(x, y, z, w) { DOMPointReadOnly.call(this, x, y, z, w); }
        DOMPoint.prototype = Object.create(DOMPointReadOnly.prototype);
        DOMPoint.prototype.constructor = DOMPoint;
        DOMPoint.fromPoint = function(o) { o = o || {}; return new DOMPoint(o.x, o.y, o.z, o.w); };
        globalThis.DOMPoint = DOMPoint;
        globalThis.DOMPointReadOnly = DOMPointReadOnly;

        // DOMMatrix / DOMMatrixReadOnly (2D affine)
        function DOMMatrixReadOnly(init) {
            this.a = 1; this.b = 0; this.c = 0; this.d = 1; this.e = 0; this.f = 0;
            this.is2D = true; this.isIdentity = true;
            if (Array.isArray(init) && init.length >= 6) {
                this.a = init[0]; this.b = init[1]; this.c = init[2]; this.d = init[3]; this.e = init[4]; this.f = init[5];
                this.isIdentity = (this.a===1&&this.b===0&&this.c===0&&this.d===1&&this.e===0&&this.f===0);
            }
        }
        Object.defineProperties(DOMMatrixReadOnly.prototype, {
            m11:{get:function(){return this.a}}, m12:{get:function(){return this.b}},
            m21:{get:function(){return this.c}}, m22:{get:function(){return this.d}},
            m41:{get:function(){return this.e}}, m42:{get:function(){return this.f}},
        });
        DOMMatrixReadOnly.prototype.toFloat64Array = function() { return new Float64Array([this.a,this.b,this.c,this.d,this.e,this.f]); };
        function DOMMatrix(init) { DOMMatrixReadOnly.call(this, init); }
        DOMMatrix.prototype = Object.create(DOMMatrixReadOnly.prototype);
        DOMMatrix.prototype.constructor = DOMMatrix;
        DOMMatrix.prototype.translateSelf = function(tx, ty) { this.e += tx; this.f += (ty||0); this.isIdentity = false; return this; };
        DOMMatrix.prototype.scaleSelf = function(sx, sy) { sy = sy === undefined ? sx : sy; this.a *= sx; this.d *= sy; this.isIdentity = false; return this; };
        DOMMatrix.prototype.rotateSelf = function(angle) { var rad = angle * Math.PI / 180; var cos = Math.cos(rad); var sin = Math.sin(rad); var a = this.a; var b = this.b; this.a = a*cos+this.c*sin; this.b = b*cos+this.d*sin; this.c = -a*sin+this.c*cos; this.d = -b*sin+this.d*cos; this.isIdentity = false; return this; };
        DOMMatrix.prototype.invertSelf = function() { var det = this.a*this.d - this.b*this.c; if (det === 0) { this.a=NaN;this.b=NaN;this.c=NaN;this.d=NaN;this.e=NaN;this.f=NaN; return this; } var a=this.d/det,b=-this.b/det,c=-this.c/det,d=this.a/det,e=(this.c*this.f-this.d*this.e)/det,f=(this.b*this.e-this.a*this.f)/det; this.a=a;this.b=b;this.c=c;this.d=d;this.e=e;this.f=f; this.isIdentity = false; return this; };
        DOMMatrix.fromMatrix = function(o) { o = o || {}; return new DOMMatrix([o.a||1,o.b||0,o.c||0,o.d||1,o.e||0,o.f||0]); };
        globalThis.DOMMatrix = DOMMatrix;
        globalThis.DOMMatrixReadOnly = DOMMatrixReadOnly;

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
            if (arguments.length === 0 || root === null || root === undefined || root.__nid === undefined) {
                throw new TypeError("Failed to execute 'createTreeWalker' on 'Document': parameter 1 is not of type 'Node'.");
            }
            return new TreeWalker(root, whatToShow, filter);
        };
        doc.createNodeIterator = function(root, whatToShow, filter) {
            if (arguments.length === 0 || root === null || root === undefined || root.__nid === undefined) {
                throw new TypeError("Failed to execute 'createNodeIterator' on 'Document': parameter 1 is not of type 'Node'.");
            }
            return new NodeIterator(root, whatToShow, filter);
        };
        doc.evaluate = function(expression, contextNode, nsResolver, type, result) {
            return __xpathEvaluate(expression, contextNode, nsResolver, type, result);
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
            // ShadowRoot cannot be adopted per spec
            if (node.__nid !== undefined && __n_isShadowRoot(node.__nid)) throw new DOMException("Failed to execute 'adoptNode' on 'Document': ShadowRoot cannot be adopted.", "HierarchyRequestError");
            // DocumentFragment with host (template content, shadow root) — per spec, just return
            if (node.__host) return node;
            // Remove from old parent
            if (node.parentNode) {
                node.parentNode.removeChild(node);
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
            var ct = (this && this.contentType) || 'text/html';
            if (ct === 'text/html') throw new DOMException("Failed to execute 'createCDATASection' on 'Document': This document is an HTML document.", "NotSupportedError");
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
        // Document.prototype.write/writeln are defined in constructors_and_wiring
        // (after Document.prototype is set up to inherit from EP).

        // window.dispatchEvent assigned after EventTarget is defined (below)

        // Track focused element for document.activeElement.
        // Context object so Rust-side validate_focus_after_styles can reach
        // it via runtime.eval through the globalThis reference.
        var __focusCtx = { el: null };
        globalThis.__focusCtx = __focusCtx;
        // Hovered node nid for CSS :hover matching (synced to Rust tree via __n_setHoveredNode)
        globalThis.__hoveredNode = -1;
        EP.focus = function() {
            var prev = __focusCtx.el;
            if (prev === this) return;
            __focusCtx.el = this;
            __n_setFocusedNode(this.__nid !== undefined ? this.__nid : -1);
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
            if (__focusCtx.el !== this) return;
            __focusCtx.el = null;
            __n_setFocusedNode(-1);
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
            activeElement: { get: function() { return __focusCtx.el || doc.querySelector('body'); }, configurable: true },
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
            scripts: { get: function() {
                return new Proxy([], {
                    get: function(t, p) {
                        var live = doc.querySelectorAll('script');
                        if (p === 'length') return live.length;
                        if (p === 'item') return function(i) { return live[i] || null; };
                        if (p === Symbol.iterator) return function() { return live[Symbol.iterator](); };
                        if (typeof p === 'string' && !isNaN(p)) return live[parseInt(p)];
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
                        var titleText = document.createTextNode(String(title));
                        titleEl.appendChild(titleText);
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
                        // Validate local name against element name rules
                        if (!__validElemNameRe.test(result.ok.localName)) {
                            throw new DOMException("Failed to execute 'createDocument' on 'DOMImplementation': The qualified name provided ('" + qnVal + "') is not a valid name.", "InvalidCharacterError");
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
                        var attr = new Attr(name, '', null, null);
                        attr.localName = name;
                        attr.prefix = null;
                        return attr;
                    };
                    return newDoc;
                },
                createDocumentType: function(qualifiedName, publicId, systemId) {
                    if (arguments.length < 3) throw new TypeError("Failed to execute 'createDocumentType' on 'DOMImplementation': 3 arguments required.");
                    var qn = String(qualifiedName);
                    // DOCTYPE names only reject NUL, ASCII whitespace (\t \n \f \r space), and >
                    if (/[\0\t\n\f\r\u0020>]/.test(qn)) {
                        throw new DOMException("Failed to execute 'createDocumentType' on 'DOMImplementation': The qualified name provided is not a valid name.", "InvalidCharacterError");
                    }
                    var nid = __n_createDoctype(qn, String(publicId), String(systemId));
                    return __w(nid);
                },
                hasFeature: function() { return true; },
            }, configurable: true },
        });
    "#
}
