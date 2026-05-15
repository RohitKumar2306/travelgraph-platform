//! HTTP wiring for the router.
//!
//! Endpoints:
//!
//!   * `GET  /health`  -> 200 OK
//!   * `GET  /metrics` -> Prometheus exposition
//!   * `POST /graphql` -> rate limit → auth → persisted query lookup →
//!         parse → validate → cache (optional) → depth/cost limits → plan →
//!         execute → merge → cache set

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::trace::TraceLayer;
use tracing::Instrument;

use crate::cache::{self, ResponseCache};
use crate::config::Config;
use crate::execute::execute;
use crate::graphql::types::{GraphQLRequest, GraphQLResponse};
use crate::graphql::{parse, validate};
use crate::limits::{analyze_cost, estimate_cost};
use crate::logging::{open_request_span, record_request_completion};
use crate::merge::merge;
use crate::persisted_queries::{PersistedQueryDecision, PersistedQueryStore};
use crate::plan::plan_operation;
use crate::plan::OperationKind;
use crate::rate_limit::ClientRateLimiter;
use crate::reliability::{breakers_for_subgraphs, CircuitBreakerConfig, SubgraphClient};
use crate::supergraph::SupergraphCatalog;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub catalog: Arc<SupergraphCatalog>,
    pub subgraph: Arc<SubgraphClient>,
    pub cache: Option<Arc<ResponseCache>>,
    pub rate_limit: Arc<ClientRateLimiter>,
    pub persisted_queries: Arc<PersistedQueryStore>,
    pub usage_http: reqwest::Client,
}

pub async fn build(config: Config) -> anyhow::Result<Router> {
    let mut catalog = crate::supergraph::load_from_file(&config.supergraph.path)?;
    apply_timeout_overrides(&mut catalog, &config);

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .pool_max_idle_per_host(config.http_client.pool_max_idle_per_host)
        .pool_idle_timeout(config.http_pool_idle_timeout())
        .tcp_keepalive(config.http_tcp_keepalive())
        .build()?;

    let brk_cfg = CircuitBreakerConfig {
        window: Duration::from_secs(config.reliability.circuit_window_sec),
        min_samples: config.reliability.circuit_min_samples,
        failure_ratio: config.reliability.circuit_failure_ratio,
        recovery: Duration::from_secs(config.reliability.circuit_open_recovery_sec),
    };
    let breakers = Arc::new(breakers_for_subgraphs(
        catalog.subgraphs.keys().cloned(),
        brk_cfg,
    ));
    let subgraph = Arc::new(SubgraphClient::new(http.clone(), breakers, &config.reliability));

    let cache = if config.cache.enabled && !config.cache.redis_url.is_empty() {
        Some(ResponseCache::connect(&config.cache).await?.into_shared())
    } else {
        None
    };

    let rate_limit_cfg = config.rate_limit.clone();
    let persisted_queries = if config.persisted_queries.path.exists() {
        PersistedQueryStore::load(&config.persisted_queries.path)?
    } else if config.persisted_queries.allow_arbitrary_queries {
        PersistedQueryStore::default()
    } else {
        PersistedQueryStore::load(&config.persisted_queries.path)?
    };
    let state = AppState {
        config: Arc::new(config),
        catalog: Arc::new(catalog),
        subgraph,
        cache,
        rate_limit: Arc::new(ClientRateLimiter::new(&rate_limit_cfg)),
        persisted_queries: Arc::new(persisted_queries),
        usage_http: http,
    };

    let rl = state.clone();
    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(|| async { metrics_text() }))
        .route(
            "/graphql",
            post(graphql).layer(axum::middleware::from_fn_with_state(
                rl,
                rate_limit_middleware,
            )),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    Ok(app)
}

fn merge_timeout_defaults(cfg: &Config) -> std::collections::HashMap<String, u64> {
    let mut m = crate::config::default_subgraph_timeouts_ms();
    for (k, v) in &cfg.timeouts {
        m.insert(k.clone(), *v);
    }
    m
}

fn apply_timeout_overrides(catalog: &mut SupergraphCatalog, cfg: &Config) {
    let defaults = merge_timeout_defaults(cfg);
    let fallback = cfg.server.default_subgraph_timeout_ms;
    for (name, route) in catalog.subgraphs.iter_mut() {
        let ms = defaults.get(name).copied().unwrap_or(fallback);
        route.timeout = Duration::from_millis(ms);
    }
}

fn metrics_text() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        crate::metrics::render_prometheus(),
    )
}

async fn rate_limit_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let client = headers
        .get("apollographql-client-name")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("__anonymous__");
    match state.rate_limit.check(client) {
        Ok(()) => next.run(request).await,
        Err(wait) => {
            crate::metrics::record_rate_limited(client);
            let secs = wait.as_secs().max(1);
            axum::response::Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header(header::RETRY_AFTER, secs.to_string())
                .body(Body::from("rate limit exceeded"))
                .unwrap()
        }
    }
}

