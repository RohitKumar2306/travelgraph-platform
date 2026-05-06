//! Redis-backed full-response cache (Phase 4.2).

use crate::config::CacheConfig;
use crate::metrics;
use crate::plan::ExecutionPlan;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

pub struct ResponseCache {
    conn: ConnectionManager,
    cfg: CacheConfig,
}

impl ResponseCache {
    pub async fn connect(cfg: &CacheConfig) -> anyhow::Result<Self> {
        let client = redis::Client::open(cfg.redis_url.as_str())
            .map_err(|e| anyhow::anyhow!("redis client: {e}"))?;
        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| anyhow::anyhow!("redis connection: {e}"))?;
        Ok(Self {
            conn,
            cfg: cfg.clone(),
        })
    }

    pub fn into_shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// SHA-256 hex key over `(document, operationName, variables JSON, identity)`.
    pub fn cache_key(
        document: &str,
        operation_name: Option<&str>,
        variables: &Value,
        identity: &str,
    ) -> String {
        let vars = serde_json::to_string(variables).unwrap_or_else(|_| "{}".to_string());
        let op = operation_name.unwrap_or("");
        let mut h = Sha256::new();
        h.update(document.as_bytes());
        h.update(b"\x1e");
        h.update(op.as_bytes());
        h.update(b"\x1e");
        h.update(vars.as_bytes());
        h.update(b"\x1e");
        h.update(identity.as_bytes());
        hex::encode(h.finalize())
    }

    pub async fn get_json(&self, key: &str) -> redis::RedisResult<Option<Vec<u8>>> {
        let mut c = self.conn.clone();
        c.get(key).await
    }

    pub async fn set_json(&self, key: &str, ttl: Duration, body: &[u8]) -> redis::RedisResult<()> {
        let mut c = self.conn.clone();
        c.set_ex::<_, _, ()>(key, body, ttl.as_secs().max(1)).await
    }

    pub fn ttl_for_plan(&self, plan: &ExecutionPlan) -> Duration {
        let mut secs: u64 = u64::MAX;
        for f in &plan.field_fetches {
            let key = f.response_key.as_str();
            if key == "searchProperties" {
                secs = secs.min(self.cfg.ttl_search_sec);
            }
            if key == "property" {
                secs = secs.min(self.cfg.ttl_property_read_sec);
            }
            if key == "reviewSummary" {
                secs = secs.min(self.cfg.ttl_review_summary_sec);
            }
            for e in &f.entity_fetches {
                if e.subgraph == "pricing" {
                    secs = secs.min(self.cfg.ttl_pricing_sec);
                }
            }
        }
        if secs == u64::MAX {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(secs.max(1))
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled && !self.cfg.redis_url.is_empty()
    }
}

/// `Ok(Some(body))` on cache hit after recording hit metric.
pub async fn try_get(
    cache: Option<&Arc<ResponseCache>>,
    key: &str,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(c) = cache.filter(|x| x.enabled()) else {
        return Ok(None);
    };
    match c.get_json(key).await {
        Ok(Some(bytes)) => {
            metrics::record_cache("hit");
            Ok(Some(bytes))
        }
        Ok(None) => {
            metrics::record_cache("miss");
            Ok(None)
        }
        Err(e) => Err(anyhow::anyhow!("redis GET: {e}")),
    }
}

pub async fn try_set(
    cache: Option<&Arc<ResponseCache>>,
    key: &str,
    ttl: Duration,
    body: &[u8],
) -> anyhow::Result<()> {
    let Some(c) = cache.filter(|x| x.enabled()) else {
        return Ok(());
    };
    c.set_json(key, ttl, body)
        .await
        .map_err(|e| anyhow::anyhow!("redis SET: {e}"))?;
    Ok(())
}
