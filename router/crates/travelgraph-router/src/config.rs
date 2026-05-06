//! Router configuration loaded at startup from a TOML file.
//!
//! Source resolution order:
//!   1. `ROUTER_CONFIG` environment variable (path to a .toml file)
//!   2. `./config/router.toml` (relative to the working directory)
//!
//! Example:
//! ```toml
//! [server]
//! port = 8080
//!
//! [subgraphs.property]
//! url        = "http://property-service:8081/graphql"
//! fields     = ["property", "searchProperties"]
//! mutations  = []
//! timeout_ms = 1000
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: Server,

    /// Map of subgraph name -> definition. The key is purely a label used in
    /// metrics / logging; routing decisions use the [`SubgraphConfig::fields`]
    /// and [`SubgraphConfig::mutations`] lists.
    pub subgraphs: HashMap<String, SubgraphConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub port: u16,
    /// Default per-subgraph HTTP timeout when a subgraph entry omits it.
    /// 1000ms matches Phase 2.4 acceptance.
    #[serde(default = "default_timeout_ms")]
    pub default_subgraph_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubgraphConfig {
    pub url: String,
    /// Top-level Query field names this subgraph owns.
    #[serde(default)]
    pub fields: Vec<String>,
    /// Top-level Mutation field names this subgraph owns.
    #[serde(default)]
    pub mutations: Vec<String>,
    /// Per-subgraph timeout override.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

fn default_timeout_ms() -> u64 {
    1000
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("ROUTER_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config/router.toml"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading config from {}: {e}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parsing config from {}: {e}", path.display()))?;
        Ok(cfg)
    }

    /// Returns the effective per-subgraph timeout, falling back to the
    /// server-level default when the subgraph itself has no override.
    pub fn timeout_for(&self, subgraph: &SubgraphConfig) -> Duration {
        Duration::from_millis(
            subgraph
                .timeout_ms
                .unwrap_or(self.server.default_subgraph_timeout_ms),
        )
    }
}
