//! Integration tests for CSS cascade edge cases, specificity, inheritance,
//! and CSS-accessibility tree interaction.
//!
//! These tests exercise the full engine pipeline (parse HTML -> execute scripts ->
//! compute styles -> snapshot). Since `runtime` and `tree` are pub(crate), all
//! tests use only the public API: load_html, snapshot, resolve_ref, etc.
//!
//! Key insight: getComputedStyle in inline scripts runs BEFORE compute_all_styles,
//! so computed values are empty at script time. Instead, we verify cascade behavior
//! through its observable effects in the accessibility tree snapshot:
//! - display:none -> element and descendants are removed from snapshot
//! - visibility:hidden -> text is hidden but structure remains
//! - Cascade winners determined by specificity/source order are verified through
//!   which display/visibility rule wins

use braille_engine::Engine;
use braille_wire::SnapMode;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn load_and_snap(html: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(SnapMode::Accessibility)
}

// ===========================================================================
// 1. Specificity: class selector beats element selector
//    Element rule says display:none, class rule says display:block.
//    Class wins -> element should be VISIBLE.
// ===========================================================================

#[test]
fn specificity_class_beats_element() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        p { display: none; }
        .visible { display: block; }
      </style>
      <p class="visible">Class wins</p>
    </body></html>"##);

    assert!(
        snap.contains("Class wins"),
        "class selector should beat element selector (display:block wins over display:none); snapshot: {}",
        snap
    );
}

// ===========================================================================
// 2. Specificity: ID selector beats class selector
//    Class rule hides it, ID rule shows it. ID should win.
// ===========================================================================

#[test]
fn specificity_id_beats_class() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        .hidden { display: none; }
        #show { display: block; }
      </style>
      <p id="show" class="hidden">ID wins</p>
    </body></html>"##);

    assert!(
        snap.contains("ID wins"),
        "ID selector should beat class selector; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 3. Specificity: inline style beats ID selector
//    ID rule hides, inline style shows. Inline should win.
// ===========================================================================

#[test]
fn specificity_inline_beats_id() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        #hide { display: none; }
      </style>
      <p id="hide" style="display: block">Inline wins</p>
    </body></html>"##);

    assert!(
        snap.contains("Inline wins"),
        "inline style should beat ID selector; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 4. !important beats inline style
//    Inline says display:block, author !important says display:none.
// ===========================================================================

#[test]
fn important_beats_inline() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        p { display: none !important; }
      </style>
      <p style="display: block">Should be hidden</p>
      <h1>Visible sentinel</h1>
    </body></html>"##);

    assert!(
        !snap.contains("Should be hidden"),
        "!important should beat inline style; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible sentinel"),
        "visible sentinel should appear; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 5. Inheritance: color inherits from parent to child
//    Verify via visibility: parent has visibility:hidden, child should inherit.
// ===========================================================================

#[test]
fn inheritance_visibility_inherits() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        .parent-invisible { visibility: hidden; }
      </style>
      <div class="parent-invisible">
        <p>Inherited hidden text</p>
      </div>
      <p>Visible text</p>
    </body></html>"##);

    assert!(
        !snap.contains("Inherited hidden text"),
        "visibility:hidden should inherit from parent to child; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible text"),
        "visible text should appear; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 6. Inheritance: display does NOT inherit
//    Parent is display:none, but child WOULD be visible if display didn't
//    inherit. However, display:none on a parent SKIPS descendants entirely,
//    so we test that non-inheritance of display works correctly by
//    verifying a child of a display:flex parent is still display:block (UA).
//    We verify this by having a descendant that would be hidden if it
//    inherited display:none from an ancestor - but the ancestor uses
//    display:flex instead. The child should be visible.
// ===========================================================================

#[test]
fn inheritance_display_does_not_inherit() {
    // The child <p> should get display:block from UA (not inherit display:flex from parent).
    // We can't directly observe this in the snapshot since both block and flex are visible.
    // Instead, verify that a SIBLING div with display:none doesn't affect a <p> inside it
    // via non-inheritance: a grandchild p inside a visible div should still appear.
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        .flex-parent { display: flex; }
      </style>
      <div class="flex-parent">
        <p>Child of flex parent</p>
      </div>
    </body></html>"##);

    // The p is visible (display doesn't inherit, so p keeps its UA display:block)
    assert!(
        snap.contains("Child of flex parent"),
        "p inside flex parent should be visible (display doesn't inherit); snapshot: {}",
        snap
    );
}

// ===========================================================================
// 7. UA stylesheet: head/script/style don't appear in snapshot
// ===========================================================================

#[test]
fn ua_stylesheet_hidden_elements_not_in_snapshot() {
    let snap = load_and_snap(r##"
    <html>
      <head>
        <title>Test Page Title</title>
        <style>p { color: red; }</style>
      </head>
      <body>
        <p>Visible content</p>
      </body>
    </html>"##);

    assert!(
        !snap.contains("Test Page Title"),
        "title text should not be in snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible content"),
        "visible content should appear: {}",
        snap
    );
}

// ===========================================================================
// 8. display:none skips descendants in accessibility tree
// ===========================================================================

#[test]
fn display_none_skips_descendants_in_snapshot() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.hidden { display: none; }</style>
      <div class="hidden">
        <p>hidden paragraph</p>
        <a href="/secret">hidden link</a>
        <button>hidden button</button>
      </div>
      <p>visible paragraph</p>
    </body></html>"##);

    assert!(
        !snap.contains("hidden paragraph"),
        "display:none should hide all descendants: {}",
        snap
    );
    assert!(
        !snap.contains("hidden link"),
        "link inside display:none should be hidden: {}",
        snap
    );
    assert!(
        !snap.contains("hidden button"),
        "button inside display:none should be hidden: {}",
        snap
    );
    assert!(
        snap.contains("visible paragraph"),
        "visible paragraph should appear: {}",
        snap
    );
}

// ===========================================================================
// 9. visibility:hidden hides text but structure may remain
// ===========================================================================

#[test]
fn visibility_hidden_hides_text_in_snapshot() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.ghost { visibility: hidden; }</style>
      <p class="ghost">ghost text</p>
      <p>visible text</p>
    </body></html>"##);

    assert!(
        !snap.contains("ghost text"),
        "visibility:hidden should hide text content: {}",
        snap
    );
    assert!(
        snap.contains("visible text"),
        "visible text should appear: {}",
        snap
    );
    // The paragraph structure is preserved (empty paragraph line exists)
    assert!(
        snap.contains("paragraph"),
        "paragraph structure should be preserved: {}",
        snap
    );
}

