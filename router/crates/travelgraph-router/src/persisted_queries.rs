use crate::graphql::types::GraphQLError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct PersistedQueryStore {
    queries: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedQueryDecision<'a> {
    UseStored(&'a str),
    UseRequestQuery,
}

impl PersistedQueryStore {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!(
                "reading persisted query manifest from {}: {e}",
                path.display()
            )
        })?;
        let queries = serde_json::from_str::<HashMap<String, String>>(&text).map_err(|e| {
            anyhow::anyhow!(
                "parsing persisted query manifest from {}: {e}",
                path.display()
            )
        })?;
        Ok(Self { queries })
    }

    pub fn resolve<'a>(
        &'a self,
        request_query: Option<&str>,
        extensions: Option<&Value>,
        allow_arbitrary_queries: bool,
    ) -> Result<PersistedQueryDecision<'a>, GraphQLError> {
        if let Some(hash) = persisted_hash(extensions) {
            return self
                .queries
                .get(hash)
                .map(|query| PersistedQueryDecision::UseStored(query.as_str()))
                .ok_or_else(persisted_query_not_found);
        }

        if allow_arbitrary_queries && request_query.map(|q| !q.trim().is_empty()).unwrap_or(false) {
            return Ok(PersistedQueryDecision::UseRequestQuery);
        }

        Err(
            GraphQLError::message("PersistedQueryRequired").with_extensions(json!({
                "code": "PERSISTED_QUERY_REQUIRED"
            })),
        )
    }
}

fn persisted_hash(extensions: Option<&Value>) -> Option<&str> {
    extensions?
        .get("persistedQuery")?
        .get("sha256Hash")?
        .as_str()
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
}

fn persisted_query_not_found() -> GraphQLError {
    GraphQLError::message("PersistedQueryNotFound").with_extensions(json!({
        "code": "PERSISTED_QUERY_NOT_FOUND"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hash_uses_manifest_query() {
        let store = PersistedQueryStore {
            queries: HashMap::from([("abc".to_string(), "query { me { id } }".to_string())]),
        };
        let extensions = json!({"persistedQuery": {"version": 1, "sha256Hash": "abc"}});

        assert_eq!(
            store.resolve(None, Some(&extensions), false).unwrap(),
            PersistedQueryDecision::UseStored("query { me { id } }")
        );
    }

    #[test]
    fn unknown_hash_returns_apollo_retry_code() {
        let store = PersistedQueryStore::default();
        let extensions = json!({"persistedQuery": {"sha256Hash": "missing"}});

        let err = store
            .resolve(Some("query { me { id } }"), Some(&extensions), true)
            .unwrap_err();
        assert_eq!(err.extensions.unwrap()["code"], "PERSISTED_QUERY_NOT_FOUND");
    }

    #[test]
    fn arbitrary_query_requires_dev_flag() {
        let store = PersistedQueryStore::default();

        let err = store
            .resolve(Some("query { me { id } }"), None, false)
            .unwrap_err();
        assert_eq!(err.extensions.unwrap()["code"], "PERSISTED_QUERY_REQUIRED");
        assert_eq!(
            store
                .resolve(Some("query { me { id } }"), None, true)
                .unwrap(),
            PersistedQueryDecision::UseRequestQuery
        );
    }
}
