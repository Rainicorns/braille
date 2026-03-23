use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use braille_engine::{Engine, FetchedResources, ScriptDescriptor};
use braille_wire::{
    DaemonCommand, DaemonResponse, EngineAction, EngineMessage, FetchOutcome, FetchRequest,
    FetchResult, HostMessage, SnapMode,
};

struct Session {
    engine: Engine,
    history: Vec<String>,
    history_index: Option<usize>,
}

impl Session {
    fn new() -> Self {
        Session {
            engine: Engine::new(),
            history: Vec::new(),
            history_index: None,
        }
    }

    fn navigate(&mut self, url: String) {
        if let Some(idx) = self.history_index {
            self.history.truncate(idx + 1);
        }
        self.history.push(url);
        self.history_index = Some(self.history.len() - 1);
    }

    fn go_back(&mut self) -> Option<&str> {
        match self.history_index {
            Some(idx) if idx > 0 => {
                self.history_index = Some(idx - 1);
                Some(&self.history[idx - 1])
            }
            _ => None,
        }
    }

    fn go_forward(&mut self) -> Option<&str> {
        match self.history_index {
            Some(idx) if idx + 1 < self.history.len() => {
                self.history_index = Some(idx + 1);
                Some(&self.history[idx + 1])
            }
            _ => None,
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut session = Session::new();

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("stdin read error: {e}");
                break;
            }
        }

        let msg: HostMessage = match serde_json::from_str(line.trim()) {
            Ok(m) => m,
            Err(e) => {
                let resp = EngineMessage::CommandResult(DaemonResponse::err(format!(
                    "invalid message: {e}"
                )));
                send(&mut writer, &resp);
                continue;
            }
        };

        match msg {
            HostMessage::Command(cmd) => {
                let response = handle_command(&mut session, &mut reader, &mut writer, cmd);
                send(&mut writer, &EngineMessage::CommandResult(response));
            }
            HostMessage::FetchResults(_) => {
                send(
                    &mut writer,
                    &EngineMessage::CommandResult(DaemonResponse::err(
                        "unexpected FetchResults without pending command".to_string(),
                    )),
                );
            }
        }
    }
}

fn send(writer: &mut impl Write, msg: &EngineMessage) {
    let json = serde_json::to_string(msg).expect("failed to serialize EngineMessage");
    writeln!(writer, "{json}").expect("failed to write to stdout");
    writer.flush().expect("failed to flush stdout");
}

fn read_host_message(reader: &mut impl BufRead) -> Option<HostMessage> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => serde_json::from_str(line.trim()).ok(),
        Err(_) => None,
    }
}

/// Request URLs from the host and wait for results. Returns the fetch results.
fn request_fetches(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    requests: Vec<FetchRequest>,
) -> Vec<FetchResult> {
    send(writer, &EngineMessage::NeedFetch(requests));
    match read_host_message(reader) {
        Some(HostMessage::FetchResults(results)) => results,
        _ => vec![],
    }
}

fn handle_command(
    session: &mut Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    cmd: DaemonCommand,
) -> DaemonResponse {
    // Drain console before command so we only capture output from this command.
    session.engine.drain_console();

    let response = handle_command_inner(session, reader, writer, cmd);

    // Attach any console output produced during this command.
    let console = session.engine.drain_console();
    response.with_console(console)
}

