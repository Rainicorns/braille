//! XPath API — document.evaluate() and XPathResult class.
//! Standalone Web API polyfill: queries the DOM through JS APIs, no Rust-side XPath needed.

use rquickjs::Ctx;

pub(super) fn register(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(XPATH_JS).unwrap();
}

const XPATH_JS: &str = r#"
(function() {
    "use strict";

    // XPathResult type constants
    var ANY_TYPE = 0;
    var NUMBER_TYPE = 1;
    var STRING_TYPE = 2;
    var BOOLEAN_TYPE = 3;
    var UNORDERED_NODE_ITERATOR_TYPE = 4;
    var ORDERED_NODE_ITERATOR_TYPE = 5;
    var UNORDERED_NODE_SNAPSHOT_TYPE = 6;
    var ORDERED_NODE_SNAPSHOT_TYPE = 7;
    var ANY_UNORDERED_NODE_TYPE = 8;
    var FIRST_ORDERED_NODE_TYPE = 9;

    function XPathResult(nodes, requestedType) {
        this._nodes = nodes || [];
        this.resultType = requestedType;
        this.invalidIteratorState = false;
        this._iterIndex = 0;
    }

    XPathResult.ANY_TYPE = ANY_TYPE;
    XPathResult.NUMBER_TYPE = NUMBER_TYPE;
    XPathResult.STRING_TYPE = STRING_TYPE;
    XPathResult.BOOLEAN_TYPE = BOOLEAN_TYPE;
    XPathResult.UNORDERED_NODE_ITERATOR_TYPE = UNORDERED_NODE_ITERATOR_TYPE;
    XPathResult.ORDERED_NODE_ITERATOR_TYPE = ORDERED_NODE_ITERATOR_TYPE;
    XPathResult.UNORDERED_NODE_SNAPSHOT_TYPE = UNORDERED_NODE_SNAPSHOT_TYPE;
    XPathResult.ORDERED_NODE_SNAPSHOT_TYPE = ORDERED_NODE_SNAPSHOT_TYPE;
    XPathResult.ANY_UNORDERED_NODE_TYPE = ANY_UNORDERED_NODE_TYPE;
    XPathResult.FIRST_ORDERED_NODE_TYPE = FIRST_ORDERED_NODE_TYPE;

    // Instance constants (spec requires these on prototype too)
    XPathResult.prototype.ANY_TYPE = ANY_TYPE;
    XPathResult.prototype.NUMBER_TYPE = NUMBER_TYPE;
    XPathResult.prototype.STRING_TYPE = STRING_TYPE;
    XPathResult.prototype.BOOLEAN_TYPE = BOOLEAN_TYPE;
    XPathResult.prototype.UNORDERED_NODE_ITERATOR_TYPE = UNORDERED_NODE_ITERATOR_TYPE;
    XPathResult.prototype.ORDERED_NODE_ITERATOR_TYPE = ORDERED_NODE_ITERATOR_TYPE;
    XPathResult.prototype.UNORDERED_NODE_SNAPSHOT_TYPE = UNORDERED_NODE_SNAPSHOT_TYPE;
    XPathResult.prototype.ORDERED_NODE_SNAPSHOT_TYPE = ORDERED_NODE_SNAPSHOT_TYPE;
    XPathResult.prototype.ANY_UNORDERED_NODE_TYPE = ANY_UNORDERED_NODE_TYPE;
    XPathResult.prototype.FIRST_ORDERED_NODE_TYPE = FIRST_ORDERED_NODE_TYPE;

    Object.defineProperties(XPathResult.prototype, {
        singleNodeValue: { get: function() {
            if (this.resultType !== ANY_UNORDERED_NODE_TYPE && this.resultType !== FIRST_ORDERED_NODE_TYPE) {
                throw new DOMException("The result type is not ANY_UNORDERED_NODE_TYPE or FIRST_ORDERED_NODE_TYPE.", "InvalidStateError");
            }
            return this._nodes.length > 0 ? this._nodes[0] : null;
        }, configurable: true },
        snapshotLength: { get: function() {
            if (this.resultType !== UNORDERED_NODE_SNAPSHOT_TYPE && this.resultType !== ORDERED_NODE_SNAPSHOT_TYPE) {
                throw new DOMException("The result type is not a snapshot type.", "InvalidStateError");
            }
            return this._nodes.length;
        }, configurable: true },
        numberValue: { get: function() {
            if (this.resultType !== NUMBER_TYPE) {
                throw new DOMException("The result type is not NUMBER_TYPE.", "InvalidStateError");
            }
            if (this._nodes.length === 0) return NaN;
            var v = this._nodes[0];
            if (v && v.nodeType) v = v.textContent;
            return Number(v);
        }, configurable: true },
        stringValue: { get: function() {
            if (this.resultType !== STRING_TYPE) {
                throw new DOMException("The result type is not STRING_TYPE.", "InvalidStateError");
            }
            if (this._nodes.length === 0) return '';
            var v = this._nodes[0];
            if (v && v.nodeType) return v.textContent || '';
            return String(v);
        }, configurable: true },
        booleanValue: { get: function() {
            if (this.resultType !== BOOLEAN_TYPE) {
                throw new DOMException("The result type is not BOOLEAN_TYPE.", "InvalidStateError");
            }
            return this._nodes.length > 0;
        }, configurable: true }
    });

    XPathResult.prototype.iterateNext = function() {
        if (this.resultType !== UNORDERED_NODE_ITERATOR_TYPE && this.resultType !== ORDERED_NODE_ITERATOR_TYPE) {
            throw new DOMException("The result type is not an iterator type.", "InvalidStateError");
        }
        if (this._iterIndex >= this._nodes.length) return null;
        return this._nodes[this._iterIndex++];
    };

    XPathResult.prototype.snapshotItem = function(index) {
        if (this.resultType !== UNORDERED_NODE_SNAPSHOT_TYPE && this.resultType !== ORDERED_NODE_SNAPSHOT_TYPE) {
            throw new DOMException("The result type is not a snapshot type.", "InvalidStateError");
        }
        if (index < 0 || index >= this._nodes.length) return null;
        return this._nodes[index];
    };

    XPathResult.prototype[Symbol.toStringTag] = 'XPathResult';

    globalThis.XPathResult = XPathResult;

    // ---- XPath expression evaluator ----
    // Translates common XPath patterns to DOM queries.

    function __xpathGetAllDescendants(ctx) {
        var result = [];
        var walker = ctx.ownerDocument
            ? ctx.ownerDocument.createTreeWalker(ctx, 0xFFFFFFFF)
            : document.createTreeWalker(ctx, 0xFFFFFFFF);
        var node;
        while ((node = walker.nextNode())) {
            result.push(node);
        }
        return result;
    }

    function __xpathGetAllElements(ctx) {
        // Use getElementsByTagName('*') for elements only
        if (ctx.getElementsByTagName) {
            var list = ctx.getElementsByTagName('*');
            var result = [];
            for (var i = 0; i < list.length; i++) result.push(list[i]);
            return result;
        }
        return [];
    }

    // Predicate parser: [condition]
    function __xpathMatchesPredicate(node, predicate) {
        predicate = predicate.trim();
        // [@attr='value'] or [@attr="value"]
        var attrValMatch = predicate.match(/^@([\w\-]+)\s*=\s*['"]([^'"]*)['"]\s*$/);
        if (attrValMatch) {
            return node.getAttribute && node.getAttribute(attrValMatch[1]) === attrValMatch[2];
        }
        // [@attr]
        var attrMatch = predicate.match(/^@([\w\-]+)$/);
        if (attrMatch) {
            return node.hasAttribute && node.hasAttribute(attrMatch[1]);
        }
        // [contains(@attr, 'value')]
        var containsMatch = predicate.match(/^contains\s*\(\s*@([\w\-]+)\s*,\s*['"]([^'"]*)['"]\s*\)$/);
        if (containsMatch) {
            var av = node.getAttribute && node.getAttribute(containsMatch[1]);
            return av != null && av.indexOf(containsMatch[2]) !== -1;
        }
        // [text()='value'] or [text()="value"]
        var textMatch = predicate.match(/^text\(\)\s*=\s*['"]([^'"]*)['"]\s*$/);
        if (textMatch) {
            return (node.textContent || '') === textMatch[1];
        }
        // [number] — positional predicate (1-based)
        var posMatch = predicate.match(/^\d+$/);
        if (posMatch) {
            // Positional predicates are handled at the step level, not here
            return true;
        }
        // [not(@attr)]
        var notAttrMatch = predicate.match(/^not\s*\(\s*@([\w\-]+)\s*\)$/);
        if (notAttrMatch) {
            return !(node.hasAttribute && node.hasAttribute(notAttrMatch[1]));
        }
        return true;
    }

    // Parse and evaluate XPath expression
    function __xpathEvaluate(expr, contextNode) {
        expr = expr.trim();

        // Handle '.' — current node
        if (expr === '.') return [contextNode];

        // Handle '..' — parent node
        if (expr === '..') return contextNode.parentNode ? [contextNode.parentNode] : [];

        // Handle '//' at start — descendant-or-self axis from context
        if (expr.indexOf('//') === 0) {
            return __xpathEvalDescendant(expr.substring(2), contextNode);
        }

        // Handle './' or './/' — relative to context
        if (expr.indexOf('.//') === 0) {
            return __xpathEvalDescendant(expr.substring(3), contextNode);
        }
        if (expr.indexOf('./') === 0) {
            return __xpathEvalAbsolute(expr.substring(2), contextNode);
        }

        // Handle '/' at start — absolute path from document root
        if (expr.charAt(0) === '/') {
            var root = contextNode.ownerDocument || contextNode;
            return __xpathEvalAbsolute(expr.substring(1), root);
        }

        // Handle axis:: patterns
        if (expr.indexOf('child::') === 0) {
            return __xpathEvalChildren(expr.substring(7), contextNode);
        }
        if (expr.indexOf('descendant::') === 0) {
            return __xpathEvalDescendant(expr.substring(12), contextNode);
        }
        if (expr.indexOf('descendant-or-self::') === 0) {
            var nodes = __xpathEvalDescendant(expr.substring(20), contextNode);
            if (__xpathNodeMatchesStep(contextNode, expr.substring(20))) {
                nodes.unshift(contextNode);
            }
            return nodes;
        }
        if (expr.indexOf('self::') === 0) {
            var selfStep = expr.substring(6);
            return __xpathNodeMatchesStep(contextNode, selfStep) ? [contextNode] : [];
        }

        // Simple name — child elements
        return __xpathEvalChildren(expr, contextNode);
    }

    function __xpathNodeMatchesStep(node, step) {
        if (step === '*' || step === 'node()') return true;
        if (step === 'text()') return node.nodeType === 3;
        if (step === 'comment()') return node.nodeType === 8;
        if (step === 'processing-instruction()') return node.nodeType === 7;
        if (node.nodeType !== 1) return false;
        var tagName = step.replace(/\[.*/, '').trim().toLowerCase();
        return (node.localName || node.nodeName || '').toLowerCase() === tagName;
    }

    // Parse step[predicate] — returns {tag, predicates}
    function __xpathParseStep(step) {
        var tag = step;
        var predicates = [];
        var bracketStart = step.indexOf('[');
        if (bracketStart !== -1) {
            tag = step.substring(0, bracketStart).trim();
            var rest = step.substring(bracketStart);
            var depth = 0;
            var predStart = -1;
            for (var i = 0; i < rest.length; i++) {
                if (rest[i] === '[') {
                    if (depth === 0) predStart = i + 1;
                    depth++;
                } else if (rest[i] === ']') {
                    depth--;
                    if (depth === 0 && predStart !== -1) {
                        predicates.push(rest.substring(predStart, i));
                        predStart = -1;
                    }
                }
            }
        }
        return { tag: tag, predicates: predicates };
    }

    function __xpathFilterByPredicates(nodes, predicates) {
        for (var p = 0; p < predicates.length; p++) {
            var pred = predicates[p].trim();
            // Numeric positional predicate
            var posMatch = pred.match(/^\d+$/);
            if (posMatch) {
                var pos = parseInt(pred, 10);
                nodes = (pos >= 1 && pos <= nodes.length) ? [nodes[pos - 1]] : [];
                continue;
            }
            var filtered = [];
            for (var i = 0; i < nodes.length; i++) {
                if (__xpathMatchesPredicate(nodes[i], pred)) {
                    filtered.push(nodes[i]);
                }
            }
            nodes = filtered;
        }
        return nodes;
    }

    function __xpathEvalDescendant(stepExpr, contextNode) {
        // Handle multi-step: //a/b/c
        var slashPos = __xpathFindSlash(stepExpr);
        var firstStep, rest;
        if (slashPos !== -1) {
            firstStep = stepExpr.substring(0, slashPos);
            rest = stepExpr.substring(slashPos + 1);
        } else {
            firstStep = stepExpr;
            rest = null;
        }

        var parsed = __xpathParseStep(firstStep);
        var tag = parsed.tag;
        var predicates = parsed.predicates;

        var matches = [];
        if (tag === '*') {
            matches = __xpathGetAllElements(contextNode);
        } else if (tag === 'node()') {
            matches = __xpathGetAllDescendants(contextNode);
        } else if (tag === 'text()') {
            var allD = __xpathGetAllDescendants(contextNode);
            for (var i = 0; i < allD.length; i++) {
                if (allD[i].nodeType === 3) matches.push(allD[i]);
            }
        } else {
            if (contextNode.getElementsByTagName) {
                var list = contextNode.getElementsByTagName(tag);
                for (var j = 0; j < list.length; j++) matches.push(list[j]);
            }
        }

        matches = __xpathFilterByPredicates(matches, predicates);

        if (rest) {
            var result = [];
            for (var k = 0; k < matches.length; k++) {
                var sub = __xpathEvalAbsolute(rest, matches[k]);
                for (var l = 0; l < sub.length; l++) {
                    if (result.indexOf(sub[l]) === -1) result.push(sub[l]);
                }
            }
            return result;
        }
        return matches;
    }

    // Find the first '/' not inside brackets
    function __xpathFindSlash(expr) {
        var depth = 0;
        for (var i = 0; i < expr.length; i++) {
            if (expr[i] === '[') depth++;
            else if (expr[i] === ']') depth--;
            else if (expr[i] === '/' && depth === 0) {
                // Check for '//' — treat as single step boundary
                if (expr[i + 1] === '/') return -1; // can't split //
                return i;
            }
        }
        return -1;
    }

    function __xpathEvalAbsolute(pathExpr, contextNode) {
        // Split on '/' but not inside brackets, and not '//'
        var steps = [];
        var current = '';
        var depth = 0;
        for (var i = 0; i < pathExpr.length; i++) {
            if (pathExpr[i] === '[') { depth++; current += pathExpr[i]; }
            else if (pathExpr[i] === ']') { depth--; current += pathExpr[i]; }
            else if (pathExpr[i] === '/' && depth === 0) {
                if (current) steps.push(current);
                current = '';
                // Check for '//' — descendant axis
                if (pathExpr[i + 1] === '/') {
                    i++;
                    var remaining = pathExpr.substring(i + 1);
                    // All current context nodes, then descendant search
                    var contexts = steps.length > 0 ? __xpathWalkSteps(steps, [contextNode]) : [contextNode];
                    var result = [];
                    for (var c = 0; c < contexts.length; c++) {
                        var sub = __xpathEvalDescendant(remaining, contexts[c]);
                        for (var s = 0; s < sub.length; s++) {
                            if (result.indexOf(sub[s]) === -1) result.push(sub[s]);
                        }
                    }
                    return result;
                }
            } else {
                current += pathExpr[i];
            }
        }
        if (current) steps.push(current);
        if (steps.length === 0) return [contextNode];

        return __xpathWalkSteps(steps, [contextNode]);
    }

    function __xpathWalkSteps(steps, contextNodes) {
        var nodes = contextNodes;
        for (var s = 0; s < steps.length; s++) {
            var nextNodes = [];
            for (var n = 0; n < nodes.length; n++) {
                var children = __xpathEvalChildren(steps[s], nodes[n]);
                for (var c = 0; c < children.length; c++) {
                    if (nextNodes.indexOf(children[c]) === -1) nextNodes.push(children[c]);
                }
            }
            nodes = nextNodes;
        }
        return nodes;
    }

    function __xpathEvalChildren(stepExpr, contextNode) {
        var parsed = __xpathParseStep(stepExpr);
        var tag = parsed.tag;
        var predicates = parsed.predicates;

        var matches = [];
        var children = contextNode.childNodes;
        if (!children) return matches;

        if (tag === '*') {
            for (var i = 0; i < children.length; i++) {
                if (children[i].nodeType === 1) matches.push(children[i]);
            }
        } else if (tag === 'node()') {
            for (var j = 0; j < children.length; j++) {
                matches.push(children[j]);
            }
        } else if (tag === 'text()') {
            for (var k = 0; k < children.length; k++) {
                if (children[k].nodeType === 3) matches.push(children[k]);
            }
        } else if (tag === 'comment()') {
            for (var l = 0; l < children.length; l++) {
                if (children[l].nodeType === 8) matches.push(children[l]);
            }
        } else {
            var tagLower = tag.toLowerCase();
            for (var m = 0; m < children.length; m++) {
                if (children[m].nodeType === 1 &&
                    (children[m].localName || children[m].nodeName || '').toLowerCase() === tagLower) {
                    matches.push(children[m]);
                }
            }
        }

        return __xpathFilterByPredicates(matches, predicates);
    }

    // Main evaluate function — will be bound to document in global_document.rs
    globalThis.__xpathEvaluate = function(expression, contextNode, nsResolver, type, result) {
        if (arguments.length < 2) {
            throw new TypeError("Failed to execute 'evaluate' on 'Document': 2 arguments required.");
        }
        if (contextNode == null || (typeof contextNode !== 'object') || contextNode.nodeType === undefined) {
            throw new TypeError("Failed to execute 'evaluate' on 'Document': parameter 2 is not of type 'Node'.");
        }

        var requestedType = type || ANY_TYPE;

        // Resolve requested type for ANY_TYPE — default to ordered node snapshot
        if (requestedType === ANY_TYPE) {
            requestedType = ORDERED_NODE_SNAPSHOT_TYPE;
        }

        var nodes = __xpathEvaluate(expression, contextNode);

        return new XPathResult(nodes, requestedType);
    };
})();
"#;
