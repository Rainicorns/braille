use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use braille_engine::transcript::{RecordingFetcher, Transcript};
use braille_engine::{Engine, FetchProvider};
use braille_wire::{
    DaemonCommand, DaemonResponse, EngineAction, EngineMessage, FetchRequest,
    FetchResult, HostMessage, SnapMode,
};

#[derive(Debug, Clone, PartialEq)]
enum WorkerStatus {
    Spawning,
    Running,
}

#[derive(Debug, Clone)]
struct WorkerState {
    url: String,
    status: WorkerStatus,
}

struct Session {
    engine: Engine,
    history: Vec<String>,
    history_index: Option<usize>,
    workers: HashMap<u64, WorkerState>,
    next_worker_id: u64,
}

impl Session {
    fn new() -> Self {
        Session {
            engine: Engine::new(),
            history: Vec::new(),
            history_index: None,
            workers: HashMap::new(),
            next_worker_id: 1,
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
                let is_close = matches!(cmd, DaemonCommand::Close);
                let response = handle_command(&mut session, &mut reader, &mut writer, cmd);
                send(&mut writer, &EngineMessage::CommandResult(response));
                if is_close {
                    break;
                }
            }
            HostMessage::FetchResults(_) => {
                send(
                    &mut writer,
                    &EngineMessage::CommandResult(DaemonResponse::err(
                        "unexpected FetchResults without pending command".to_string(),
                    )),
                );
            }
            HostMessage::WorkerMessage { worker_id, data } => {
                handle_worker_message(&mut session, worker_id, &data);
            }
            HostMessage::WorkerSpawned { worker_id } => {
                if let Some(w) = session.workers.get_mut(&worker_id) {
                    w.status = WorkerStatus::Running;
                }
            }
            HostMessage::WorkerError { worker_id, error } => {
                deliver_worker_error(&mut session, worker_id, &error);
                session.workers.remove(&worker_id);
            }
            HostMessage::WorkerExited { worker_id } => {
                session.workers.remove(&worker_id);
            }
            HostMessage::PrepareCheckpoint => {
                let active_workers: Vec<braille_wire::WorkerDescriptor> = session
                    .workers
                    .iter()
                    .map(|(&id, w)| braille_wire::WorkerDescriptor {
                        id,
                        url: w.url.clone(),
                    })
                    .collect();
                send(
                    &mut writer,
                    &EngineMessage::CheckpointReady { active_workers },
                );
            }
            HostMessage::WorkerRestored { worker_id, url } => {
                session.workers.insert(
                    worker_id,
                    WorkerState {
                        url,
                        status: WorkerStatus::Running,
                    },
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

    // Emit any pending worker operations before returning the result
    drain_pending_workers(session, writer);

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
        DaemonCommand::Goto { url, mode, record_path } => match fetch_and_load(session, reader, writer, &url, mode, record_path) {
            Ok(snapshot) => DaemonResponse::ok(snapshot),
            Err(e) => DaemonResponse::err(e),
        },
        DaemonCommand::Click { selector } => {
            session.engine.snapshot(SnapMode::Compact);
            let action = session.engine.handle_click(&selector);
            match action {
                EngineAction::Navigate(nav_req) => {
                    match fetch_and_load(session, reader, writer, &nav_req.url, SnapMode::Compact, None) {
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
                match fetch_and_load(session, reader, writer, &url, SnapMode::Compact, None) {
                    Ok(snapshot) => DaemonResponse::ok(snapshot),
                    Err(e) => DaemonResponse::err(e),
                }
            }
            None => DaemonResponse::err("no previous page in history".to_string()),
        },
        DaemonCommand::Forward => match session.go_forward() {
            Some(url) => {
                let url = url.to_string();
                match fetch_and_load(session, reader, writer, &url, SnapMode::Compact, None) {
                    Ok(snapshot) => DaemonResponse::ok(snapshot),
                    Err(e) => DaemonResponse::err(e),
                }
            }
            None => DaemonResponse::err("no forward page in history".to_string()),
        },
        DaemonCommand::Close => {
            // Return response first — the caller needs confirmation before we exit.
            // The main loop will send this via EngineMessage::CommandResult,
            // then we rely on the host closing stdin (triggering EOF) to stop the loop.
            DaemonResponse::ok("session closed".to_string())
        }
        DaemonCommand::NewSession | DaemonCommand::DaemonStop => {
            DaemonResponse::err("unexpected command for engine process".to_string())
        }
    }
}

/// IPC-based fetch provider that delegates to the host process over stdin/stdout.
struct IpcFetchProvider<'a, R: BufRead, W: Write> {
    reader: &'a mut R,
    writer: &'a mut W,
}

impl<R: BufRead, W: Write> FetchProvider for IpcFetchProvider<'_, R, W> {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        request_fetches(self.reader, self.writer, requests)
    }
}

/// Fetch a URL via the host, load HTML with two-phase script loading, return snapshot.
/// When BRAILLE_RECORD is set, wraps the fetcher with RecordingFetcher and saves the
/// transcript to the specified path after navigation completes.
fn fetch_and_load(
    session: &mut Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    url: &str,
    snap_mode: SnapMode,
    record_path: Option<String>,
) -> Result<String, String> {
    let ipc = IpcFetchProvider { reader, writer };
    let mut recorder = RecordingFetcher::new(ipc);
    let result = session.engine.navigate(url, &mut recorder, snap_mode.clone());

    // Save transcript: use explicit record_path, fall back to BRAILLE_RECORD env var
    let save_path = record_path.or_else(|| std::env::var("BRAILLE_RECORD").ok());
    if let Some(path) = save_path {
        let transcript = Transcript {
            url: url.to_string(),
            exchanges: recorder.into_exchanges(),
        };
        let json = serde_json::to_string_pretty(&transcript)
            .expect("failed to serialize transcript");
        std::fs::write(&path, json)
            .unwrap_or_else(|e| eprintln!("[record] failed to write transcript to {path}: {e}"));
        eprintln!("[record] saved transcript to {path}");
    }

    let snapshot = result?;
    session.navigate(url.to_string());
    Ok(snapshot)
}

/// Service all pending fetch requests from the engine's JS runtime via IPC.
fn resolve_pending_fetches(
    session: &mut Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) {
    let mut fetcher = IpcFetchProvider { reader, writer };
    session.engine.settle_with_fetches(&mut fetcher);
}

/// Deliver a worker message to the JS runtime via __braille_deliver_worker_message.
fn handle_worker_message(session: &mut Session, worker_id: u64, data: &str) {
    let js = format!(
        "__braille_deliver_worker_message({}, {})",
        worker_id,
        serde_json::to_string(data).unwrap_or_else(|_| "\"\"".to_string())
    );
    let _ = session.engine.eval_js(&js);
    session.engine.settle_no_advance();
}

/// Deliver a worker error to the JS runtime via __braille_deliver_worker_error.
fn deliver_worker_error(session: &mut Session, worker_id: u64, error: &str) {
    let js = format!(
        "__braille_deliver_worker_error({}, {})",
        worker_id,
        serde_json::to_string(error).unwrap_or_else(|_| "\"\"".to_string())
    );
    let _ = session.engine.eval_js(&js);
    session.engine.settle_no_advance();
}

/// Drain pending worker operations from the engine and emit them as EngineMessages.
fn drain_pending_workers(session: &mut Session, writer: &mut impl Write) {
    let spawns = session.engine.drain_pending_worker_spawns();
    for (url,) in spawns {
        let worker_id = session.next_worker_id;
        session.next_worker_id += 1;
        session.workers.insert(
            worker_id,
            WorkerState {
                url: url.clone(),
                status: WorkerStatus::Spawning,
            },
        );
        // Tell JS the worker_id so it can route messages
        let js = format!("__braille_assign_worker_id({worker_id})");
        let _ = session.engine.eval_js(&js);
        send(writer, &EngineMessage::SpawnWorker { worker_id, url });
    }
    let messages = session.engine.drain_pending_worker_messages();
    for (worker_id, data) in messages {
        send(
            writer,
            &EngineMessage::PostToWorker { worker_id, data },
        );
    }
    let terminates = session.engine.drain_pending_worker_terminates();
    for worker_id in terminates {
        session.workers.remove(&worker_id);
        send(writer, &EngineMessage::TerminateWorker { worker_id });
    }
}

