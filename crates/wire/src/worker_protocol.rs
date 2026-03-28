//! Protocol types for worker process communication.
//!
//! Worker processes are stripped-down engine processes that run JS without DOM.
//! They communicate with the CLI host via JSON lines on stdin/stdout.

use serde::{Deserialize, Serialize};

use crate::{FetchRequest, FetchResult};

/// Message sent from the CLI host to a worker process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HostToWorker {
    /// Execute this JavaScript code (the worker script).
    Execute { code: String },
    /// A message from the main thread (worker.postMessage).
    PostMessage { data: String },
    /// Fetch results for requests the worker made.
    FetchResults(Vec<FetchResult>),
}

/// Message sent from a worker process to the CLI host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerToHost {
    /// Worker is calling postMessage() back to the main thread.
    PostMessage { data: String },
    /// Worker needs URLs fetched (same as engine NeedFetch).
    NeedFetch(Vec<FetchRequest>),
    /// Worker script finished executing (no more work).
    Done,
    /// Worker encountered an error.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_roundtrip {
        ($val:expr, $ty:ty) => {
            let val = $val;
            let json = serde_json::to_string(&val).unwrap();
            let deserialized: $ty = serde_json::from_str(&json).unwrap();
            assert_eq!(val, deserialized);
        };
    }

    #[test]
    fn host_to_worker_execute_roundtrip() {
        assert_roundtrip!(
            HostToWorker::Execute { code: "postMessage('hello')".into() },
            HostToWorker
        );
    }

    #[test]
    fn host_to_worker_post_message_roundtrip() {
        assert_roundtrip!(
            HostToWorker::PostMessage { data: r#"{"nonce":42}"#.into() },
            HostToWorker
        );
    }

    #[test]
    fn host_to_worker_fetch_results_roundtrip() {
        assert_roundtrip!(
            HostToWorker::FetchResults(vec![]),
            HostToWorker
        );
    }

    #[test]
    fn worker_to_host_post_message_roundtrip() {
        assert_roundtrip!(
            WorkerToHost::PostMessage { data: "result".into() },
            WorkerToHost
        );
    }

    #[test]
    fn worker_to_host_need_fetch_roundtrip() {
        assert_roundtrip!(
            WorkerToHost::NeedFetch(vec![]),
            WorkerToHost
        );
    }

    #[test]
    fn worker_to_host_done_roundtrip() {
        assert_roundtrip!(WorkerToHost::Done, WorkerToHost);
    }

    #[test]
    fn worker_to_host_error_roundtrip() {
        assert_roundtrip!(
            WorkerToHost::Error { message: "ReferenceError".into() },
            WorkerToHost
        );
    }
}
