//! Response merger for Phase 3.
//!
//! Folds [`FieldFetchResult`]s from the executor into a single
//! `{ data, errors }` envelope:
//!
//!   * Initial fetch returns OK -> copy the field's response value.
//!   * Initial fetch fails       -> set the field to `null` and emit a
//!     path-scoped error (matches Phase 2.4 policy).
//!   * Each entity fetch         -> merge its `_entities[i]` into the
//!     entity at index `i` of the initial response (the executor extracts
//!     entity refs in the same DFS order, so indices align).
//!
//! Synthetic bookkeeping fields the planner injected (`__typename`, and
//! `id` when the client didn't ask for it) are stripped from each entity
//! before the merged response goes back to the client.

use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::error::SubgraphError;
use crate::execute::{EntityFetchResult, FieldFetchResult, InitialFetchResult};
use crate::graphql::types::{GraphQLError, GraphQLResponse};

pub struct MergedResponse {
    pub response: GraphQLResponse,
    /// `subgraph_name -> duration_ms`, surfaced so the request-scoped logger
    /// can emit it as a single structured event.
    pub subgraph_durations_ms: Vec<(String, u128)>,
}

pub fn merge(field_results: Vec<FieldFetchResult>) -> MergedResponse {
    let mut data = Map::new();
    let mut errors = Vec::new();
    let mut durations: Vec<(String, u128)> = Vec::new();

    for FieldFetchResult {
        field,
        initial,
        entities,
    } in field_results
    {
        durations.push((initial.plan.subgraph.clone(), initial.duration.as_millis()));

        let response_key = field.response_key.clone();
        let entity_type = field.entity_type.clone();
        let key_field = field
            .entity_fetches
            .first()
            .map(|e| e.key_field.clone())
            .unwrap_or_else(|| "id".to_string());

        match initial.outcome {
            Ok(resp) => {
                let mut value = take_field(resp.data, &response_key);
                errors.extend(resp.errors);

                // Merge each entity fetch's data back by index.
                for entity in entities {
                    durations.push((entity.plan.subgraph.clone(), entity.duration.as_millis()));
                    apply_entity_outcome(&mut value, &response_key, entity, &mut errors);
                }

                // Strip synthetic bookkeeping fields after all enrichments
                // are merged. We can't drop them earlier - the merger needs
                // `__typename` to skip non-entity nodes - but the wire
                // response should match the client's selection.
                if entity_type.is_some() {
                    let drop_id = !field.client_requested_id_keys.contains(&key_field);
                    strip_synthetic(&mut value, drop_id, &key_field);
                }

                data.insert(response_key, value);
            }
            Err(err) => {
                errors.push(error_for(&err, &response_key, &initial.plan.subgraph));
                data.insert(response_key, Value::Null);
            }
        }
    }

    MergedResponse {
        response: GraphQLResponse {
            data: Some(Value::Object(data)),
            errors,
            extensions: None,
        },
        subgraph_durations_ms: durations,
    }
}

/// Pull the `response_key` out of `data` (which is `{ <key>: ..., ... }`).
fn take_field(data: Option<Value>, key: &str) -> Value {
    let Some(Value::Object(mut map)) = data else {
        return Value::Null;
    };
    map.remove(key).unwrap_or(Value::Null)
}

/// Apply a single entity-fetch outcome to the (mutable) initial value at
/// `response_key`. `_entities[i]` enriches the i-th entity discovered by
/// the executor.
fn apply_entity_outcome(
    initial_value: &mut Value,
    response_key: &str,
    entity: EntityFetchResult,
    errors: &mut Vec<GraphQLError>,
) {
    match entity.outcome {
        Ok(resp) => {
            // Pull `_entities` array out of the response.
            let _entities = match resp.data {
                Some(Value::Object(mut m)) => m.remove("_entities").unwrap_or(Value::Null),
                _ => Value::Null,
            };
            // Subgraph-emitted GraphQL errors get surfaced as-is. They
            // already carry their own paths if the subgraph cared to set
            // them; if not, scope to this top-level field.
            for err in resp.errors {
                let scoped = if err.path.is_empty() {
                    err.with_path(vec![Value::String(response_key.to_string())])
                } else {
                    err
                };
                errors.push(scoped);
            }
            let Value::Array(arr) = _entities else { return };
            apply_entities_array(initial_value, &arr);
        }
        Err(err) => {
            // Non-critical failure: leave the initial-fetch data intact, no
            // null-stamping (we don't know which fields belonged to this
            // extender at this point), but emit a path-scoped error so the
            // client sees what went wrong.
            errors.push(error_for(&err, response_key, &entity.plan.subgraph));
        }
    }
}

