//! Plan executor.
//!
//! Drives an [`ExecutionPlan`] in two phases per top-level field:
//!
//!   1. **Initial fetch** - one parallel call per top-level `FieldFetch`,
//!      hitting the field's owning subgraph.
//!   2. **Entity fetches** - once the initial response is in, each owning
//!      `FieldFetch` fans out one parallel `_entities` call per extending
//!      subgraph with **all** the entity ids in a single batched
//!      `representations` array (the "1 batched call per subgraph"
//!      guarantee from Phase 3.4).
//!
//! Across the entire request the network shape is therefore:
//!
//!   * top-level fields:     N parallel initial calls
//!   * for each entity-shaped field with extenders: 1 batched call per
//!     extending subgraph (also parallel within that field)

use crate::error::SubgraphError;
use crate::graphql::types::GraphQLResponse;
use crate::plan::{EntityFetch, ExecutionPlan, FieldFetch, InitialFetch};
use futures::future::join_all;
use serde_json::{Map, Value};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct InitialFetchResult {
    pub plan: InitialFetch,
    pub duration: Duration,
    pub outcome: Result<GraphQLResponse, SubgraphError>,
}

#[derive(Debug)]
pub struct EntityFetchResult {
    pub plan: EntityFetch,
    pub duration: Duration,
    /// Entity references the executor sent. Currently unused by the merger
    /// (which aligns `_entities` results by DFS-walk index), but retained
    /// so logs and debug tooling can correlate request-side IDs with
    /// response-side enrichments.
    #[allow(dead_code)]
    pub representations: Vec<EntityRef>,
    pub outcome: Result<GraphQLResponse, SubgraphError>,
}

/// Single entity reference, e.g. `{__typename: "Property", id: "uuid"}`.
#[derive(Debug, Clone)]
pub struct EntityRef {
    pub typename: String,
    pub key_field: String,
    pub key_value: Value,
}

#[derive(Debug)]
pub struct FieldFetchResult {
    pub field: FieldFetch,
    pub initial: InitialFetchResult,
    pub entities: Vec<EntityFetchResult>,
}

pub async fn execute(
    plan: ExecutionPlan,
    variables: Value,
    operation_name: Option<String>,
    http: reqwest::Client,
) -> Vec<FieldFetchResult> {
    let futures = plan
        .field_fetches
        .into_iter()
        .map(|f| execute_field(f, variables.clone(), operation_name.clone(), http.clone()));
    join_all(futures).await
}

async fn execute_field(
    field: FieldFetch,
    variables: Value,
    operation_name: Option<String>,
    http: reqwest::Client,
) -> FieldFetchResult {
    // ---- Stage 1: initial fetch ------------------------------------------
    let init_start = Instant::now();
    let pruned_vars = subset_variables(&variables, &field.initial.variable_names);
    let init_outcome = dispatch(
        &field.initial.url,
        &field.initial.query_text,
        &pruned_vars,
        operation_name.as_deref(),
        field.initial.timeout,
        &http,
    )
    .await;
    let init_duration = init_start.elapsed();
    let initial = InitialFetchResult {
        plan: field.initial.clone(),
        duration: init_duration,
        outcome: init_outcome,
    };

    // ---- Stage 2: entity fetches -----------------------------------------
    // Skip if there are no extenders or the initial call failed (we have no
    // entity ids to batch).
    if field.entity_fetches.is_empty() {
        return FieldFetchResult { field, initial, entities: Vec::new() };
    }

    let entity_refs: Vec<EntityRef> = match &initial.outcome {
        Ok(resp) => extract_entity_refs(
            resp.data.as_ref(),
            &field.response_key,
            field
                .entity_type
                .as_deref()
                .unwrap_or("Unknown"),
            field
                .entity_fetches
                .first()
                .map(|e| e.key_field.as_str())
                .unwrap_or("id"),
        ),
        Err(_) => Vec::new(),
    };

    if entity_refs.is_empty() {
        return FieldFetchResult { field, initial, entities: Vec::new() };
    }

    let representations: Vec<Value> = entity_refs
        .iter()
        .map(|r| {
            Value::Object({
                let mut m = Map::new();
                m.insert("__typename".to_string(), Value::String(r.typename.clone()));
                m.insert(r.key_field.clone(), r.key_value.clone());
                m
            })
        })
        .collect();

    let entity_futures = field
        .entity_fetches
        .iter()
        .cloned()
        .map(|ef| {
            let representations = representations.clone();
            let entity_refs = entity_refs.clone();
            let http = http.clone();
            async move {
                let query = format!(
                    "query($representations: [_Any!]!) {{ _entities(representations: $representations) {{ ... on {ty} {{ {body} }} }} }}",
                    ty = ef.type_name,
                    body = ef.fragment_body,
                );
                let vars = Value::Object({
                    let mut m = Map::new();
                    m.insert("representations".to_string(), Value::Array(representations));
                    m
                });
                let start = Instant::now();
                let outcome = dispatch(&ef.url, &query, &vars, None, ef.timeout, &http).await;
                EntityFetchResult {
                    plan: ef,
                    duration: start.elapsed(),
                    representations: entity_refs,
                    outcome,
                }
            }
        });
    let entities = join_all(entity_futures).await;

    FieldFetchResult { field, initial, entities }
}

// ---- HTTP dispatch --------------------------------------------------------

async fn dispatch(
    url: &str,
    query: &str,
    variables: &Value,
    operation_name: Option<&str>,
    timeout: Duration,
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
        .post(url)
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&body).expect("subgraph body must serialize"));
    let result = tokio::time::timeout(timeout, request.send()).await;

    let response = match result {
        Err(_elapsed) => return Err(SubgraphError::Timeout(timeout)),
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

// ---- helpers --------------------------------------------------------------

/// Reduce the inbound request's variables map down to the subset actually
/// referenced by a synthesized subgraph operation. Subgraphs reject
/// "variable defined but unused" per GraphQL spec rule 5.8.4, and conversely
/// a variable used but missing from the map blows up validation in the
/// downstream service.
fn subset_variables(variables: &Value, names: &[String]) -> Value {
    if names.is_empty() {
        return Value::Object(Map::new());
    }
    let Value::Object(src) = variables else {
        return Value::Object(Map::new());
    };
    let mut out = Map::new();
    for name in names {
        if let Some(v) = src.get(name) {
            out.insert(name.clone(), v.clone());
        }
    }
    Value::Object(out)
}

/// Walk the initial response to extract entity references (each entity's
/// `__typename` + key field) in DFS order. The merger walks the response in
/// the same order to map `_entities` results back by index.
pub(crate) fn extract_entity_refs(
    data: Option<&Value>,
    response_key: &str,
    typename: &str,
    key_field: &str,
) -> Vec<EntityRef> {
    let Some(Value::Object(top)) = data else {
        return Vec::new();
    };
    let Some(value) = top.get(response_key) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    extract_into(value, typename, key_field, &mut out);
    out
}

fn extract_into(value: &Value, typename: &str, key_field: &str, out: &mut Vec<EntityRef>) {
    match value {
        Value::Array(items) => {
            for item in items {
                extract_into(item, typename, key_field, out);
            }
        }
        Value::Object(map) => {
            if let Some(key_value) = map.get(key_field) {
                out.push(EntityRef {
                    typename: typename.to_string(),
                    key_field: key_field.to_string(),
                    key_value: key_value.clone(),
                });
            }
        }
        _ => {}
    }
}
