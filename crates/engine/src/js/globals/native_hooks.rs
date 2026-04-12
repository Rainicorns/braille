//! Rust-native functions registered on the JS context via Function::new.
//! These are the bridge between JS code and Rust engine state (navigation,
//! dialogs, clipboard, form submission). Only Rust Function::new code goes here.
//! Pure JS implementations belong in web_apis.rs or dom_bridge modules.

use rquickjs::{Ctx, Function};

use crate::js::dom_bridge::with_state_mut;

pub(super) fn register(ctx: &Ctx<'_>) {
    // Register __braille_navigate — called by location.href setter to signal pending navigation
    let navigate_fn = Function::new(ctx.clone(), move |url: String| {
        with_state_mut(|s| s.pending_navigation = Some(url));
    }).unwrap();
    ctx.globals().set("__braille_navigate", navigate_fn).unwrap();

    // alert() — queues a blocking Alert browser event, returns undefined
    let alert_fn = Function::new(ctx.clone(), |msg: Option<String>| {
        let message = msg.unwrap_or_default();
        with_state_mut(|s| {
            let id = s.browser_events.push(braille_wire::BrowserEventKind::Alert { message });
            s.blocking_event_id = Some(id);
        });
    }).unwrap();
    ctx.globals().set("alert", alert_fn).unwrap();

    // confirm() — queues a blocking Confirm browser event, returns false by default
    let confirm_fn = Function::new(ctx.clone(), |msg: Option<String>| -> bool {
        let message = msg.unwrap_or_default();
        with_state_mut(|s| {
            let id = s.browser_events.push(braille_wire::BrowserEventKind::Confirm { message });
            s.blocking_event_id = Some(id);
            // Default: return false (agent can override by responding before next settle)
            if let Some(resp) = s.blocking_event_response.take() {
                s.blocking_event_id = None;
                resp == "true" || resp == "yes" || resp == "ok"
            } else {
                false
            }
        })
    }).unwrap();
    ctx.globals().set("confirm", confirm_fn).unwrap();

    // prompt() — queues a blocking Prompt browser event, returns null by default
    let prompt_fn = Function::new(ctx.clone(), |msg: Option<String>, default: Option<String>| -> rquickjs::Null {
        let message = msg.unwrap_or_default();
        with_state_mut(|s| {
            let id = s.browser_events.push(braille_wire::BrowserEventKind::Prompt { message, default_value: default });
            s.blocking_event_id = Some(id);
        });
        // Return null (spec default when user cancels)
        rquickjs::Null
    }).unwrap();
    ctx.globals().set("prompt", prompt_fn).unwrap();

    // __braille_clipboard_write — native hook for clipboard.writeText
    let clipboard_write_fn = Function::new(ctx.clone(), |text: String| {
        with_state_mut(|s| {
            s.clipboard_buffer = text;
        });
    }).unwrap();
    ctx.globals().set("__braille_clipboard_write", clipboard_write_fn).unwrap();

    // __braille_clipboard_read — native hook for clipboard.readText
    let clipboard_read_fn = Function::new(ctx.clone(), || -> String {
        with_state_mut(|s| s.clipboard_buffer.clone())
    }).unwrap();
    ctx.globals().set("__braille_clipboard_read", clipboard_read_fn).unwrap();

    // __braille_form_submit — native hook for form POST submission
    let form_submit_fn = Function::new(ctx.clone(), |url: String, method: String, body: String, content_type: String| {
        with_state_mut(|s| {
            s.pending_form_submit = Some(crate::js::state::PendingFormSubmit {
                url, method, body, content_type,
            });
        });
    }).unwrap();
    ctx.globals().set("__braille_form_submit", form_submit_fn).unwrap();

    // __braille_set_focus — JS pushes focus changes to Rust (avoids eval polling)
    let set_focus_fn = Function::new(ctx.clone(), |nid: rquickjs::Value<'_>| {
        let val = nid.as_int().map(|n| n as usize).filter(|&n| n < usize::MAX);
        with_state_mut(|s| s.focused_nid = val);
    }).unwrap();
    ctx.globals().set("__braille_set_focus", set_focus_fn).unwrap();

    // __braille_set_hover — JS pushes hover changes to Rust (avoids eval polling)
    let set_hover_fn = Function::new(ctx.clone(), |nid: rquickjs::Value<'_>| {
        let val = nid.as_int().map(|n| n as usize).filter(|&n| n < usize::MAX);
        with_state_mut(|s| s.hovered_nid = val);
    }).unwrap();
    ctx.globals().set("__braille_set_hover", set_hover_fn).unwrap();
}
