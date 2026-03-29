use rquickjs::Ctx;

/// Register `window.postMessage()` and `MessageEvent` constructor.
pub(super) fn register_messaging(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(
        r#"
        (function() {
            // MessageEvent constructor
            globalThis.MessageEvent = class MessageEvent extends Event {
                constructor(type, opts) {
                    super(type, opts);
                    this.data = (opts && opts.data !== undefined) ? opts.data : null;
                    this.origin = (opts && opts.origin) || '';
                    this.lastEventId = (opts && opts.lastEventId) || '';
                    this.source = (opts && opts.source !== undefined) ? opts.source : null;
                    this.ports = (opts && opts.ports) || [];
                }
            };

            // window.postMessage(data, targetOrigin)
            // Defers delivery to all 'message' listeners via setTimeout(0)
            window.postMessage = function(data, targetOrigin) {
                var serialized = data;
                // Clone via JSON to simulate structured clone
                if (typeof data === 'object' && data !== null) {
                    serialized = JSON.parse(JSON.stringify(data));
                }
                setTimeout(function() {
                    var event = new MessageEvent('message', {
                        data: serialized,
                        origin: (typeof location !== 'undefined' && location.origin) || '',
                        source: window
                    });
                    // Fire via EventTarget listeners on window
                    if (window.__et_listeners) {
                        var cbs = window.__et_listeners['message_b'];
                        if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(window, event); }
                        cbs = window.__et_listeners['message_c'];
                        if (cbs) { var s = cbs.slice(); for (var i = 0; i < s.length; i++) s[i].call(window, event); }
                    }
                    if (typeof window.onmessage === 'function') {
                        window.onmessage(event);
                    }
                }, 0);
            };
        })();
    "#,
    )
    .unwrap();
}
