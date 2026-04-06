//! TreeWalker and NodeIterator — spec-compliant DOM traversal APIs.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(TREE_TRAVERSAL_JS).unwrap();
}

const TREE_TRAVERSAL_JS: &str = r#"
        // TreeWalker — spec-compliant traversal with whatToShow/filter support
        globalThis.TreeWalker = class TreeWalker {
            constructor(root, whatToShow, filter) {
                this.root = root;
                this.whatToShow = whatToShow === undefined ? 0xFFFFFFFF : (whatToShow >>> 0);
                this.filter = filter || null;
                this.currentNode = root;
            }
            _acceptNode(node) {
                var nodeType = node.nodeType;
                // whatToShow bitmask: bit (nodeType - 1)
                if (!((1 << (nodeType - 1)) & this.whatToShow)) return 3; // FILTER_SKIP
                if (!this.filter) return 1; // FILTER_ACCEPT
                if (typeof this.filter === 'function') return this.filter(node);
                if (typeof this.filter.acceptNode === 'function') return this.filter.acceptNode(node);
                return 1;
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
                var node = this.currentNode;
                if (node === this.root) return null;
                while (true) {
                    var sib = next ? node.nextSibling : node.previousSibling;
                    while (sib) {
                        var r = this._acceptNode(sib);
                        if (r === 1) { this.currentNode = sib; return sib; }
                        if (r === 3) { // FILTER_SKIP — descend into children
                            var child = next ? sib.firstChild : sib.lastChild;
                            if (child) { sib = child; continue; }
                        }
                        sib = next ? sib.nextSibling : sib.previousSibling;
                    }
                    node = node.parentNode;
                    if (!node || node === this.root) return null;
                    var pr = this._acceptNode(node);
                    if (pr === 1) return null; // parent accepted = no more siblings to traverse
                }
            }
            nextSibling() { return this._traverseSiblings(true); }
            previousSibling() { return this._traverseSiblings(false); }
            nextNode() {
                var node = this.currentNode;
                // Try first child
                var child = node.firstChild;
                while (child) {
                    var r = this._acceptNode(child);
                    if (r === 1) { this.currentNode = child; return child; }
                    if (r === 3 && child.firstChild) { child = child.firstChild; continue; } // SKIP — descend
                    // REJECT or SKIP-no-children — try next sibling, then walk up
                    break;
                }
                // Depth-first: try siblings and ancestors' siblings
                var cur = child || node;
                while (cur && cur !== this.root) {
                    var sib = cur.nextSibling;
                    while (sib) {
                        var r = this._acceptNode(sib);
                        if (r === 1) { this.currentNode = sib; return sib; }
                        if (r === 3 && sib.firstChild) { sib = sib.firstChild; continue; }
                        sib = sib.nextSibling;
                    }
                    cur = cur.parentNode;
                }
                return null;
            }
            previousNode() {
                var node = this.currentNode;
                while (node !== this.root) {
                    var sib = node.previousSibling;
                    while (sib) {
                        // Descend to last accepted descendant
                        var r = this._acceptNode(sib);
                        var last = sib.lastChild;
                        while (last) {
                            var lr = this._acceptNode(last);
                            if (lr === 2) { // REJECT — try previous sibling of last
                                last = last.previousSibling;
                                continue;
                            }
                            var deeper = last.lastChild;
                            if (deeper) { last = deeper; continue; }
                            if (lr === 1) { this.currentNode = last; return last; }
                            last = last.previousSibling;
                        }
                        if (r === 1) { this.currentNode = sib; return sib; }
                        sib = sib.previousSibling;
                    }
                    var parent = node.parentNode;
                    if (!parent || parent === this.root) return null;
                    var pr = this._acceptNode(parent);
                    if (pr === 1) { this.currentNode = parent; return parent; }
                    node = parent;
                }
                return null;
            }
        };

        // NodeIterator — flat pre-order traversal with whatToShow/filter support
        globalThis.NodeIterator = class NodeIterator {
            constructor(root, whatToShow, filter) {
                this.root = root;
                this.whatToShow = whatToShow === undefined ? 0xFFFFFFFF : (whatToShow >>> 0);
                this.filter = filter || null;
                this.referenceNode = root;
                this.pointerBeforeReferenceNode = true;
            }
            _acceptNode(node) {
                if (!((1 << (node.nodeType - 1)) & this.whatToShow)) return 3; // FILTER_SKIP
                if (!this.filter) return 1;
                if (typeof this.filter === 'function') return this.filter(node);
                if (typeof this.filter.acceptNode === 'function') return this.filter.acceptNode(node);
                return 1;
            }
            _nextInPreOrder(node) {
                if (node.firstChild) return node.firstChild;
                var cur = node;
                while (cur && cur !== this.root) {
                    if (cur.nextSibling) return cur.nextSibling;
                    cur = cur.parentNode;
                }
                return null;
            }
            _prevInPreOrder(node) {
                if (node === this.root) return null;
                var sib = node.previousSibling;
                if (sib) {
                    // Descend to last descendant
                    while (sib.lastChild) sib = sib.lastChild;
                    return sib;
                }
                return node.parentNode;
            }
            nextNode() {
                var node = this.referenceNode;
                var beforeRef = this.pointerBeforeReferenceNode;
                if (beforeRef) {
                    // pointer is before reference — first candidate is reference itself
                    var r = this._acceptNode(node);
                    this.pointerBeforeReferenceNode = false;
                    if (r === 1) return node;
                    // If rejected/skipped, fall through to walk forward
                }
                while (true) {
                    node = this._nextInPreOrder(node);
                    if (!node) return null;
                    this.referenceNode = node;
                    this.pointerBeforeReferenceNode = false;
                    var r = this._acceptNode(node);
                    if (r === 1) return node;
                }
            }
            previousNode() {
                var node = this.referenceNode;
                var beforeRef = this.pointerBeforeReferenceNode;
                if (!beforeRef) {
                    // pointer is after reference — first candidate is reference itself
                    var r = this._acceptNode(node);
                    this.pointerBeforeReferenceNode = true;
                    if (r === 1) return node;
                }
                while (true) {
                    node = this._prevInPreOrder(node);
                    if (!node) return null;
                    this.referenceNode = node;
                    this.pointerBeforeReferenceNode = true;
                    var r = this._acceptNode(node);
                    if (r === 1) return node;
                }
            }
            detach() {} // legacy no-op
        };
"#;
