//! Plan executor (Phase 3 + 4): uses [`crate::reliability::SubgraphClient`] for
//! tower [`Service`]-backed subgraph calls.

use crate::error::SubgraphError;
use crate::graphql::types::GraphQLResponse;
use crate::plan::{EntityFetch, ExecutionPlan, FieldFetch, InitialFetch, OperationKind};
use crate::reliability::SubgraphClient;
use crate::reliability::SubgraphHttpCall;
use futures::future::join_all;
use serde_json::{Map, Value};
use std::sync::Arc;
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
    #[allow(dead_code)]
    pub representations: Vec<EntityRef>,
    pub outcome: Result<GraphQLResponse, SubgraphError>,
}

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
    identity_headers: Vec<(&'static str, String)>,
    client: Arc<SubgraphClient>,
) -> Vec<FieldFetchResult> {
    let op = plan.operation_kind;
    let futures = plan.field_fetches.into_iter().map(|f| {
        let vars = variables.clone();
        let op_name = operation_name.clone();
        let headers = identity_headers.clone();
        let c = client.clone();
        async move { execute_field(f, vars, op_name, headers, op, c).await }
    });
    join_all(futures).await
}

async fn execute_field(
    field: FieldFetch,
    variables: Value,
    operation_name: Option<String>,
    identity_headers: Vec<(&'static str, String)>,
    op_kind: OperationKind,
    client: Arc<SubgraphClient>,
) -> FieldFetchResult {
    let init_start = Instant::now();
    let pruned_vars = subset_variables(&variables, &field.initial.variable_names);
    let body = build_body(
        &field.initial.query_text,
        &pruned_vars,
        operation_name.as_deref(),
    );
    let init_outcome = client
        .send(SubgraphHttpCall {
            subgraph: field.initial.subgraph.clone(),
            url: field.initial.url.clone(),
            body,
            headers: identity_headers.clone(),
            timeout: field.initial.timeout,
            operation: op_kind,
        })
        .await;
    let init_duration = init_start.elapsed();
    let initial = InitialFetchResult {
        plan: field.initial.clone(),
        duration: init_duration,
        outcome: init_outcome,
    };

    if field.entity_fetches.is_empty() {
        return FieldFetchResult {
            field,
            initial,
            entities: Vec::new(),
        };
    }

    let entity_refs: Vec<EntityRef> = match &initial.outcome {
        Ok(resp) => extract_entity_refs(
            resp.data.as_ref(),
            &field.response_key,
            field.entity_type.as_deref().unwrap_or("Unknown"),
            field
                .entity_fetches
                .first()
                .map(|e| e.key_field.as_str())
                .unwrap_or("id"),
        ),
        Err(_) => Vec::new(),
    };

    if entity_refs.is_empty() {
        return FieldFetchResult {
            field,
            initial,
            entities: Vec::new(),
        };
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

    let entity_futures = field.entity_fetches.iter().cloned().map(|ef| {
        let representations = representations.clone();
        let entity_refs = entity_refs.clone();
        let headers = identity_headers.clone();
        let c = client.clone();
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
            let body_bytes = build_body(&query, &vars, None);
            let start = Instant::now();
            let outcome = c
                .send(SubgraphHttpCall {
                    subgraph: ef.subgraph.clone(),
                    url: ef.url.clone(),
                    body: body_bytes,
                    headers,
                    timeout: ef.timeout,
                    operation: OperationKind::Query,
                })
                .await;
            EntityFetchResult {
                plan: ef,
                duration: start.elapsed(),
                representations: entity_refs,
                outcome,
            }
        }
    });
    let entities = join_all(entity_futures).await;

    FieldFetchResult {
        field,
        initial,
        entities,
    }
}

fn build_body(query: &str, variables: &Value, operation_name: Option<&str>) -> Vec<u8> {
    let mut body = serde_json::json!({
        "query": query,
        "variables": variables,
    });
    if let Some(op) = operation_name {
        if let Value::Object(map) = &mut body {
            map.insert("operationName".to_owned(), Value::String(op.to_owned()));
        }
    }
    serde_json::to_vec(&body).expect("subgraph body must serialize")
}

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
