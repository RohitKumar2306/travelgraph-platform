//! Field-name -> subgraph registry.
//!
//! Phase 2 routes by hand: each top-level Query/Mutation field name has a
//! single owner declared in `config/router.toml`. Phase 3 will replace the
//! entire registry with a supergraph-driven query planner.

use apollo_compiler::ast::OperationType;
use std::collections::HashMap;
use std::time::Duration;

use crate::config::Config;

/// Static lookup table built from [`Config`] at startup.
#[derive(Debug, Clone)]
pub struct SubgraphRegistry {
    /// Lookup: query field name -> subgraph name.
    queries: HashMap<String, String>,
    /// Lookup: mutation field name -> subgraph name.
    mutations: HashMap<String, String>,
    /// Per-subgraph URL + timeout.
    by_name: HashMap<String, SubgraphRoute>,
}

#[derive(Debug, Clone)]
pub struct SubgraphRoute {
    pub url: String,
    pub timeout: Duration,
}

impl SubgraphRegistry {
    pub fn from_config(cfg: &Config) -> Result<Self, RegistryError> {
        let mut queries: HashMap<String, String> = HashMap::new();
        let mut mutations: HashMap<String, String> = HashMap::new();
        let mut by_name: HashMap<String, SubgraphRoute> = HashMap::new();

        for (name, sub) in &cfg.subgraphs {
            for q in &sub.fields {
                if let Some(prev) = queries.insert(q.clone(), name.clone()) {
                    return Err(RegistryError::DuplicateField {
                        field: q.clone(),
                        first: prev,
                        second: name.clone(),
                    });
                }
            }
            for m in &sub.mutations {
                if let Some(prev) = mutations.insert(m.clone(), name.clone()) {
                    return Err(RegistryError::DuplicateField {
                        field: m.clone(),
                        first: prev,
                        second: name.clone(),
                    });
                }
            }
            by_name.insert(
                name.clone(),
                SubgraphRoute {
                    url: sub.url.clone(),
                    timeout: cfg.timeout_for(sub),
                },
            );
        }

        Ok(SubgraphRegistry {
            queries,
            mutations,
            by_name,
        })
    }

    /// Returns the subgraph that owns the given top-level field, or `None`
    /// if no subgraph claims it. Subscriptions are explicitly out of scope.
    pub fn subgraph_for(&self, op: OperationType, field: &str) -> Option<&str> {
        match op {
            OperationType::Query => self.queries.get(field).map(String::as_str),
            OperationType::Mutation => self.mutations.get(field).map(String::as_str),
            OperationType::Subscription => None,
        }
    }

    pub fn route(&self, name: &str) -> Option<&SubgraphRoute> {
        self.by_name.get(name)
    }

    /// Iterate `(name, route)` pairs; useful for startup validation logs.
    #[allow(dead_code)]
    pub fn all(&self) -> impl Iterator<Item = (&String, &SubgraphRoute)> {
        self.by_name.iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error(
        "field \"{field}\" is owned by both subgraph \"{first}\" and subgraph \"{second}\""
    )]
    DuplicateField {
        field: String,
        first: String,
        second: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Server, SubgraphConfig};

    fn test_subgraph(url: &str, fields: &[&str]) -> SubgraphConfig {
        SubgraphConfig {
            url: url.to_owned(),
            fields: fields.iter().map(|s| s.to_string()).collect(),
            mutations: Vec::new(),
            timeout_ms: None,
        }
    }

    fn cfg() -> Config {
        let mut subgraphs = HashMap::new();
        subgraphs.insert(
            "property".to_owned(),
            test_subgraph("http://property:8081/graphql", &["property", "searchProperties"]),
        );
        subgraphs.insert(
            "review".to_owned(),
            test_subgraph("http://review:8085/graphql", &["reviews", "reviewSummary"]),
        );
        Config {
            server: Server {
                port: 8080,
                default_subgraph_timeout_ms: 1000,
            },
            subgraphs,
        }
    }

    #[test]
    fn maps_field_to_owner() {
        let r = SubgraphRegistry::from_config(&cfg()).unwrap();
        assert_eq!(r.subgraph_for(OperationType::Query, "searchProperties"), Some("property"));
        assert_eq!(r.subgraph_for(OperationType::Query, "reviewSummary"), Some("review"));
        assert_eq!(r.subgraph_for(OperationType::Query, "unknownField"), None);
    }

    #[test]
    fn detects_duplicate_field_ownership() {
        let mut bad = cfg();
        bad.subgraphs
            .get_mut("review")
            .unwrap()
            .fields
            .push("searchProperties".to_owned());
        let err = SubgraphRegistry::from_config(&bad).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("searchProperties"), "msg: {msg}");
    }
}
