//! TravelGraph router binary entrypoint.
//!
//! Modules:
//!   * [`config`]      - runtime configuration loaded from TOML.
//!   * [`graphql`]     - parser, validator, response shapes (Phase 2.2).
//!   * [`supergraph`]  - parsed Apollo Federation v2 supergraph (Phase 3.3).
//!   * [`plan`]        - supergraph-aware query planner (Phase 3.3 / 3.4).
//!   * [`execute`]     - initial fetch + batched `_entities` executor.
//!   * [`merge`]       - response stitcher (Phase 3.4).
//!   * [`cache`]       - Redis response cache (Phase 4.2).
//!   * [`metrics`]     - Prometheus recorder (Phase 4.2 / 4.4).
//!   * [`limits`]      - depth + complexity limits (Phase 4.3).
//!   * [`rate_limit`]  - per-client governor buckets (Phase 4.4).
//!   * [`reliability`] - subgraph tower dispatch: timeout, retry, breaker (4.1).

mod auth;
mod cache;
mod config;
mod error;
mod execute;
mod graphql;
mod limits;
mod logging;
mod merge;
mod metrics;
mod persisted_queries;
mod plan;
mod rate_limit;
mod reliability;
mod server;
mod supergraph;
mod usage;

use anyhow::Context;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace as sdktrace, Resource};
use std::net::SocketAddr;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing().context("initializing tracing")?;
    metrics::init_prometheus().context("prometheus metrics")?;

    let mut config = config::Config::load().context("loading router config")?;
    if std::env::args().any(|arg| arg == "--allow-arbitrary-queries") {
        config.persisted_queries.allow_arbitrary_queries = true;
    }
    tracing::info!(
        port = config.server.port,
        supergraph = %config.supergraph.path.display(),
        "router starting"
    );

    let app = server::build(config.clone()).await?;

    let addr: SocketAddr = ([0, 0, 0, 0], config.server.port).into();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;
    tracing::info!(%addr, "router listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() -> anyhow::Result<()> {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false);
    let registry = tracing_subscriber::registry().with(filter).with(json_layer);

    if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint);
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(sdktrace::config().with_resource(Resource::new(vec![
                KeyValue::new("service.name", "travelgraph-router"),
            ])))
            .install_batch(opentelemetry_sdk::runtime::Tokio)?;
        registry
            .with(OpenTelemetryLayer::new(tracer))
            .try_init()
            .map_err(|e| anyhow::anyhow!("tracing subscriber: {e}"))?;
    } else {
        registry
            .try_init()
            .map_err(|e| anyhow::anyhow!("tracing subscriber: {e}"))?;
    }
    Ok(())
}
