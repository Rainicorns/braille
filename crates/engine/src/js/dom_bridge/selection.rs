/// Selection API: BrailleSelection singleton wrapping a live Range.
/// Provides window.getSelection() / document.getSelection() with
/// proper anchor/focus directionality and moveBefore integration
/// (via __liveRanges automatic adjustment).
pub(super) fn selection_js() -> &'static str {
    r#"
        // Selection singleton — one per document
        function BrailleSelection() {
            this._range = null;
            this._anchorIsStart = true;
        }

        Object.defineProperties(BrailleSelection.prototype, {
            anchorNode: { get: function() {
                if (!this._range) return null;
                return this._anchorIsStart ? this._range.startContainer : this._range.endContainer;
            }, enumerable: true, configurable: true },
            anchorOffset: { get: function() {
                if (!this._range) return 0;
                return this._anchorIsStart ? this._range.startOffset : this._range.endOffset;
            }, enumerable: true, configurable: true },
            focusNode: { get: function() {
                if (!this._range) return null;
                return this._anchorIsStart ? this._range.endContainer : this._range.startContainer;
            }, enumerable: true, configurable: true },
            focusOffset: { get: function() {
                if (!this._range) return 0;
                return this._anchorIsStart ? this._range.endOffset : this._range.startOffset;
            }, enumerable: true, configurable: true },
            rangeCount: { get: function() {
                return this._range ? 1 : 0;
            }, enumerable: true, configurable: true },
            isCollapsed: { get: function() {
                if (!this._range) return true;
                return this._range.collapsed;
            }, enumerable: true, configurable: true },
            type: { get: function() {
                if (!this._range) return 'None';
                if (this._range.collapsed) return 'Caret';
                return 'Range';
            }, enumerable: true, configurable: true },
        });

        BrailleSelection.prototype.getRangeAt = function(index) {
            if (index !== 0 || !this._range) {
                throw new DOMException("Failed to execute 'getRangeAt' on 'Selection': " + index + " is not a valid index.", "IndexSizeError");
            }
            return this._range;
        };

        BrailleSelection.prototype.addRange = function(range) {
            if (this._range) return; // spec: only one range
            this._range = range;
            this._anchorIsStart = true;
            // Register in __liveRanges so __adjustRangesForRemoval keeps it updated
            if (__liveRanges.indexOf(range) < 0) __liveRanges.push(range);
        };

        BrailleSelection.prototype.removeAllRanges = function() {
            this._range = null;
        };

        BrailleSelection.prototype.removeRange = function(range) {
            if (this._range === range) this._range = null;
        };

        BrailleSelection.prototype.empty = function() {
            this.removeAllRanges();
        };

        BrailleSelection.prototype.collapse = function(node, offset) {
            if (node === null || node === undefined) {
                this.removeAllRanges();
                return;
            }
            var r = new Range();
            r.setStart(node, offset || 0);
            r.collapse(true);
            this._range = r;
            this._anchorIsStart = true;
            if (__liveRanges.indexOf(r) < 0) __liveRanges.push(r);
        };

        BrailleSelection.prototype.collapseToStart = function() {
            if (!this._range) throw new DOMException("Failed to execute 'collapseToStart' on 'Selection': there is no selection.", "InvalidStateError");
            this._range.collapse(true);
            this._anchorIsStart = true;
        };

        BrailleSelection.prototype.collapseToEnd = function() {
            if (!this._range) throw new DOMException("Failed to execute 'collapseToEnd' on 'Selection': there is no selection.", "InvalidStateError");
            this._range.collapse(false);
            this._anchorIsStart = true;
        };

        BrailleSelection.prototype.setPosition = function(node, offset) {
            this.collapse(node, offset);
        };

        BrailleSelection.prototype.extend = function(node, offset) {
            if (!this._range) throw new DOMException("Failed to execute 'extend' on 'Selection': there is no selection.", "InvalidStateError");
            offset = offset || 0;
            // The anchor stays fixed, focus moves to (node, offset)
            var anchorNode = this.anchorNode;
            var anchorOffset = this.anchorOffset;
            var r = new Range();
            // Determine order: if (node, offset) is before anchor, anchor becomes end
            var tmpRange = new Range();
            tmpRange.setStart(anchorNode, anchorOffset);
            tmpRange.setEnd(anchorNode, anchorOffset);
            var cmp = tmpRange.compareBoundaryPoints(Range.START_TO_START, (function() {
                var r2 = new Range(); r2.setStart(node, offset); r2.setEnd(node, offset); return r2;
            })());
            if (cmp > 0) {
                // focus is before anchor
                r.setStart(node, offset);
                r.setEnd(anchorNode, anchorOffset);
                this._anchorIsStart = false;
            } else {
                r.setStart(anchorNode, anchorOffset);
                r.setEnd(node, offset);
                this._anchorIsStart = true;
            }
            this._range = r;
            if (__liveRanges.indexOf(r) < 0) __liveRanges.push(r);
        };

        BrailleSelection.prototype.selectAllChildren = function(node) {
            var r = new Range();
            r.selectNodeContents(node);
            this._range = r;
            this._anchorIsStart = true;
            if (__liveRanges.indexOf(r) < 0) __liveRanges.push(r);
        };

        BrailleSelection.prototype.containsNode = function(node, allowPartial) {
            if (!this._range) return false;
            if (allowPartial) return this._range.intersectsNode(node);
            // Full containment: node's range must be within selection range
            var nodeRange = new Range();
            nodeRange.selectNode(node);
            return this._range.compareBoundaryPoints(Range.START_TO_START, nodeRange) <= 0 &&
                   this._range.compareBoundaryPoints(Range.END_TO_END, nodeRange) >= 0;
        };

        BrailleSelection.prototype.deleteFromDocument = function() {
            if (this._range) this._range.deleteContents();
        };

        BrailleSelection.prototype.toString = function() {
            return this._range ? this._range.toString() : '';
        };

        BrailleSelection.prototype.setBaseAndExtent = function(anchorNode, anchorOffset, focusNode, focusOffset) {
            var r = new Range();
            // Determine order
            var tmpA = new Range(); tmpA.setStart(anchorNode, anchorOffset); tmpA.setEnd(anchorNode, anchorOffset);
            var tmpF = new Range(); tmpF.setStart(focusNode, focusOffset); tmpF.setEnd(focusNode, focusOffset);
            var cmp = tmpA.compareBoundaryPoints(Range.START_TO_START, tmpF);
            if (cmp <= 0) {
                r.setStart(anchorNode, anchorOffset);
                r.setEnd(focusNode, focusOffset);
                this._anchorIsStart = true;
            } else {
                r.setStart(focusNode, focusOffset);
                r.setEnd(anchorNode, anchorOffset);
                this._anchorIsStart = false;
            }
            this._range = r;
            if (__liveRanges.indexOf(r) < 0) __liveRanges.push(r);
        };

        // Install singleton
        var __selection = new BrailleSelection();
        doc.getSelection = function() { return __selection; };
        globalThis.getSelection = function() { return __selection; };
        globalThis.Selection = BrailleSelection;
    "#
}
