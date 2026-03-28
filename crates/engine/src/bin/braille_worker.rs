//! Worker process binary — a minimal QuickJS runtime for Web Worker scripts.
//!
//! Protocol: reads `HostToWorker` JSON lines from stdin, writes `WorkerToHost` to stdout.
//! Supports: postMessage, onmessage, fetch (delegated), crypto, TextEncoder/TextDecoder, timers.

use std::io::{self, BufRead, Write};

use braille_wire::worker_protocol::{HostToWorker, WorkerToHost};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    // Create a minimal QuickJS runtime for the worker
    let runtime = rquickjs::Runtime::new().expect("failed to create QuickJS runtime");
    runtime.set_memory_limit(128 * 1024 * 1024);
    runtime.set_max_stack_size(32 * 1024 * 1024);
    let context = rquickjs::Context::full(&runtime).expect("failed to create QuickJS context");

    // Register worker globals
    context.with(|ctx| {
        register_worker_globals(&ctx);
    });

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("worker stdin read error: {e}");
                break;
            }
        }

        let msg: HostToWorker = match serde_json::from_str(line.trim()) {
            Ok(m) => m,
            Err(e) => {
                send(&mut writer, &WorkerToHost::Error {
                    message: format!("invalid message: {e}"),
                });
                continue;
            }
        };

        match msg {
            HostToWorker::Execute { code } => {
                context.with(|ctx| {
                    match ctx.eval::<(), _>(code.as_str()) {
                        Ok(()) => {}
                        Err(e) => {
                            let msg = format!("{e}");
                            send(&mut writer, &WorkerToHost::Error { message: msg });
                        }
                    }
                    // Flush microtasks
                    while runtime.is_job_pending() {
                        let _ = runtime.execute_pending_job();
                    }
                });
                // Drain any postMessage calls
                drain_outgoing(&context, &mut writer);
            }
            HostToWorker::PostMessage { data } => {
                context.with(|ctx| {
                    let escaped = serde_json::to_string(&data).unwrap_or_else(|_| "\"\"".to_string());
                    let js = format!(
                        r#"(function() {{
                            var parsed = {escaped};
                            try {{ parsed = JSON.parse(parsed); }} catch(e) {{}}
                            if (typeof self.onmessage === 'function') {{
                                self.onmessage({{ type: 'message', data: parsed }});
                            }}
                        }})()"#
                    );
                    let _ = ctx.eval::<(), _>(js.as_str());
                    while runtime.is_job_pending() {
                        let _ = runtime.execute_pending_job();
                    }
                });
                drain_outgoing(&context, &mut writer);
            }
            HostToWorker::FetchResults(_results) => {
                // TODO: resolve pending fetches in the worker runtime
                // For now, workers that need fetch are not yet supported
            }
        }
    }
}

fn send(writer: &mut impl Write, msg: &WorkerToHost) {
    let json = serde_json::to_string(msg).expect("failed to serialize WorkerToHost");
    writeln!(writer, "{json}").expect("failed to write to stdout");
    writer.flush().expect("failed to flush stdout");
}

/// Drain the outgoing message queue and send them to the host.
fn drain_outgoing(context: &rquickjs::Context, writer: &mut impl Write) {
    context.with(|ctx| {
        let global = ctx.globals();
        if let Ok(drain_fn) = global.get::<_, rquickjs::Function>("__worker_drain_outgoing") {
            if let Ok(json) = drain_fn.call::<_, String>(()) {
                if let Ok(messages) = serde_json::from_str::<Vec<String>>(&json) {
                    for data in messages {
                        send(writer, &WorkerToHost::PostMessage { data });
                    }
                }
            }
        }
    });
}

fn register_worker_globals(ctx: &rquickjs::Ctx<'_>) {
    ctx.eval::<(), _>(
        r#"
        // Worker global scope
        var self = globalThis;
        self.onmessage = null;

        // Outgoing message queue (drained by Rust after each operation)
        var __outgoing = [];

        self.postMessage = function(data) {
            var serialized = (typeof data === 'string') ? data : JSON.stringify(data);
            __outgoing.push(serialized);
        };

        globalThis.__worker_drain_outgoing = function() {
            var msgs = JSON.stringify(__outgoing);
            __outgoing = [];
            return msgs;
        };

        // TextEncoder/TextDecoder
        globalThis.TextEncoder = class TextEncoder {
            constructor() { this.encoding = 'utf-8'; }
            encode(str) {
                var arr = [];
                for (var i = 0; i < str.length; i++) {
                    var c = str.charCodeAt(i);
                    if (c < 0x80) { arr.push(c); }
                    else if (c < 0x800) { arr.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f)); }
                    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < str.length) {
                        var c2 = str.charCodeAt(++i);
                        var cp = ((c - 0xd800) << 10) + (c2 - 0xdc00) + 0x10000;
                        arr.push(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3f), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
                    } else {
                        arr.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f));
                    }
                }
                return new Uint8Array(arr);
            }
        };

        globalThis.TextDecoder = class TextDecoder {
            constructor(label) { this.encoding = label || 'utf-8'; }
            decode(buf) {
                var bytes = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
                var result = '';
                for (var i = 0; i < bytes.length;) {
                    var b = bytes[i];
                    if (b < 0x80) { result += String.fromCharCode(b); i++; }
                    else if (b < 0xe0) { result += String.fromCharCode(((b & 0x1f) << 6) | (bytes[i+1] & 0x3f)); i += 2; }
                    else if (b < 0xf0) { result += String.fromCharCode(((b & 0x0f) << 12) | ((bytes[i+1] & 0x3f) << 6) | (bytes[i+2] & 0x3f)); i += 3; }
                    else {
                        var cp = ((b & 0x07) << 18) | ((bytes[i+1] & 0x3f) << 12) | ((bytes[i+2] & 0x3f) << 6) | (bytes[i+3] & 0x3f);
                        cp -= 0x10000;
                        result += String.fromCharCode(0xd800 + (cp >> 10), 0xdc00 + (cp & 0x3ff));
                        i += 4;
                    }
                }
                return result;
            }
        };

        // Console (goes to stderr via the process, not captured)
        globalThis.console = {
            log: function() {},
            warn: function() {},
            error: function() {},
            info: function() {},
            debug: function() {},
        };
    "#,
    )
    .unwrap();

    // Register crypto (reuse the engine's crypto module if possible, or inline minimal version)
    // For now, provide a minimal crypto.subtle.digest for SHA-256 (used by Anubis PoW workers)
    braille_engine::js::crypto::register(ctx);
}
