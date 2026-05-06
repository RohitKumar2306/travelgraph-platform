//! Prometheus metrics (Phase 4.2 / 4.4).

use metrics_exporter_prometheus::PrometheusBuilder;
use once_cell::sync::OnceCell;
use std::sync::Arc;

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
}

/// Rate limit rejections (Phase 4.4).
pub fn record_rate_limited(client: &str) {
    metrics::counter!("graphql_rate_limited_total", "client" => client.to_string()).increment(1);
}
