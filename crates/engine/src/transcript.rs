use braille_wire::{FetchRequest, FetchResult};
use serde::{Deserialize, Serialize};

use crate::navigation::FetchProvider;

/// A single request/response exchange in a recorded session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exchange {
    pub requests: Vec<FetchRequest>,
    pub results: Vec<FetchResult>,
}

/// A recorded session transcript: the URL visited and all fetch exchanges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub url: String,
    pub exchanges: Vec<Exchange>,
}

/// Wraps any FetchProvider, recording every exchange for later replay.
pub struct RecordingFetcher<F> {
    inner: F,
    exchanges: Vec<Exchange>,
}

impl<F> RecordingFetcher<F> {
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            exchanges: Vec::new(),
        }
    }

    /// Consume the recorder and return the captured exchanges.
    pub fn into_exchanges(self) -> Vec<Exchange> {
        self.exchanges
    }
}

impl<F: FetchProvider> FetchProvider for RecordingFetcher<F> {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        let results = self.inner.fetch_batch(requests.clone());
        self.exchanges.push(Exchange {
            requests,
            results: results.clone(),
        });
        results
    }
}

/// Replays a previously recorded transcript, serving responses sequentially.
pub struct ReplayFetcher {
    exchanges: Vec<Exchange>,
    cursor: usize,
}

impl ReplayFetcher {
    /// Load a transcript from a JSON file.
    pub fn load(path: &str) -> Result<Self, String> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read transcript {path}: {e}"))?;
        let transcript: Transcript = serde_json::from_str(&data)
            .map_err(|e| format!("failed to parse transcript {path}: {e}"))?;
        Ok(Self {
            exchanges: transcript.exchanges,
            cursor: 0,
        })
    }

    /// Create from an in-memory transcript.
    pub fn from_transcript(transcript: Transcript) -> Self {
        Self {
            exchanges: transcript.exchanges,
            cursor: 0,
        }
    }
}

impl FetchProvider for ReplayFetcher {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        assert!(
            self.cursor < self.exchanges.len(),
            "ReplayFetcher: no more exchanges (cursor={}, total={})",
            self.cursor,
            self.exchanges.len()
        );
        let exchange = &self.exchanges[self.cursor];
        self.cursor += 1;
        // Remap IDs: match by position, use the live request's ID
        requests
            .iter()
            .zip(exchange.results.iter())
            .map(|(req, recorded)| FetchResult {
                id: req.id,
                outcome: recorded.outcome.clone(),
            })
            .collect()
    }
}
