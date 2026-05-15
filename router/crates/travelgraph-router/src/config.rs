//! Router configuration loaded from TOML (Phase 3 + Phase 4).

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: Server,
    pub supergraph: SupergraphPath,
    /// Per-subgraph HTTP timeout overrides (ms). Keys match `@join__graph(name: "...")`.
    #[serde(default)]
    pub timeouts: HashMap<String, u64>,
    #[serde(default)]
    pub http_client: HttpClientConfig,
    #[serde(default)]
    pub reliability: ReliabilityConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub persisted_queries: PersistedQueriesConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub port: u16,
    /// Fallback when a subgraph has no `[timeouts]` entry.
    #[serde(default = "default_timeout_ms")]
    pub default_subgraph_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupergraphPath {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HttpClientConfig {
    #[serde(default = "default_pool_max_idle")]
    pub pool_max_idle_per_host: usize,
    /// Router "keep-alive" tuning: idle socket lifetime in the pool.
    #[serde(default = "default_pool_idle_sec")]
    pub pool_idle_timeout_sec: u64,
    #[serde(default = "default_tcp_keepalive_sec")]
    pub tcp_keepalive_sec: u64,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            pool_max_idle_per_host: default_pool_max_idle(),
            pool_idle_timeout_sec: default_pool_idle_sec(),
            tcp_keepalive_sec: default_tcp_keepalive_sec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReliabilityConfig {
    /// Extra attempts after the first (max total = 1 + retry_max_attempts).
    /// Phase 4.1: "up to 2 times" => 2 retries here.
    #[serde(default = "default_retry_max")]
    pub retry_max_attempts: u32,
    #[serde(default = "default_retry_initial_backoff_ms")]
    pub retry_initial_backoff_ms: u64,
    #[serde(default = "default_retry_max_backoff_ms")]
    pub retry_max_backoff_ms: u64,
    #[serde(default = "default_circuit_window_sec")]
    pub circuit_window_sec: u64,
    #[serde(default = "default_circuit_min_samples")]
    pub circuit_min_samples: usize,
    #[serde(default = "default_circuit_failure_ratio")]
    pub circuit_failure_ratio: f64,
    #[serde(default = "default_circuit_open_recovery_sec")]
    pub circuit_open_recovery_sec: u64,
}

impl Default for ReliabilityConfig {
    fn default() -> Self {
        Self {
            retry_max_attempts: default_retry_max(),
            retry_initial_backoff_ms: default_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_retry_max_backoff_ms(),
            circuit_window_sec: default_circuit_window_sec(),
            circuit_min_samples: default_circuit_min_samples(),
            circuit_failure_ratio: default_circuit_failure_ratio(),
            circuit_open_recovery_sec: default_circuit_open_recovery_sec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub redis_url: String,
    #[serde(default = "default_ttl_property_read_sec")]
    pub ttl_property_read_sec: u64,
    #[serde(default = "default_ttl_search_sec")]
    pub ttl_search_sec: u64,
    #[serde(default = "default_ttl_pricing_sec")]
    pub ttl_pricing_sec: u64,
    #[serde(default = "default_ttl_review_summary_sec")]
    pub ttl_review_summary_sec: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            redis_url: String::new(),
            ttl_property_read_sec: default_ttl_property_read_sec(),
            ttl_search_sec: default_ttl_search_sec(),
            ttl_pricing_sec: default_ttl_pricing_sec(),
            ttl_review_summary_sec: default_ttl_review_summary_sec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_max_cost")]
    pub max_cost: u32,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_depth: default_max_depth(),
            max_cost: default_max_cost(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rl_rps")]
    pub default_requests_per_sec: u32,
    #[serde(default = "default_rl_burst")]
    pub default_burst: u32,
    #[serde(default)]
    pub clients: HashMap<String, ClientRateLimitOverride>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            default_requests_per_sec: default_rl_rps(),
            default_burst: default_rl_burst(),
            clients: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientRateLimitOverride {
    pub requests_per_sec: u32,
    #[serde(default = "default_rl_burst")]
    pub burst: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_identity_signature_secret")]
    pub identity_signature_secret: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            identity_signature_secret: default_identity_signature_secret(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersistedQueriesConfig {
    #[serde(default = "default_persisted_queries_path")]
    pub path: PathBuf,
    #[serde(default)]
    pub allow_arbitrary_queries: bool,
}

impl Default for PersistedQueriesConfig {
    fn default() -> Self {
        Self {
            path: default_persisted_queries_path(),
            allow_arbitrary_queries: false,
        }
    }
}

fn default_timeout_ms() -> u64 {
    1000
}

fn default_pool_max_idle() -> usize {
    50
}

fn default_pool_idle_sec() -> u64 {
    90
}

fn default_tcp_keepalive_sec() -> u64 {
    90
}

fn default_retry_max() -> u32 {
    2
}

fn default_retry_initial_backoff_ms() -> u64 {
    10
}

fn default_retry_max_backoff_ms() -> u64 {
    250
}

fn default_circuit_window_sec() -> u64 {
    60
}

fn default_circuit_min_samples() -> usize {
    8
}

fn default_circuit_failure_ratio() -> f64 {
    0.5
}

fn default_circuit_open_recovery_sec() -> u64 {
    15
}

fn default_ttl_property_read_sec() -> u64 {
    600
}

fn default_ttl_search_sec() -> u64 {
    120
}

fn default_ttl_pricing_sec() -> u64 {
    30
}

fn default_ttl_review_summary_sec() -> u64 {
    300
}

fn default_max_depth() -> usize {
    10
}

fn default_max_cost() -> u32 {
    1000
}

fn default_rl_rps() -> u32 {
    100
}

fn default_rl_burst() -> u32 {
    100
}

fn default_jwt_secret() -> String {
    "travelgraph-dev-jwt-secret".to_string()
}

fn default_identity_signature_secret() -> String {
    "travelgraph-dev-identity-secret".to_string()
}

fn default_persisted_queries_path() -> PathBuf {
    PathBuf::from("persisted-queries.json")
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("ROUTER_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config/router.toml"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading config from {}: {e}", path.display()))?;
        let mut cfg: Config = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parsing config from {}: {e}", path.display()))?;
        if let Ok(url) = std::env::var("ROUTER_REDIS_URL") {
            if !url.is_empty() {
                cfg.cache.redis_url = url;
            }
        }
        if let Ok(v) = std::env::var("ROUTER_CACHE_ENABLED") {
            cfg.cache.enabled = matches!(v.to_lowercase().as_str(), "1" | "true" | "yes");
        }
        if let Ok(v) = std::env::var("ROUTER_JWT_SECRET") {
            if !v.is_empty() {
                cfg.auth.jwt_secret = v;
            }
        }
        if let Ok(v) = std::env::var("TRAVELGRAPH_IDENTITY_SECRET") {
            if !v.is_empty() {
                cfg.auth.identity_signature_secret = v;
            }
        }
        if let Ok(v) = std::env::var("ROUTER_PERSISTED_QUERIES_PATH") {
            if !v.is_empty() {
                cfg.persisted_queries.path = PathBuf::from(v);
            }
        }
        if let Ok(v) = std::env::var("ROUTER_ALLOW_ARBITRARY_QUERIES") {
            cfg.persisted_queries.allow_arbitrary_queries =
                matches!(v.to_lowercase().as_str(), "1" | "true" | "yes");
        }
        Ok(cfg)
    }

    /// Effective HTTP client pool / keep-alive knobs.
    pub fn http_pool_idle_timeout(&self) -> Duration {
        Duration::from_secs(self.http_client.pool_idle_timeout_sec)
    }

    pub fn http_tcp_keepalive(&self) -> Option<Duration> {
        if self.http_client.tcp_keepalive_sec == 0 {
            None
        } else {
            Some(Duration::from_secs(self.http_client.tcp_keepalive_sec))
        }
    }
}

/// Phase 4.1 default subgraph timeouts when no `[timeouts]` entry exists.
pub fn default_subgraph_timeouts_ms() -> HashMap<String, u64> {
    [
        ("property".to_string(), 300),
        ("pricing".to_string(), 500),
        ("booking".to_string(), 700),
        ("review".to_string(), 500),
        ("user".to_string(), 500),
    ]
    .into_iter()
    .collect()
}
