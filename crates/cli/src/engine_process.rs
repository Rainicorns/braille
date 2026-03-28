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

/// Perform a single HTTP fetch with manual redirect following (used from parallel threads).
///
/// Follows up to 10 redirects, accumulating Set-Cookie headers from every hop
/// so the engine's cookie jar sees cookies set during intermediate 3xx responses.
fn do_fetch(
    client: &reqwest::blocking::Client,
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> FetchOutcome {
    let max_redirects = 10;
    let mut current_url = match url::Url::parse(url) {
        Ok(u) => u,
        Err(e) => return FetchOutcome::Err(format!("invalid URL {url}: {e}")),
    };
    let mut current_method = method.to_uppercase();
    let mut current_body: Option<String> = body.map(|s| s.to_string());
    let mut accumulated_cookies: Vec<(String, String)> = Vec::new();
    let mut redirect_chain: Vec<braille_wire::RedirectHop> = Vec::new();

    for _ in 0..max_redirects {
        let mut builder = match current_method.as_str() {
            "POST" => client.post(current_url.as_str()),
            "PUT" => client.put(current_url.as_str()),
            "DELETE" => client.delete(current_url.as_str()),
            "PATCH" => client.patch(current_url.as_str()),
            "HEAD" => client.head(current_url.as_str()),
            _ => client.get(current_url.as_str()),
        };
        // Determine if this hop is cross-origin relative to the initial URL
        let initial_origin = origin_of(url);
        let current_origin = origin_of(current_url.as_str());
        let is_cross_origin = initial_origin != current_origin;

        // Forward original request headers, but merge accumulated cookies
        // into the Cookie header so redirect hops see cookies set by prior hops.
        // Strip Authorization and Cookie headers on cross-origin redirects.
        let mut has_cookie_header = false;
        for (name, value) in headers {
            if is_cross_origin && (name.eq_ignore_ascii_case("authorization") || name.eq_ignore_ascii_case("cookie")) {
                // Don't forward sensitive headers cross-origin (accumulated cookies still sent below)
                if name.eq_ignore_ascii_case("cookie") {
                    has_cookie_header = true;
                }
                continue;
            }
            if name.eq_ignore_ascii_case("cookie") {
                has_cookie_header = true;
                if !accumulated_cookies.is_empty() {
                    // Merge original Cookie header with accumulated set-cookie values
                    let extra = build_cookie_header_from_set_cookies(&accumulated_cookies);
                    let merged = if value.is_empty() {
                        extra
                    } else {
                        format!("{value}; {extra}")
                    };
                    builder = builder.header("Cookie", merged);
                } else {
                    builder = builder.header(name.as_str(), value.as_str());
                }
            } else {
                builder = builder.header(name.as_str(), value.as_str());
            }
        }
        // If no Cookie header was in the original request (or it was stripped) but we have accumulated cookies
        if !has_cookie_header && !accumulated_cookies.is_empty() {
            let cookie_val = build_cookie_header_from_set_cookies(&accumulated_cookies);
            builder = builder.header("Cookie", cookie_val);
        }
        if let Some(ref body_str) = current_body {
            builder = builder.body(body_str.clone());
        }

        let response = match builder.send() {
            Ok(r) => r,
            Err(e) => return FetchOutcome::Err(format!("fetch failed: {e}")),
        };

        let status = response.status().as_u16();

        // Accumulate Set-Cookie headers from this response
        for value in response.headers().get_all("set-cookie") {
            if let Ok(v) = value.to_str() {
                accumulated_cookies.push(("set-cookie".to_string(), v.to_string()));
            }
        }

        // Check for redirect
        if (300..400).contains(&status) {
            if let Some(location) = response.headers().get("location") {
                let location_str = match location.to_str() {
                    Ok(s) => s,
                    Err(e) => return FetchOutcome::Err(format!("invalid Location header: {e}")),
                };
                let mut next_url = match current_url.join(location_str) {
                    Ok(u) => u,
                    Err(e) => return FetchOutcome::Err(format!("invalid redirect URL {location_str}: {e}")),
                };

                // Never downgrade from https to http (matches browser HSTS behavior)
                if current_url.scheme() == "https" && next_url.scheme() == "http" {
                    let _ = next_url.set_scheme("https");
                }

                // Record this hop's Set-Cookie headers
                let hop_set_cookies: Vec<String> = response.headers().get_all("set-cookie")
                    .iter()
                    .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
                    .collect();

                redirect_chain.push(braille_wire::RedirectHop {
                    status,
                    url: current_url.to_string(),
                    location: next_url.to_string(),
                    set_cookies: hop_set_cookies,
                });

                current_url = next_url;

                // RFC 7231: 301/302/303 → switch to GET, drop body. 307/308 → preserve.
                if (301..=303).contains(&status) {
                    current_method = "GET".to_string();
                    current_body = None;
                }
                continue;
            }
        }

        // Non-redirect (or redirect without Location): this is the final response
        let final_url = current_url.to_string();
        let mut resp_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
            })
            .collect();

        // Prepend accumulated Set-Cookie headers from intermediate redirects
        // (the final response's own set-cookie headers are already in resp_headers)
        if !accumulated_cookies.is_empty() {
            // Collect cookie names already present in the final response's set-cookie headers
            let final_cookie_names: Vec<String> = resp_headers
                .iter()
                .filter(|(n, _)| n == "set-cookie")
                .filter_map(|(_, v)| {
                    let nv = v.split(';').next()?.trim();
                    nv.find('=').map(|eq| nv[..eq].to_string())
                })
                .collect();
            // Only keep intermediate cookies whose name isn't already set by the final response
            let intermediate_cookies: Vec<(String, String)> = accumulated_cookies
                .into_iter()
                .filter(|(_, v)| {
                    let name = v.split(';').next()
                        .and_then(|nv| nv.trim().find('=').map(|eq| nv.trim()[..eq].to_string()));
                    match name {
                        Some(n) => !final_cookie_names.contains(&n),
                        None => false,
                    }
                })
                .collect();
            resp_headers.extend(intermediate_cookies);
        }

        let body = response.text().unwrap_or_default();
        return FetchOutcome::Ok(braille_wire::FetchResponseData {
            status,
            status_text: status_text_for_code(status).to_string(),
            headers: resp_headers,
            body,
            url: final_url,
            redirect_chain,
        });
    }
    let hops: Vec<String> = redirect_chain.iter().map(|h| format!("{} {} -> {}", h.status, h.url, h.location)).collect();
    FetchOutcome::Err(format!("too many redirects (>{max_redirects}) for {url}\n  {}", hops.join("\n  ")))
}

