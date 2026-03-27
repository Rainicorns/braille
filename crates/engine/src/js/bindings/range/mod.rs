//! Range API — full implementation for WPT dom/ranges tests.
//!
//! Implements: new Range(), createRange, setStart/End, setStart/EndBefore/After,
//! collapsed, commonAncestorContainer, detach, collapse, cloneRange,
//! selectNode, selectNodeContents, toString, compareBoundaryPoints,
//! comparePoint, isPointInRange, intersectsNode, cloneContents,
//! deleteContents, extractContents, insertNode, surroundContents.

mod types;
mod helpers;
mod contents;
mod methods;
mod registration;
mod live_ranges;
mod static_range;

// Re-export the public API so callers see the same items as before.
pub(crate) use types::{RangeInner, JsRange};
pub(crate) use registration::{create_range_prototype, register_range_global, create_range};
pub(crate) use live_ranges::{
    update_ranges_for_insert,
    update_ranges_for_remove,
    update_ranges_for_char_data,
    update_ranges_for_split_text,
};
pub(crate) use static_range::register_static_range_global;
