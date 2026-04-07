/// Slot assignment algorithm, HTMLSlotElement API (assignedNodes/assignedElements),
/// and slotchange event dispatching.
pub(super) fn slot_assignment_js() -> &'static str {
    r#"
        // Batch slotchange events via microtask (same pattern as MutationObserver)
        var __pendingSlotChanges = [];
        var __slotChangeMicrotaskQueued = false;

        function __signalSlotChange(slotNode) {
            if (__pendingSlotChanges.indexOf(slotNode) < 0) {
                __pendingSlotChanges.push(slotNode);
            }
            if (!__slotChangeMicrotaskQueued) {
                __slotChangeMicrotaskQueued = true;
                Promise.resolve().then(function() {
                    __slotChangeMicrotaskQueued = false;
                    var slots = __pendingSlotChanges.slice();
                    __pendingSlotChanges = [];
                    for (var i = 0; i < slots.length; i++) {
                        var ev = new Event('slotchange', { bubbles: true });
                        slots[i].dispatchEvent(ev);
                    }
                });
            }
        }

        // Compute slot assignments for a shadow host
        function __computeSlotAssignments(hostNid) {
            var shadowId = __n_getShadowRootId(hostNid);
            if (shadowId < 0) return {};
            // Find all <slot> elements in the shadow root
            var slots = {};
            function findSlots(nid) {
                var kids = __n_getAllChildIds(nid);
                for (var i = 0; i < kids.length; i++) {
                    var w = _cache[kids[i]] || __w(kids[i]);
                    if (w.tagName === 'SLOT') {
                        var name = w.getAttribute('name') || '';
                        if (!slots[name]) slots[name] = w;
                    }
                    findSlots(kids[i]);
                }
            }
            findSlots(shadowId);
            // Get light DOM children of host
            var lightChildren = __n_getAllChildIds(hostNid);
            var assignments = {};
            for (var key in slots) assignments[key] = [];
            // Assign light DOM children to slots
            for (var ci = 0; ci < lightChildren.length; ci++) {
                var child = _cache[lightChildren[ci]] || __w(lightChildren[ci]);
                var slotAttr = (child.nodeType === 1 && child.getAttribute('slot')) || '';
                if (assignments[slotAttr]) {
                    assignments[slotAttr].push(child);
                } else if (assignments[''] !== undefined) {
                    // Text nodes and elements without slot attr go to default slot
                    if (child.nodeType === 3 || (child.nodeType === 1 && !child.getAttribute('slot'))) {
                        assignments[''].push(child);
                    }
                }
            }
            return { slots: slots, assignments: assignments };
        }

        // HTMLSlotElement.name reflects the 'name' attribute
        Object.defineProperty(HTMLSlotElement.prototype, 'name', {
            get: function() { return this.getAttribute('name') || ''; },
            set: function(v) { this.setAttribute('name', String(v)); },
            configurable: true, enumerable: true,
        });

        // Element.slot reflects the 'slot' attribute (per DOM spec)
        Object.defineProperty(EP, 'slot', {
            get: function() { return this.getAttribute('slot') || ''; },
            set: function(v) { this.setAttribute('slot', String(v)); },
            configurable: true, enumerable: true,
        });

        // HTMLSlotElement prototype methods
        HTMLSlotElement.prototype.assignedNodes = function(options) {
            if (this.__nid === undefined) return [];
            // Walk up to find the shadow host
            var shadowRoot = this;
            while (shadowRoot && !(shadowRoot instanceof ShadowRoot)) {
                shadowRoot = shadowRoot.parentNode;
            }
            if (!shadowRoot || !shadowRoot.host) return [];
            var hostNid = shadowRoot.host.__nid;
            if (hostNid === undefined) return [];
            var result = __computeSlotAssignments(hostNid);
            var name = this.getAttribute('name') || '';
            var assigned = (result.assignments && result.assignments[name]) || [];
            if (options && options.flatten && assigned.length === 0) {
                // Return fallback content (slot's own children)
                var fallback = [];
                var kids = this.childNodes;
                for (var i = 0; i < kids.length; i++) fallback.push(kids[i]);
                return fallback;
            }
            return assigned;
        };

        HTMLSlotElement.prototype.assignedElements = function(options) {
            return this.assignedNodes(options).filter(function(n) { return n.nodeType === 1; });
        };

        // Check if a mutation should trigger slotchange events
        globalThis.__checkSlotChange = function(parent, child) {
            if (!parent || parent.__nid === undefined) return;
            // Case 1: parent is a shadow host — slot assignments may have changed
            if (typeof __n_hasShadowRoot === 'function' && __n_hasShadowRoot(parent.__nid)) {
                var shadowId = __n_getShadowRootId(parent.__nid);
                if (shadowId >= 0) {
                    // Signal all slots in the shadow root
                    function signalAllSlots(nid) {
                        var kids = __n_getAllChildIds(nid);
                        for (var i = 0; i < kids.length; i++) {
                            var w = _cache[kids[i]] || __w(kids[i]);
                            if (w.tagName === 'SLOT') __signalSlotChange(w);
                            signalAllSlots(kids[i]);
                        }
                    }
                    signalAllSlots(shadowId);
                }
            }
            // Case 2: parent is a <slot> element — default content changed
            if (parent.tagName === 'SLOT') {
                __signalSlotChange(parent);
            }
            // Case 3: child is a <slot> element being added/removed
            if (child && child.tagName === 'SLOT') {
                __signalSlotChange(child);
            }
        };
    "#
}
