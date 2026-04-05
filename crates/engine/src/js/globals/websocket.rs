//! WebSocket JS class implementation.
//!
//! Provides a standards-compliant WebSocket constructor that dispatches events via
//! EventTarget. The actual network I/O is handled by the host (cli crate) —
//! this module just provides the JS-side API.

/// Returns the JS source code that registers the WebSocket class globally.
pub fn websocket_js() -> &'static str {
    r#"
    (function() {
        var nextWsId = 1;
        var wsInstances = {};

        globalThis.WebSocket = class extends EventTarget {
            static CONNECTING = 0;
            static OPEN = 1;
            static CLOSING = 2;
            static CLOSED = 3;

            constructor(url, protocols) {
                super();
                this._id = nextWsId++;
                this._url = String(url);
                this._readyState = 0; // CONNECTING
                this._protocol = '';
                this._extensions = '';
                this._binaryType = 'blob';
                this._bufferedAmount = 0;
                this.onopen = null;
                this.onmessage = null;
                this.onerror = null;
                this.onclose = null;

                // Normalize protocols
                if (typeof protocols === 'string') protocols = [protocols];
                this._protocols = protocols || [];

                wsInstances[this._id] = this;

                // Signal to engine that we want to connect
                if (typeof __braille_ws_connect === 'function') {
                    __braille_ws_connect(this._id, this._url, this._protocols.join(','));
                }
            }

            get url() { return this._url; }
            get readyState() { return this._readyState; }
            get protocol() { return this._protocol; }
            get extensions() { return this._extensions; }
            get binaryType() { return this._binaryType; }
            set binaryType(v) { this._binaryType = v; }
            get bufferedAmount() { return this._bufferedAmount; }

            get CONNECTING() { return 0; }
            get OPEN() { return 1; }
            get CLOSING() { return 2; }
            get CLOSED() { return 3; }

            send(data) {
                if (this._readyState === 0) {
                    throw new DOMException("Failed to execute 'send' on 'WebSocket': Still in CONNECTING state.", 'InvalidStateError');
                }
                if (this._readyState >= 2) return; // CLOSING or CLOSED — silently ignore
                if (typeof __braille_ws_send === 'function') {
                    __braille_ws_send(this._id, String(data));
                }
            }

            close(code, reason) {
                if (this._readyState >= 2) return;
                this._readyState = 2; // CLOSING
                if (typeof __braille_ws_close === 'function') {
                    __braille_ws_close(this._id, code || 1000, reason || '');
                }
            }
        };

        // Called by engine to deliver events to a WebSocket instance
        globalThis.__braille_ws_deliver = function(id, type, data) {
            var ws = wsInstances[id];
            if (!ws) return;

            if (type === 'open') {
                ws._readyState = 1;
                var evt = new Event('open');
                if (ws.onopen) ws.onopen(evt);
                ws.dispatchEvent(evt);
            } else if (type === 'message') {
                ws._readyState = 1;
                var evt = new MessageEvent('message', {data: data, origin: ws._url});
                if (ws.onmessage) ws.onmessage(evt);
                ws.dispatchEvent(evt);
            } else if (type === 'error') {
                var evt = new Event('error');
                if (ws.onerror) ws.onerror(evt);
                ws.dispatchEvent(evt);
            } else if (type === 'close') {
                ws._readyState = 3;
                var evt = new CloseEvent('close', {code: 1000, reason: data || '', wasClean: true});
                if (ws.onclose) ws.onclose(evt);
                ws.dispatchEvent(evt);
                delete wsInstances[id];
            }
        };

        // CloseEvent class
        if (typeof globalThis.CloseEvent === 'undefined') {
            globalThis.CloseEvent = class CloseEvent extends Event {
                constructor(type, init) {
                    super(type, init);
                    init = init || {};
                    this.code = init.code !== undefined ? init.code : 0;
                    this.reason = init.reason || '';
                    this.wasClean = !!init.wasClean;
                }
            };
        }

        // MessageEvent class (if not already defined)
        if (typeof globalThis.MessageEvent === 'undefined') {
            globalThis.MessageEvent = class MessageEvent extends Event {
                constructor(type, init) {
                    super(type, init);
                    init = init || {};
                    this.data = init.data !== undefined ? init.data : null;
                    this.origin = init.origin || '';
                    this.lastEventId = init.lastEventId || '';
                    this.source = init.source || null;
                    this.ports = init.ports || [];
                }
            };
        }
    })();
    "#
}
