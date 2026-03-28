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
}

criterion_group!(benches, bench_spa);
criterion_main!(benches);
