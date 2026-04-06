/// Core element prototype: shared helpers, attribute methods,
/// selectors/collections, node traversal, CharacterData methods,
/// attribute node methods, and Node-level defineProperties(EP, {...}).
pub(crate) fn element_prototype_js() -> &'static str {
    r#"
        // ElemProto inherits from EP (Node prototype).
        // Element-specific methods go on ElemProto, Node methods stay on EP.
        var ElemProto = Object.create(EP);
        globalThis.__ElemProto = ElemProto;

        // Cache key for __attrCache: includes namespace to avoid collisions
        function __attrCK(name, ns) { return (ns || '') + '\0' + name; }

        // Per spec: HTML elements in HTML documents ASCII-lowercase attribute names
        function __asciiLower(s) {
            for (var i = 0, r = ''; i < s.length; i++) {
                var c = s.charCodeAt(i);
                r += (c >= 65 && c <= 90) ? String.fromCharCode(c + 32) : s[i];
            }
            return r;
        }
        function __attrName(el, name) {
            var n = String(name);
            if (!el.namespaceURI || el.namespaceURI === 'http://www.w3.org/1999/xhtml') n = __asciiLower(n);
            return n;
        }

        // Generic DOMTokenList factory for attribute-backed token lists (sizes, relList, sandbox)
        function __makeDOMTokenList(el, attrName) {
            var cacheKey = '__dtl_' + attrName;
            if (el[cacheKey]) { el[cacheKey]._sync(); return el[cacheKey]; }
            function _tokens() { var raw=(el.getAttribute(attrName)||'').split(/\s+/).filter(Boolean),seen={},out=[]; for(var i=0;i<raw.length;i++){if(!seen[raw[i]]){seen[raw[i]]=true;out.push(raw[i]);}} return out; }
            function _validateToken(t) { if(t==='') throw new DOMException("The token provided must not be empty.","SyntaxError"); if(/\s/.test(t)) throw new DOMException("The token provided ('"+t+"') contains HTML space characters, which are not valid in tokens.","InvalidCharacterError"); }
            var obj = Object.create(DOMTokenList.prototype);
            function _update(c) { if(el.hasAttribute(attrName)||c.length>0) el.setAttribute(attrName,c.join(' ')); obj._sync(); }
            obj.add = function() { for(var i=0;i<arguments.length;i++) _validateToken(String(arguments[i])); var c=_tokens(); for(var i=0;i<arguments.length;i++){var s=String(arguments[i]);if(c.indexOf(s)<0) c.push(s);} _update(c); };
            obj.remove = function() { for(var i=0;i<arguments.length;i++) _validateToken(String(arguments[i])); var c=_tokens(); for(var i=0;i<arguments.length;i++){var s=String(arguments[i]);var idx=c.indexOf(s);if(idx>=0)c.splice(idx,1);} _update(c); };
            obj.contains = function(cls) { return _tokens().indexOf(String(cls))>=0; };
            obj.toggle = function(cls,force) { _validateToken(String(cls)); if(force!==undefined){if(force){var c=_tokens();if(c.indexOf(String(cls))<0){c.push(String(cls));_update(c);}return true;}else{var c=_tokens();var idx=c.indexOf(String(cls));if(idx>=0){c.splice(idx,1);_update(c);}return false;}} var c=_tokens();var idx=c.indexOf(String(cls));if(idx>=0){c.splice(idx,1);_update(c);return false;}c.push(String(cls));_update(c);return true; };
            obj.replace = function(o, n) { var os=String(o),ns=String(n); _validateToken(os); _validateToken(ns); var c=_tokens(); if(c.indexOf(os)<0) return false; var first=-1; for(var i=0;i<c.length;i++){if(c[i]===os||c[i]===ns){first=i;break;}} c[first]=ns; for(var i=c.length-1;i>=0;i--){if(i!==first&&(c[i]===os||c[i]===ns))c.splice(i,1);} _update(c); return true; };
            obj.item = function(i) { var c=_tokens(); return (i>=0&&i<c.length)?c[i]:null; };
            obj.toString = function() { return el.getAttribute(attrName)||''; };
            Object.defineProperty(obj, 'value', { get: function() { return el.getAttribute(attrName)||''; }, set: function(v) { el.setAttribute(attrName, v); obj._sync(); }, configurable: true });
            obj._sync = function() { var c = _tokens(); for (var i = c.length; i < (obj.length || 0); i++) delete obj[i]; obj.length = c.length; for (var i = 0; i < c.length; i++) obj[i] = c[i]; };
            obj._sync();
            el[cacheKey] = obj;
            return obj;
        }

        ElemProto.getAttribute = function(name) {
            name = __attrName(this, name);
            var v = __n_getAttribute(this.__nid, name);
            return __n_hasAttrValue(this.__nid, name) ? v : null;
        };
        ElemProto.setAttribute = function(name, value) {
            name = String(name);
            if (name === '') throw new DOMException("The string contains invalid characters.", "InvalidCharacterError");
            name = __attrName(this, name);
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
            name = __attrName(this, name);
            var old = __n_hasAttrValue(this.__nid, name) ? __n_getAttribute(this.__nid, name) : null;
            // Clear ownerElement on cached Attr — find the actual attr being removed
            if (this.__attrCache) {
                // Look up full attr info to get correct cache key (name may match across namespaces)
                var full = JSON.parse(__n_getAttributesFull(this.__nid));
                for (var fi = 0; fi < full.length; fi++) {
                    if (full[fi].name === name || full[fi].localName === name) {
                        var fck = __attrCK(full[fi].name, full[fi].ns);
                        if (this.__attrCache[fck]) {
                            this.__attrCache[fck].ownerElement = null;
                            delete this.__attrCache[fck];
                        }
                        break;
                    }
                }
            }
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
        ElemProto.hasAttribute = function(name) { return __n_hasAttribute(this.__nid, __attrName(this, name)); };
        ElemProto.hasAttributes = function() { return __n_hasAttributes(this.__nid); };

        ElemProto.querySelector = function(sel) {
            var id = __n_querySelector(this.__nid, sel, this.__nid);
            return id >= 0 ? __w(id) : null;
        };
        ElemProto.querySelectorAll = function(sel) {
            return __makeStaticNodeList(__n_querySelectorAll(this.__nid, sel, this.__nid).map(__w));
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
            return __makeHTMLCollection(function() { return __getElemsByClassName(self, cls); });
        };
        Object.defineProperty(ElemProto, 'attributes', {
            get: function() {
                if (this.__nid === undefined) return undefined;
                if (this.__cachedAttrsProxy) return this.__cachedAttrsProxy;
                var el = this;
                if (!el.__attrCache) el.__attrCache = {};
                function getAttrs() {
                    var full = JSON.parse(__n_getAttributesFull(el.__nid));
                    var attrs = [];
                    for (var i = 0; i < full.length; i++) {
                        var a = full[i];
                        var ck = __attrCK(a.name, a.ns);
                        // Reuse cached Attr if available (preserves identity from setNamedItem)
                        if (el.__attrCache[ck]) {
                            var cached = el.__attrCache[ck];
                            cached._value = a.value;
                            attrs.push(cached);
                        } else {
                            var attr = new Attr(a.name, a.value, a.ns, a.prefix);
                            attr.localName = a.localName;
                            attr.ownerElement = el;
                            attr.__ownerDoc = el.__ownerDoc;
                            el.__attrCache[ck] = attr;
                            attrs.push(attr);
                        }
                    }
                    return attrs;
                }
                var target = Object.create(NamedNodeMap.prototype);
                target.__el = el;
                target.__getAttrs = getAttrs;
                var proxy = new Proxy(target, {
                    get: function(t, p) {
                        // Internal properties needed by prototype methods
                        if (p === '__el' || p === '__getAttrs') return t[p];
                        if (p === 'length') return t.__getAttrs().length;
                        if (p === Symbol.iterator) { var a = t.__getAttrs(); return function() { return a[Symbol.iterator](); }; }
                        if (__isArrayIndex(p)) return t.__getAttrs()[p >>> 0];
                        // Prototype methods take priority over named attr access
                        var proto = NamedNodeMap.prototype[p];
                        if (proto !== undefined) return proto;
                        // Inherited methods (toString etc.) take priority
                        if (typeof p === 'string' && p in Object.prototype) return Object.prototype[p];
                        // Named access by attribute name
                        if (typeof p === 'string') {
                            var attrs = t.__getAttrs();
                            for (var i = 0; i < attrs.length; i++) if (attrs[i].name === p) return attrs[i];
                        }
                        return undefined;
                    },
                    ownKeys: function(t) {
                        var attrs = t.__getAttrs();
                        var keys = [];
                        for (var i = 0; i < attrs.length; i++) keys.push(String(i));
                        var elNs = el.namespaceURI;
                        var docCt = (el.ownerDocument || document).contentType;
                        var htmlFilter = elNs === 'http://www.w3.org/1999/xhtml' && docCt === 'text/html';
                        var seen = {};
                        for (var i = 0; i < attrs.length; i++) {
                            var n = attrs[i].name;
                            if (htmlFilter && n !== n.toLowerCase()) continue;
                            if (!seen[n]) { keys.push(n); seen[n] = true; }
                        }
                        return keys;
                    },
                    has: function(t, p) {
                        if (__isArrayIndex(p)) return (p >>> 0) < t.__getAttrs().length;
                        if (p === 'length' || p in NamedNodeMap.prototype) return true;
                        if (typeof p === 'string') {
                            var attrs = t.__getAttrs();
                            for (var i = 0; i < attrs.length; i++) if (attrs[i].name === p) return true;
                        }
                        return p in Object.prototype;
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
                this.__cachedAttrsProxy = proxy;
                return proxy;
            },
            enumerable: true, configurable: true
        });
        EP.contains = function(other) {
            if (!other || other.__nid === undefined || this.__nid === undefined) {
                // Handle non-Rust-backed nodes: walk parentNode chain in JS
                if (this === other) return true;
                if (!other) return false;
                var node = other;
                while (node) {
                    if (node === this) return true;
                    node = node.parentNode;
                }
                return false;
            }
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
            // Capture ownerDocument BEFORE mutation (tree walk changes after replace)
            var parentDoc = this.ownerDocument || (this.nodeType === 9 ? this : document);
            var childDoc = (newChild && newChild.__nid !== undefined) ? (newChild.ownerDocument || document) : null;
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
            // Adopt: update ownerDocument if moving between documents
            if (childDoc && parentDoc !== childDoc) {
                __adoptSubtree(newChild, parentDoc);
            }
            // Removed node retains ownerDocument per spec
            if (oldChild && oldChild.__nid !== undefined) {
                oldChild.__ownerDoc = parentDoc;
            }
            __ceFlushReactions();
            return oldChild;
        };
        EP.hasChildNodes = function() { return __n_getFirstChild(this.__nid) >= 0; };

        EP.isSameNode = function(other) { return this === other; };

        EP.isEqualNode = function(other) {
            if (other === null) return false;
            if (this === other) return true;
            if (this.__nid === undefined || other.__nid === undefined) return false;
            return __n_isEqualNode(this.__nid, other.__nid);
        };

        EP.normalize = function() {
            if (this.__nid === undefined) return;
            __n_normalize(this.__nid);
        };

        EP.lookupNamespaceURI = function(prefix) {
            if (prefix === '') prefix = null;
            if (prefix !== undefined && prefix !== null) prefix = String(prefix);
            var nt = this.nodeType;
            if (nt === 1) {
                // Built-in prefix mappings per spec (only for elements)
                if (prefix === 'xml') return 'http://www.w3.org/XML/1998/namespace';
                if (prefix === 'xmlns') return 'http://www.w3.org/2000/xmlns/';
                var ns = this.namespaceURI;
                if (ns && this.prefix === prefix) return ns;
                // Check xmlns attributes
                if (this.__nid !== undefined) {
                    var attrs = JSON.parse(__n_getAttributesFull(this.__nid));
                    for (var i = 0; i < attrs.length; i++) {
                        var a = attrs[i];
                        if (a.ns === 'http://www.w3.org/2000/xmlns/') {
                            if (a.prefix === 'xmlns' && a.name.substring(6) === prefix) return a.value || null;
                            if (prefix === null && a.name === 'xmlns' && !a.prefix) return a.value || null;
                        }
                    }
                }
                var pe = this.parentElement;
                return pe ? pe.lookupNamespaceURI(prefix) : null;
            }
            if (nt === 9) {
                var de = this.documentElement;
                return de ? de.lookupNamespaceURI(prefix) : null;
            }
            if (nt === 10 || nt === 11) return null;
            var pe = this.parentElement;
            return pe ? pe.lookupNamespaceURI(prefix) : null;
        };

        EP.isDefaultNamespace = function(ns) {
            if (ns === '') ns = null;
            if (ns === undefined) ns = null;
            var defaultNs = this.lookupNamespaceURI(null);
            return defaultNs === ns;
        };

        EP.lookupPrefix = function(ns) {
            if (ns === null || ns === undefined || ns === '') return null;
            var nt = this.nodeType;
            if (nt === 1) return __lookupPrefixOnElement(this, ns);
            if (nt === 9) {
                var de = this.documentElement;
                return de ? __lookupPrefixOnElement(de, ns) : null;
            }
            if (nt === 10 || nt === 11) return null;
            var pe = this.parentElement;
            return pe ? __lookupPrefixOnElement(pe, ns) : null;
        };

        function __lookupPrefixOnElement(el, ns) {
            if (el.namespaceURI === ns && el.prefix !== null) {
                if (el.lookupNamespaceURI(el.prefix) === ns) return el.prefix;
            }
            if (el.__nid !== undefined) {
                var attrs = JSON.parse(__n_getAttributesFull(el.__nid));
                for (var i = 0; i < attrs.length; i++) {
                    var a = attrs[i];
                    if (a.ns === 'http://www.w3.org/2000/xmlns/' && a.prefix === 'xmlns' && a.value === ns) {
                        var localPart = a.name.substring(6);
                        if (el.lookupNamespaceURI(localPart) === ns) return localPart;
                    }
                }
            }
            var pe = el.parentElement;
            return pe ? __lookupPrefixOnElement(pe, ns) : null;
        }

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

        // Ensure __attrCache exists and return cached or fresh Attr for a given qualified name
        function __getOrCacheAttr(el, name) {
            if (!el.__attrCache) el.__attrCache = {};
            // For non-namespaced lookup, use null ns
            var ck = __attrCK(name, null);
            if (el.__attrCache[ck]) {
                var cached = el.__attrCache[ck];
                cached._value = el.getAttribute(name);
                return cached;
            }
            // Build from attributes list to get full ns/prefix/localName info
            var full = JSON.parse(__n_getAttributesFull(el.__nid));
            for (var i = 0; i < full.length; i++) {
                if (full[i].name === name) {
                    var a = full[i];
                    var ack = __attrCK(a.name, a.ns);
                    if (el.__attrCache[ack]) return el.__attrCache[ack];
                    var attr = new Attr(a.name, a.value, a.ns, a.prefix);
                    attr.localName = a.localName;
                    attr.ownerElement = el;
                    attr.__ownerDoc = el.__ownerDoc;
                    el.__attrCache[ack] = attr;
                    return attr;
                }
            }
            return null;
        }
        ElemProto.getAttributeNode = function(name) {
            name = __attrName(this, name);
            if (!this.hasAttribute(name)) return null;
            return __getOrCacheAttr(this, name);
        };
        ElemProto.getAttributeNodeNS = function(ns, localName) {
            ns = (ns === null || ns === undefined) ? '' : String(ns);
            localName = String(localName);
            var json = __n_getAttributeNodeNS(this.__nid, ns, localName);
            if (!json) return null;
            var info = JSON.parse(json);
            var qualName = info.prefix ? (info.prefix + ':' + info.localName) : info.localName;
            if (!this.__attrCache) this.__attrCache = {};
            var ck = __attrCK(qualName, info.namespace || null);
            if (this.__attrCache[ck]) {
                var cached = this.__attrCache[ck];
                cached._value = info.value;
                return cached;
            }
            var attr = new Attr(qualName, info.value, info.namespace || null, info.prefix || null);
            attr.localName = info.localName;
            attr.ownerElement = this;
            attr.__ownerDoc = this.__ownerDoc;
            this.__attrCache[ck] = attr;
            return attr;
        };
        ElemProto.setAttributeNode = function(attr) {
            if (!attr || !(attr instanceof Attr)) throw new TypeError("Failed to execute 'setAttributeNode' on 'Element': parameter 1 is not of type 'Attr'.");
            if (attr.ownerElement && attr.ownerElement !== this) throw new DOMException("The attribute is in use.", "InUseAttributeError");
            // If attr has namespace, delegate to setAttributeNodeNS
            if (attr.namespaceURI) return this.setAttributeNodeNS(attr);
            if (!this.__attrCache) this.__attrCache = {};
            var name = __attrName(this, attr.name);
            var ck = __attrCK(name, null);
            // Check for existing attr with same name
            var oldAttr = null;
            if (this.hasAttribute(name)) {
                oldAttr = this.__attrCache[ck] || null;
                if (!oldAttr) oldAttr = __getOrCacheAttr(this, name);
                if (oldAttr === attr) return attr; // replacing by itself
                oldAttr.ownerElement = null;
                delete this.__attrCache[ck];
            }
            __n_setAttribute(this.__nid, name, attr.value);
            attr.ownerElement = this;
            attr.__ownerDoc = this.ownerDocument || document;
            this.__attrCache[ck] = attr;
            return oldAttr;
        };
        ElemProto.setAttributeNodeNS = function(attr) {
            if (!attr || !(attr instanceof Attr)) throw new TypeError("Failed to execute 'setAttributeNodeNS' on 'Element': parameter 1 is not of type 'Attr'.");
            if (attr.ownerElement && attr.ownerElement !== this) throw new DOMException("The attribute is in use.", "InUseAttributeError");
            if (!this.__attrCache) this.__attrCache = {};
            var ns = attr.namespaceURI || '';
            var localName = attr.localName;
            var qualName = attr.name;
            var ck = __attrCK(qualName, attr.namespaceURI);
            // Check for existing attr with same ns+localName
            var oldAttr = null;
            if (__n_hasAttributeNS(this.__nid, ns, localName)) {
                oldAttr = this.__attrCache[ck] || null;
                if (oldAttr === attr) return attr;
                if (oldAttr) { oldAttr.ownerElement = null; delete this.__attrCache[ck]; }
            }
            __n_setAttributeNS(this.__nid, ns, qualName, attr.value);
            attr.ownerElement = this;
            attr.__ownerDoc = this.ownerDocument || document;
            this.__attrCache[ck] = attr;
            return oldAttr;
        };
        ElemProto.removeAttributeNode = function(attr) {
            if (!attr || !(attr instanceof Attr)) throw new TypeError("Failed to execute 'removeAttributeNode' on 'Element': parameter 1 is not of type 'Attr'.");
            if (attr.ownerElement !== this) throw new DOMException("The node to be removed is not an attribute of this element.", "NotFoundError");
            var name = attr.name;
            if (attr.namespaceURI) {
                __n_removeAttributeNS(this.__nid, attr.namespaceURI, attr.localName);
            } else {
                __n_removeAttribute(this.__nid, name);
            }
            attr.ownerElement = null;
            if (this.__attrCache) delete this.__attrCache[__attrCK(name, attr.namespaceURI)];
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
            if (!other || (other.__nid === undefined && other.nodeType === undefined)) return 0;
            if (this === other) return 0;
            // If either node lacks __nid (non-Rust-backed, e.g. foreign/xml doc), they're disconnected
            if (other.__nid === undefined || this.__nid === undefined) {
                // DISCONNECTED | IMPLEMENTATION_SPECIFIC | PRECEDING or FOLLOWING
                // Use a consistent ordering based on some stable property
                var thisId = this.__nid !== undefined ? this.__nid : -1;
                var otherId = other.__nid !== undefined ? other.__nid : -2;
                var dir = otherId < thisId ? 2 : 4; // PRECEDING=2, FOLLOWING=4
                return 1 | 32 | dir; // DISCONNECTED | IMPLEMENTATION_SPECIFIC | dir
            }
            return __n_compareDocumentPosition(this.__nid, other.__nid);
        };

        // === Node-level properties (stay on EP) ===
        Object.defineProperties(EP, {
            textContent: {
                get: function() {
                    if (this.__nid === undefined) return '';
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 9 || nt === 10) return null;
                    return __n_getTextContent(this.__nid);
                },
                set: function(v) {
                    if (this.__nid === undefined) return;
                    var nt = __n_getNodeType(this.__nid);
                    // Document and Doctype: setting textContent is a no-op
                    if (nt === 9 || nt === 10) return;
                    // CharacterData nodes (Text=3, Comment=8, PI=7, CDATA=4): set data directly
                    if (nt === 3 || nt === 8 || nt === 7 || nt === 4) {
                        __n_setCharData(this.__nid, v === null || v === undefined ? '' : String(v));
                        return;
                    }
                    // Element and DocumentFragment: remove all children, optionally add text node
                    var removedNodes = [];
                    var isElement = (nt === 1);
                    if (isElement && typeof __mo_notify === 'function') {
                        var kids = this.childNodes;
                        for (var i = 0; i < kids.length; i++) removedNodes.push(kids[i]);
                    }
                    var str = v === null || v === undefined ? '' : String(v);
                    if (str === '') {
                        // Remove all children, don't create a text node
                        __n_removeAllChildren(this.__nid);
                    } else {
                        __n_setTextContent(this.__nid, str);
                    }
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
                if (nt === 10) { var dn = __n_getDoctypeName(this.__nid); return dn || ''; }
                if (nt === 11) return '#document-fragment';
                if (nt === 7) return __n_getPITarget(this.__nid) || '';
                if (nt === 1) return this.tagName;
                return __n_getTagName(this.__nid) || '#node';
            }, configurable: true },
            nodeType: { get: function() { if (this.__nid === undefined) return undefined; return __n_getNodeType(this.__nid); }, configurable: true },
            baseURI: {
                get: function() {
                    var doc = this.ownerDocument || (this.nodeType === 9 ? this : document);
                    if (doc && doc.URL) return doc.URL;
                    return (typeof location !== 'undefined' && location.href) || '';
                },
                configurable: true
            },
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
                get: function() {
                    if (this.__nid === undefined) return [];
                    if (this.__childNodesList) return this.__childNodesList;
                    var self = this;
                    var list = new Proxy(Object.create(NodeList.prototype), {
                        get: function(t, p) {
                            if (p === Symbol.iterator) return Array.prototype[Symbol.iterator];
                            if (p === 'keys') return Array.prototype.keys;
                            if (p === 'values') return Array.prototype.values;
                            if (p === 'entries') return Array.prototype.entries;
                            if (p === 'forEach') return Array.prototype.forEach;
                            var kids = __n_getAllChildIds(self.__nid).map(__w);
                            if (p === 'length') return kids.length;
                            if (typeof p === 'string' && p === (p >>> 0).toString() && (p >>> 0) !== 0xFFFFFFFF) return kids[p >>> 0];
                            if (p === 'item') return function(i) { return (i >= 0 && i < kids.length) ? kids[i] : null; };
                            return t[p];
                        },
                        has: function(t, p) {
                            if (p === 'length' || p === 'item' || p === 'forEach' || p === 'keys' || p === 'values' || p === 'entries' || p === Symbol.iterator) return true;
                            if (typeof p === 'string' && p === (p >>> 0).toString() && (p >>> 0) !== 0xFFFFFFFF) return (p >>> 0) < __n_getAllChildIds(self.__nid).length;
                            return p in t;
                        },
                        ownKeys: function() {
                            var kids = __n_getAllChildIds(self.__nid);
                            var keys = [];
                            for (var i = 0; i < kids.length; i++) keys.push(String(i));
                            return keys;
                        },
                        getOwnPropertyDescriptor: function(t, p) {
                            if (p === 'length') return { value: __n_getAllChildIds(self.__nid).length, writable: false, enumerable: false, configurable: true };
                            if (typeof p === 'string' && p === (p >>> 0).toString() && (p >>> 0) !== 0xFFFFFFFF) {
                                var kids = __n_getAllChildIds(self.__nid);
                                var idx = p >>> 0;
                                if (idx < kids.length) return { value: __w(kids[idx]), writable: false, enumerable: true, configurable: true };
                            }
                            return Object.getOwnPropertyDescriptor(t, p);
                        }
                    });
                    this.__childNodesList = list;
                    return list;
                },
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
                    if (nt === 3 || nt === 8 || nt === 7) return __n_getNodeValue(this.__nid);
                    return null;
                },
                set: function(v) {
                    if (this.__nid === undefined) return;
                    var nt = __n_getNodeType(this.__nid);
                    if (nt === 3 || nt === 8 || nt === 7) __n_setCharData(this.__nid, v === null ? '' : String(v));
                },
                configurable: true
            },
            ownerDocument: { get: function() {
                if (this.__ownerDoc) return this.__ownerDoc;
                if (this.__nid === undefined) return document;
                var cur = __n_getParent(this.__nid);
                while (cur >= 0) {
                    var w = _cache[cur];
                    if (w && w.__ownerDoc) return w.__ownerDoc;
                    if (__n_getNodeType(cur) === 9) {
                        w = __w(cur);
                        return w.__ownerDoc || w;
                    }
                    cur = __n_getParent(cur);
                }
                return document;
            }, configurable: true },
            isConnected: {
                get: function() {
                    if (this.__nid === undefined) return false;
                    return __isConnected(this.__nid);
                },
                configurable: true
            },
        });
    "#
}
