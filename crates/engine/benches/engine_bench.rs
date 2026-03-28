use braille_engine::Engine;
use braille_wire::SnapMode;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn spa_page() -> &'static str {
    r#"<html><body>
        <nav>
            <a href="/" id="home">Home</a>
            <a href="/products" id="products">Products</a>
            <a href="/about" id="about">About</a>
        </nav>
        <div id="app"><p>Loading...</p></div>
        <script>
        function render(page) {
            var app = document.getElementById('app');
            app.innerHTML = '';
            if (page === 'home') {
                app.innerHTML = '<h1>Welcome</h1><p>Our store has great products.</p>' +
                    '<button id="cta">Shop Now</button>';
            } else if (page === 'products') {
                var h = document.createElement('h1');
                h.textContent = 'Products';
                app.appendChild(h);
                for (var i = 0; i < 30; i++) {
                    var card = document.createElement('div');
                    card.innerHTML = '<h3>Product ' + i + '</h3><p>$' + (i * 10 + 9.99) + '</p>' +
                        '<button>Add to Cart</button>';
                    app.appendChild(card);
                }
            } else {
                app.innerHTML = '<h1>About Us</h1><p>We are a small team.</p>';
            }
        }
        render('products');
        </script>
    </body></html>"#
}

fn bench_spa(c: &mut Criterion) {
    // End-to-end: init + parse + JS exec + snapshot
    c.bench_function("spa_products_e2e", |b| {
        b.iter(|| {
            let mut engine = Engine::new();
            engine.load_html(black_box(spa_page()));
            engine.snapshot(SnapMode::Compact)
        })
    });

    // Phase: Engine::new() — QuickJS runtime init, global setup
    c.bench_function("engine_init", |b| {
        b.iter(|| {
            black_box(Engine::new());
        })
    });

    // Phase: load_html — HTML parse + JS execution + settle
    c.bench_function("load_html", |b| {
        b.iter_batched(
            Engine::new,
            |mut engine| {
                engine.load_html(black_box(spa_page()));
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Phase: snapshot on a pre-loaded page
    c.bench_function("snapshot_compact", |b| {
        let mut engine = Engine::new();
        engine.load_html(spa_page());
        b.iter(|| {
            black_box(engine.snapshot(SnapMode::Compact));
        })
    });

    // Phase: snapshot text mode
    c.bench_function("snapshot_text", |b| {
        let mut engine = Engine::new();
        engine.load_html(spa_page());
        b.iter(|| {
            black_box(engine.snapshot(SnapMode::Text));
        })
    });

    // Phase: snapshot accessibility mode (heaviest serializer)
    c.bench_function("snapshot_a11y", |b| {
        let mut engine = Engine::new();
        engine.load_html(spa_page());
        b.iter(|| {
            black_box(engine.snapshot(SnapMode::Accessibility));
        })
    });

    // Minimal page — baseline for init + parse overhead
    c.bench_function("minimal_page_e2e", |b| {
        b.iter(|| {
            let mut engine = Engine::new();
            engine.load_html(black_box("<html><body><p>Hello</p></body></html>"));
            engine.snapshot(SnapMode::Compact)
        })
    });

    // Second page load in same engine — fast mode (runtime reuse)
    c.bench_function("second_page_load", |b| {
        let mut engine = Engine::new();
        engine.load_html(spa_page());
        b.iter(|| {
            engine.load_html(black_box(spa_page()));
            engine.snapshot(SnapMode::Compact)
        })
    });

    // Second page load — clean mode (fresh runtime, no reuse)
    c.bench_function("second_page_load_clean", |b| {
        let mut engine = Engine::new();
        engine.runtime_mode = braille_engine::RuntimeMode::Clean;
        engine.load_html(spa_page());
        b.iter(|| {
            engine.load_html(black_box(spa_page()));
            engine.snapshot(SnapMode::Compact)
        })
    });

    // Isolate: rebind_for_new_page cost
    c.bench_function("rebind_for_new_page", |b| {
        use std::cell::RefCell;
        use std::rc::Rc;
        let tree = Rc::new(RefCell::new(braille_engine::dom::tree::DomTree::new()));
        let mut rt = braille_engine::js::JsRuntime::new(Rc::clone(&tree));
        b.iter(|| {
            let new_tree = Rc::new(RefCell::new(braille_engine::dom::tree::DomTree::new()));
            let new_state = Rc::new(RefCell::new(braille_engine::js::state::EngineState::new()));
            rt.rebind_for_new_page(Rc::clone(&new_tree), new_state);
        })
    });

    // Isolate: QuickJS Runtime + Context creation (no globals registration)
    c.bench_function("quickjs_runtime_context", |b| {
        b.iter(|| {
            let runtime = rquickjs::Runtime::new().unwrap();
            let _ctx = rquickjs::Context::full(&runtime).unwrap();
        })
    });

    // Isolate: Context creation on existing Runtime
    c.bench_function("quickjs_context_only", |b| {
        let runtime = rquickjs::Runtime::new().unwrap();
        b.iter(|| {
            let _ctx = rquickjs::Context::full(black_box(&runtime)).unwrap();
        })
    });

    // Isolate: JsRuntime::new (Runtime + Context + register_all globals)
    c.bench_function("jsruntime_new", |b| {
        use std::cell::RefCell;
        use std::rc::Rc;
        let tree = Rc::new(RefCell::new(braille_engine::dom::tree::DomTree::new()));
        b.iter(|| {
            let _rt = braille_engine::js::JsRuntime::new(Rc::clone(&tree));
        })
    });
}

criterion_group!(benches, bench_spa);
criterion_main!(benches);
