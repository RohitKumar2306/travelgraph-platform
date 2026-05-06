//! GraphQL request / response shapes following the spec
//! (<https://spec.graphql.org/October2021/#sec-Response-Format>).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// HTTP body for `POST /graphql`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "operationName")]
    pub operation_name: Option<String>,
    /// Default to an empty object so subgraphs that strictly type the
    /// `variables` field never see `null` (which some validators reject).
    #[serde(default = "empty_object")]
    pub variables: Value,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

/// HTTP body for `POST /graphql` responses, per the GraphQL spec.
///
/// `data` is `Some(Value::Null)` when an executed top-level field failed
/// (the spec distinguishes "field is null" from "no data was attempted").
/// `data` is `None` when the request never executed (parse / validation
/// failure).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GraphQLResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<GraphQLError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphQLError {
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

impl GraphQLError {
    pub fn message(message: impl Into<String>) -> Self {
        GraphQLError {
            message: message.into(),
            locations: Vec::new(),
            path: Vec::new(),
            extensions: None,
        }
    }

    pub fn with_path(mut self, path: Vec<Value>) -> Self {
        self.path = path;
        self
    }

    pub fn with_extensions(mut self, ext: Value) -> Self {
        self.extensions = Some(ext);
        self
    }
}

/// `{ "line": 1, "column": 5 }` per the GraphQL spec.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}