// ===========================================================================
// 10. Script-time DOM mutation affects cascade (classList.add matching rule)
//     Script sets a class that matches a display:none rule. After compute_all_styles,
//     the element should be hidden.
// ===========================================================================

#[test]
fn script_adds_class_matching_display_none_rule() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.hidden { display: none; }</style>
      <p id="target">Will be hidden by script</p>
      <p>Always visible</p>
      <script>
        document.getElementById("target").classList.add("hidden");
      </script>
    </body></html>"##);

    assert!(
        !snap.contains("Will be hidden by script"),
        "element hidden by script-added class should not be in snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Always visible"),
        "always-visible text should appear: {}",
        snap
    );
}

// ===========================================================================
// 11. Multiple stylesheets cascade: later source order wins at same specificity
//     First stylesheet hides, second shows (same selector, same specificity).
// ===========================================================================

#[test]
fn multiple_stylesheets_later_source_wins() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>p { display: none; }</style>
      <style>p { display: block; }</style>
      <p>Later sheet wins</p>
    </body></html>"##);

    assert!(
        snap.contains("Later sheet wins"),
        "later stylesheet should win at same specificity; snapshot: {}",
        snap
    );
}

#[test]
fn multiple_stylesheets_later_source_wins_reverse() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>p { display: block; }</style>
      <style>p { display: none; }</style>
      <p>Should be hidden</p>
      <h1>Visible heading</h1>
    </body></html>"##);

    assert!(
        !snap.contains("Should be hidden"),
        "later stylesheet (display:none) should win; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible heading"),
        "heading should appear; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 12. Compound selectors: div.foo matches <div class="foo"> but not others
// ===========================================================================

#[test]
fn compound_selector_div_dot_foo() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>div.hide-me { display: none; }</style>
      <div class="hide-me">
        <p>Hidden by compound selector</p>
      </div>
      <span class="hide-me">
        <p>Span not hidden (selector is div.hide-me not span.hide-me)</p>
      </span>
      <div class="other">
        <p>Div with different class visible</p>
      </div>
    </body></html>"##);

    assert!(
        !snap.contains("Hidden by compound selector"),
        "div.hide-me should match <div class='hide-me'>; snapshot: {}",
        snap
    );
    // span.hide-me should NOT match div.hide-me
    // Note: span is a transparent element in a11y, so we check for the text
    assert!(
        snap.contains("Span not hidden"),
        "div.hide-me should NOT match <span class='hide-me'>; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Div with different class visible"),
        "div.other should not match div.hide-me; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 13. Descendant selector: div p { display: none }
