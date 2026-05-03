use braille_engine::Engine;

// ---------------------------------------------------------------------------
// 1. transitionend fires after class change on element with transition
// ---------------------------------------------------------------------------

#[test]
fn transitionend_fires_after_class_change() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><head><style>
    .box { opacity: 1; transition: opacity 0.3s; }
    .box.fade { opacity: 0; }
</style></head>
<body>
    <div id="box" class="box"></div>
    <script>
        var transitionFired = false;
        document.getElementById('box').addEventListener('transitionend', function(e) {
            transitionFired = true;
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    // Trigger the transition AFTER initial styles are computed (like a click handler would)
    engine.eval_js("document.getElementById('box').classList.add('fade')").unwrap();
    engine.settle();

    let result = engine.eval_js("String(transitionFired)").unwrap();
    eprintln!("transitionFired: {}", result);
    assert_eq!(result, "true", "transitionend should fire after class change triggers opacity transition");
}

// ---------------------------------------------------------------------------
// 2. animationend fires after animation completes
// ---------------------------------------------------------------------------

#[test]
fn animationend_fires_after_animation_completes() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><head><style>
    @keyframes fadeout {
        from { opacity: 1; }
        to { opacity: 0; }
    }
    .animated { animation: fadeout 0.5s forwards; }
</style></head>
<body>
    <div id="box"></div>
    <script>
        var animationFired = false;
        document.getElementById('box').addEventListener('animationend', function(e) {
            animationFired = true;
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    // Add animation class after initial load (like a click handler would)
    engine.eval_js("document.getElementById('box').classList.add('animated')").unwrap();
    engine.settle();

    let result = engine.eval_js("String(animationFired)").unwrap();
    eprintln!("animationFired: {}", result);
    assert_eq!(result, "true", "animationend should fire after animation completes");
}

// ---------------------------------------------------------------------------
// 3. transitionend has correct properties
// ---------------------------------------------------------------------------

#[test]
fn transitionend_has_correct_properties() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><head><style>
    .box { opacity: 1; transition: opacity 0.3s; }
    .box.fade { opacity: 0; }
</style></head>
<body>
    <div id="box" class="box"></div>
    <script>
        var eventProps = {};
        document.getElementById('box').addEventListener('transitionend', function(e) {
            eventProps.propertyName = e.propertyName;
            eventProps.elapsedTime = e.elapsedTime;
            eventProps.pseudoElement = e.pseudoElement;
            eventProps.type = e.type;
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    // Trigger transition after initial styles are computed
    engine.eval_js("document.getElementById('box').classList.add('fade')").unwrap();
    engine.settle();

    let prop_name = engine.eval_js("eventProps.propertyName").unwrap();
    eprintln!("propertyName: {}", prop_name);
    assert_eq!(prop_name, "opacity", "propertyName should be 'opacity'");

    let elapsed = engine.eval_js("eventProps.elapsedTime").unwrap();
    eprintln!("elapsedTime: {}", elapsed);
    assert_eq!(elapsed, "0.3", "elapsedTime should match transition-duration");

    let pseudo = engine.eval_js("eventProps.pseudoElement").unwrap();
    eprintln!("pseudoElement: {}", pseudo);
    assert_eq!(pseudo, "", "pseudoElement should be empty string");

    let event_type = engine.eval_js("eventProps.type").unwrap();
    assert_eq!(event_type, "transitionend", "type should be 'transitionend'");
}

// ---------------------------------------------------------------------------
// 4. Modal removal pattern (exact Proton pattern)
// ---------------------------------------------------------------------------

#[test]
fn modal_removal_pattern() {
    let mut engine = Engine::new();
    engine.load_html(
        r#"<!DOCTYPE html>
<html><head><style>
    .modal { opacity: 1; transform: scale(1); transition: opacity 0.25s, transform 0.25s; }
    .modal.modal--out { opacity: 0; transform: scale(0.9); }
</style></head>
<body>
    <div id="container">
        <div id="modal" class="modal">Modal Content</div>
    </div>
    <script>
        var modal = document.getElementById('modal');
        modal.addEventListener('transitionend', function(e) {
            // Remove modal from DOM when transition completes (Proton pattern)
            if (e.target === modal) {
                modal.remove();
            }
        });
    </script>
</body></html>"#,
    );
    engine.settle();

    // Trigger the "out" transition (like clicking a dismiss button)
    engine.eval_js("document.getElementById('modal').classList.add('modal--out')").unwrap();
    engine.settle();

    // Modal should be removed from the DOM after transitionend fires
    let modal_exists = engine
        .eval_js("document.getElementById('modal') !== null")
        .unwrap();
    eprintln!("modal still in DOM: {}", modal_exists);
    assert_eq!(
        modal_exists, "false",
        "Modal should be removed from DOM after transitionend handler runs"
    );

    // Container should be empty
    let container_children = engine
        .eval_js("document.getElementById('container').children.length")
        .unwrap();
    assert_eq!(container_children, "0", "Container should have no children after modal removal");
}
