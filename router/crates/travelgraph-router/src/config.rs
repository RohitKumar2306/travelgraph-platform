//! Router configuration loaded at startup from a TOML file.
//!
//! Phase 3 changes the schema: instead of a hand-coded `[subgraphs.NAME]`
//! map, the router reads the composed supergraph at `[supergraph].path`
//! and learns subgraph URLs from `enum join__Graph`. The TOML now only
//! holds operational settings (port, default timeout, optional per-subgraph
//! overrides).
//!
//! Source resolution order:
//!   1. `ROUTER_CONFIG` environment variable (path to a .toml file)
//!   2. `./config/router.toml` (relative to the working directory)
//!
//! Example:
//! ```toml
//! [server]
//! port = 8080
//! default_subgraph_timeout_ms = 1000
//!
//! [supergraph]
//! path = "/app/supergraph/supergraph.graphql"
//!
//! [timeouts]
//! pricing = 1500
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: Server,
    pub supergraph: SupergraphPath,
    /// Optional per-subgraph timeout overrides (key = subgraph name as
    /// declared in the supergraph's `enum join__Graph`).
    #[serde(default)]
    pub timeouts: HashMap<String, u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub port: u16,
    /// Default per-subgraph HTTP timeout when no override is configured.
    /// 1000ms matches Phase 2.4 acceptance.
    #[serde(default = "default_timeout_ms")]
    pub default_subgraph_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupergraphPath {
    pub path: PathBuf,
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
}