/// Extract the origin (scheme + host + port) from a URL string for cross-origin comparison.
fn origin_of(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), u.port().map(|p| format!(":{p}")).unwrap_or_default()),
        Err(_) => url.to_string(),
    }
}

/// Extract "name=value" pairs from Set-Cookie headers and join them into a Cookie header value.
/// When the same cookie name appears multiple times (e.g., a clear followed by a set),
/// the last value wins — matching how browsers process Set-Cookie headers.
fn build_cookie_header_from_set_cookies(set_cookies: &[(String, String)]) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut values: Vec<String> = Vec::new();
    for (_, v) in set_cookies {
        let name_value = match v.split(';').next() {
            Some(nv) => nv.trim(),
            None => continue,
        };
        if name_value.is_empty() {
            continue;
        }
        let name = match name_value.find('=') {
            Some(eq) => name_value[..eq].to_string(),
            None => continue,
        };
        // If we already have this cookie name, replace it (last wins)
        if let Some(pos) = names.iter().position(|n| *n == name) {
            values[pos] = name_value.to_string();
        } else {
            names.push(name);
            values.push(name_value.to_string());
        }
    }
    values.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_cookie_header_deduplicates_by_name_last_wins() {
        // Anubis pattern: 302 response sets auth= (clear) then auth=JWT (real value).
        // The Cookie header sent on the redirect hop must contain only the last value.
        let set_cookies = vec![
            ("set-cookie".into(), "auth=; Path=/; Max-Age=0".into()),
            ("set-cookie".into(), "verification=abc123; Path=/".into()),
            ("set-cookie".into(), "auth=eyJhbGciOiJFZERTQSJ9.payload.sig; Path=/".into()),
        ];
        let cookie = build_cookie_header_from_set_cookies(&set_cookies);

        // Should contain the JWT, not the empty value
        assert!(
            cookie.contains("auth=eyJ"),
            "should contain the JWT auth value, got: {cookie}"
        );
        // Should NOT contain the bare "auth=" clear
        // Count occurrences of "auth=" — should be exactly 1
        let auth_count = cookie.matches("auth=").count();
        assert_eq!(
            auth_count, 1,
            "should have exactly one auth= entry, got {auth_count} in: {cookie}"
        );
        // Should still contain verification
        assert!(
            cookie.contains("verification=abc123"),
            "should contain verification cookie, got: {cookie}"
        );
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
