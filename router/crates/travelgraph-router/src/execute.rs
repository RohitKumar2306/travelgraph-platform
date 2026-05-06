//! Parallel subgraph execution.
//!
//! For each [`PerSubgraphPlan`] we POST a synthesized GraphQL operation to
//! the owning subgraph. All requests run in parallel via
//! [`futures::future::join_all`]. Each per-subgraph call is bounded by a
//! configurable timeout (Phase 2.4 default = 1s).

use crate::error::SubgraphError;
use crate::graphql::project::PerSubgraphPlan;
use crate::graphql::types::GraphQLResponse;
use crate::registry::{SubgraphRegistry, SubgraphRoute};
use futures::future::join_all;
use serde_json::Value;
use std::time::{Duration, Instant};

/// One subgraph call's outcome. Ownership of `plan` is preserved so the
/// merger can scope errors to the right response keys.
pub struct SubgraphResult {
    pub plan: PerSubgraphPlan,
    pub duration: Duration,
    pub outcome: Result<GraphQLResponse, SubgraphError>,
}

/// Dispatch every plan in parallel and return the results once they settle
/// (or time out). The function never panics; transport failures live inside
/// `outcome`.
pub async fn dispatch_all(
    plans: Vec<PerSubgraphPlan>,
    variables: Value,
    operation_name: Option<String>,
    registry: &SubgraphRegistry,
    http: &reqwest::Client,
) -> Vec<SubgraphResult> {
    let futures = plans.into_iter().map(|plan| {
        let route = registry.route(&plan.subgraph).cloned();
        let vars = variables.clone();
        let op_name = operation_name.clone();
        let http = http.clone();
        async move {
            let start = Instant::now();
            let outcome = match route {
                Some(route) => dispatch_one(&route, &plan.query_text, &vars, op_name.as_deref(), &http).await,
                None => Err(SubgraphError::Transport(format!(
                    "subgraph \"{}\" has no route in the registry",
                    plan.subgraph
                ))),
            };
            SubgraphResult {
                plan,
                duration: start.elapsed(),
                outcome,
            }
        }
    });
    join_all(futures).await
}

async fn dispatch_one(
    route: &SubgraphRoute,
    query: &str,
    variables: &Value,
    operation_name: Option<&str>,
    http: &reqwest::Client,
) -> Result<GraphQLResponse, SubgraphError> {
    let mut body = serde_json::json!({
        "query": query,
        "variables": variables,
    });
    if let Some(op) = operation_name {
        if let Value::Object(map) = &mut body {
            map.insert("operationName".to_owned(), Value::String(op.to_owned()));
        }
    }

    let request = http
        .post(&route.url)
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&body).expect("subgraph body must serialize"));

    let result = tokio::time::timeout(route.timeout, request.send()).await;

    let response = match result {
        Err(_elapsed) => return Err(SubgraphError::Timeout(route.timeout)),
        Ok(Err(e)) => return Err(SubgraphError::Transport(e.to_string())),
        Ok(Ok(r)) => r,
    };

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| SubgraphError::Transport(e.to_string()))?;

    if !status.is_success() {
        return Err(SubgraphError::BadStatus {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }

    serde_json::from_slice::<GraphQLResponse>(&bytes)
        .map_err(|e| SubgraphError::Decode(e.to_string()))
}
