//! Worker process manager — spawns and manages worker child processes.
//!
//! Each Web Worker requested by the engine becomes a `braille-worker` child process.
//! The manager routes messages between the engine and worker processes.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use braille_wire::worker_protocol::{HostToWorker, WorkerToHost};
use braille_wire::HostMessage;

use crate::network::NetworkClient;

struct WorkerProcess {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    #[allow(dead_code)]
    url: String,
}

impl WorkerProcess {
    fn send(&mut self, msg: &HostToWorker) {
        let json = serde_json::to_string(msg).expect("failed to serialize HostToWorker");
        writeln!(self.stdin, "{json}").expect("failed to write to worker stdin");
        self.stdin.flush().expect("failed to flush worker stdin");
    }

    fn try_read(&mut self) -> Option<WorkerToHost> {
        let mut line = String::new();
        match self.stdout.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => serde_json::from_str(line.trim()).ok(),
            Err(_) => None,
        }
    }
}

/// Manages all active worker child processes.
pub struct WorkerManager {
    workers: HashMap<u64, WorkerProcess>,
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerManager {
    pub fn new() -> Self {
        WorkerManager {
            workers: HashMap::new(),
        }
    }

    /// Spawn a new worker process for the given worker_id and script URL.
    /// Fetches the script from the network, sends it to the worker, and
    /// returns any immediate messages from the worker.
    pub fn spawn_worker(
        &mut self,
        worker_id: u64,
        url: &str,
        net: &mut NetworkClient,
    ) -> Vec<HostMessage> {
        let worker_bin = worker_binary_path();
        let mut child = Command::new(&worker_bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| {
                panic!(
                    "failed to spawn worker process at {}: {e}",
                    worker_bin.display()
                )
            });

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        let mut worker = WorkerProcess {
            _child: child,
            stdin,
            stdout,
            url: url.to_string(),
        };

        // Fetch the worker script
        let code = fetch_worker_script(url, net);

        // Send the script to the worker
        worker.send(&HostToWorker::Execute { code });

        self.workers.insert(worker_id, worker);

        // Collect any immediate messages (e.g., postMessage in script body)
        self.collect_worker_messages(worker_id)
    }

    /// Post a message from the main thread to a worker.
    /// Returns any response messages from the worker.
    pub fn post_to_worker(&mut self, worker_id: u64, data: &str) -> Vec<HostMessage> {
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.send(&HostToWorker::PostMessage {
                data: data.to_string(),
            });
            self.collect_worker_messages(worker_id)
        } else {
            vec![]
        }
    }

    /// Terminate a worker process.
    pub fn terminate_worker(&mut self, worker_id: u64) {
        if let Some(mut worker) = self.workers.remove(&worker_id) {
            // Drop stdin to signal EOF, child will exit
            drop(worker.stdin);
            let _ = worker._child.wait();
        }
    }

    /// Collect messages from a specific worker, handling fetch delegation.
    fn collect_worker_messages(&mut self, worker_id: u64) -> Vec<HostMessage> {
        let mut host_messages = Vec::new();
        let worker = match self.workers.get_mut(&worker_id) {
            Some(w) => w,
            None => return host_messages,
        };

        // Read available messages from the worker (non-blocking read by checking lines)
        while let Some(msg) = worker.try_read() {
            match msg {
                WorkerToHost::PostMessage { data } => {
                    host_messages.push(HostMessage::WorkerMessage {
                        worker_id,
                        data,
                    });
                }
                WorkerToHost::NeedFetch(_requests) => {
                    // TODO: handle worker fetch delegation
                    // For now, workers that need fetch are not supported
                }
                WorkerToHost::Done => break,
                WorkerToHost::Error { message } => {
                    host_messages.push(HostMessage::WorkerError {
                        worker_id,
                        error: message,
                    });
                    break;
                }
            }
        }

        host_messages
    }

    /// Get all active worker IDs and URLs.
    #[allow(dead_code)]
    pub fn active_workers(&self) -> Vec<(u64, String)> {
        self.workers
            .iter()
            .map(|(&id, w)| (id, w.url.clone()))
            .collect()
    }

    /// Terminate all workers (used before checkpoint).
    pub fn terminate_all(&mut self) {
        let ids: Vec<u64> = self.workers.keys().copied().collect();
        for id in ids {
            self.terminate_worker(id);
        }
    }
}

impl Drop for WorkerManager {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

fn worker_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("cannot determine current executable path");
    let dir = exe.parent().expect("executable has no parent directory");

    let candidate = dir.join("braille-worker");
    if candidate.exists() {
        return candidate;
    }

    if let Some(parent) = dir.parent() {
        let candidate = parent.join("braille-worker");
        if candidate.exists() {
            return candidate;
        }
    }

    std::path::PathBuf::from("braille-worker")
}

/// Fetch a worker script from a URL. Handles data: URLs inline.
fn fetch_worker_script(url: &str, net: &mut NetworkClient) -> String {
    // Handle data: URLs
    if let Some(rest) = url.strip_prefix("data:") {
        if let Some(comma_idx) = rest.find(',') {
            let meta = &rest[..comma_idx];
            let payload = &rest[comma_idx + 1..];
            if meta.contains("base64") {
                // base64 decode
                let decoded = base64_decode(payload);
                return String::from_utf8_lossy(&decoded).into_owned();
            }
            return urlencoding_decode(payload);
        }
        return String::new();
    }

    // Fetch via network with manual redirect following
    let resolved = net.resolve_url(url);
    let client = net.client().clone();
    let mut current_url = resolved;
    for _ in 0..10 {
        match client.get(&current_url).send() {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if (300..400).contains(&status) {
                    if let Some(location) = resp.headers().get("location") {
                        if let Ok(loc) = location.to_str() {
                            if let Ok(base) = url::Url::parse(&current_url) {
                                if let Ok(mut next) = base.join(loc) {
                                    if base.scheme() == "https" && next.scheme() == "http" {
                                        let _ = next.set_scheme("https");
                                    }
                                    current_url = next.to_string();
                                    continue;
                                }
                            }
                        }
                    }
                }
                return resp.text().unwrap_or_default();
            }
            Err(e) => {
                eprintln!("[worker] failed to fetch script {url}: {e}");
                return String::new();
            }
        }
    }
    eprintln!("[worker] too many redirects for {url}");
    String::new()
}

fn base64_decode(input: &str) -> Vec<u8> {
    // Simple base64 decoder
    let table: Vec<u8> = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .to_vec();
    let mut out = Vec::new();
    let chars: Vec<u8> = input.bytes().filter(|&b| b != b'\n' && b != b'\r' && b != b' ').collect();
    let mut i = 0;
    while i + 3 < chars.len() {
        let a = table.iter().position(|&c| c == chars[i]).unwrap_or(0) as u32;
        let b = table.iter().position(|&c| c == chars[i + 1]).unwrap_or(0) as u32;
        let c_val = if chars[i + 2] == b'=' { 0 } else { table.iter().position(|&c| c == chars[i + 2]).unwrap_or(0) as u32 };
        let d = if chars[i + 3] == b'=' { 0 } else { table.iter().position(|&c| c == chars[i + 3]).unwrap_or(0) as u32 };
        let n = (a << 18) | (b << 12) | (c_val << 6) | d;
        out.push((n >> 16) as u8);
        if chars[i + 2] != b'=' { out.push((n >> 8 & 0xff) as u8); }
        if chars[i + 3] != b'=' { out.push((n & 0xff) as u8); }
        i += 4;
    }
    out
}

fn urlencoding_decode(input: &str) -> String {
    let mut result = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00"),
                16,
            ) {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}