fn handle_command_inner(
    session: &mut Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    cmd: DaemonCommand,
) -> DaemonResponse {
    match cmd {
        DaemonCommand::Goto { url, mode } => match fetch_and_load(session, reader, writer, &url, mode) {
            Ok(snapshot) => DaemonResponse::ok(snapshot),
            Err(e) => DaemonResponse::err(e),
        },
        DaemonCommand::Click { selector } => {
            session.engine.snapshot(SnapMode::Compact);
            let action = session.engine.handle_click(&selector);
            match action {
                EngineAction::Navigate(nav_req) => {
                    match fetch_and_load(session, reader, writer, &nav_req.url, SnapMode::Compact) {
                        Ok(snapshot) => DaemonResponse::ok(snapshot),
                        Err(e) => DaemonResponse::err(e),
                    }
                }
                EngineAction::Error(msg) => DaemonResponse::err(msg),
                EngineAction::None => {
                    session.engine.settle();
                    resolve_pending_fetches(session, reader, writer);
                    DaemonResponse::ok(session.engine.snapshot(SnapMode::Compact))
                }
            }
        }
        DaemonCommand::Type { selector, text } => {
            session.engine.snapshot(SnapMode::Compact);
            match session.engine.handle_type(&selector, &text) {
                Ok(()) => {
                    session.engine.settle();
                    resolve_pending_fetches(session, reader, writer);
                    DaemonResponse::ok(session.engine.snapshot(SnapMode::Compact))
                }
                Err(e) => DaemonResponse::err(e),
            }
        }
        DaemonCommand::Select { selector, value } => {
            session.engine.snapshot(SnapMode::Compact);
            match session.engine.handle_select(&selector, &value) {
                Ok(()) => {
                    session.engine.settle();
                    resolve_pending_fetches(session, reader, writer);
                    DaemonResponse::ok(session.engine.snapshot(SnapMode::Compact))
                }
                Err(e) => DaemonResponse::err(e),
            }
        }
        DaemonCommand::Snap { mode } => DaemonResponse::ok(session.engine.snapshot(mode)),
        DaemonCommand::Console => {
            // Console output is already drained and attached by handle_command.
            // Return empty OK — the console field on the response has the data.
            DaemonResponse::ok(String::new())
        }
        DaemonCommand::Back => match session.go_back() {
            Some(url) => {
                let url = url.to_string();
                match fetch_and_load(session, reader, writer, &url, SnapMode::Compact) {
                    Ok(snapshot) => DaemonResponse::ok(snapshot),
                    Err(e) => DaemonResponse::err(e),
                }
            }
            None => DaemonResponse::err("no previous page in history".to_string()),
        },
        DaemonCommand::Forward => match session.go_forward() {
            Some(url) => {
                let url = url.to_string();
                match fetch_and_load(session, reader, writer, &url, SnapMode::Compact) {
                    Ok(snapshot) => DaemonResponse::ok(snapshot),
                    Err(e) => DaemonResponse::err(e),
                }
            }
            None => DaemonResponse::err("no forward page in history".to_string()),
        },
        DaemonCommand::Close => {
            std::process::exit(0);
        }
        DaemonCommand::NewSession | DaemonCommand::DaemonStop => {
            DaemonResponse::err("unexpected command for engine process".to_string())
        }
    }
}