// ---------- handlers --------------------------------------------------------

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn graphql(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut req): Json<GraphQLRequest>,
) -> impl IntoResponse {
    let identity = crate::auth::Identity::from_headers(&headers, &state.config.auth.jwt_secret);
    let client_name = header_value(&headers, "apollographql-client-name", "__anonymous__");
    let client_version = header_value(&headers, "apollographql-client-version", "__unknown__");
    let operation_label = operation_label(req.operation_name.as_deref());
    let span_guard = open_request_span(
        req.operation_name.as_deref(),
        &client_name,
        &client_version,
        &identity.sub,
    );
    let request_id = span_guard.request_id.clone();
    let span = span_guard.span;

    async move {
        let start = Instant::now();
        let identity_headers =
            identity.signed_headers(&state.config.auth.identity_signature_secret);

        {
            let _span = tracing::info_span!(
                "graphql.persisted_query",
                client_name = %client_name,
                client_version = %client_version,
                operation_name = %operation_label,
                user_id = %identity.sub
            )
            .entered();
            match state.persisted_queries.resolve(
                Some(req.query.as_str()).filter(|query| !query.trim().is_empty()),
                req.extensions.as_ref(),
                state.config.persisted_queries.allow_arbitrary_queries,
            ) {
                Ok(PersistedQueryDecision::UseStored(query)) => req.query = query.to_string(),
                Ok(PersistedQueryDecision::UseRequestQuery) => {}
                Err(error) => {
                    crate::metrics::record_graphql_request(
                        &operation_label,
                        "persisted_query_error",
                        start.elapsed(),
                    );
                    let body = GraphQLResponse {
                        data: None,
                        errors: vec![error],
                        extensions: None,
                    };
                    return (StatusCode::OK, Json(body)).into_response();
                }
            }
        }

        // ---- parse (Phase 2.2 / 6.1) --------------------------------------
        let parsed = {
            let _span = tracing::info_span!(
                "graphql.parse",
                client_name = %client_name,
                client_version = %client_version,
                operation_name = %operation_label,
                user_id = %identity.sub
            )
            .entered();
            match parse::parse(&req.query) {
                Ok(p) => p,
                Err(errors) => {
                    crate::metrics::record_graphql_request(
                        &operation_label,
                        "parse_error",
                        start.elapsed(),
                    );
                    let body = GraphQLResponse {
                        data: None,
                        errors,
                        extensions: None,
                    };
                    tracing::warn!("parse error");
                    return (StatusCode::BAD_REQUEST, Json(body)).into_response();
                }
            }
        };

        // ---- validate (Phase 2.2 / 6.1) -----------------------------------
        let validation_errors = {
            let _span = tracing::info_span!(
                "graphql.validate",
                client_name = %client_name,
                client_version = %client_version,
                operation_name = %operation_label,
                user_id = %identity.sub
            )
            .entered();
            validate::validate(&parsed.document)
        };
        if !validation_errors.is_empty() {
            crate::metrics::record_graphql_request(
                &operation_label,
                "validation_error",
                start.elapsed(),
            );
            let body = GraphQLResponse {
                data: None,
                errors: validation_errors,
                extensions: None,
            };
            tracing::warn!("validation error");
            return (StatusCode::OK, Json(body)).into_response();
        }

        if let Ok(report) = estimate_cost(
            &parsed.document,
            req.operation_name.as_deref(),
            &state.catalog,
        ) {
            crate::metrics::record_query_complexity(report.cost);
        }

        // ---- Redis cache (read path, Phase 4.2) ---------------------------
        if let Some(cache) = state.cache.as_ref() {
            if cache.enabled() {
                let key = ResponseCache::cache_key(
                    &req.query,
                    req.operation_name.as_deref(),
                    &req.variables,
                    &identity.sub,
                );
                match cache::try_get(Some(cache), &key).await {
                    Ok(Some(bytes)) => match serde_json::from_slice::<GraphQLResponse>(&bytes) {
                        Ok(resp) => {
                            tracing::info!(cache = "hit", key = %key, "graphql response cache");
                            crate::metrics::record_graphql_request(
                                &operation_label,
                                "cache_hit",
                                start.elapsed(),
                            );
                            return (StatusCode::OK, Json(resp)).into_response();
                        }
                        Err(e) => tracing::warn!(key = %key, %e, "cache entry decode failed"),
                    },
                    Ok(None) => {}
                    Err(e) => tracing::warn!(key = %key, %e, "redis GET failed; continuing"),
                }
            }
        }

        // ---- depth / complexity (Phase 4.3) -------------------------------
        let cost_result = {
            let _span = tracing::info_span!(
                "graphql.cost_limit",
                client_name = %client_name,
                client_version = %client_version,
                operation_name = %operation_label,
                user_id = %identity.sub
            )
            .entered();
            analyze_cost(
                &parsed.document,
                req.operation_name.as_deref(),
                &state.catalog,
                state.config.limits.max_depth,
                state.config.limits.max_cost,
            )
        };
        if let Err(e) = cost_result {
            crate::metrics::record_graphql_request(
                &operation_label,
                "cost_limit_error",
                start.elapsed(),
            );
            let body = GraphQLResponse {
                data: None,
                errors: vec![e.into_graphql()],
                extensions: None,
            };
            return (StatusCode::OK, Json(body)).into_response();
        }

        // ---- plan against the supergraph (Phase 3.3 / 6.1) ----------------
        let plan = {
            let _span = tracing::info_span!(
                "graphql.plan",
                client_name = %client_name,
                client_version = %client_version,
                operation_name = %operation_label,
                user_id = %identity.sub
            )
            .entered();
            match plan_operation(
                &parsed.document,
                req.operation_name.as_deref(),
                &state.catalog,
            ) {
                Ok(p) => p,
                Err(e) => {
                    crate::metrics::record_graphql_request(
                        &operation_label,
                        "plan_error",
                        start.elapsed(),
                    );
                    let body = GraphQLResponse {
                        data: None,
                        errors: vec![e.into_graphql()],
                        extensions: None,
                    };
                    return (StatusCode::OK, Json(body)).into_response();
                }
            }
        };

        log_plan(&plan);

        // ---- execute + entity stitching (Phase 3.4 / 4.1 / 6.1) ----------
        let plan_for_cache = plan.clone();
        let results = execute(
            plan,
            req.variables.clone(),
            req.operation_name.clone(),
            identity_headers,
            client_name.clone(),
            client_version.clone(),
            operation_label.clone(),
            identity.sub.clone(),
            state.subgraph.clone(),
        )
        .await;
        let merged = {
            let _span = tracing::info_span!(
                "graphql.response_merge",
                client_name = %client_name,
                client_version = %client_version,
                operation_name = %operation_label,
                user_id = %identity.sub
            )
            .entered();
            merge(results)
        };
        let total = start.elapsed();
        let status = if merged.response.errors.is_empty() {
            "ok"
        } else {
            "graphql_error"
        };
        crate::metrics::record_graphql_request(&operation_label, status, total);
        if status == "ok" && state.config.usage.enabled {
            let events = crate::usage::collect_usage_events(
                &parsed.document,
                &plan_for_cache,
                &state.catalog,
                &operation_label,
                &client_name,
                &client_version,
            );
            if !events.is_empty() {
                if let Err(e) = state
                    .usage_http
                    .post(&state.config.usage.endpoint)
                    .json(&events)
                    .send()
                    .await
                {
                    tracing::warn!(error = %e, "field usage publication failed");
                }
            }
        }
        record_request_completion(
            &tracing::Span::current(),
            &request_id,
            total,
            &merged.subgraph_durations_ms,
        );

        // ---- cache successful read operations (Phase 4.2) -----------------
        if response_cacheable(
            &plan_for_cache,
            Some(identity.sub.as_str()),
            &merged.response,
        ) {
            if let Some(cache) = state.cache.as_ref() {
                if cache.enabled() {
                    let key = ResponseCache::cache_key(
                        &req.query,
                        req.operation_name.as_deref(),
                        &req.variables,
                        &identity.sub,
                    );
                    let ttl = cache.ttl_for_plan(&plan_for_cache);
                    match serde_json::to_vec(&merged.response) {
                        Ok(bytes) => {
                            if let Err(e) = cache::try_set(Some(cache), &key, ttl, &bytes).await {
                                tracing::warn!(key = %key, %e, "redis SET failed");
                            }
                        }
                        Err(e) => tracing::warn!(key = %key, %e, "cache body serialization failed"),
                    }
                }
            }
        }

        (StatusCode::OK, Json(merged.response)).into_response()
    }
    .instrument(span)
    .await
}

/*
 * The old linear pipeline is intentionally kept out of the function body by
 * the phase 6 span blocks above.
 */
#[allow(dead_code)]
fn _phase6_marker() {}

fn header_value(headers: &HeaderMap, name: &str, fallback: &str) -> String {
    headers
        .get(name)
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn operation_label(operation_name: Option<&str>) -> String {
    operation_name.unwrap_or("<anonymous>").to_string()
}

fn response_cacheable(
    plan: &crate::plan::ExecutionPlan,
    identity: Option<&str>,
    resp: &GraphQLResponse,
) -> bool {
    plan.operation_kind == OperationKind::Query
        && identity.map(|s| !s.is_empty()).unwrap_or(false)
        && resp.errors.is_empty()
        && resp.data.is_some()
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
            f.entity_fetches
                .iter()
                .map(move |e| format!("{}.{}->{}", f.response_key, e.type_name, e.subgraph))
        })
        .collect();
    tracing::debug!(
        operation = %plan.operation_name.as_deref().unwrap_or(""),
        ?initial,
        entity_fetches = ?entity,
        "execution plan"
    );
}
