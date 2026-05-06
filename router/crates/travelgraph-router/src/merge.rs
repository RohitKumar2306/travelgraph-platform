//! Response merger.
//!
//! Takes the [`SubgraphResult`]s from the executor and folds them into a
//! single `{ data, errors }` envelope, following Phase 2.4's policy:
//!
//! * 200 + errors -> preserve errors verbatim.
//! * Timeout / 5xx / transport error -> set every top-level response key for
//!   that subgraph to `null` and emit a synthetic GraphQL error pinned to
//!   each affected path.
//! * Any individual top-level field failure does NOT fail the overall request:
//!   the HTTP response stays 200 (per the GraphQL HTTP spec).

use serde_json::{Map, Value};

use crate::error::SubgraphError;
use crate::execute::SubgraphResult;
use crate::graphql::types::{GraphQLError, GraphQLResponse};

pub struct MergedResponse {
    pub response: GraphQLResponse,
    /// `subgraph_name -> duration_ms`, surfaced so the request-scoped logger
    /// can emit it as a single structured event.
    pub subgraph_durations_ms: Vec<(String, u128)>,
}

pub fn merge(results: Vec<SubgraphResult>) -> MergedResponse {
    let mut data = Map::new();
    let mut errors = Vec::new();
    let mut durations = Vec::with_capacity(results.len());

    for SubgraphResult {
        plan,
        duration,
        outcome,
    } in results
    {
        durations.push((plan.subgraph.clone(), duration.as_millis()));

        match outcome {
            Ok(resp) => {
                if let Some(Value::Object(obj)) = resp.data {
                    for key in &plan.response_keys {
                        let value = obj.get(key).cloned().unwrap_or(Value::Null);
                        data.insert(key.clone(), value);
                    }
                } else {
                    // Either `null` or non-object data: stamp every owned key as null.
                    for key in &plan.response_keys {
                        data.insert(key.clone(), Value::Null);
                    }
                }
                errors.extend(resp.errors);
            }
            Err(err) => {
                let (message, ext_code) = describe(&err);
                for key in &plan.response_keys {
                    data.insert(key.clone(), Value::Null);
                    errors.push(
                        GraphQLError::message(message.clone())
                            .with_path(vec![Value::String(key.clone())])
                            .with_extensions(serde_json::json!({
                                "code": ext_code,
                                "subgraph": plan.subgraph,
                            })),
                    );
                }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::project::PerSubgraphPlan;
    use std::time::Duration;

    fn plan(subgraph: &str, keys: &[&str]) -> PerSubgraphPlan {
        PerSubgraphPlan {
            subgraph: subgraph.to_owned(),
            query_text: String::from("{ stub }"),
            response_keys: keys.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn merges_successful_subgraph_responses_and_preserves_their_errors() {
        let property_resp = GraphQLResponse {
            data: Some(serde_json::json!({ "searchProperties": [{"id": "1"}] })),
            errors: vec![GraphQLError::message("warning from property")],
            extensions: None,
        };
        let review_resp = GraphQLResponse {
            data: Some(serde_json::json!({ "reviewSummary": {"count": 5} })),
            errors: Vec::new(),
            extensions: None,
        };

        let merged = merge(vec![
            SubgraphResult {
                plan: plan("property", &["searchProperties"]),
                duration: Duration::from_millis(12),
                outcome: Ok(property_resp),
            },
            SubgraphResult {
                plan: plan("review", &["reviewSummary"]),
                duration: Duration::from_millis(8),
                outcome: Ok(review_resp),
            },
        ]);

        let data = merged.response.data.unwrap();
        assert_eq!(data["searchProperties"][0]["id"], "1");
        assert_eq!(data["reviewSummary"]["count"], 5);
        assert_eq!(merged.response.errors.len(), 1);
        assert_eq!(merged.response.errors[0].message, "warning from property");
        assert_eq!(merged.subgraph_durations_ms.len(), 2);
    }

    #[test]
    fn timeout_yields_null_data_and_path_scoped_error() {
        let merged = merge(vec![
            SubgraphResult {
                plan: plan("property", &["searchProperties"]),
                duration: Duration::from_millis(5),
                outcome: Ok(GraphQLResponse {
                    data: Some(serde_json::json!({ "searchProperties": [{"id": "1"}] })),
                    errors: Vec::new(),
                    extensions: None,
                }),
            },
            SubgraphResult {
                plan: plan("pricing", &["price"]),
                duration: Duration::from_millis(1000),
                outcome: Err(SubgraphError::Timeout(Duration::from_millis(1000))),
            },
        ]);

        let data = merged.response.data.unwrap();
        assert_eq!(data["searchProperties"][0]["id"], "1");
        assert!(data["price"].is_null(), "price should be null when its subgraph timed out");

        assert_eq!(merged.response.errors.len(), 1);
        let err = &merged.response.errors[0];
        assert_eq!(err.path, vec![serde_json::Value::String("price".into())]);
        assert!(err.message.contains("timed out"));
        let ext = err.extensions.as_ref().unwrap();
        assert_eq!(ext["code"], "SUBGRAPH_TIMEOUT");
        assert_eq!(ext["subgraph"], "pricing");
    }
}
