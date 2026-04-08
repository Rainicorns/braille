//! DOM tree structural invariant tests.
//!
//! Verifies that the DOM tree maintains consistency after complex mutation
//! sequences: parent/child relationships, sibling chains, node ordering,
//! and tree integrity after appendChild, removeChild, insertBefore, replaceChild.
//! All tests exercise the public API through JS — real web patterns.

use braille_engine::Engine;
use braille_wire::SnapMode;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

// ===========================================================================
// Parent/Child Consistency
// ===========================================================================

#[test]
fn parent_child_consistency_after_append() {
    let mut e = engine_with_html("<html><body><div id='parent'></div></body></html>");

    e.eval_js(r#"
        var parent = document.getElementById('parent');
        var child = document.createElement('span');
        child.id = 'child';
        parent.appendChild(child);
    "#).unwrap();

    // Child's parentNode should be parent
    let parent_check = e.eval_js(
        "document.getElementById('child').parentNode === document.getElementById('parent')"
    ).unwrap();
    assert_eq!(parent_check, "true", "child.parentNode should be parent");

    // Parent's children should include child
    let child_check = e.eval_js(
        "document.getElementById('parent').children[0] === document.getElementById('child')"
    ).unwrap();
    assert_eq!(child_check, "true", "parent.children[0] should be child");
}

#[test]
fn parent_child_consistency_after_remove() {
    let mut e = engine_with_html(
        "<html><body><div id='parent'><span id='child'>text</span></div></body></html>"
    );

    e.eval_js(r#"
        var parent = document.getElementById('parent');
        var child = document.getElementById('child');
        parent.removeChild(child);
    "#).unwrap();

    // After removal, getElementById may or may not find detached nodes.
    // The key invariant: parent has no children.
    let child_count = e.eval_js(
        "document.getElementById('parent').children.length"
    ).unwrap();
    assert_eq!(child_count, "0", "parent should have no children after removeChild");
}

#[test]
fn parent_child_consistency_after_move() {
    // Moving a node from one parent to another should update both parents
    let mut e = engine_with_html(r#"<html><body>
        <div id='old'><span id='moveme'>content</span></div>
        <div id='new'></div>
    </body></html>"#);

    e.eval_js(r#"
        var newParent = document.getElementById('new');
        var child = document.getElementById('moveme');
        newParent.appendChild(child);
    "#).unwrap();

    let old_count = e.eval_js("document.getElementById('old').children.length").unwrap();
    assert_eq!(old_count, "0", "old parent should lose child after move");

    let new_count = e.eval_js("document.getElementById('new').children.length").unwrap();
    assert_eq!(new_count, "1", "new parent should gain child after move");

    let new_parent = e.eval_js(
        "document.getElementById('moveme').parentNode.id"
    ).unwrap();
    assert_eq!(new_parent, "new", "moved child's parentNode should be new parent");
}

// ===========================================================================
// Sibling Chain Integrity
// ===========================================================================

#[test]
fn sibling_chain_after_multiple_appends() {
    let mut e = engine_with_html("<html><body><div id='parent'></div></body></html>");

    e.eval_js(r#"
        var p = document.getElementById('parent');
        for (var i = 0; i < 5; i++) {
            var el = document.createElement('span');
            el.id = 's' + i;
            p.appendChild(el);
        }
    "#).unwrap();

    // Verify forward chain: firstChild -> nextSibling -> ... -> lastChild
    let forward = e.eval_js(r#"
        var ids = [];
        var node = document.getElementById('parent').firstChild;
        while (node) {
            ids.push(node.id);
            node = node.nextSibling;
        }
        ids.join(',')
    "#).unwrap();
    assert_eq!(forward, "s0,s1,s2,s3,s4", "forward sibling chain: {}", forward);

    // Verify backward chain: lastChild -> previousSibling -> ... -> firstChild
    let backward = e.eval_js(r#"
        var ids = [];
        var node = document.getElementById('parent').lastChild;
        while (node) {
            ids.push(node.id);
            node = node.previousSibling;
        }
        ids.join(',')
    "#).unwrap();
    assert_eq!(backward, "s4,s3,s2,s1,s0", "backward sibling chain: {}", backward);
}

#[test]
fn sibling_chain_after_insert_before() {
    let mut e = engine_with_html(r#"<html><body>
        <div id='parent'><span id='a'>A</span><span id='c'>C</span></div>
    </body></html>"#);

    // Insert B between A and C
    e.eval_js(r#"
        var parent = document.getElementById('parent');
        var b = document.createElement('span');
        b.id = 'b';
        b.textContent = 'B';
        parent.insertBefore(b, document.getElementById('c'));
    "#).unwrap();

    let order = e.eval_js(r#"
        var ids = [];
        var node = document.getElementById('parent').firstChild;
        while (node) {
            if (node.id) ids.push(node.id);
            node = node.nextSibling;
        }
        ids.join(',')
    "#).unwrap();
    assert_eq!(order, "a,b,c", "insertBefore should maintain correct sibling order: {}", order);

    // Verify previousSibling links
    let b_prev = e.eval_js("document.getElementById('b').previousSibling.id").unwrap();
    assert_eq!(b_prev, "a", "b.previousSibling should be a");

    let b_next = e.eval_js("document.getElementById('b').nextSibling.id").unwrap();
    assert_eq!(b_next, "c", "b.nextSibling should be c");
}

#[test]
fn sibling_chain_after_remove_middle() {
    // No whitespace between elements — whitespace creates text nodes that
    // make nextSibling/previousSibling return text nodes instead of elements.
    let mut e = engine_with_html(
        r#"<html><body><div id='parent'><span id='a'>A</span><span id='b'>B</span><span id='c'>C</span></div></body></html>"#
    );

    // Remove B
    e.eval_js(r#"
        var b = document.getElementById('b');
        b.parentNode.removeChild(b);
    "#).unwrap();

    // A.nextElementSibling should now be C
    let a_next = e.eval_js("document.getElementById('a').nextElementSibling.id").unwrap();
    assert_eq!(a_next, "c", "after removing B, A.nextElementSibling should be C: {}", a_next);

    // C.previousElementSibling should now be A
    let c_prev = e.eval_js("document.getElementById('c').previousElementSibling.id").unwrap();
    assert_eq!(c_prev, "a", "after removing B, C.previousElementSibling should be A: {}", c_prev);
}

// ===========================================================================
// replaceChild
// ===========================================================================

#[test]
fn replace_child_maintains_tree_integrity() {
    let mut e = engine_with_html(
        r#"<html><body><div id='parent'><span id='old'>Old</span></div></body></html>"#
    );

    e.eval_js(r#"
        var parent = document.getElementById('parent');
        var newNode = document.createElement('div');
        newNode.id = 'replacement';
        newNode.textContent = 'New';
        parent.replaceChild(newNode, document.getElementById('old'));
    "#).unwrap();

    let child_id = e.eval_js("document.getElementById('parent').firstElementChild.id").unwrap();
    assert_eq!(child_id, "replacement", "replaced child should be in place: {}", child_id);

    let count = e.eval_js("document.getElementById('parent').children.length").unwrap();
    assert_eq!(count, "1", "should still have exactly one child after replace");
}

// ===========================================================================
// Complex Mutation Sequences
// ===========================================================================

#[test]
fn rapid_mutation_sequence_maintains_consistency() {
    let mut e = engine_with_html("<html><body><div id='root'></div></body></html>");

    // Simulate a framework reconciler: create, append, move, remove in rapid succession
    e.eval_js(r#"
        var root = document.getElementById('root');

        // Phase 1: build initial structure
        for (var i = 0; i < 10; i++) {
            var el = document.createElement('div');
            el.id = 'n' + i;
            root.appendChild(el);
        }

        // Phase 2: reverse the order by moving each to the beginning
        for (var i = 9; i >= 0; i--) {
            root.insertBefore(document.getElementById('n' + i), root.firstChild);
        }

        // Phase 3: remove even-indexed nodes
        for (var i = 0; i < 10; i += 2) {
            var el = document.getElementById('n' + i);
            if (el && el.parentNode) el.parentNode.removeChild(el);
        }
    "#).unwrap();

    // Verify remaining nodes and their order
    let remaining = e.eval_js(r#"
        var ids = [];
        var node = document.getElementById('root').firstChild;
        while (node) {
            ids.push(node.id);
            node = node.nextSibling;
        }
        ids.join(',')
    "#).unwrap();

    // The reversal loop (i=9 down to 0, insertBefore(n_i, firstChild)) undoes itself:
    // each iteration prepends, and the full sweep restores [n0,n1,...,n9].
    // After removing even-numbered nodes (n0,n2,n4,n6,n8): [n1,n3,n5,n7,n9]
    assert_eq!(remaining, "n1,n3,n5,n7,n9",
        "complex mutation sequence should produce correct result: {}", remaining);

    // Verify children count matches
    let count = e.eval_js("document.getElementById('root').children.length").unwrap();
    assert_eq!(count, "5", "should have 5 remaining children");
}

// ===========================================================================
// Document Fragment
// ===========================================================================

#[test]
fn document_fragment_batch_append() {
    let mut e = engine_with_html("<html><body><div id='target'></div></body></html>");

    e.eval_js(r#"
        var frag = document.createDocumentFragment();
        for (var i = 0; i < 3; i++) {
            var el = document.createElement('p');
            el.id = 'p' + i;
            el.textContent = 'Paragraph ' + i;
            frag.appendChild(el);
        }
        document.getElementById('target').appendChild(frag);
    "#).unwrap();

    let count = e.eval_js("document.getElementById('target').children.length").unwrap();
    assert_eq!(count, "3", "fragment children should be moved to target");

    let first = e.eval_js("document.getElementById('target').firstChild.id").unwrap();
    assert_eq!(first, "p0", "first child should be p0");
}

// ===========================================================================
// Tree Structure in Snapshot
// ===========================================================================

#[test]
fn dom_mutations_reflected_in_snapshot() {
    let mut e = engine_with_html("<html><body><div id='container'></div></body></html>");

    e.eval_js(r#"
        var c = document.getElementById('container');
        var h = document.createElement('h2');
        h.textContent = 'Dynamic Heading';
        c.appendChild(h);
        var p = document.createElement('p');
        p.textContent = 'Dynamic paragraph content';
        c.appendChild(p);
    "#).unwrap();

    e.settle();
    let snap = e.snapshot(SnapMode::Text);
    assert!(snap.contains("Dynamic Heading"), "dynamically added heading should appear in snapshot: {}", snap);
    assert!(snap.contains("Dynamic paragraph content"), "dynamically added paragraph should appear in snapshot: {}", snap);
}
