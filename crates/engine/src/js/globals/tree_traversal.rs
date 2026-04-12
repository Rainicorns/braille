//! TreeWalker and NodeIterator — spec-compliant DOM traversal APIs.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(TREE_TRAVERSAL_JS).unwrap();
}

const TREE_TRAVERSAL_JS: &str = r#"
        // NodeIterator pre-removal registry (DOM spec §6.1)
        var __liveNodeIterators = [];

        function __isInclusiveAncestorOf(ancestor, node) {
            while (node) {
                if (node === ancestor) return true;
                node = node.parentNode;
            }
            return false;
        }

        function __previousNodeInTree(node, root) {
            if (node === root) return null;
            var sib = node.previousSibling;
            if (sib) {
                while (sib.lastChild) sib = sib.lastChild;
                return sib;
            }
            return node.parentNode;
        }

        function __nextNodeDescendants(node) {
            while (node && !node.nextSibling) {
                node = node.parentNode;
            }
            return node ? node.nextSibling : null;
        }

        // Called before removeChild/replaceChild to adjust all live NodeIterators
        // per DOM spec §6.1 "NodeIterator pre-removing steps"
        globalThis.__adjustNodeIteratorsForRemoval = function(node) {
            for (var i = __liveNodeIterators.length - 1; i >= 0; i--) {
                var iter = __liveNodeIterators[i];
                if (!iter) { __liveNodeIterators.splice(i, 1); continue; }

                // Step 1: if node is root, an ancestor of root, or not an inclusive
                // ancestor of referenceNode, skip (spec §6.1 + implicit ancestor-of-root rule)
                if (__isInclusiveAncestorOf(node, iter.root)) continue;
                if (!__isInclusiveAncestorOf(node, iter.referenceNode)) continue;

                // Step 2: if pointer is after referenceNode
                if (!iter.pointerBeforeReferenceNode) {
                    var prev = __previousNodeInTree(node, iter.root);
                    if (prev) {
                        iter.referenceNode = prev;
                    }
                    continue;
                }

                // Step 3: try node following last inclusive descendant of node
                var next = __nextNodeDescendants(node);
                if (next) {
                    iter.referenceNode = next;
                    continue;
                }

                // Step 4-5: set to previous node and flip pointer
                var prev = __previousNodeInTree(node, iter.root);
                if (prev) {
                    iter.referenceNode = prev;
                    iter.pointerBeforeReferenceNode = false;
                }
            }
        };

        // TreeWalker — spec-compliant traversal with whatToShow/filter support
        globalThis.TreeWalker = class TreeWalker {
            constructor(root, whatToShow, filter) {
                var _currentNode = root;
                Object.defineProperties(this, {
                    root: { value: root, enumerable: true },
                    whatToShow: { value: whatToShow === undefined ? 0xFFFFFFFF : (whatToShow >>> 0), enumerable: true },
                    filter: { value: filter == null ? null : filter, enumerable: true },
                    currentNode: {
                        get: function() { return _currentNode; },
                        set: function(v) {
                            if (v === null || v === undefined || v.__nid === undefined) {
                                throw new TypeError("Failed to set 'currentNode' on 'TreeWalker': The provided value is not of type 'Node'.");
                            }
                            _currentNode = v;
                        },
                        enumerable: true, configurable: true
                    }
                });
                this._active = false;
            }
            _acceptNode(node) {
                var nodeType = node.nodeType;
                // whatToShow bitmask: bit (nodeType - 1)
                if (!((1 << (nodeType - 1)) & this.whatToShow)) return 3; // FILTER_SKIP
                if (this.filter == null) return 1; // FILTER_ACCEPT (null or undefined)
                if (this._active) throw new DOMException("Failed to execute 'acceptNode' on 'NodeFilter': filter is active", "InvalidStateError");
                this._active = true;
                var result;
                try {
                    if (typeof this.filter === 'function') {
                        result = this.filter(node);
                    } else {
                        var acceptNode = this.filter.acceptNode;
                        if (typeof acceptNode !== 'function') {
                            throw new TypeError("Failed to execute 'acceptNode' on 'NodeFilter': acceptNode is not a function");
                        }
                        result = acceptNode.call(this.filter, node);
                    }
                } finally {
                    this._active = false;
                }
                return (result >>> 0) & 0xFFFF;
            }
            parentNode() {
                var node = this.currentNode;
                while (node && node !== this.root) {
                    node = node.parentNode;
                    if (node) {
                        var r = this._acceptNode(node);
                        if (r === 1) { this.currentNode = node; return node; }
                    }
                }
                return null;
            }
            _traverseChildren(first) {
                var node = first ? this.currentNode.firstChild : this.currentNode.lastChild;
                while (node) {
                    var r = this._acceptNode(node);
                    if (r === 1) { this.currentNode = node; return node; }
                    if (r === 3) { // FILTER_SKIP — try children
                        var child = first ? node.firstChild : node.lastChild;
                        if (child) { node = child; continue; }
                    }
                    // FILTER_REJECT or SKIP with no children — try siblings
                    while (node) {
                        var sib = first ? node.nextSibling : node.previousSibling;
                        if (sib) { node = sib; break; }
                        var parent = node.parentNode;
                        if (!parent || parent === this.root || parent === this.currentNode) { return null; }
                        node = parent;
                    }
                }
                return null;
            }
            firstChild() { return this._traverseChildren(true); }
            lastChild() { return this._traverseChildren(false); }
            _traverseSiblings(next) {
                // Spec: https://dom.spec.whatwg.org/#concept-traverse-siblings
                var node = this.currentNode;
                if (node === this.root) return null;
                while (true) {
                    var sibling = next ? node.nextSibling : node.previousSibling;
                    while (sibling) {
                        node = sibling;
                        var result = this._acceptNode(node);
                        if (result === 1) { this.currentNode = node; return node; }
                        // Always try children first
                        sibling = next ? node.firstChild : node.lastChild;
                        // If REJECT or no children, try next/prev sibling instead
                        if (result === 2 || !sibling) {
                            sibling = next ? node.nextSibling : node.previousSibling;
                        }
                    }
                    node = node.parentNode;
                    if (!node || node === this.root) return null;
                    if (this._acceptNode(node) === 1) return null;
                }
            }
            nextSibling() { return this._traverseSiblings(true); }
            previousSibling() { return this._traverseSiblings(false); }
            nextNode() {
                // Spec: https://dom.spec.whatwg.org/#dom-treewalker-nextnode
                var node = this.currentNode;
                var result = 1; // FILTER_ACCEPT
                while (true) {
                    // Descend into children while not REJECT
                    while (result !== 2 && node.firstChild) {
                        node = node.firstChild;
                        result = this._acceptNode(node);
                        if (result === 1) { this.currentNode = node; return node; }
                    }
                    // Find next sibling, walking up ancestors
                    var sibling = null;
                    var temp = node;
                    while (temp) {
                        if (temp === this.root) return null;
                        sibling = temp.nextSibling;
                        if (sibling) { node = sibling; break; }
                        temp = temp.parentNode;
                    }
                    if (!temp) return null;
                    result = this._acceptNode(node);
                    if (result === 1) { this.currentNode = node; return node; }
                }
            }
            previousNode() {
                // Spec: https://dom.spec.whatwg.org/#dom-treewalker-previousnode
                var node = this.currentNode;
                while (node !== this.root) {
                    var sib = node.previousSibling;
                    while (sib) {
                        node = sib;
                        var result = this._acceptNode(node);
                        // Descend to last descendant while not REJECT and has children
                        while (result !== 2 && node.lastChild) {
                            node = node.lastChild;
                            result = this._acceptNode(node);
                        }
                        if (result === 1) { this.currentNode = node; return node; }
                        sib = node.previousSibling;
                    }
                    // No more siblings — go to parent
                    if (node === this.root || !node.parentNode) return null;
                    node = node.parentNode;
                    if (this._acceptNode(node) === 1) { this.currentNode = node; return node; }
                }
                return null;
            }
        };

        TreeWalker.prototype[Symbol.toStringTag] = 'TreeWalker';

        // NodeIterator — flat pre-order traversal with whatToShow/filter support
        globalThis.NodeIterator = function NodeIterator(root, whatToShow, filter) {
            Object.defineProperties(this, {
                root: { value: root, enumerable: true },
                whatToShow: { value: whatToShow === undefined ? 0xFFFFFFFF : (whatToShow >>> 0), enumerable: true },
                filter: { value: filter || null, enumerable: true },
                referenceNode: { value: root, writable: true, enumerable: true },
                pointerBeforeReferenceNode: { value: true, writable: true, enumerable: true }
            });
            this._active = false;
            __liveNodeIterators.push(this);
        };
        NodeIterator.prototype._acceptNode = function(node) {
            if (!((1 << (node.nodeType - 1)) & this.whatToShow)) return 3; // FILTER_SKIP
            if (this.filter == null) return 1;
            if (this._active) throw new DOMException("Failed to execute 'acceptNode' on 'NodeFilter': filter is active", "InvalidStateError");
            this._active = true;
            var result;
            try {
                if (typeof this.filter === 'function') {
                    result = this.filter(node);
                } else {
                    var acceptNode = this.filter.acceptNode;
                    if (typeof acceptNode !== 'function') {
                        throw new TypeError("Failed to execute 'acceptNode' on 'NodeFilter': acceptNode is not a function");
                    }
                    result = acceptNode.call(this.filter, node);
                }
            } finally {
                this._active = false;
            }
            return (result >>> 0) & 0xFFFF;
        };
        NodeIterator.prototype._nextInPreOrder = function(node) {
            if (node.firstChild) return node.firstChild;
            var cur = node;
            while (cur && cur !== this.root) {
                if (cur.nextSibling) return cur.nextSibling;
                cur = cur.parentNode;
            }
            return null;
        };
        NodeIterator.prototype._prevInPreOrder = function(node) {
            if (node === this.root) return null;
            var sib = node.previousSibling;
            if (sib) {
                while (sib.lastChild) sib = sib.lastChild;
                return sib;
            }
            return node.parentNode;
        };
        NodeIterator.prototype.nextNode = function() {
            var node = this.referenceNode;
            var beforeNode = this.pointerBeforeReferenceNode;
            while (true) {
                if (!beforeNode) {
                    node = this._nextInPreOrder(node);
                    if (!node) return null;
                } else {
                    beforeNode = false;
                }
                var r = this._acceptNode(node);
                if (r === 1) {
                    this.referenceNode = node;
                    this.pointerBeforeReferenceNode = beforeNode;
                    return node;
                }
            }
        };
        NodeIterator.prototype.previousNode = function() {
            var node = this.referenceNode;
            var beforeNode = this.pointerBeforeReferenceNode;
            while (true) {
                if (beforeNode) {
                    node = this._prevInPreOrder(node);
                    if (!node) return null;
                } else {
                    beforeNode = true;
                }
                var r = this._acceptNode(node);
                if (r === 1) {
                    this.referenceNode = node;
                    this.pointerBeforeReferenceNode = beforeNode;
                    return node;
                }
            }
        };
        NodeIterator.prototype.detach = function() {}; // legacy no-op
        NodeIterator.prototype[Symbol.toStringTag] = 'NodeIterator';
"#;
