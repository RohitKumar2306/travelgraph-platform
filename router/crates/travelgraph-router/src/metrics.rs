//! Prometheus metrics (Phase 4.2 / 4.4 / 6.1).

use metrics_exporter_prometheus::PrometheusBuilder;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::time::Duration;

static PROM_HANDLE: OnceCell<Arc<metrics_exporter_prometheus::PrometheusHandle>> = OnceCell::new();

pub fn init_prometheus() -> anyhow::Result<()> {
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("prometheus recorder: {e}"))?;
    PROM_HANDLE
        .set(Arc::new(handle))
        .map_err(|_| anyhow::anyhow!("prometheus already initialized"))?;
    Ok(())
}

pub fn render_prometheus() -> String {
    PROM_HANDLE
        .get()
        .expect("prometheus not initialized")
        .render()
}

/// Cache hit (labels: result = hit|miss).
pub fn record_cache(result: &'static str) {
    metrics::counter!("graphql_response_cache_total", "result" => result).increment(1);
    match result {
        "hit" => metrics::counter!("graphql_cache_hits_total").increment(1),
        "miss" => metrics::counter!("graphql_cache_misses_total").increment(1),
        _ => {}
    }
}

/// Rate limit rejections (Phase 4.4).
pub fn record_rate_limited(client: &str) {
    metrics::counter!("graphql_rate_limited_total", "client" => client.to_string()).increment(1);
}

pub fn record_graphql_request(operation: &str, status: &str, duration: Duration) {
    metrics::counter!(
        "graphql_requests_total",
        "operation" => operation.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
    metrics::histogram!(
        "graphql_request_duration_ms",
        "operation" => operation.to_string()
    )
    .record(duration.as_secs_f64() * 1000.0);
}

pub fn record_subgraph_duration(subgraph: &str, duration: Duration) {
    metrics::histogram!(
        "graphql_subgraph_duration_ms",
        "subgraph" => subgraph.to_string()
    )
    .record(duration.as_secs_f64() * 1000.0);
}

pub fn record_subgraph_error(subgraph: &str) {
    metrics::counter!(
        "graphql_subgraph_errors_total",
        "subgraph" => subgraph.to_string()
    )
    .increment(1);
}

pub fn record_circuit_breaker_open(subgraph: &str) {
    metrics::counter!(
        "graphql_circuit_breaker_open",
        "subgraph" => subgraph.to_string()
    )
    .increment(1);
}

pub fn record_query_complexity(cost: u32) {
    metrics::histogram!("graphql_query_complexity").record(cost as f64);
}