/// Fetch a URL via the host, load HTML with two-phase script loading, return snapshot.
fn fetch_and_load(
    session: &mut Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    url: &str,
    snap_mode: SnapMode,
) -> Result<String, String> {
    // Ask host to fetch the page
    let page_request = FetchRequest {
        id: 0,
        url: url.to_string(),
        method: "GET".to_string(),
        headers: vec![],
        body: None,
    };
    let results = request_fetches(reader, writer, vec![page_request]);
    let page_result = results
        .into_iter()
        .next()
        .ok_or_else(|| "no fetch result received".to_string())?;

    let page_data = match page_result.outcome {
        FetchOutcome::Ok(data) => data,
        FetchOutcome::Err(e) => return Err(format!("fetch failed: {e}")),
    };

    let html = &page_data.body;
    let descriptors = session.engine.parse_and_collect_scripts(html);

    // Collect external script URLs that need fetching
    let import_map_urls = Engine::import_map_urls(&descriptors);
    let mut script_requests: Vec<FetchRequest> = Vec::new();
    let mut next_id = 1u64;

    for desc in &descriptors {
        if let ScriptDescriptor::External(src_url) | ScriptDescriptor::ExternalModule(src_url) = desc {
            script_requests.push(FetchRequest {
                id: next_id,
                url: src_url.clone(),
                method: "GET".to_string(),
                headers: vec![],
                body: None,
            });
            next_id += 1;
        }
    }
    for import_url in &import_map_urls {
        script_requests.push(FetchRequest {
            id: next_id,
            url: import_url.clone(),
            method: "GET".to_string(),
            headers: vec![],
            body: None,
        });
        next_id += 1;
    }

    let mut fetched = HashMap::new();
    if !script_requests.is_empty() {
        // Map id back to URL for building the fetched map
        let id_to_url: HashMap<u64, String> = script_requests
            .iter()
            .map(|r| (r.id, r.url.clone()))
            .collect();

        let script_results = request_fetches(reader, writer, script_requests);
        for result in script_results {
            if let (Some(url), FetchOutcome::Ok(data)) = (id_to_url.get(&result.id), &result.outcome)
            {
                fetched.insert(url.clone(), data.body.clone());
            }
        }
    }

    // Set URL before script execution so location.pathname is correct for routers
    session.engine.set_url(&page_data.url);

    let errors = session
        .engine
        .execute_scripts_lossy(&descriptors, &FetchedResources::scripts_only(fetched));
    for err in &errors {
        eprintln!("[JS ERROR] {err}");
    }

    // Interleave settle + fetch until quiescent (handles dynamic script loading).
    // Use settle_no_advance to avoid firing interval timers (version polling)
    // repeatedly. Only advance time at the very end.
    for round in 0..30 {
        session.engine.settle_no_advance();
        if !session.engine.has_pending_fetches() {
            eprintln!("[settle/fetch] quiescent after {round} rounds");
            break;
        }
        eprintln!("[settle/fetch] round {round} — has pending fetches");
        resolve_pending_fetches(session, reader, writer);
    }
    // Final settle — no time advance. The page is loaded; interval timers
    // (polling) should not fire during initial load. Time advances will happen
    // when the user interacts (click, type) and we call settle().
    session.engine.settle_no_advance();

    session.navigate(page_data.url);

    Ok(session.engine.snapshot(snap_mode))
}

/// Service all pending fetch requests from the engine's JS runtime.
/// Fetches all pending URLs in parallel, resolves them, settles (to fire
/// timers like React's scheduler), then repeats for any NEW fetches.
/// Stops as soon as a wave produces no new unique URLs.
fn resolve_pending_fetches(
    session: &mut Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) {
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for wave in 0..50 {
        if !session.engine.has_pending_fetches() {
            break;
        }
        let pending = session.engine.pending_fetches();

        // Partition into new vs repeat requests
        let mut batch = Vec::new();
        let mut has_new = false;
        for req in pending {
            let key = format!("{} {}", req.method, req.url);
            let is_new = seen_urls.insert(key);
            if is_new {
                has_new = true;
            }
            eprintln!("  [fetch w{wave}{}] {} {}",
                if is_new { "" } else { " repeat" },
                req.method, &req.url[..req.url.len().min(120)]);
            // Log request headers for API calls (helps debug auth/session issues)
            if req.url.contains("/api/") {
                if req.headers.is_empty() {
                    eprintln!("    (no headers)");
                }
                for (h, v) in &req.headers {
                    eprintln!("    {h}: {}", &v[..v.len().min(80)]);
                }
                if let Some(b) = &req.body {
                    eprintln!("    body: {}", &b[..b.len().min(120)]);
                }
            }
            batch.push(FetchRequest {
                id: req.id,
                url: req.url,
                method: req.method,
                headers: req.headers,
                body: req.body,
            });
        }

        // Fetch everything in parallel (new + repeats all go out together)
        let results = request_fetches(reader, writer, batch);
        for result in results {
            match result.outcome {
                FetchOutcome::Ok(data) => {
                    session.engine.resolve_fetch(result.id, &data);
                }
                FetchOutcome::Err(e) => {
                    session.engine.reject_fetch(result.id, &e);
                }
            }
        }

        // Settle (no time advance) to fire ready timers like React's scheduler
        session.engine.settle_no_advance();

        // If this wave had no new URLs, we're done — only polling remains
        if !has_new {
            break;
        }
    }
}
