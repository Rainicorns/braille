use braille_wire::{FetchRequest, FetchResult};
use serde::{Deserialize, Serialize};

use crate::navigation::FetchProvider;

/// A single request/response exchange in a recorded session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exchange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub requests: Vec<FetchRequest>,
    pub results: Vec<FetchResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub console: Vec<String>,
}

/// A recorded session transcript: all fetch exchanges across commands.
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
            label: None,
            requests,
            results: results.clone(),
            console: vec![],
        });
        results
    }
}

/// Replays a previously recorded transcript, serving responses matched by URL.
///
/// All recorded exchanges are flattened into a URL-keyed map at load time,
/// so responses are matched by request URL rather than positional order.
pub struct ReplayFetcher {
    responses: std::collections::HashMap<String, braille_wire::FetchOutcome>,
}

impl ReplayFetcher {
    /// Load a transcript from a JSON file.
    pub fn load(path: &str) -> Result<Self, String> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read transcript {path}: {e}"))?;
        let transcript: Transcript = serde_json::from_str(&data)
            .map_err(|e| format!("failed to parse transcript {path}: {e}"))?;
        Ok(Self::from_transcript(transcript))
    }

    /// Create from an in-memory transcript.
    pub fn from_transcript(transcript: Transcript) -> Self {
        let mut responses = std::collections::HashMap::new();
        for exchange in &transcript.exchanges {
            for (req, result) in exchange.requests.iter().zip(exchange.results.iter()) {
                if let braille_wire::FetchOutcome::Ok(ref data) = result.outcome {
                    responses.insert(data.url.clone(), result.outcome.clone());
                    if req.url != data.url {
                        responses.insert(req.url.clone(), result.outcome.clone());
                    }
                }
            }
        }
        Self { responses }
    }
}

impl FetchProvider for ReplayFetcher {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        requests
            .into_iter()
            .map(|r| {
                let outcome = self
                    .responses
                    .get(&r.url)
                    .cloned()
                    .unwrap_or_else(|| {
                        braille_wire::FetchOutcome::Err(format!("not recorded: {}", r.url))
                    });
                FetchResult {
                    id: r.id,
                    outcome,
                }
            })
            .collect()
    }
}
