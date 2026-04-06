//! DOM collection types: NodeFilter, NodeList, HTMLCollection, DOMTokenList, Attr.
//! These define the interface objects and prototypes for DOM collections.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(COLLECTIONS_JS).unwrap();
}

const COLLECTIONS_JS: &str = r#"
        // NodeFilter constants (callback interface, not constructible)
        globalThis.NodeFilter = {
            FILTER_ACCEPT: 1,
            FILTER_REJECT: 2,
            FILTER_SKIP: 3,
            SHOW_ALL: 0xFFFFFFFF,
            SHOW_ELEMENT: 0x1,
            SHOW_ATTRIBUTE: 0x2,
            SHOW_TEXT: 0x4,
            SHOW_CDATA_SECTION: 0x8,
            SHOW_PROCESSING_INSTRUCTION: 0x40,
            SHOW_COMMENT: 0x80,
            SHOW_DOCUMENT: 0x100,
            SHOW_DOCUMENT_TYPE: 0x200,
            SHOW_DOCUMENT_FRAGMENT: 0x400
        };

        // NodeList class
        globalThis.NodeList = class NodeList {};
        Object.defineProperty(NodeList.prototype, 'length', {
            get: function() { return this.__nlLen !== undefined ? this.__nlLen : 0; },
            configurable: true
        });
        NodeList.prototype.item = function(i) { var idx = i >>> 0; return idx < this.length ? this[idx] : null; };
        NodeList.prototype.forEach = function(cb, thisArg) { for (var i = 0; i < this.length; i++) cb.call(thisArg, this[i], i, this); };
        NodeList.prototype.keys = function() {
            var self = this; var idx = 0;
            return { next: function() { return idx < self.length ? { value: idx++, done: false } : { value: undefined, done: true }; }, [Symbol.iterator]: function() { return this; } };
        };
        NodeList.prototype.values = function() {
            var self = this; var idx = 0;
            return { next: function() { return idx < self.length ? { value: self[idx++], done: false } : { value: undefined, done: true }; }, [Symbol.iterator]: function() { return this; } };
        };
        NodeList.prototype.entries = function() {
            var self = this; var idx = 0;
            return { next: function() { return idx < self.length ? { value: [idx, self[idx++]], done: false } : { value: undefined, done: true }; }, [Symbol.iterator]: function() { return this; } };
        };
        NodeList.prototype[Symbol.iterator] = NodeList.prototype.values;
        NodeList.prototype[Symbol.toStringTag] = 'NodeList';

        globalThis.__makeStaticNodeList = function(items) {
            var obj = Object.create(NodeList.prototype);
            for (var i = 0; i < items.length; i++) {
                Object.defineProperty(obj, String(i), { value: items[i], writable: false, enumerable: true, configurable: true });
            }
            Object.defineProperty(obj, '__nlLen', { value: items.length, writable: false, enumerable: false, configurable: false });
            return obj;
        };

        // HTMLCollection class
        globalThis.HTMLCollection = class HTMLCollection {};
        HTMLCollection.prototype.item = function(i) { var idx = i >>> 0; return idx < this.length ? this[idx] : null; };
        HTMLCollection.prototype.namedItem = function(name) {
            var s = String(name);
            if (!s) return null;
            for (var i = 0; i < this.length; i++) {
                var el = this[i];
                if (!el.getAttribute) continue;
                if (el.getAttribute('id') === s) return el;
                var ns = el.namespaceURI;
                if (ns === 'http://www.w3.org/1999/xhtml' && el.getAttribute('name') === s) return el;
            }
            return null;
        };
        globalThis.DOMTokenList = class DOMTokenList {};
        DOMTokenList.prototype[Symbol.toStringTag] = 'DOMTokenList';
        DOMTokenList.prototype.keys = Array.prototype.keys;
        DOMTokenList.prototype.values = Array.prototype.values;
        DOMTokenList.prototype.entries = Array.prototype.entries;
        DOMTokenList.prototype.forEach = Array.prototype.forEach;
        DOMTokenList.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];
        globalThis.Attr = class Attr extends Node {};
"#;
