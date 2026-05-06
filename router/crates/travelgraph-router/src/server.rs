//! HTTP wiring for the router.
//!
//! Two endpoints:
//!
//!   * `GET  /health`  -> 200 OK, used by docker / k8s healthchecks.
//!   * `POST /graphql` -> the full pipeline:
//!         parse -> validate -> plan (supergraph-aware) -> execute -> merge.
//!
//! Status code rules:
//!
//!   * Parse failure        -> HTTP 400 (per Phase 2.2 prompt).
//!   * Validation failure   -> HTTP 200 + errors (GraphQL-over-HTTP spec).
//!   * Routing/exec failure -> HTTP 200 + per-field error (Phase 2.4 policy
//!                              preserved across Phase 3).

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
use crate::execute::execute;
use crate::graphql::types::{GraphQLRequest, GraphQLResponse};
use crate::graphql::{parse, validate};
use crate::logging::{open_request_span, record_request_completion};
use crate::merge::merge;
use crate::plan::plan_operation;
use crate::supergraph::SupergraphCatalog;

#[derive(Clone)]
pub struct AppState {
    /// Held mainly so the auth phase can read settings off it later.
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub catalog: Arc<SupergraphCatalog>,
    pub http: reqwest::Client,
}

pub async fn build(config: Config) -> anyhow::Result<Router> {
    // Phase 3: load the composed supergraph and build a [`SupergraphCatalog`]
    // describing entity ownership and field-level subgraph routing. Replaces
    // Phase 2.3's hand-coded `SubgraphRegistry`.
    let mut catalog = crate::supergraph::load_from_file(&config.supergraph.path)?;
    apply_timeout_overrides(&mut catalog, &config);

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
        catalog: Arc::new(catalog),
        http,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/graphql", post(graphql))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    Ok(app)
}

fn apply_timeout_overrides(catalog: &mut SupergraphCatalog, config: &Config) {
    let default = Duration::from_millis(config.server.default_subgraph_timeout_ms);
    for (name, route) in catalog.subgraphs.iter_mut() {
        let override_ms = config.timeouts.get(name).copied();
        route.timeout = override_ms
            .map(Duration::from_millis)
            .unwrap_or(default);
    }
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
            return (StatusCode::OK, Json(body)).into_response();
        }

        // ---- plan against the supergraph (Phase 3.3) -----------------------
        let plan = match plan_operation(
            &parsed.document,
            req.operation_name.as_deref(),
            &state.catalog,
        ) {
            Ok(p) => p,
            Err(e) => {
                let body = GraphQLResponse {
                    data: None,
                    errors: vec![e.into_graphql()],
                    extensions: None,
                };
                return (StatusCode::OK, Json(body)).into_response();
            }
        };

        // Surface the plan at debug level - one event per request describing
        // every initial fetch and every batched _entities follow-up.
        log_plan(&plan);

        // ---- execute + entity stitching (Phase 3.4) -----------------------
        let results = execute(
            plan,
            req.variables.clone(),
            req.operation_name.clone(),
            state.http.clone(),
        )
        .await;

        // ---- merge + log --------------------------------------------------
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

fn log_plan(plan: &crate::plan::ExecutionPlan) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let initial: Vec<String> = plan
        .field_fetches
        .iter()
        .map(|f| format!("{}->{}", f.response_key, f.initial.subgraph))
        .collect();
    let entity: Vec<String> = plan
        .field_fetches
        .iter()
        .flat_map(|f| {
            f.entity_fetches.iter().map(move |e| {
                format!(
                    "{}.{}->{}",
                    f.response_key, e.type_name, e.subgraph
                )
            })
        })
        .collect();
    tracing::debug!(
        operation = %plan.operation_name.as_deref().unwrap_or(""),
        ?initial,
        entity_fetches = ?entity,
        "execution plan"
    );
}