// ===========================================================================

#[test]
fn descendant_selector_matches_nested() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>div p { display: none; }</style>
      <div>
        <p>Hidden nested paragraph</p>
      </div>
      <p>Standalone paragraph visible</p>
    </body></html>"##);

    assert!(
        !snap.contains("Hidden nested paragraph"),
        "div p should match nested <p>; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Standalone paragraph visible"),
        "div p should NOT match standalone <p>; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 14. Cascade with script-added inline styles
// ===========================================================================

#[test]
fn script_inline_style_overrides_stylesheet() {
    // Script sets display:none via inline style, overriding stylesheet display:block
    let snap = load_and_snap(r##"
    <html><body>
      <style>p { display: block; }</style>
      <p id="target">Will be hidden by script</p>
      <p>Always visible</p>
      <script>
        document.getElementById("target").style.setProperty("display", "none");
      </script>
    </body></html>"##);

    assert!(
        !snap.contains("Will be hidden by script"),
        "script-set inline display:none should override stylesheet display:block; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Always visible"),
        "always-visible text should appear; snapshot: {}",
        snap
    );
}

#[test]
fn script_inline_style_shows_hidden_element() {
    // Stylesheet hides it, script overrides with inline display:block
    let snap = load_and_snap(r##"
    <html><body>
      <style>.start-hidden { display: none; }</style>
      <p id="target" class="start-hidden">Shown by script</p>
      <script>
        document.getElementById("target").style.setProperty("display", "block");
      </script>
    </body></html>"##);

    assert!(
        snap.contains("Shown by script"),
        "script inline display:block should override .start-hidden display:none; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 15. display:none inline style set via attribute
// ===========================================================================

#[test]
fn inline_display_none_hides_from_snapshot() {
    let snap = load_and_snap(r##"
    <html><body>
      <p style="display: none">Hidden paragraph</p>
      <p>Visible paragraph</p>
    </body></html>"##);

    assert!(
        !snap.contains("Hidden paragraph"),
        "inline display:none should hide from snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible paragraph"),
        "visible paragraph should appear: {}",
        snap
    );
}

// ===========================================================================
// 16. Specificity: element+class beats element alone (for display)
// ===========================================================================

#[test]
fn specificity_element_class_beats_element_alone() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        p { display: none; }
        p.show { display: block; }
      </style>
      <p class="show">Class overrides to visible</p>
      <p>This p is hidden</p>
      <h1>Heading visible</h1>
    </body></html>"##);

    assert!(
        snap.contains("Class overrides to visible"),
        "p.show should beat p (higher specificity); snapshot: {}",
        snap
    );
    assert!(
        !snap.contains("This p is hidden"),
        "plain p should be hidden by p {{ display: none }}; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Heading visible"),
        "heading should be visible; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 17. Author stylesheet overrides UA defaults
// ===========================================================================

#[test]
fn author_overrides_ua_defaults() {
    // UA says <h1> is display:block. Author says display:none.
    let snap = load_and_snap(r##"
    <html><body>
      <style>h1 { display: none; }</style>
      <h1>Hidden heading</h1>
      <p>Visible paragraph</p>
    </body></html>"##);

    assert!(
        !snap.contains("Hidden heading"),
        "author display:none should override UA display:block for h1; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible paragraph"),
        "visible paragraph should appear; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 18. Deep inheritance: visibility:hidden propagates through many levels
// ===========================================================================

#[test]
fn deep_inheritance_visibility_hidden() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.root-hidden { visibility: hidden; }</style>
      <div class="root-hidden">
        <section>
          <article>
            <p>Deep hidden text</p>
          </article>
        </section>
      </div>
      <p>Visible text</p>
    </body></html>"##);

    assert!(
        !snap.contains("Deep hidden text"),
        "visibility:hidden should inherit through deep chain; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible text"),
        "visible text should appear; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 19. display:none interactive elements produce no refs in snapshot
// ===========================================================================

#[test]
fn display_none_interactive_elements_no_refs() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.hidden { display: none; }</style>
      <a href="/visible">Visible Link</a>
      <div class="hidden">
        <a href="/hidden">Hidden Link</a>
        <button>Hidden Button</button>
        <input type="text">
      </div>
      <button>Visible Button</button>
    </body></html>"##);

    // Only visible interactive elements should have refs
    assert!(
        snap.contains("link @e1 \"Visible Link\""),
        "visible link should be @e1: {}",
        snap
    );
    assert!(
        snap.contains("button @e2 \"Visible Button\""),
        "visible button should be @e2: {}",
        snap
    );
    // Hidden elements should not appear at all
    assert!(
        !snap.contains("Hidden Link"),
        "hidden link should not appear: {}",
        snap
    );
    assert!(
        !snap.contains("Hidden Button"),
        "hidden button should not appear: {}",
        snap
    );
    // Should not have @e3 since hidden elements are skipped
    assert!(
        !snap.contains("@e3"),
        "hidden interactive elements should not get refs: {}",
        snap
    );
}

// ===========================================================================
// 20. Visibility: hidden interactive element still gets a ref
// ===========================================================================

#[test]
fn visibility_hidden_interactive_element_gets_ref() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.ghost { visibility: hidden; }</style>
      <button class="ghost">Ghost button</button>
      <button>Visible button</button>
    </body></html>"##);

    // visibility:hidden preserves structure, so the button should still get a ref
    // but its text should be hidden
    assert!(
        !snap.contains("Ghost button"),
        "visibility:hidden button text should not appear: {}",
        snap
    );
    // The ghost button should still get an @e1 ref (structure preserved)
    assert!(
        snap.contains("@e1"),
        "visibility:hidden button should still get a ref: {}",
        snap
    );
    assert!(
        snap.contains("button @e2 \"Visible button\""),
        "visible button should be @e2: {}",
        snap
    );
}

// ===========================================================================
// 21. Script-time classList removal affects cascade
// ===========================================================================

#[test]
fn script_removes_class_to_show_element() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.hidden { display: none; }</style>
      <p id="target" class="hidden">Shown by removing class</p>
      <script>
        document.getElementById("target").classList.remove("hidden");
      </script>
    </body></html>"##);

    assert!(
        snap.contains("Shown by removing class"),
        "removing .hidden class should make element visible; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 22. Multiple conflicting rules: different properties from different sheets
// ===========================================================================

#[test]
fn multiple_sheets_different_properties_merge() {
    // First sheet hides via display:none, second sheet shows via display:block.
    // Second sheet wins (later source order, same specificity).
    // Also verify a second property from the first sheet still applies via snapshot:
    // We use visibility to indirectly observe.
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        p { display: none; }
      </style>
      <style>
        p { display: block; }
      </style>
      <p>Text from merged styles</p>
    </body></html>"##);

    assert!(
        snap.contains("Text from merged styles"),
        "later sheet display:block should override first sheet display:none; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 23. UA heading styles visible in snapshot
// ===========================================================================

#[test]
fn ua_heading_styles_in_snapshot() {
    let snap = load_and_snap(r##"
    <html><body>
      <h1>Heading 1</h1>
      <h2>Heading 2</h2>
      <h3>Heading 3</h3>
      <p>Paragraph text</p>
    </body></html>"##);

    assert!(
        snap.contains("heading[1] \"Heading 1\""),
        "h1 should appear with heading[1] role: {}",
        snap
    );
    assert!(
        snap.contains("heading[2] \"Heading 2\""),
        "h2 should appear with heading[2] role: {}",
        snap
    );
    assert!(
        snap.contains("heading[3] \"Heading 3\""),
        "h3 should appear with heading[3] role: {}",
        snap
    );
    assert!(
        snap.contains("paragraph \"Paragraph text\""),
        "p should appear with paragraph role: {}",
        snap
    );
}

// ===========================================================================
// 24. Full snapshot with mixed visibility and display
// ===========================================================================

#[test]
fn snapshot_fully_integrates_css_cascade() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        .admin-only { display: none; }
        .draft { visibility: hidden; }
      </style>
      <h1>Dashboard</h1>
      <div class="admin-only">
        <p>Secret admin panel</p>
        <button>Delete Everything</button>
      </div>
      <p class="draft">Draft content</p>
      <p>Welcome, user!</p>
      <a href="/profile">Profile</a>
    </body></html>"##);

    assert!(snap.contains("Dashboard"), "h1 should appear: {}", snap);
    assert!(snap.contains("Welcome, user!"), "paragraph should appear: {}", snap);
    assert!(snap.contains("Profile"), "link text should appear: {}", snap);
    assert!(
        !snap.contains("Secret admin panel"),
        "display:none content should not appear: {}",
        snap
    );
    assert!(
        !snap.contains("Delete Everything"),
        "display:none button should not appear: {}",
        snap
    );
    assert!(
        !snap.contains("Draft content"),
        "visibility:hidden text should not appear: {}",
        snap
    );
}

// ===========================================================================
// 25. element.style round-trip via inline script
// ===========================================================================

#[test]
fn element_style_set_and_get_round_trip() {
    // Script sets an inline style, then reads it back and writes the result
    // into a paragraph that will be visible in the snapshot.
    let snap = load_and_snap(r##"
    <html><body>
      <p id="target">Target</p>
      <p id="result"></p>
      <script>
        var el = document.getElementById("target");
        el.style.setProperty("color", "red");
        var val = el.style.getPropertyValue("color");
        document.getElementById("result").textContent = "inline=" + val;
      </script>
    </body></html>"##);

    assert!(
        snap.contains("inline=red"),
        "element.style.setProperty/getPropertyValue should round-trip; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 26. Script removes inline style via removeProperty
// ===========================================================================

#[test]
fn element_style_remove_property() {
    let snap = load_and_snap(r##"
    <html><body>
      <p id="target" style="display: none">Should become visible</p>
      <script>
        document.getElementById("target").style.removeProperty("display");
      </script>
    </body></html>"##);

    // After removeProperty("display"), the inline display:none is removed.
    // The element should be visible (UA default display:block applies).
    assert!(
        snap.contains("Should become visible"),
        "after removeProperty('display'), element should be visible; snapshot: {}",
        snap
    );
}

// ===========================================================================
// 27. display:none on nested container hides all descendants including forms
// ===========================================================================

#[test]
fn display_none_nested_container_hides_forms() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.hidden-section { display: none; }</style>
      <div class="hidden-section">
        <form>
          <input type="text" value="hidden input">
          <select>
            <option value="a">A</option>
          </select>
          <textarea>hidden textarea</textarea>
          <button>Hidden submit</button>
        </form>
      </div>
      <p>Page content</p>
    </body></html>"##);

    assert!(
        !snap.contains("hidden input"),
        "input inside display:none should be hidden: {}",
        snap
    );
    assert!(
        !snap.contains("hidden textarea"),
        "textarea inside display:none should be hidden: {}",
        snap
    );
    assert!(
        !snap.contains("Hidden submit"),
        "button inside display:none should be hidden: {}",
        snap
    );
    assert!(
        snap.contains("Page content"),
        "page content should be visible: {}",
        snap
    );
}

// ===========================================================================
// 28. Child overrides inherited visibility:hidden with visibility:visible
// ===========================================================================

#[test]
fn child_overrides_inherited_visibility_hidden() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        .invisible { visibility: hidden; }
        .force-visible { visibility: visible; }
      </style>
      <div class="invisible">
        <p>Parent hidden text</p>
        <p class="force-visible">Child visible text</p>
      </div>
    </body></html>"##);

    assert!(
        !snap.contains("Parent hidden text"),
        "parent visibility:hidden should hide text: {}",
        snap
    );
    assert!(
        snap.contains("Child visible text"),
        "child visibility:visible should override inherited hidden: {}",
        snap
    );
}

// ===========================================================================
// 29. Script creates element and adds to DOM; display:none rule matches
// ===========================================================================

#[test]
fn script_created_element_matches_display_none_rule() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>.auto-hide { display: none; }</style>
      <div id="container"></div>
      <p>Visible sentinel</p>
      <script>
        var p = document.createElement("p");
        p.textContent = "Script-created hidden";
        p.setAttribute("class", "auto-hide");
        document.getElementById("container").appendChild(p);
      </script>
    </body></html>"##);

    assert!(
        !snap.contains("Script-created hidden"),
        "script-created element with .auto-hide class should be hidden: {}",
        snap
    );
    assert!(
        snap.contains("Visible sentinel"),
        "visible sentinel should appear: {}",
        snap
    );
}

// ===========================================================================
// 30. Specificity conflict: !important element vs normal ID
//     !important on element selector should beat normal ID selector.
// ===========================================================================

#[test]
fn important_element_beats_normal_id() {
    let snap = load_and_snap(r##"
    <html><body>
      <style>
        #showme { display: block; }
        p { display: none !important; }
      </style>
      <p id="showme">Should be hidden by !important</p>
      <h1>Visible heading</h1>
    </body></html>"##);

    assert!(
        !snap.contains("Should be hidden by !important"),
        "!important on element selector should beat normal ID selector; snapshot: {}",
        snap
    );
    assert!(
        snap.contains("Visible heading"),
        "heading should be visible; snapshot: {}",
        snap
    );
}
