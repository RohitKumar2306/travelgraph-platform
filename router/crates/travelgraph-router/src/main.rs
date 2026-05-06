//! TravelGraph router binary entrypoint.
//!
//! Modules:
//!   * [`config`]      - runtime configuration loaded from TOML.
//!   * [`graphql`]     - parser, validator, response shapes (Phase 2.2).
//!   * [`supergraph`]  - parsed Apollo Federation v2 supergraph (Phase 3.3).
//!   * [`plan`]        - supergraph-aware query planner (Phase 3.3 / 3.4).
//!   * [`execute`]     - initial fetch + batched `_entities` executor.
//!   * [`merge`]       - response stitcher (Phase 3.4).
//!   * [`logging`]     - request-scoped span helpers.
//!   * [`server`]      - axum router wiring everything together.

mod config;
mod error;
mod execute;
mod graphql;
mod logging;
mod merge;
mod plan;
mod server;
mod supergraph;

use anyhow::Context;
use std::net::SocketAddr;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = config::Config::load().context("loading router config")?;
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

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false);
    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .init();
}
