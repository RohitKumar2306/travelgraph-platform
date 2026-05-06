//! Subgraph HTTP dispatch with Phase 4.1 reliability:
//! circuit breaker → per-attempt timeout → exponential backoff retries for
//! idempotent read operations (never for mutations).
//!
//! Tower [`Service`] is implemented so the stack can be documented/wrapped
//! with additional `tower::Layer`s without changing call sites.

mod circuit;
mod tower_dispatch;

pub use circuit::{breakers_for_subgraphs, CircuitBreakerConfig};
pub use tower_dispatch::{SubgraphClient, SubgraphHttpCall};
