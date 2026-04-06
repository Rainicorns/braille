/// Element-specific `Object.defineProperties(ElemProto, {...})` block:
/// tagName, id, className, value, checked, disabled, style proxy, classList,
/// dataset, innerHTML, outerHTML, innerText, scroll metrics, offset dimensions,
/// content (template), validity, shadowRoot, open/returnValue.
pub(super) fn element_properties_js() -> &'static str {
    r#"
        // === Element-specific properties (on ElemProto) ===
        Object.defineProperties(ElemProto, {
            tagName: { get: function() {
                var prefix = this.prefix;
                var ln = this.localName;
                if (ln === undefined || ln === null) return __n_getTagName(this.__nid);
                var tn = prefix ? prefix + ':' + ln : ln;
                // HTML elements in HTML documents get uppercased tagName
                // In XML documents (contentType !== 'text/html'), preserve case
                if (this.namespaceURI === 'http://www.w3.org/1999/xhtml') {
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
                if (this.__prefix !== undefined) return this.__prefix;
                if (this.__nid !== undefined) {
                    var p = __n_getPrefix(this.__nid);
                    if (p) return p;
                }
                return null;
            }, set: function(v) { this.__prefix = v; }, configurable: true },
            namespaceURI: { get: function() {
                if (this.__namespaceURI !== undefined) return this.__namespaceURI;
                if (this.__nid !== undefined) {
                    var ns = __n_getNamespace(this.__nid);
                    if (ns) return ns;
                }
                return 'http://www.w3.org/1999/xhtml';
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
            // type: smart version with INPUT/BUTTON defaults is defined below (~line 1595)
            // disabled: defined below (~line 1570) — identical, just deduplicated
            // form: proper version with form-attribute lookup is in form_bindings.rs
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
            sizes: {
                get: function() {
                    if (this.tagName !== 'LINK') return undefined;
                    return __makeDOMTokenList(this, 'sizes');
                },
                configurable: true
            },
            relList: {
                get: function() {
                    var t = this.tagName;
                    if (t !== 'A' && t !== 'AREA' && t !== 'LINK') return undefined;
                    var supported = ['alternate','author','dns-prefetch','help','icon','license','modulepreload','nofollow','noopener','noreferrer','opener','prefetch','preconnect','preload','prerender','stylesheet','tag'];
                    return __makeDOMTokenList(this, 'rel', supported);
                },
                configurable: true
            },
            sandbox: {
                get: function() {
                    if (this.tagName !== 'IFRAME') return undefined;
                    var supported = ['allow-downloads','allow-forms','allow-modals','allow-orientation-lock','allow-pointer-lock','allow-popups','allow-popups-to-escape-sandbox','allow-presentation','allow-same-origin','allow-scripts','allow-top-navigation','allow-top-navigation-by-user-activation','allow-top-navigation-to-custom-protocols'];
                    return __makeDOMTokenList(this, 'sandbox', supported);
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
            shadowRoot: {
                get: function() {
                    if (this.__nid === undefined) return null;
                    var srId = __n_getShadowRootId(this.__nid);
                    if (srId < 0) return null;
                    // Only open shadow roots are exposed via .shadowRoot
                    var mode = __n_getShadowRootMode(srId);
                    if (mode !== 'open') return null;
                    return __w(srId);
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
