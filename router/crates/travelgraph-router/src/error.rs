//! Internal error types used by the executor and merger. Anything that's
//! returned to the client gets converted to a `GraphQLError` first.

use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubgraphError {
    #[error("subgraph timed out after {0:?}")]
    Timeout(Duration),
    #[error("subgraph returned HTTP {status}: {body}")]
    BadStatus { status: u16, body: String },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("could not deserialize subgraph response: {0}")]
    Decode(String),
    #[error("circuit breaker open for this subgraph (retry after {retry_after:?})")]
    CircuitOpen { retry_after: Duration },
}
