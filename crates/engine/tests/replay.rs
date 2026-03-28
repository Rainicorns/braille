use braille_engine::transcript::ReplayFetcher;
use braille_engine::Engine;
use braille_wire::SnapMode;

#[test]
fn replay_anubis_techaro() {
    let mut fetcher =
        ReplayFetcher::load("tests/fixtures/anubis_techaro.json").unwrap();
    let mut engine = Engine::new();
    let snapshot = engine
        .navigate("https://anubis.techaro.lol", &mut fetcher, SnapMode::Text)
        .unwrap();

    // The Anubis challenge page should contain its signature text
    assert!(
        snapshot.contains("Making sure you're not a bot"),
        "snapshot should contain Anubis challenge text, got:\n{snapshot}"
    );
    assert!(
        snapshot.contains("Anubis"),
        "snapshot should mention Anubis, got:\n{snapshot}"
    );
}

/// Replay the Anubis transcript and verify the Preact app rendered.
/// The useEffect chain that computes the hash and sets location.href
/// does not complete because Preact's hooks scheduling (setState → rAF → re-render → effect)
/// doesn't fully resolve during settle. This is a known gap — Preact effects
/// need multiple render cycles with rAF scheduling that our virtual clock doesn't
/// fully exercise.
#[test]
fn replay_anubis_preact_renders() {
    use braille_engine::transcript::Transcript;
    use braille_engine::FetchedResources;
    use braille_wire::FetchOutcome;

    let data = std::fs::read_to_string("tests/fixtures/anubis_techaro.json").unwrap();
    let transcript: Transcript = serde_json::from_str(&data).unwrap();

    let body = match &transcript.exchanges[0].results[0].outcome {
        FetchOutcome::Ok(data) => &data.body,
        FetchOutcome::Err(e) => panic!("page fetch failed: {e}"),
    };

    let mut engine = Engine::new();
    let descriptors = engine.parse_and_collect_scripts(body);
    let errors = engine.execute_scripts_lossy(&descriptors, &FetchedResources::default());
    assert!(errors.is_empty(), "JS errors: {errors:?}");

    // Instrument AFTER scripts run — wrap timer functions for future calls
    let _ = engine.eval_js(r#"
        var __dbg_raf_calls = 0;
        var __dbg_st_calls = 0;
        var __dbg_st_fired = 0;
        var _origFire = __braille_fire_timer;
        __braille_fire_timer = function(id) { __dbg_st_fired++; return _origFire(id); };
    "#);

    engine.set_url("https://anubis.techaro.lol/");

    eprintln!("[anubis] after script exec:");
    // Check if the settle loop is actually advancing time
    // Try a manual approach: explicitly advance virtual clock and fire
    engine.settle();
    // Check how many timer callback fires happened
    let timer_fire_test = engine.eval_js(r#"
        (function() {
            // Check if Preact's internal render queue has items
            // The minified code uses variable A for the render queue
            return 'timers_ok';
        })()
    "#).unwrap();
    eprintln!("[anubis] timer_fire_test: {timer_fire_test}");

    // Let's try something different: manually trigger a settle+advance cycle many times
    // to see if we can pump the Preact pipeline
    for _round in 0..20 {
        engine.settle();
    }
    let loc_after_20 = engine.eval_js("window.location.href").unwrap();
    let status_after_20 = engine.eval_js("document.getElementById('status')?.textContent || 'N/A'").unwrap();
    let app_html = engine.eval_js("document.getElementById('app')?.innerHTML?.substring(0,500) || 'NOT_FOUND'").unwrap();
    eprintln!("[anubis] after 20 settles: loc={loc_after_20} status={status_after_20}");
    eprintln!("[anubis] app innerHTML: {app_html}");

    // Try manually running the Anubis logic to see if it CAN work
    let manual_test = engine.eval_js(r#"
        (function() {
            try {
                var info = JSON.parse(document.getElementById('preact_info').textContent);
                var result = 'info_ok, challenge_len=' + info.challenge.length + ', difficulty=' + info.difficulty;
                // Try creating URL like Anubis does
                var u = new URL(info.redir, window.location.href);
                result += ', url_ok=' + u.toString().substring(0,80);
                return result;
            } catch(e) {
                return 'ERROR: ' + e.message + '\n' + (e.stack || '');
            }
        })()
    "#).unwrap();
    eprintln!("[anubis] manual_test: {manual_test}");

    // Instrument: hook into Preact's error handler to catch silent failures
    let _ = engine.eval_js(r#"
        var __anubis_debug = [];
        var _origE = globalThis.__PREACT_OPTIONS_E;
        // Hook Preact's __e (error handler) on the options object
        // In minified Preact, d is options, d.__e is the error handler
    "#);

    // Try running the full Anubis logic manually (bypass Preact)
    let manual_full = engine.eval_js(r#"
        (function() {
            try {
                var info = JSON.parse(document.getElementById('preact_info').textContent);
                // Replicate the Sha256 hash computation from the Anubis script
                // The script defines H (Sha256) and $e (Hmac) and it() (hex)
                // After the module runs, these are in the module scope — not accessible
                return 'module_scope: cannot access H/$e/it from outside module';
            } catch(e) {
                return 'ERROR: ' + e.message;
            }
        })()
    "#).unwrap();
    eprintln!("[anubis] manual_full: {manual_full}");

    // Instrument: wrap requestAnimationFrame and setTimeout to trace calls
    let _ = engine.eval_js(r#"
        var __dbg_raf_calls = 0;
        var __dbg_st_calls = 0;
        var __dbg_st_details = [];
        var _origRAF = requestAnimationFrame;
        var _origST = setTimeout;
        requestAnimationFrame = function(cb) { __dbg_raf_calls++; return _origRAF(cb); };
        setTimeout = function(cb, delay) {
            __dbg_st_calls++;
            if (__dbg_st_details.length < 20) __dbg_st_details.push({delay: delay||0, type: typeof cb});
            return _origST(cb, delay);
        };
    "#);

    // Check if there are pending timers via the engine API
    eprintln!("[anubis] has_pending_timers={}", engine.has_pending_timers());

    // Settle multiple times and check timer errors + state
    for i in 0..10 {
        engine.settle();
        let timer_errs = engine.eval_js("JSON.stringify(__braille_timer_errors || [])").unwrap();
        let loc = engine.eval_js("window.location.href").unwrap();
        let pending = engine.take_pending_navigation();
        let has_timers = engine.has_pending_timers();
        let has_fetches = engine.has_pending_fetches();
        let status = engine.eval_js("document.getElementById('status')?.textContent || 'N/A'").unwrap();
        let raf_calls = engine.eval_js("'' + __dbg_raf_calls").unwrap();
        let st_calls = engine.eval_js("'' + __dbg_st_calls").unwrap();
        let st_fired = engine.eval_js("'' + __dbg_st_fired").unwrap();
        eprintln!("[anubis] settle {i}: loc={loc} status={status} pending={pending:?} timers={has_timers} fetches={has_fetches} raf={raf_calls} st={st_calls} fired={st_fired} timer_errs={timer_errs}");
        if pending.is_some() {
            break;
        }
    }

    // Preact did render — status shows "Loading..."
    let snap = engine.snapshot(SnapMode::Text);
    assert!(
        snap.contains("Loading..."),
        "Preact initial render should show Loading..., got:\n{snap}"
    );
}
