use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use braille_wire::{
    DaemonCommand, DaemonResponse, EngineMessage, FetchOutcome, FetchResult, HostMessage,
};

use crate::network::NetworkClient;
use crate::worker_manager::WorkerManager;

/// A handle to a running engine child process.
pub struct EngineProcess {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    workers: WorkerManager,
}

impl EngineProcess {
    /// Spawn a new engine child process.
    pub fn spawn() -> Self {
        let engine_bin = engine_binary_path();
        let mut child = Command::new(&engine_bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn engine process at {}: {e}", engine_bin.display()));

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        EngineProcess {
            _child: child,
            stdin,
            stdout,
            workers: WorkerManager::new(),
        }
    }

    /// Send a command to the engine and handle the fetch delegation loop.
    /// Returns the final DaemonResponse.
    pub fn send_command(&mut self, cmd: DaemonCommand, net: &mut NetworkClient) -> DaemonResponse {
        self.send_host_message(&HostMessage::Command(cmd));

        loop {
            let engine_msg = self.read_engine_message();
            match engine_msg {
                Some(EngineMessage::CommandResult(response)) => return response,
                Some(EngineMessage::SpawnWorker { worker_id, url }) => {
                    eprintln!("[cli] spawning worker: id={worker_id} url={}", &url[..url.len().min(120)]);
                    let messages = self.workers.spawn_worker(worker_id, &url, net);
                    // Send WorkerSpawned confirmation, then relay any immediate messages
                    self.send_host_message(&HostMessage::WorkerSpawned { worker_id });
                    for msg in messages {
                        self.send_host_message(&msg);
                    }
                }
                Some(EngineMessage::PostToWorker { worker_id, data }) => {
                    let messages = self.workers.post_to_worker(worker_id, &data);
                    for msg in messages {
                        self.send_host_message(&msg);
                    }
                }
                Some(EngineMessage::TerminateWorker { worker_id }) => {
                    self.workers.terminate_worker(worker_id);
                }
                Some(EngineMessage::CheckpointReady { active_workers }) => {
                    eprintln!("[cli] engine checkpoint ready, {} active workers", active_workers.len());
                    // TODO: Phase 7 — checkpoint container
                    let _ = active_workers;
                }
                Some(EngineMessage::NeedFetch(requests)) => {
                    // Resolve URLs and prepare requests before spawning threads
                    #[allow(clippy::type_complexity)]
                    let prepared: Vec<(u64, String, String, Vec<(String, String)>, Option<String>)> =
                        requests
                            .into_iter()
                            .map(|req| {
                                let resolved = net.resolve_url(&req.url);
                                (req.id, resolved, req.method, req.headers, req.body)
                            })
                            .collect();

                    // Fetch all URLs in parallel using scoped threads
                    let client = net.client().clone();
                    let results: Vec<FetchResult> = std::thread::scope(|s| {
                        let handles: Vec<_> = prepared
                            .into_iter()
                            .map(|(id, url, method, headers, body)| {
                                let client = &client;
                                s.spawn(move || {
                                    let outcome = do_fetch(client, &url, &method, &headers, body.as_deref());
                                    FetchResult { id, outcome }
                                })
                            })
                            .collect();
                        handles.into_iter().map(|h| h.join().unwrap()).collect()
                    });

                    // Update base URL from first successful fetch
                    for r in &results {
                        if let FetchOutcome::Ok(data) = &r.outcome {
                            net.set_base_url(&data.url);
                            break;
                        }
                    }

                    self.send_host_message(&HostMessage::FetchResults(results));
                }
                None => {
                    return DaemonResponse::err("engine process died".to_string());
                }
            }
        }
    }

    fn send_host_message(&mut self, msg: &HostMessage) {
        let json = serde_json::to_string(msg).expect("failed to serialize HostMessage");
        writeln!(self.stdin, "{json}").expect("failed to write to engine stdin");
        self.stdin.flush().expect("failed to flush engine stdin");
    }

    fn read_engine_message(&mut self) -> Option<EngineMessage> {
        let mut line = String::new();
        match self.stdout.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => serde_json::from_str(line.trim()).ok(),
            Err(_) => None,
        }
    }
}

fn engine_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("cannot determine current executable path");
    let dir = exe.parent().expect("executable has no parent directory");

    // Check same directory (normal case)
    let candidate = dir.join("braille-engine");
    if candidate.exists() {
        return candidate;
    }

    // Check parent directory (test binaries are in target/debug/deps/,
    // but the engine binary is in target/debug/)
    if let Some(parent) = dir.parent() {
        let candidate = parent.join("braille-engine");
        if candidate.exists() {
            return candidate;
        }
    }

    // Fall back to PATH
    std::path::PathBuf::from("braille-engine")
}

/// Perform a single HTTP fetch (used from parallel threads).
fn do_fetch(
    client: &reqwest::blocking::Client,
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> FetchOutcome {
    let mut builder = match method.to_uppercase().as_str() {
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        "HEAD" => client.head(url),
        _ => client.get(url),
    };
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    if let Some(body_str) = body {
        builder = builder.body(body_str.to_string());
    }
    match builder.send() {
        Ok(response) => {
            let final_url = response.url().to_string();
            let status = response.status().as_u16();
            let headers: Vec<(String, String)> = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
                })
                .collect();
            let body = response.text().unwrap_or_default();
            FetchOutcome::Ok(braille_wire::FetchResponseData {
                status,
                status_text: status_text_for_code(status).to_string(),
                headers,
                body,
                url: final_url,
            })
        }
        Err(e) => FetchOutcome::Err(format!("fetch failed: {e}")),
    }
}

fn status_text_for_code(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}
