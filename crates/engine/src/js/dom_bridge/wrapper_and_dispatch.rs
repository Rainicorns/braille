/// Wrapper factory: _ctorMap, __w, dirty values, window.event legacy getter.
pub(super) fn wrapper_factory_js() -> &'static str {
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
    "#
}

/// Constructors + prototype wiring: Node, Document, EventTarget, CharacterData,
/// Text, Comment, Attr, NamedNodeMap, DocumentType, XMLDocument, DocumentFragment,
/// ProcessingInstruction, mixins, CE lifecycle.
pub(super) fn constructors_and_wiring_js() -> &'static str {
    r#"
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
            // Fire on<type> IDL handler (e.g. onload, onmessage) — consistent with DOM dispatch
            if (!event._stopImmediate) {
                var handlerName = 'on' + event.type;
                var handler = self[handlerName];
                if (typeof handler === 'function') {
                    var ret = handler.call(self, event);
                    if (ret === false && event.cancelable) event.preventDefault();
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

        // NamedNodeMap constructor and prototype
        function NamedNodeMap() {}
        NamedNodeMap.prototype.item = function(i) {
            var attrs = this.__getAttrs ? this.__getAttrs() : [];
            return attrs[i] || null;
        };
        NamedNodeMap.prototype.getNamedItem = function(n) {
            var attrs = this.__getAttrs ? this.__getAttrs() : [];
            for (var i = 0; i < attrs.length; i++) if (attrs[i].name === n) return attrs[i];
            return null;
        };
        NamedNodeMap.prototype.getNamedItemNS = function(ns, n) {
            var attrs = this.__getAttrs ? this.__getAttrs() : [];
            for (var i = 0; i < attrs.length; i++) if (attrs[i].localName === n) return attrs[i];
            return null;
        };
        NamedNodeMap.prototype.setNamedItem = function(a) {
            if (!a || !(a instanceof Attr)) throw new TypeError("Failed to execute 'setNamedItem' on 'NamedNodeMap': parameter 1 is not of type 'Attr'.");
            var el = this.__el;
            if (!el) return null;
            var inUse = a.ownerElement && a.ownerElement !== el && a.ownerElement.hasAttribute && a.ownerElement.hasAttribute(a.name);
            if (inUse) throw new DOMException("The attribute is in use.", "InUseAttributeError");
            el.setAttribute(a.name, a.value);
            a.ownerElement = el;
            a.__ownerDoc = el.ownerDocument || document;
            if (!el.__attrCache) el.__attrCache = {};
            el.__attrCache['\0' + a.name] = a;
            return a;
        };
        NamedNodeMap.prototype.setNamedItemNS = function(a) { return NamedNodeMap.prototype.setNamedItem.call(this, a); };
        NamedNodeMap.prototype.removeNamedItem = function(n) {
            var el = this.__el;
            if (!el) return null;
            if (!el.hasAttribute(n)) throw new DOMException("The attribute '" + n + "' was not found.", "NotFoundError");
            var ck = '\0' + n;
            var removed = (el.__attrCache && el.__attrCache[ck]) || null;
            if (!removed && this.__getAttrs) {
                var attrs = this.__getAttrs();
                for (var i = 0; i < attrs.length; i++) if (attrs[i].name === n) { removed = attrs[i]; break; }
            }
            el.removeAttribute(n);
            if (el.__attrCache) delete el.__attrCache[ck];
            if (removed) removed.ownerElement = null;
            return removed || null;
        };
        NamedNodeMap.prototype.removeNamedItemNS = function(ns, n) { return NamedNodeMap.prototype.removeNamedItem.call(this, n); };
        NamedNodeMap.prototype[Symbol.toStringTag] = 'NamedNodeMap';
        globalThis.NamedNodeMap = NamedNodeMap;

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
            return __makeHTMLCollection(function() { return __getElemsByClassName(de, cls); });
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

        // === Split EP into DOM interface mixins ===
        // Per spec, Node is the base. Mixins (ParentNode, ChildNode,
        // NonDocumentTypeChildNode) are applied only to the correct interfaces.

        // --- ParentNode mixin ---
        var __parentNodeMixin = {
            append: EP.append,
            prepend: EP.prepend,
            replaceChildren: EP.replaceChildren,
            moveBefore: EP.moveBefore,
        };
        ['children', 'firstElementChild', 'lastElementChild', 'childElementCount'].forEach(function(p) {
            Object.defineProperty(__parentNodeMixin, p, Object.getOwnPropertyDescriptor(EP, p));
            delete EP[p];
        });
        delete EP.append; delete EP.prepend; delete EP.replaceChildren; delete EP.moveBefore;

        // --- ChildNode mixin ---
        var __childNodeMixin = {
            before: EP.before,
            after: EP.after,
            replaceWith: EP.replaceWith,
            remove: EP.remove,
        };
        delete EP.before; delete EP.after; delete EP.replaceWith; delete EP.remove;

        // --- NonDocumentTypeChildNode mixin ---
        var __nonDocTypeChildNodeMixin = {};
        ['nextElementSibling', 'previousElementSibling'].forEach(function(p) {
            Object.defineProperty(__nonDocTypeChildNodeMixin, p, Object.getOwnPropertyDescriptor(EP, p));
            delete EP[p];
        });

        // --- CharacterData-only methods (already on CharacterData.prototype) ---
        delete EP.substringData; delete EP.appendData; delete EP.insertData;
        delete EP.deleteData; delete EP.replaceData;

        // --- Element-only methods ---
        __ElemProto.focus = EP.focus; delete EP.focus;
        __ElemProto.blur = EP.blur; delete EP.blur;
        __ElemProto.requestFullscreen = EP.requestFullscreen; delete EP.requestFullscreen;

        // === Unify DOM prototype chains ===
        // Copy remaining EP (now Node-only) onto Node.prototype
        Object.defineProperties(globalThis.Node.prototype, Object.getOwnPropertyDescriptors(EP));
        // Wire prototype chains per DOM spec hierarchy
        Object.setPrototypeOf(__ElemProto, globalThis.Node.prototype);
        Object.setPrototypeOf(globalThis.Element.prototype, __ElemProto);
        Object.setPrototypeOf(Document.prototype, globalThis.Node.prototype);
        Object.setPrototypeOf(DocumentType.prototype, globalThis.Node.prototype);
        Object.setPrototypeOf(DocumentFragment.prototype, globalThis.Node.prototype);
        // Fix CharacterData: was Object.create(EP), now re-parent to Node.prototype
        Object.setPrototypeOf(CharacterData.prototype, globalThis.Node.prototype);

        // === Apply mixins to correct prototypes ===
        // ParentNode → Element, Document, DocumentFragment
        [__ElemProto, Document.prototype, DocumentFragment.prototype].forEach(function(proto) {
            Object.defineProperties(proto, Object.getOwnPropertyDescriptors(__parentNodeMixin));
        });
        // ChildNode → Element, CharacterData, DocumentType
        [__ElemProto, CharacterData.prototype, DocumentType.prototype].forEach(function(proto) {
            Object.defineProperties(proto, Object.getOwnPropertyDescriptors(__childNodeMixin));
        });
        // NonDocumentTypeChildNode → Element, CharacterData
        [__ElemProto, CharacterData.prototype].forEach(function(proto) {
            Object.defineProperties(proto, Object.getOwnPropertyDescriptors(__nonDocTypeChildNodeMixin));
        });

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
            // Call the constructor via the upgrade target mechanism
            __ceUpgradeTarget = el;
            try { new ctor(); } catch(e) { __ceUpgradeTarget = null; }
            __ceUpgradeTarget = null;
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
