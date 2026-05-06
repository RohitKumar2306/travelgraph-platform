//! HTTP wiring for the router.
//!
//! Two endpoints:
//!
//!   * `GET  /health`  -> 200 OK, used by docker / k8s healthchecks.
//!   * `POST /graphql` -> the full pipeline:
//!         parse -> validate -> project -> dispatch -> merge.
//!
//! Status code rules:
//!
//!   * Parse failure        -> HTTP 400 (per Phase 2.2 prompt).
//!   * Validation failure   -> HTTP 200 + errors (GraphQL-over-HTTP spec).
//!   * Routing/exec failure -> HTTP 200 + per-field error (Phase 2.4 policy).

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::trace::TraceLayer;
use tracing::Instrument;

use crate::config::Config;
use crate::graphql::project::plan;
use crate::graphql::types::{GraphQLError, GraphQLRequest, GraphQLResponse};
use crate::graphql::{parse, validate};
use crate::logging::{open_request_span, record_request_completion};
use crate::merge::merge;
use crate::registry::SubgraphRegistry;

#[derive(Clone)]
pub struct AppState {
    /// Kept for future use (Phase 5 will read auth settings out of it).
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub registry: Arc<SubgraphRegistry>,
    pub http: reqwest::Client,
}

pub async fn build(config: Config) -> anyhow::Result<Router> {
    let registry = SubgraphRegistry::from_config(&config)?;
    let http = reqwest::Client::builder()
        // Each subgraph call additionally times out via tokio::time::timeout
        // in the executor. The connect timeout below stops a misconfigured URL
        // from blocking the entire pool.
        .connect_timeout(Duration::from_millis(500))
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(16)
        .build()?;

    let state = AppState {
        config: Arc::new(config),
        registry: Arc::new(registry),
        http,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/graphql", post(graphql))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    Ok(app)
}

// ---------- handlers --------------------------------------------------------

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn graphql(
    State(state): State<AppState>,
    Json(req): Json<GraphQLRequest>,
) -> impl IntoResponse {
    let span_guard = open_request_span(req.operation_name.as_deref());
    let request_id = span_guard.request_id.clone();
    let span = span_guard.span;

    async move {
        let start = Instant::now();

        // ---- parse (Phase 2.2) ---------------------------------------------
        let parsed = match parse::parse(&req.query) {
            Ok(p) => p,
            Err(errors) => {
                let body = GraphQLResponse {
                    data: None,
                    errors,
                    extensions: None,
                };
                tracing::warn!("parse error");
                return (StatusCode::BAD_REQUEST, Json(body)).into_response();
            }
        };

        // ---- validate (Phase 2.2) ------------------------------------------
        let validation_errors = validate::validate(&parsed.document);
        if !validation_errors.is_empty() {
            let body = GraphQLResponse {
                data: None,
                errors: validation_errors,
                extensions: None,
            };
            tracing::warn!("validation error");
            // GraphQL-over-HTTP spec: validation errors return 200.
            return (StatusCode::OK, Json(body)).into_response();
        }

        // ---- plan (Phase 2.3) ----------------------------------------------
        let plans = match plan(&parsed.document, req.operation_name.as_deref(), &state.registry) {
            Ok(p) => p,
            Err(errors) => {
                let body = GraphQLResponse {
                    data: None,
                    errors,
                    extensions: None,
                };
                return (StatusCode::OK, Json(body)).into_response();
            }
        };

        if plans.is_empty() {
            let body = GraphQLResponse {
                data: Some(serde_json::Value::Object(Default::default())),
                errors: vec![GraphQLError::message(
                    "Operation has no top-level fields to execute.",
                )],
                extensions: None,
            };
            return (StatusCode::OK, Json(body)).into_response();
        }

        // ---- dispatch in parallel (Phase 2.3) -----------------------------
        let results = crate::execute::dispatch_all(
            plans,
            req.variables.clone(),
            req.operation_name.clone(),
            &state.registry,
            &state.http,
        )
        .await;

        // ---- merge + log (Phase 2.4) --------------------------------------
        let merged = merge(results);
        let total = start.elapsed();
        record_request_completion(
            &tracing::Span::current(),
            &request_id,
            total,
            &merged.subgraph_durations_ms,
        );
        (StatusCode::OK, Json(merged.response)).into_response()
    }
    .instrument(span)
    .await
}
