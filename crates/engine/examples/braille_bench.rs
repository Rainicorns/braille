//! Standalone Braille benchmark — load HTML file, settle, extract text.
//! Build: cargo rustc --release -p braille-engine --example braille_bench -- (or just compile this)
//! Usage: braille-bench <html-file>

use std::env;
use std::time::Instant;

use braille_engine::Engine;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: braille-bench <html-file>");
        std::process::exit(1);
    }
    let html_path = &args[1];
    let html = std::fs::read_to_string(html_path).expect("Failed to read HTML file");

    let t_start = Instant::now();

    let mut engine = Engine::new();

    let t_engine_ready = Instant::now();
    eprintln!(
        "braille startup: {:.1}ms",
        t_engine_ready.duration_since(t_start).as_secs_f64() * 1000.0
    );

    engine.load_html(&html);
    engine.settle();

    let t_loaded = Instant::now();
    eprintln!(
        "page load+settle: {:.1}ms",
        t_loaded.duration_since(t_engine_ready).as_secs_f64() * 1000.0
    );

    let text = engine.eval_js("document.body.innerText").unwrap_or_default();

    let t_extracted = Instant::now();
    eprintln!(
        "text extract: {:.1}ms",
        t_extracted.duration_since(t_loaded).as_secs_f64() * 1000.0
    );
    eprintln!(
        "TOTAL: {:.1}ms",
        t_extracted.duration_since(t_start).as_secs_f64() * 1000.0
    );

    eprintln!("extracted {} chars", text.len());
    if text.len() > 200 {
        println!("{}...", &text[..200]);
    } else {
        println!("{}", text);
    }
}
