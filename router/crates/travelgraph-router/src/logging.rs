//! Request-scoped logging helpers.
//!
//! Every `/graphql` request is wrapped in an `info_span!("graphql_request",
//! request_id, operation_name)`. After execution we record:
//!
//!   * total_duration_ms       - on the span itself
//!   * subgraph_durations_ms   - one structured event with the full map
//!
//! The subscriber (configured in `main`) is JSON, so log lines come out as
//! one JSON object per record - easy to ingest into the OTel pipeline that
//! arrives in Phase 6.

use std::time::Duration;
use tracing::{field::Empty, Span};
use uuid::Uuid;

pub struct RequestSpanGuard {
    pub span: Span,
    pub request_id: String,
}

pub fn open_request_span(operation_name: Option<&str>) -> RequestSpanGuard {
    let request_id = Uuid::new_v4().to_string();
    let span = tracing::info_span!(
        "graphql_request",
        request_id = %request_id,
        operation_name = operation_name.unwrap_or("<anonymous>"),
        total_duration_ms = Empty,
    );
    RequestSpanGuard { span, request_id }
}

/// Record the total wall-clock duration on the active span and emit a single
/// structured event with the per-subgraph timing map.
pub fn record_request_completion(
    span: &Span,
    request_id: &str,
    total: Duration,
    subgraph_durations_ms: &[(String, u128)],
) {
    span.record("total_duration_ms", total.as_millis() as u64);
    let map: serde_json::Map<String, serde_json::Value> = subgraph_durations_ms
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::from(*v as u64)))
        .collect();
    tracing::info!(
        request_id = %request_id,
        total_duration_ms = total.as_millis() as u64,
        subgraph_durations_ms = %serde_json::Value::Object(map),
        "graphql_request_completed"
    );
}
