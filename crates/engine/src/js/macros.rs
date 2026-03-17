//! Convenience macros for downcasting `this` in native JS method implementations.
//!
//! These macros eliminate the repetitive two-step downcast pattern
//! (`this.as_object()` + `obj.downcast_ref::<T>()`) that appears in almost every
//! binding function.  Each macro emits two `let` bindings: an intermediate
//! `JsObject` reference and the final `GcRef` guard.  On failure they return a
//! descriptive `JsError`.
//!
//! # Usage
//!
//! ```ignore
//! extract_element!(elem, this, "getAttribute");
//! extract_event!(evt, this, "Event.bubbles getter");
//! extract_mutation_observer!(mo, this, "observe");
//! ```

/// Downcast `this` to a `JsElement` reference.
///
/// Emits two `let` bindings in the caller's scope so the intermediate
/// `&JsObject` lives long enough for the `GcRef<JsElement>`.
#[allow(unused_macros)]
macro_rules! extract_element {
    ($var:ident, $this:expr, $method:expr) => {
        let __extract_obj = $this.as_object().ok_or_else(|| {
            boa_engine::JsError::from_opaque(
                boa_engine::js_string!(concat!($method, ": this is not an Element")).into(),
            )
        })?;
        let $var = __extract_obj
            .downcast_ref::<$crate::js::bindings::element::JsElement>()
            .ok_or_else(|| {
                boa_engine::JsError::from_opaque(
                    boa_engine::js_string!(concat!($method, ": this is not an Element")).into(),
                )
            })?;
    };
}

/// Downcast `this` to a `JsEvent` reference.
///
/// Emits two `let` bindings in the caller's scope so the intermediate
/// `&JsObject` lives long enough for the `GcRef<JsEvent>`.
#[allow(unused_macros)]
macro_rules! extract_event {
    ($var:ident, $this:expr, $method:expr) => {
        let __extract_obj = $this.as_object().ok_or_else(|| {
            boa_engine::JsError::from_opaque(
                boa_engine::js_string!(concat!($method, ": this is not an Event")).into(),
            )
        })?;
        let $var = __extract_obj
            .downcast_ref::<$crate::js::bindings::event::JsEvent>()
            .ok_or_else(|| {
                boa_engine::JsError::from_opaque(
                    boa_engine::js_string!(concat!($method, ": this is not an Event")).into(),
                )
            })?;
    };
}

/// Downcast `this` to a `JsMutationObserver` reference.
///
/// Emits two `let` bindings in the caller's scope so the intermediate
/// `&JsObject` lives long enough for the `GcRef<JsMutationObserver>`.
#[allow(unused_macros)]
macro_rules! extract_mutation_observer {
    ($var:ident, $this:expr, $method:expr) => {
        let __extract_obj = $this.as_object().ok_or_else(|| {
            boa_engine::JsError::from_opaque(
                boa_engine::js_string!(concat!($method, ": this is not a MutationObserver")).into(),
            )
        })?;
        let $var = __extract_obj
            .downcast_ref::<$crate::js::bindings::mutation_observer::JsMutationObserver>()
            .ok_or_else(|| {
                boa_engine::JsError::from_opaque(
                    boa_engine::js_string!(concat!($method, ": this is not a MutationObserver")).into(),
                )
            })?;
    };
}
