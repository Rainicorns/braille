/// Label association properties: label.htmlFor, label.control, input.labels.
pub(super) fn label_bindings_js() -> &'static str {
    r#"
        // --- Label association properties ---
        // label.htmlFor — reflects the `for` attribute
        Object.defineProperty(EP, 'htmlFor', {
            get: function() {
                if (this.tagName !== 'LABEL') return undefined;
                return this.getAttribute('for') || '';
            },
            set: function(v) {
                if (this.tagName === 'LABEL') this.setAttribute('for', String(v));
            },
            configurable: true
        });

        // label.control — returns the associated form control element
        Object.defineProperty(EP, 'control', {
            get: function() {
                if (this.tagName !== 'LABEL') return undefined;
                var id = __n_findLabelControl(this.__nid);
                return id >= 0 ? __w(id) : null;
            },
            configurable: true
        });

        // input.labels — returns a NodeList of all <label> elements associated with this input
        Object.defineProperty(EP, 'labels', {
            get: function() {
                var tag = this.tagName;
                if (tag !== 'INPUT' && tag !== 'SELECT' && tag !== 'TEXTAREA' && tag !== 'BUTTON') return undefined;
                // Hidden inputs have no labels per spec
                if (tag === 'INPUT' && (this.getAttribute('type') || '').toLowerCase() === 'hidden') return [];
                var ids = __n_findLabelsForControl(this.__nid);
                return ids.map(__w);
            },
            configurable: true
        });
    "#
}