/// Walk `initial_value` (object or array) in DFS order and merge fields
/// from `_entities[i]` into the i-th entity object encountered.
fn apply_entities_array(initial_value: &mut Value, entities: &[Value]) {
    let mut idx = 0;
    walk_and_merge(initial_value, entities, &mut idx);
}

fn walk_and_merge(value: &mut Value, entities: &[Value], idx: &mut usize) {
    match value {
        Value::Array(items) => {
            for item in items.iter_mut() {
                walk_and_merge(item, entities, idx);
            }
        }
        Value::Object(map) => {
            // Treat any object that has the entity key (or `__typename`)
            // as the entity. Recursive walk would dive into nested objects
            // but for Phase 3.4 we only batch one level deep, so a flat
            // merge is correct.
            if let Some(extra) = entities.get(*idx) {
                if let Value::Object(extra_map) = extra {
                    for (k, v) in extra_map {
                        map.insert(k.clone(), v.clone());
                    }
                }
            }
            *idx += 1;
        }
        _ => {}
    }
}

/// Drop synthetic bookkeeping fields from each entity object so the wire
/// response matches the client's selection.
fn strip_synthetic(value: &mut Value, drop_id: bool, key_field: &str) {
    let drops: HashSet<&str> = {
        let mut s = HashSet::new();
        s.insert("__typename");
        if drop_id {
            s.insert(key_field);
        }
        s
    };
    fn walk(value: &mut Value, drops: &HashSet<&str>) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(|i| walk(i, drops)),
            Value::Object(map) => {
                map.retain(|k, _| !drops.contains(k.as_str()));
                for (_, v) in map.iter_mut() {
                    walk(v, drops);
                }
            }
            _ => {}
        }
    }
    walk(value, &drops);
}

// ---- error helpers --------------------------------------------------------

fn error_for(err: &SubgraphError, path_root: &str, subgraph: &str) -> GraphQLError {
    let (msg, code) = describe(err);
    GraphQLError::message(msg)
        .with_path(vec![Value::String(path_root.to_string())])
        .with_extensions(serde_json::json!({
            "code": code,
            "subgraph": subgraph,
        }))
}

fn describe(err: &SubgraphError) -> (String, &'static str) {
    match err {
        SubgraphError::Timeout(d) => (
            format!("Subgraph timed out after {:?}.", d),
            "SUBGRAPH_TIMEOUT",
        ),
        SubgraphError::BadStatus { status, .. } => (
            format!("Subgraph responded with HTTP {status}."),
            "SUBGRAPH_HTTP_ERROR",
        ),
        SubgraphError::Transport(msg) => (
            format!("Subgraph transport error: {msg}."),
            "SUBGRAPH_TRANSPORT_ERROR",
        ),
        SubgraphError::Decode(msg) => (
            format!("Subgraph returned an undecodable response: {msg}."),
            "SUBGRAPH_DECODE_ERROR",
        ),
        SubgraphError::CircuitOpen { .. } => (
            "Subgraph circuit breaker is open; upstream calls are temporarily skipped.".into(),
            "SUBGRAPH_CIRCUIT_OPEN",
        ),
    }
}

