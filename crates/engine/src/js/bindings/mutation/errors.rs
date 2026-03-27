use boa_engine::{JsError, JsNativeError};

pub(crate) fn hierarchy_request_error(msg: &str) -> JsError {
    JsNativeError::typ()
        .with_message(format!("HierarchyRequestError: {}", msg))
        .into()
}

pub(crate) fn not_found_error(msg: &str) -> JsError {
    JsNativeError::typ()
        .with_message(format!("NotFoundError: {}", msg))
        .into()
}
