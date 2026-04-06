/// DOM mutation methods: appendChild, removeChild, insertBefore, moveBefore,
/// and fullscreen tracking.
pub(super) fn dom_mutation_js() -> &'static str {
    r#"
        // Element mutation methods that operate on the real DomTree
        // Helper: when a node is implicitly removed (moved) by appendChild/insertBefore,
        // blur the focused element if it's inside the moving subtree
        function __loseFocusIfRemoving(node) {
            if (__focusCtx.el && node && node.__nid !== undefined && __focusCtx.el.__nid !== undefined) {
                if (__focusCtx.el === node || (node.contains && node.contains(__focusCtx.el))) {
                    var prev = __focusCtx.el;
                    __focusCtx.el = null;
                    __n_setFocusedNode(-1);
                    prev.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: null }));
                    prev.dispatchEvent(new FocusEvent('blur', { bubbles: false, relatedTarget: null }));
                }
            }
        }

        EP.appendChild = function(child) {
            if (child === null || child === undefined || (typeof child === 'object' && child.__nid === undefined && child.nodeType === undefined)) {
                throw new TypeError("Failed to execute 'appendChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            // Attr nodes (nodeType 2) cannot be inserted as children per DOM spec
            if (child && child.nodeType === 2) {
                throw new DOMException("Cannot insert an Attr node", "HierarchyRequestError");
            }
            // CharacterData nodes (Text=3, PI=7, Comment=8) cannot have children
            var pnt = this.nodeType;
            if (pnt === 3 || pnt === 7 || pnt === 8) {
                throw new DOMException("CharacterData type " + this.nodeName + " must not have children", "HierarchyRequestError");
            }
            if (this.__nid === undefined) return child;
            // Blur focused element if it's in the subtree being moved
            if (child && child.__nid !== undefined && child.nodeType !== 11 && __n_getParent(child.__nid) >= 0) {
                __loseFocusIfRemoving(child);
            }
            // Capture ownerDocument BEFORE mutation (tree walk changes after append)
            var parentDoc = this.ownerDocument || (this.nodeType === 9 ? this : document);
            var childDoc = (child && child.__nid !== undefined) ? (child.ownerDocument || document) : null;
            if (child && child.__nid !== undefined) {
                var err = __n_validatePreInsert(this.__nid, child.__nid, -1);
                if (err) __throwValidationError(err);
                if (child.nodeType === 11) {
                    var kids = __n_getAllChildIds(child.__nid);
                    var added = [];
                    for (var i = 0; i < kids.length; i++) {
                        __n_appendChild(this.__nid, kids[i]);
                        added.push(__w(kids[i]));
                    }
                    if (typeof __mo_notify === 'function' && added.length) __mo_notify('childList', this, {addedNodes: added});
                } else {
                    __n_appendChild(this.__nid, child.__nid);
                    if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [child]});
                }
            }
            // Adopt: update ownerDocument for child (and descendants) if parent is in a different document
            // Per spec, when child is a DocumentFragment, adopt the moved children, not the fragment itself
            if (childDoc && parentDoc !== childDoc) {
                if (child.nodeType === 11) {
                    for (var ai = 0; ai < added.length; ai++) __adoptSubtree(added[ai], parentDoc);
                } else {
                    __adoptSubtree(child, parentDoc);
                }
            }
            // CE lifecycle: connectedCallback for inserted nodes
            // For fragments, walk the already-moved children (fragment is now empty)
            if (typeof __ceConnected === 'function' && __isConnected(this.__nid)) {
                if (child.nodeType === 11 && added && added.length) {
                    for (var ci = 0; ci < added.length; ci++) __ceConnected(added[ci]);
                } else {
                    __ceConnected(child);
                }
            }
            // Upgrade custom elements in inserted subtree
            if (typeof __ceUpgradeTree === 'function' && child && child.__nid !== undefined) {
                __ceUpgradeTree(child);
            }
            // Auto-execute scripts when inserting into a browsing context:
            // - The main document tree (nid 0)
            // - Iframe documents (their iframe element is in the main document)
            // Standalone documents (new Document(), createHTMLDocument) are NOT browsing contexts.
            var parentInMainDoc = __isConnectedToMainDoc(this.__nid);
            if (!parentInMainDoc && typeof __braille_find_owning_iframe_realm === 'function') {
                parentInMainDoc = !!__braille_find_owning_iframe_realm(this);
            }
            if (child.nodeType === 11 && added && added.length) {
                for (var fi = 0; fi < added.length; fi++) {
                    if (parentInMainDoc) __braille_maybe_load_scripts_in_subtree(added[fi]);
                    __braille_maybe_load_link(added[fi]);
                    if (typeof __braille_maybe_init_iframe === 'function') __braille_maybe_init_iframe(added[fi]);
                }
            } else {
                if (parentInMainDoc) __braille_maybe_load_scripts_in_subtree(child);
                __braille_maybe_load_link(child);
                if (typeof __braille_maybe_init_iframe === 'function') __braille_maybe_init_iframe(child);
            }
            __ceFlushReactions();
            return child;
        };
        EP.removeChild = function(child) {
            if (child === null || child === undefined || (typeof child === 'object' && child.__nid === undefined)) {
                throw new TypeError("Failed to execute 'removeChild' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (child && child.__nid !== undefined && this.__nid !== undefined) {
                if (__n_getParent(child.__nid) !== this.__nid) {
                    throw new DOMException("The node to be removed is not a child of this node.", "NotFoundError");
                }
                __loseFocusIfRemoving(child);
                // CE lifecycle: disconnectedCallback before removal
                if (typeof __ceDisconnected === 'function' && __isConnected(this.__nid)) {
                    __ceDisconnected(child);
                }
                __n_removeChild(this.__nid, child.__nid);
                if (typeof __mo_notify === 'function') __mo_notify('childList', this, {removedNodes: [child]});
            }
            __ceFlushReactions();
            return child;
        };
        EP.insertBefore = function(newChild, refChild) {
            if (newChild === null || newChild === undefined || (typeof newChild === 'object' && newChild.__nid === undefined && newChild.nodeType === undefined)) {
                throw new TypeError("Failed to execute 'insertBefore' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (arguments.length < 2) {
                throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required, but only 1 present.");
            }
            // Attr nodes (nodeType 2) cannot be inserted as children per DOM spec
            if (newChild && newChild.nodeType === 2) {
                throw new DOMException("Cannot insert an Attr node", "HierarchyRequestError");
            }
            if (refChild !== null && refChild !== undefined && (typeof refChild !== 'object' || refChild.__nid === undefined)) {
                throw new TypeError("Failed to execute 'insertBefore' on 'Node': parameter 2 is not of type 'Node'.");
            }
            if (this.__nid === undefined) return newChild;
            // Blur focused element if it's in the subtree being moved
            if (newChild && newChild.__nid !== undefined && newChild.nodeType !== 11 && __n_getParent(newChild.__nid) >= 0) {
                __loseFocusIfRemoving(newChild);
            }
            if (newChild && newChild.__nid !== undefined) {
                var refId = (refChild && refChild.__nid !== undefined) ? refChild.__nid : -1;
                var err = __n_validatePreInsert(this.__nid, newChild.__nid, refId);
                if (err) __throwValidationError(err);
                if (newChild.nodeType === 11) {
                    var kids = __n_getAllChildIds(newChild.__nid);
                    var added = [];
                    for (var i = 0; i < kids.length; i++) {
                        __n_insertBefore(this.__nid, kids[i], refId);
                        added.push(__w(kids[i]));
                    }
                    if (typeof __mo_notify === 'function' && added.length) __mo_notify('childList', this, {addedNodes: added});
                } else {
                    if (refId >= 0 && newChild.__nid === refId) {
                        return newChild;
                    }
                    __n_insertBefore(this.__nid, newChild.__nid, refId);
                    if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [newChild]});
                }
            }
            // CE lifecycle: connectedCallback for inserted nodes
            if (typeof __ceConnected === 'function' && __isConnected(this.__nid)) {
                __ceConnected(newChild);
            }
            if (typeof __ceUpgradeTree === 'function' && newChild && newChild.__nid !== undefined) {
                __ceUpgradeTree(newChild);
            }
            var parentInMainDoc = __isConnectedToMainDoc(this.__nid);
            if (!parentInMainDoc && typeof __braille_find_owning_iframe_realm === 'function') {
                parentInMainDoc = !!__braille_find_owning_iframe_realm(this);
            }
            if (newChild.nodeType === 11 && added && added.length) {
                for (var fi = 0; fi < added.length; fi++) {
                    if (parentInMainDoc) __braille_maybe_load_scripts_in_subtree(added[fi]);
                    __braille_maybe_load_link(added[fi]);
                    if (typeof __braille_maybe_init_iframe === 'function') __braille_maybe_init_iframe(added[fi]);
                }
            } else {
                if (parentInMainDoc) __braille_maybe_load_scripts_in_subtree(newChild);
                __braille_maybe_load_link(newChild);
                if (typeof __braille_maybe_init_iframe === 'function') __braille_maybe_init_iframe(newChild);
            }
            __ceFlushReactions();
            return newChild;
        };

        // moveBefore() — atomic move with CE lifecycle (ParentNode mixin)
        EP.moveBefore = function(node, child) {
            var pt = this.nodeType;
            // Type checks per spec
            if (arguments.length < 2) {
                throw new TypeError("Failed to execute 'moveBefore' on 'Node': 2 arguments required, but only " + arguments.length + " present.");
            }
            if (node === null || node === undefined || typeof node !== 'object' || node.nodeType === undefined) {
                throw new TypeError("Failed to execute 'moveBefore' on 'Node': parameter 1 is not of type 'Node'.");
            }
            if (child !== null && child !== undefined && (typeof child !== 'object' || child.nodeType === undefined)) {
                throw new TypeError("Failed to execute 'moveBefore' on 'Node': parameter 2 is not of type 'Node'.");
            }
            // Only Element (1) and CharacterData (3, 7, 8) nodes can be moved
            var nt = node.nodeType;
            if (nt !== 1 && nt !== 3 && nt !== 7 && nt !== 8) {
                throw new DOMException("The node to be moved is not an Element or CharacterData node.", "HierarchyRequestError");
            }
            if (this.__nid === undefined || node.__nid === undefined) return;
            // node must have a parent (i.e., be in a tree)
            var nodeParent = __n_getParent(node.__nid);
            if (nodeParent < 0) {
                throw new DOMException("The node to be moved must have a parent.", "HierarchyRequestError");
            }
            // node's parent must be connected iff this is connected (same connectivity)
            var thisConnected = __isConnected(this.__nid);
            var nodeConnected = __isConnected(node.__nid);
            if (thisConnected !== nodeConnected) {
                throw new DOMException("Cannot move between connected and disconnected trees.", "HierarchyRequestError");
            }
            // If both disconnected, they must share a root (traversing shadow hosts)
            if (!thisConnected && !nodeConnected) {
                function findRoot(nid) {
                    var cur = nid;
                    while (true) {
                        var p = __n_getParent(cur);
                        if (p >= 0) { cur = p; continue; }
                        // Check for shadow root → host relationship
                        var w = __w(cur);
                        if (w && w.host && w.host.__nid !== undefined) { cur = w.host.__nid; continue; }
                        break;
                    }
                    return cur;
                }
                if (findRoot(this.__nid) !== findRoot(node.__nid)) {
                    throw new DOMException("Cannot move between different disconnected trees.", "HierarchyRequestError");
                }
            }
            // Ancestor check: node must not be an ancestor of parent
            var anc = this.__nid;
            while (anc >= 0) {
                if (anc === node.__nid) {
                    throw new DOMException("The new parent is a descendant of the node to be moved.", "HierarchyRequestError");
                }
                anc = __n_getParent(anc);
            }
            // Document parent: Text not allowed, only one Element allowed
            if (pt === 9) {
                if (nt === 3) throw new DOMException("Cannot insert a Text node under a Document.", "HierarchyRequestError");
                if (nt === 1) {
                    var docKids = __n_getAllChildIds(this.__nid);
                    for (var di = 0; di < docKids.length; di++) {
                        var dkw = __w(docKids[di]);
                        if (dkw.nodeType === 1 && dkw.__nid !== node.__nid) {
                            throw new DOMException("Only one element child is allowed in a Document.", "HierarchyRequestError");
                        }
                    }
                }
            }
            // If child is specified and not null, it must be a child of this node
            if (child !== null && child !== undefined) {
                if (child.__nid === undefined || __n_getParent(child.__nid) !== this.__nid) {
                    throw new DOMException("The child to insert before is not a child of this node.", "NotFoundError");
                }
                if (node.__nid === child.__nid) return;
            }
            // Collect upgraded custom elements in the subtree (tree order) for lifecycle
            var ceNodes = [];
            function __collectCE(n) {
                if (n.__ce_upgraded) ceNodes.push(n);
                if (n.__nid !== undefined) {
                    var kids = __n_getAllChildIds(n.__nid);
                    for (var ci = 0; ci < kids.length; ci++) {
                        var ck = _cache[kids[ci]];
                        if (ck) __collectCE(ck);
                    }
                }
            }
            __collectCE(node);

            // Perform atomic move
            var refId = (child && child.__nid !== undefined) ? child.__nid : -1;
            __n_insertBefore(this.__nid, node.__nid, refId);
            if (typeof __mo_notify === 'function') __mo_notify('childList', this, {addedNodes: [node]});

            // Fire CE lifecycle: per spec, element stays connected during move
            for (var cei = 0; cei < ceNodes.length; cei++) {
                var ceEl = ceNodes[cei];
                if (typeof ceEl.connectedMoveCallback === 'function') {
                    ceEl.connectedMoveCallback();
                } else {
                    if (typeof ceEl.disconnectedCallback === 'function') ceEl.disconnectedCallback();
                    if (typeof ceEl.connectedCallback === 'function') ceEl.connectedCallback();
                }
            }
        };

        // Fullscreen tracking
        var __fullscreenElement = null;
        EP.requestFullscreen = function() { __fullscreenElement = this; doc.dispatchEvent(new Event('fullscreenchange')); return Promise.resolve(); };
    "#
}