// Re-export so the executor's InitialFetchResult docs compile cleanly.
#[allow(dead_code)]
fn _link_initial(_: InitialFetchResult) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execute::{EntityFetchResult, EntityRef, FieldFetchResult, InitialFetchResult};
    use crate::plan::{EntityFetch, FieldFetch, InitialFetch, SelectionPath};
    use std::time::Duration;

    fn ok_initial(data: Value, response_key: &str) -> InitialFetchResult {
        InitialFetchResult {
            plan: InitialFetch {
                subgraph: "property".into(),
                url: "http://prop/graphql".into(),
                timeout: Duration::from_millis(1000),
                query_text: "{ stub }".into(),
                variable_names: Vec::new(),
            },
            duration: Duration::from_millis(10),
            outcome: Ok(GraphQLResponse {
                data: Some(Value::Object({
                    let mut m = Map::new();
                    m.insert(response_key.to_string(), data);
                    m
                })),
                errors: Vec::new(),
                extensions: None,
            }),
        }
    }

    #[test]
    fn merges_initial_and_one_entity_fetch_by_index() {
        let initial_value = serde_json::json!([
            {"__typename": "Property", "id": "1", "name": "Alpha"},
            {"__typename": "Property", "id": "2", "name": "Beta"},
        ]);
        let entities_data = serde_json::json!({
            "_entities": [
                {"price": {"totalAmount": "100.00"}},
                {"price": {"totalAmount": "200.00"}},
            ]
        });
        let entity_result = EntityFetchResult {
            plan: EntityFetch {
                subgraph: "pricing".into(),
                url: "http://pricing/graphql".into(),
                timeout: Duration::from_millis(1000),
                type_name: "Property".into(),
                key_field: "id".into(),
                fragment_body: "price { totalAmount }".into(),
                selection_path: SelectionPath {
                    root_key: "searchProperties".into(),
                    is_list: true,
                },
            },
            duration: Duration::from_millis(20),
            representations: vec![
                EntityRef {
                    typename: "Property".into(),
                    key_field: "id".into(),
                    key_value: Value::String("1".into()),
                },
                EntityRef {
                    typename: "Property".into(),
                    key_field: "id".into(),
                    key_value: Value::String("2".into()),
                },
            ],
            outcome: Ok(GraphQLResponse {
                data: Some(entities_data),
                errors: Vec::new(),
                extensions: None,
            }),
        };
        let field = FieldFetch {
            response_key: "searchProperties".into(),
            is_list: true,
            entity_type: Some("Property".into()),
            initial: InitialFetch {
                subgraph: "property".into(),
                url: "http://prop/graphql".into(),
                timeout: Duration::from_millis(1000),
                query_text: "{ stub }".into(),
                variable_names: Vec::new(),
            },
            entity_fetches: vec![entity_result.plan.clone()],
            client_requested_id_keys: ["id".to_string()].into_iter().collect(),
        };
        let merged = merge(vec![FieldFetchResult {
            field,
            initial: ok_initial(initial_value, "searchProperties"),
            entities: vec![entity_result],
        }]);

        let data = merged.response.data.unwrap();
        let arr = data["searchProperties"].as_array().unwrap();
        assert_eq!(arr[0]["name"], "Alpha");
        assert_eq!(arr[0]["price"]["totalAmount"], "100.00");
        assert_eq!(arr[1]["price"]["totalAmount"], "200.00");
        // __typename stripped (synthetic).
        assert!(arr[0].get("__typename").is_none());
        // id retained because client requested it.
        assert_eq!(arr[0]["id"], "1");
        assert!(merged.response.errors.is_empty());
    }

    #[test]
    fn extender_subgraph_failure_emits_error_but_keeps_initial_data() {
        let initial_value = serde_json::json!([
            {"__typename": "Property", "id": "1", "name": "Alpha"}
        ]);
        let entity_result = EntityFetchResult {
            plan: EntityFetch {
                subgraph: "pricing".into(),
                url: "http://pricing/graphql".into(),
                timeout: Duration::from_millis(1000),
                type_name: "Property".into(),
                key_field: "id".into(),
                fragment_body: "price { totalAmount }".into(),
                selection_path: SelectionPath {
                    root_key: "searchProperties".into(),
                    is_list: true,
                },
            },
            duration: Duration::from_millis(1000),
            representations: vec![EntityRef {
                typename: "Property".into(),
                key_field: "id".into(),
                key_value: Value::String("1".into()),
            }],
            outcome: Err(SubgraphError::Timeout(Duration::from_millis(1000))),
        };
        let field = FieldFetch {
            response_key: "searchProperties".into(),
            is_list: true,
            entity_type: Some("Property".into()),
            initial: InitialFetch {
                subgraph: "property".into(),
                url: "http://prop/graphql".into(),
                timeout: Duration::from_millis(1000),
                query_text: "{ stub }".into(),
                variable_names: Vec::new(),
            },
            entity_fetches: vec![entity_result.plan.clone()],
            client_requested_id_keys: HashSet::new(),
        };
        let merged = merge(vec![FieldFetchResult {
            field,
            initial: ok_initial(initial_value, "searchProperties"),
            entities: vec![entity_result],
        }]);

        let data = merged.response.data.unwrap();
        let arr = data["searchProperties"].as_array().unwrap();
        assert_eq!(
            arr[0]["name"], "Alpha",
            "initial fields preserved on extender failure"
        );
        assert_eq!(merged.response.errors.len(), 1);
        let err = &merged.response.errors[0];
        assert!(err.message.contains("timed out"));
        assert_eq!(err.path, vec![Value::String("searchProperties".into())]);
        let ext = err.extensions.as_ref().unwrap();
        assert_eq!(ext["subgraph"], "pricing");
    }
}
