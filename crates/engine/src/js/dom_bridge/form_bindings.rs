pub(crate) fn form_bindings_js() -> &'static str {
    r#"
        // --- Form-related properties and methods ---
        // form property: check form attribute first, then walk up ancestors
        Object.defineProperty(ElemProto, 'form', {
            get: function() {
                if (this.__nid === undefined) return null;
                // Per HTML spec: if element has a form attribute, use getElementById to find the form
                var formAttr = __n_getAttribute(this.__nid, 'form');
                if (formAttr && formAttr !== '') {
                    var formNid = __n_getElementById(formAttr);
                    if (formNid >= 0 && __n_getTagName(formNid) === 'FORM') return __w(formNid);
                    return null;
                }
                // Fallback: walk up ancestors to find enclosing <form>
                var cur = __n_getParent(this.__nid);
                while (cur >= 0) {
                    if (__n_getTagName(cur) === 'FORM') return __w(cur);
                    cur = __n_getParent(cur);
                }
                return null;
            },
            configurable: true
        });

        // Form-specific methods (only meaningful on <form> elements but safe on all)
        ElemProto.submit = function() {
            if (this.tagName === 'FORM') {
                var evt = new Event('submit', {bubbles: true, cancelable: true});
                evt.target = this;
                this.dispatchEvent(evt);
            }
        };
        ElemProto.requestSubmit = function(submitter) {
            if (this.tagName !== 'FORM') return;
            // If submitter is provided, validate it
            if (submitter !== undefined && submitter !== null) {
                if (!submitter.form || submitter.form !== this) {
                    throw new DOMException(
                        "The specified element is not a submit button of this form",
                        "NotFoundError"
                    );
                }
            }
            // Run constraint validation on all controls
            var controls = this.querySelectorAll('input, textarea, select');
            var allValid = true;
            for (var i = 0; i < controls.length; i++) {
                if (!controls[i].checkValidity()) {
                    allValid = false;
                }
            }
            if (!allValid) return;
            // Fire the submit event (cancelable)
            var evt = new Event('submit', {bubbles: true, cancelable: true});
            evt.submitter = submitter || null;
            var dispatched = this.dispatchEvent(evt);
            // If preventDefault was called, do not submit
            if (!dispatched) return;
        };
        ElemProto.reset = function() {
            if (this.tagName !== 'FORM') return;
            // Clear dirty flags on all descendant controls
            var controls = this.querySelectorAll('input, textarea, select');
            for (var i = 0; i < controls.length; i++) {
                var c = controls[i];
                if (c.__props) {
                    delete c.__props._value;
                    delete c.__props._checked;
                    delete c.__props._selected;
                }
                // Sync attribute back (value falls back to defaultValue)
                var dv = c.getAttribute('value');
                if (dv !== null) __n_setAttribute(c.__nid, 'value', dv);
                else __n_removeAttribute(c.__nid, 'value');
            }
            var evt = new Event('reset', {bubbles: true, cancelable: true});
            evt.target = this;
            this.dispatchEvent(evt);
        };
        ElemProto.setCustomValidity = function(msg) {
            if (!this.__props) this.__props = {};
            this.__props._customValidity = String(msg);
        };
        ElemProto.checkValidity = function() {
            var v = this.validity;
            if (!v.valid) {
                this.dispatchEvent(new Event('invalid', {bubbles: false, cancelable: true}));
                return false;
            }
            return true;
        };
        ElemProto.reportValidity = function() { return this.checkValidity(); };

        // elements property for <form>: returns live HTMLFormControlsCollection
        Object.defineProperty(ElemProto, 'elements', {
            get: function() {
                if (this.tagName !== 'FORM') return undefined;
                var self = this;
                return new Proxy([], {
                    get: function(t, prop) {
                        var descendantControls = self.querySelectorAll('input, textarea, select, button');
                        var allControls = [];
                        for (var i = 0; i < descendantControls.length; i++) { allControls.push(descendantControls[i]); }
                        // Also include external elements that reference this form via form="<id>"
                        var formId = self.getAttribute('id');
                        if (formId) {
                            var externals = document.querySelectorAll('input[form="' + formId + '"], textarea[form="' + formId + '"], select[form="' + formId + '"], button[form="' + formId + '"]');
                            for (var j = 0; j < externals.length; j++) {
                                var dup = false;
                                for (var k = 0; k < allControls.length; k++) {
                                    if (allControls[k] === externals[j]) { dup = true; break; }
                                }
                                if (!dup) allControls.push(externals[j]);
                            }
                        }
                        var controls = allControls;
                        if (prop === 'length') return controls.length;
                        if (prop === 'item') return function(i) { return controls[i] || null; };
                        if (prop === Symbol.iterator) return function() { return controls[Symbol.iterator](); };
                        if (typeof prop === 'string' && !isNaN(prop)) return controls[parseInt(prop)];
                        if (prop === 'forEach') return function(cb) { for (var i = 0; i < controls.length; i++) cb(controls[i], i); };
                        if (prop === 'namedItem') return function(name) {
                            for (var i = 0; i < controls.length; i++) {
                                if (controls[i].getAttribute('name') === name || controls[i].getAttribute('id') === name) return controls[i];
                            }
                            return null;
                        };
                        // Named access by string key
                        if (typeof prop === 'string' && isNaN(prop)) {
                            for (var i = 0; i < controls.length; i++) {
                                if (controls[i].getAttribute('name') === prop || controls[i].getAttribute('id') === prop) return controls[i];
                            }
                            return undefined;
                        }
                        return controls[prop];
                    }
                });
            },
            configurable: true
        });

        // action/method properties for all elements (meaningful on <form>)
        Object.defineProperty(ElemProto, 'action', {
            get: function() { return this.getAttribute('action') || ''; },
            set: function(v) { this.setAttribute('action', String(v)); },
            configurable: true
        });
        Object.defineProperty(ElemProto, 'method', {
            get: function() { return (this.getAttribute('method') || 'get').toLowerCase(); },
            set: function(v) { this.setAttribute('method', String(v)); },
            configurable: true
        });

        // enctype property (defaults to "application/x-www-form-urlencoded", validates values)
        Object.defineProperty(ElemProto, 'enctype', {
            get: function() {
                var v = (this.getAttribute('enctype') || '').toLowerCase();
                if (v === 'application/x-www-form-urlencoded' || v === 'multipart/form-data' || v === 'text/plain') return v;
                return 'application/x-www-form-urlencoded';
            },
            set: function(v) { this.setAttribute('enctype', String(v)); },
            configurable: true
        });

        // encoding property (alias for enctype per spec)
        Object.defineProperty(ElemProto, 'encoding', {
            get: function() { return this.enctype; },
            set: function(v) { this.enctype = v; },
            configurable: true
        });

        // noValidate property (boolean attribute)
        Object.defineProperty(ElemProto, 'noValidate', {
            get: function() { return this.hasAttribute('novalidate'); },
            set: function(v) { if (v) this.setAttribute('novalidate', ''); else this.removeAttribute('novalidate'); },
            configurable: true
        });

        // target property
        Object.defineProperty(ElemProto, 'target', {
            get: function() { return this.getAttribute('target') || ''; },
            set: function(v) { this.setAttribute('target', String(v)); },
            configurable: true
        });

        // acceptCharset property (reflects "accept-charset" attribute)
        Object.defineProperty(ElemProto, 'acceptCharset', {
            get: function() { return this.getAttribute('accept-charset') || ''; },
            set: function(v) { this.setAttribute('accept-charset', String(v)); },
            configurable: true
        });

        // autocomplete property (defaults to "on")
        Object.defineProperty(ElemProto, 'autocomplete', {
            get: function() { return this.getAttribute('autocomplete') || 'on'; },
            set: function(v) { this.setAttribute('autocomplete', String(v)); },
            configurable: true
        });

        // <select> selectedIndex property
        Object.defineProperty(ElemProto, 'selectedIndex', {
            get: function() {
                if (this.tagName !== 'SELECT') return -1;
                var opts = this.querySelectorAll('option');
                for (var i = 0; i < opts.length; i++) {
                    if (opts[i].__props && opts[i].__props._selected) return i;
                    if (opts[i].hasAttribute('selected')) return i;
                }
                return opts.length > 0 ? 0 : -1;
            },
            set: function(idx) {
                if (this.tagName !== 'SELECT') return;
                var opts = this.querySelectorAll('option');
                for (var i = 0; i < opts.length; i++) {
                    if (!opts[i].__props) opts[i].__props = {};
                    opts[i].__props._selected = (i === idx);
                }
            },
            configurable: true
        });

        // <select> options property
        Object.defineProperty(ElemProto, 'options', {
            get: function() {
                if (this.tagName !== 'SELECT') return undefined;
                var sel = this;
                var opts = this.querySelectorAll('option');
                return new Proxy(opts, {
                    get: function(arr, p) {
                        if (p === 'length') return arr.length;
                        if (p === 'selectedIndex') return sel.selectedIndex;
                        if (p === 'item') return function(i) { return arr[i] || null; };
                        if (p === 'namedItem') return function(name) {
                            for (var i = 0; i < arr.length; i++) {
                                if (arr[i].getAttribute('name') === name || arr[i].getAttribute('id') === name) return arr[i];
                            }
                            return null;
                        };
                        if (typeof p === 'string' && !isNaN(p)) return arr[parseInt(p)];
                        if (p === Symbol.iterator) return function() { return arr[Symbol.iterator](); };
                        return arr[p];
                    }
                });
            },
            configurable: true
        });

        // <select> selectedOptions property
        Object.defineProperty(ElemProto, 'selectedOptions', {
            get: function() {
                if (this.tagName !== 'SELECT') return [];
                var opts = this.querySelectorAll('option');
                var result = [];
                for (var i = 0; i < opts.length; i++) {
                    if (opts[i].hasAttribute('selected') || (opts[i].__props && opts[i].__props._selected)) {
                        result.push(opts[i]);
                    }
                }
                return result;
            },
            configurable: true
        });

        // length property for <select> (number of options) and <form> (number of controls)
        Object.defineProperty(ElemProto, 'length', {
            get: function() {
                if (this.tagName === 'SELECT') {
                    var opts = this.querySelectorAll('option');
                    return opts.length;
                }
                if (this.tagName === 'FORM') {
                    return this.querySelectorAll('input, textarea, select, button').length;
                }
                return undefined;
            },
            configurable: true
        });

        // <option> text property
        Object.defineProperty(ElemProto, 'text', {
            get: function() {
                if (this.tagName === 'OPTION') return (this.textContent || '').trim();
                return undefined;
            },
            set: function(v) {
                if (this.tagName === 'OPTION') this.textContent = String(v);
            },
            configurable: true
        });

        // <option> index property
        Object.defineProperty(ElemProto, 'index', {
            get: function() {
                if (this.tagName !== 'OPTION') return undefined;
                var parent = this.parentNode;
                if (!parent || parent.tagName !== 'SELECT') return 0;
                var opts = parent.querySelectorAll('option');
                for (var i = 0; i < opts.length; i++) {
                    if (opts[i].__nid === this.__nid) return i;
                }
                return 0;
            },
            configurable: true
        });

        // <option> label property
        Object.defineProperty(ElemProto, 'label', {
            get: function() {
                if (this.tagName !== 'OPTION') return '';
                return this.getAttribute('label') || (this.textContent || '').trim();
            },
            set: function(v) {
                if (this.tagName === 'OPTION') this.setAttribute('label', String(v));
            },
            configurable: true
        });

    "#
}
