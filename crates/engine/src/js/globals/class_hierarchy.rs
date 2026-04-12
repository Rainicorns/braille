//! DOM class hierarchy: Node, Element, HTMLElement and all subclasses,
//! DocumentFragment, ShadowRoot, CustomElementRegistry, event handler setup,
//! and element-specific prototype stubs.
//! Loaded BEFORE dom_bridge. Defines the class tree that constructors_and_wiring
//! later rewires with proper prototype chains inheriting from EP.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(CLASS_HIERARCHY_JS).unwrap();
}

const CLASS_HIERARCHY_JS: &str = r#"
        // Class hierarchy — Node, Element, HTMLElement and subclasses
        globalThis.Node = class Node {};
        globalThis.Element = class Element extends Node {};
        // HTMLElement constructor supports Custom Elements:
        // When called via `new MyElement()` where MyElement extends HTMLElement,
        // new.target is the CE constructor. We look up the tag from the registry
        // and create the backing DOM node. During upgrades, __ceUpgradeTarget is set
        // to the existing element so super() returns it instead of creating a new one.
        globalThis.__ceUpgradeTarget = null;
        globalThis.HTMLElement = class HTMLElement extends Element {
            constructor() {
                super();
                var upgradeTarget = __ceUpgradeTarget;
                if (upgradeTarget) {
                    __ceUpgradeTarget = null;
                    return upgradeTarget;
                }
                if (typeof customElements !== 'undefined' && customElements._ctorToName) {
                    var name = customElements._ctorToName.get(new.target);
                    if (name) {
                        var entry = customElements._registry.get(name);
                        var tagToCreate = (entry && entry.extends) ? entry.extends : name;
                        var nid = __n_createElement(tagToCreate);
                        this.__nid = nid;
                        this.__props = {};
                        this.__ce_upgraded = true;
                        if (entry && entry.extends) {
                            __n_setAttribute(nid, 'is', name);
                        }
                        if (typeof __cache !== 'undefined') __cache[nid] = this;
                    }
                }
            }
        };
        globalThis.HTMLIFrameElement = class HTMLIFrameElement extends HTMLElement {};
        globalThis.HTMLInputElement = class HTMLInputElement extends HTMLElement {};
        globalThis.HTMLTextAreaElement = class HTMLTextAreaElement extends HTMLElement {};
        globalThis.HTMLSelectElement = class HTMLSelectElement extends HTMLElement {};
        globalThis.HTMLFormElement = class HTMLFormElement extends HTMLElement {};
        globalThis.HTMLAnchorElement = class HTMLAnchorElement extends HTMLElement {};
        globalThis.HTMLImageElement = class HTMLImageElement extends HTMLElement {};
        globalThis.HTMLButtonElement = class HTMLButtonElement extends HTMLElement {};
        globalThis.HTMLOptionElement = class HTMLOptionElement extends HTMLElement {};
        globalThis.HTMLBodyElement = class HTMLBodyElement extends HTMLElement {};
        globalThis.HTMLHeadElement = class HTMLHeadElement extends HTMLElement {};
        globalThis.HTMLFrameSetElement = class HTMLFrameSetElement extends HTMLElement {};
        globalThis.HTMLHtmlElement = class HTMLHtmlElement extends HTMLElement {};
        globalThis.HTMLTitleElement = class HTMLTitleElement extends HTMLElement {};
        globalThis.HTMLDivElement = class HTMLDivElement extends HTMLElement {};
        globalThis.HTMLSpanElement = class HTMLSpanElement extends HTMLElement {};
        globalThis.HTMLParagraphElement = class HTMLParagraphElement extends HTMLElement {};
        globalThis.HTMLScriptElement = class HTMLScriptElement extends HTMLElement {};
        globalThis.HTMLStyleElement = class HTMLStyleElement extends HTMLElement {};
        globalThis.HTMLLinkElement = class HTMLLinkElement extends HTMLElement {};
        globalThis.HTMLMetaElement = class HTMLMetaElement extends HTMLElement {};
        globalThis.HTMLTableElement = class HTMLTableElement extends HTMLElement {};
        globalThis.HTMLTableRowElement = class HTMLTableRowElement extends HTMLElement {};
        globalThis.HTMLTableCellElement = class HTMLTableCellElement extends HTMLElement {};
        globalThis.HTMLUListElement = class HTMLUListElement extends HTMLElement {};
        globalThis.HTMLOListElement = class HTMLOListElement extends HTMLElement {};
        globalThis.HTMLLIElement = class HTMLLIElement extends HTMLElement {};
        globalThis.HTMLPreElement = class HTMLPreElement extends HTMLElement {};
        globalThis.HTMLCanvasElement = class HTMLCanvasElement extends HTMLElement {};
        globalThis.HTMLVideoElement = class HTMLVideoElement extends HTMLElement {};
        globalThis.HTMLAudioElement = class HTMLAudioElement extends HTMLElement {};
        globalThis.HTMLSourceElement = class HTMLSourceElement extends HTMLElement {};
        globalThis.HTMLLabelElement = class HTMLLabelElement extends HTMLElement {};
        globalThis.HTMLTemplateElement = class HTMLTemplateElement extends HTMLElement {};
        globalThis.HTMLUnknownElement = class HTMLUnknownElement extends HTMLElement {};
        globalThis.HTMLAreaElement = class HTMLAreaElement extends HTMLElement {};
        globalThis.HTMLBaseElement = class HTMLBaseElement extends HTMLElement {};
        globalThis.HTMLBRElement = class HTMLBRElement extends HTMLElement {};
        globalThis.HTMLDataElement = class HTMLDataElement extends HTMLElement {};
        globalThis.HTMLDataListElement = class HTMLDataListElement extends HTMLElement {};
        globalThis.HTMLDetailsElement = class HTMLDetailsElement extends HTMLElement {};
        globalThis.HTMLDialogElement = class HTMLDialogElement extends HTMLElement {};
        globalThis.HTMLDirectoryElement = class HTMLDirectoryElement extends HTMLElement {};
        globalThis.HTMLDListElement = class HTMLDListElement extends HTMLElement {};
        globalThis.HTMLEmbedElement = class HTMLEmbedElement extends HTMLElement {};
        globalThis.HTMLFieldSetElement = class HTMLFieldSetElement extends HTMLElement {};
        globalThis.HTMLFontElement = class HTMLFontElement extends HTMLElement {};
        globalThis.HTMLFrameElement = class HTMLFrameElement extends HTMLElement {};
        globalThis.HTMLHeadingElement = class HTMLHeadingElement extends HTMLElement {};
        globalThis.HTMLHRElement = class HTMLHRElement extends HTMLElement {};
        globalThis.HTMLLegendElement = class HTMLLegendElement extends HTMLElement {};
        globalThis.HTMLMapElement = class HTMLMapElement extends HTMLElement {};
        globalThis.HTMLMarqueeElement = class HTMLMarqueeElement extends HTMLElement {};
        globalThis.HTMLMenuElement = class HTMLMenuElement extends HTMLElement {};
        globalThis.HTMLMeterElement = class HTMLMeterElement extends HTMLElement {};
        globalThis.HTMLModElement = class HTMLModElement extends HTMLElement {};
        globalThis.HTMLObjectElement = class HTMLObjectElement extends HTMLElement {};
        globalThis.HTMLOptGroupElement = class HTMLOptGroupElement extends HTMLElement {};
        globalThis.HTMLOutputElement = class HTMLOutputElement extends HTMLElement {};
        globalThis.HTMLParamElement = class HTMLParamElement extends HTMLElement {};
        globalThis.HTMLPictureElement = class HTMLPictureElement extends HTMLElement {};
        globalThis.HTMLProgressElement = class HTMLProgressElement extends HTMLElement {};
        globalThis.HTMLQuoteElement = class HTMLQuoteElement extends HTMLElement {};
        globalThis.HTMLTableCaptionElement = class HTMLTableCaptionElement extends HTMLElement {};
        globalThis.HTMLTableColElement = class HTMLTableColElement extends HTMLElement {};
        globalThis.HTMLTableSectionElement = class HTMLTableSectionElement extends HTMLElement {};
        globalThis.HTMLTimeElement = class HTMLTimeElement extends HTMLElement {};
        globalThis.HTMLTrackElement = class HTMLTrackElement extends HTMLElement {};
        globalThis.HTMLSlotElement = class HTMLSlotElement extends HTMLElement {};
        globalThis.SVGElement = class SVGElement extends Element {};

        // Symbol.unscopables for Element.prototype — prevents these methods from
        // shadowing same-named variables inside `with` blocks (used by event handlers)
        Element.prototype[Symbol.unscopables] = {
            slot: true,
            before: true,
            after: true,
            replaceWith: true,
            remove: true,
            prepend: true,
            append: true,
        };

        globalThis.Window = class Window {
            constructor() { throw new TypeError("Illegal constructor"); }
        };

        globalThis.DocumentFragment = class DocumentFragment extends Node {};
        globalThis.ShadowRoot = class ShadowRoot extends DocumentFragment {
            get mode() { return this._mode || 'open'; }
            get host() { return this._host || null; }
            get innerHTML() {
                if (this.__nid !== undefined) return __n_getInnerHTML(this.__nid);
                return '';
            }
            set innerHTML(v) {
                if (this.__nid !== undefined) {
                    __n_setInnerHTML(this.__nid, String(v));
                    // Upgrade custom elements in new content
                    if (typeof customElements !== 'undefined' && customElements._registry) {
                        __ceUpgradeTree(this);
                    }
                }
            }
        };

        // Custom Elements Registry
        globalThis.CustomElementRegistry = class CustomElementRegistry {
            constructor() {
                this._registry = new Map();      // name → {ctor, observedAttrs}
                this._ctorToName = new Map();     // ctor → name
                this._whenDefined = new Map();    // name → {promise, resolve}
            }
            define(name, ctor, options) {
                name = String(name).toLowerCase();
                if (!/^[a-z]/.test(name) || name.indexOf('-') === -1) {
                    throw new DOMException("'" + name + "' is not a valid custom element name", "SyntaxError");
                }
                if (this._registry.has(name)) {
                    throw new DOMException("The name '" + name + "' has already been used with this registry", "NotSupportedError");
                }
                if (this._ctorToName.has(ctor)) {
                    throw new DOMException("This constructor has already been used with this registry", "NotSupportedError");
                }
                var observedAttrs = [];
                if (ctor.observedAttributes && Array.isArray(ctor.observedAttributes)) {
                    observedAttrs = ctor.observedAttributes.slice();
                }
                var extendsTag = (options && options.extends) ? String(options.extends).toLowerCase() : null;
                this._registry.set(name, {ctor: ctor, observedAttrs: observedAttrs, extends: extendsTag});
                this._ctorToName.set(ctor, name);
                // Upgrade existing elements in the DOM
                __ceUpgradeAll(name, ctor, observedAttrs);
                // Resolve whenDefined promise
                var wd = this._whenDefined.get(name);
                if (wd) {
                    wd.resolve(ctor);
                    this._whenDefined.delete(name);
                }
            }
            get(name) {
                var entry = this._registry.get(String(name).toLowerCase());
                return entry ? entry.ctor : undefined;
            }
            getName(ctor) {
                var name = this._ctorToName.get(ctor);
                return name !== undefined ? name : null;
            }
            whenDefined(name) {
                name = String(name).toLowerCase();
                if (!/^[a-z]/.test(name) || name.indexOf('-') === -1) {
                    return Promise.reject(new DOMException("'" + name + "' is not a valid custom element name", "SyntaxError"));
                }
                var entry = this._registry.get(name);
                if (entry) return Promise.resolve(entry.ctor);
                var wd = this._whenDefined.get(name);
                if (wd) return wd.promise;
                var resolve;
                var promise = new Promise(function(r) { resolve = r; });
                this._whenDefined.set(name, {promise: promise, resolve: resolve});
                return promise;
            }
        };
        globalThis.customElements = new CustomElementRegistry();

        // CE upgrade/lifecycle stubs — real implementations are installed by dom_bridge IIFE
        // which has access to _cache. These get overwritten.
        globalThis.__ceUpgradeAll = function() {};
        globalThis.__ceUpgradeTree = function() {};
        globalThis.__ceConnected = function() {};
        globalThis.__ceDisconnected = function() {};

        // Window-reflecting body/frameset event handlers (onblur, onerror, onfocus, onload, onscroll, onresize)
        // These forward to window when set on body or frameset elements.
        var _windowEventHandlers = ['onblur', 'onerror', 'onfocus', 'onload', 'onscroll', 'onresize',
            'onbeforeunload', 'onhashchange', 'onlanguagechange', 'onmessage', 'onmessageerror',
            'onoffline', 'ononline', 'onpagehide', 'onpageshow', 'onpopstate',
            'onrejectionhandled', 'onstorage', 'onunhandledrejection', 'onunload'];
        var _wehSet = new Set(_windowEventHandlers);
        [HTMLBodyElement, HTMLFrameSetElement].forEach(function(Ctor) {
            _windowEventHandlers.forEach(function(attr) {
                Object.defineProperty(Ctor.prototype, attr, {
                    get: function() {
                        return window['_weh_' + attr] || null;
                    },
                    set: function(v) {
                        window['_weh_' + attr] = typeof v === 'function' ? v : null;
                    },
                    enumerable: true,
                    configurable: true
                });
            });
            // Hook setAttribute to compile event handler content attributes
            var origSetAttr = Ctor.prototype.setAttribute;
            Ctor.prototype.setAttribute = function(name, value) {
                if (origSetAttr) origSetAttr.call(this, name, value);
                else if (this.__nid !== undefined) __n_setAttribute(this.__nid, name, String(value));
                if (_wehSet.has(name)) {
                    window['_weh_' + name] = new Function('event', String(value));
                }
            };
        });
        // Window on* getters/setters forwarding to stored handlers
        _windowEventHandlers.forEach(function(attr) {
            Object.defineProperty(window, attr, {
                get: function() { return window['_weh_' + attr] || null; },
                set: function(v) { window['_weh_' + attr] = typeof v === 'function' ? v : null; },
                enumerable: true,
                configurable: true
            });
        });

        // Standard event handler properties on HTMLElement.prototype
        var _elementEventHandlers = ['onclick', 'ondblclick', 'onmousedown', 'onmouseup',
            'onmouseover', 'onmouseout', 'onmousemove', 'onkeydown', 'onkeyup', 'onkeypress',
            'onchange', 'oninput', 'onsubmit', 'onreset', 'onselect',
            'ondrag', 'ondragstart', 'ondragend', 'ondragover', 'ondragenter', 'ondragleave', 'ondrop',
            'onscroll', 'onscrollend',
            'ontouchstart', 'ontouchmove', 'ontouchend', 'ontouchcancel',
            'onpointerdown', 'onpointerup', 'onpointermove', 'onpointerover', 'onpointerout',
            'onpointerenter', 'onpointerleave', 'onpointercancel', 'ongotpointercapture', 'onlostpointercapture',
            'oncontextmenu', 'onwheel', 'onanimationstart', 'onanimationend', 'onanimationiteration',
            'ontransitionend', 'ontransitionrun', 'ontransitionstart', 'ontransitioncancel',
            'onwebkitanimationstart', 'onwebkitanimationend', 'onwebkitanimationiteration',
            'onwebkittransitionend'];
        _elementEventHandlers.forEach(function(attr) {
            if (!(attr in HTMLElement.prototype)) {
                Object.defineProperty(HTMLElement.prototype, attr, {
                    get: function() { return this['_eh_' + attr] || null; },
                    set: function(v) { this['_eh_' + attr] = typeof v === 'function' ? v : null; },
                    enumerable: true,
                    configurable: true
                });
            }
        });

        // Also define element event handlers on window (animation events bubble to window)
        _elementEventHandlers.forEach(function(attr) {
            if (!(attr in window)) {
                Object.defineProperty(window, attr, {
                    get: function() { return this['_eh_' + attr] || null; },
                    set: function(v) { this['_eh_' + attr] = typeof v === 'function' ? v : null; },
                    enumerable: true,
                    configurable: true
                });
            }
        });

        // --- Attribute reflection helper ---
        function __reflectAttr(proto, prop, attr) {
            Object.defineProperty(proto, prop, {
                get: function() { return this.getAttribute(attr || prop) || ''; },
                set: function(v) { this.setAttribute(attr || prop, String(v)); },
                configurable: true, enumerable: true,
            });
        }
        function __reflectBool(proto, prop, attr) {
            Object.defineProperty(proto, prop, {
                get: function() { return this.hasAttribute(attr || prop); },
                set: function(v) { if (v) this.setAttribute(attr || prop, ''); else this.removeAttribute(attr || prop); },
                configurable: true, enumerable: true,
            });
        }

        // --- HTMLAnchorElement ---
        __reflectAttr(HTMLAnchorElement.prototype, 'target', 'target');
        __reflectAttr(HTMLAnchorElement.prototype, 'download', 'download');
        Object.defineProperty(HTMLAnchorElement.prototype, 'text', {
            get: function() { return this.textContent || ''; },
            set: function(v) { this.textContent = v; },
            configurable: true, enumerable: true,
        });
        // URL decomposition properties for anchor elements
        (function() {
            function anchorURL(el) {
                var h = el.getAttribute('href') || '';
                var m = String(h).match(/^(https?:)\/\/([^/:]+)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                if (!m) return null;
                return { protocol: m[1], hostname: m[2], port: m[3] || '', pathname: m[4] || '/', search: m[5] || '', hash: m[6] || '' };
            }
            var urlProps = {
                protocol: function(u) { return u ? u.protocol : ''; },
                hostname: function(u) { return u ? u.hostname : ''; },
                port: function(u) { return u ? u.port : ''; },
                pathname: function(u) { return u ? u.pathname : ''; },
                search: function(u) { return u ? u.search : ''; },
                hash: function(u) { return u ? u.hash : ''; },
                host: function(u) { return u ? (u.port ? u.hostname + ':' + u.port : u.hostname) : ''; },
                origin: function(u) { return u ? u.protocol + '//' + (u.port ? u.hostname + ':' + u.port : u.hostname) : ''; },
            };
            var keys = Object.keys(urlProps);
            for (var i = 0; i < keys.length; i++) {
                (function(k, fn) {
                    Object.defineProperty(HTMLAnchorElement.prototype, k, {
                        get: function() { return fn(anchorURL(this)); },
                        configurable: true, enumerable: true,
                    });
                })(keys[i], urlProps[keys[i]]);
            }
        })();

        // --- HTMLImageElement ---
        __reflectAttr(HTMLImageElement.prototype, 'alt', 'alt');
        Object.defineProperty(HTMLImageElement.prototype, 'width', {
            get: function() { return parseInt(this.getAttribute('width'), 10) || 0; },
            set: function(v) { this.setAttribute('width', String(v)); },
            configurable: true, enumerable: true,
        });
        Object.defineProperty(HTMLImageElement.prototype, 'height', {
            get: function() { return parseInt(this.getAttribute('height'), 10) || 0; },
            set: function(v) { this.setAttribute('height', String(v)); },
            configurable: true, enumerable: true,
        });
        Object.defineProperty(HTMLImageElement.prototype, 'naturalWidth', {
            get: function() { return 0; },
            configurable: true, enumerable: true,
        });
        Object.defineProperty(HTMLImageElement.prototype, 'naturalHeight', {
            get: function() { return 0; },
            configurable: true, enumerable: true,
        });
        Object.defineProperty(HTMLImageElement.prototype, 'complete', {
            get: function() { return true; },
            configurable: true, enumerable: true,
        });

        // --- HTMLSelectElement ---
        HTMLSelectElement.prototype.add = function(element, before) {
            if (before && before.parentNode === this) {
                this.insertBefore(element, before);
            } else if (typeof before === 'number') {
                var opts = this.querySelectorAll('option');
                if (before < opts.length) {
                    this.insertBefore(element, opts[before]);
                } else {
                    this.appendChild(element);
                }
            } else {
                this.appendChild(element);
            }
        };
        HTMLSelectElement.prototype.remove = function(index) {
            if (typeof index === 'number') {
                var opts = this.querySelectorAll('option');
                if (index >= 0 && index < opts.length) {
                    opts[index].parentNode.removeChild(opts[index]);
                }
            } else {
                // Element.prototype.remove() — remove self from parent
                if (this.parentNode) this.parentNode.removeChild(this);
            }
        };
        Object.defineProperty(HTMLSelectElement.prototype, 'length', {
            get: function() { return this.querySelectorAll('option').length; },
            configurable: true,
        });

        // --- HTMLInputElement ---
        __reflectBool(HTMLInputElement.prototype, 'required', 'required');

        // --- HTMLTextAreaElement ---
        __reflectBool(HTMLTextAreaElement.prototype, 'required', 'required');

        // --- HTMLCanvasElement ---
        Object.defineProperty(HTMLCanvasElement.prototype, 'width', {
            get: function() { return parseInt(this.getAttribute('width'), 10) || 300; },
            set: function(v) { this.setAttribute('width', String(v)); },
            configurable: true, enumerable: true,
        });
        Object.defineProperty(HTMLCanvasElement.prototype, 'height', {
            get: function() { return parseInt(this.getAttribute('height'), 10) || 150; },
            set: function(v) { this.setAttribute('height', String(v)); },
            configurable: true, enumerable: true,
        });
        HTMLCanvasElement.prototype.getContext = function(type) {
            if (type === '2d') {
                return {
                    canvas: this,
                    fillRect: function() {}, clearRect: function() {}, strokeRect: function() {},
                    fillText: function() {}, strokeText: function() {}, measureText: function(t) { return { width: 0 }; },
                    beginPath: function() {}, closePath: function() {}, moveTo: function() {},
                    lineTo: function() {}, arc: function() {}, arcTo: function() {},
                    bezierCurveTo: function() {}, quadraticCurveTo: function() {},
                    rect: function() {}, fill: function() {}, stroke: function() {},
                    clip: function() {}, save: function() {}, restore: function() {},
                    translate: function() {}, rotate: function() {}, scale: function() {},
                    setTransform: function() {}, resetTransform: function() {},
                    drawImage: function() {}, createLinearGradient: function() { return { addColorStop: function() {} }; },
                    createRadialGradient: function() { return { addColorStop: function() {} }; },
                    createPattern: function() { return {}; },
                    getImageData: function(x, y, w, h) { return { data: new Uint8ClampedArray(w * h * 4), width: w, height: h }; },
                    putImageData: function() {},
                    fillStyle: '#000', strokeStyle: '#000', lineWidth: 1, font: '10px sans-serif',
                    textAlign: 'start', textBaseline: 'alphabetic', globalAlpha: 1,
                    globalCompositeOperation: 'source-over',
                };
            }
            return null;
        };
        HTMLCanvasElement.prototype.toDataURL = function() { return 'data:image/png;base64,'; };
        HTMLCanvasElement.prototype.toBlob = function(cb) { if (cb) cb(new Blob([])); };
"#;
