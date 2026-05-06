//! Per-client GraphQL rate limiting (Phase 4.4) using [`governor`] token buckets.

use crate::config::RateLimitConfig;
use dashmap::DashMap;
use governor::clock::{Clock, DefaultClock};
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

type Bucket = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

pub struct ClientRateLimiter {
    default_rps: u32,
    default_burst: u32,
    overrides: HashMap<String, (u32, u32)>,
    limiters: DashMap<String, Arc<Bucket>>,
}

impl ClientRateLimiter {
    pub fn new(cfg: &RateLimitConfig) -> Self {
        let mut overrides = HashMap::new();
        for (name, o) in &cfg.clients {
            overrides.insert(name.clone(), (o.requests_per_sec, o.burst));
        }
        Self {
            default_rps: cfg.default_requests_per_sec,
            default_burst: cfg.default_burst,
            overrides,
            limiters: DashMap::new(),
        }
    }

    fn quota(rps: u32, burst: u32) -> Quota {
        let rps = NonZeroU32::new(rps.max(1)).expect("rps");
        let burst = NonZeroU32::new(burst.max(1)).expect("burst");
        Quota::per_second(rps).allow_burst(burst)
    }

    fn limiter_for(&self, client: &str) -> Arc<Bucket> {
        self.limiters
            .entry(client.to_string())
            .or_insert_with(|| {
                let (rps, burst) = self
                    .overrides
                    .get(client)
                    .copied()
                    .unwrap_or((self.default_rps, self.default_burst));
                let q = Self::quota(rps, burst);
                Arc::new(RateLimiter::direct(q))
            })
            .clone()
    }

    /// `Ok(())` when allowed; `Err` carries a delay suitable for `Retry-After`.
    pub fn check(&self, client: &str) -> Result<(), Duration> {
        let lim = self.limiter_for(client);
        match lim.check() {
            Ok(()) => Ok(()),
            Err(not_until) => Err(not_until.wait_time_from(DefaultClock::default().now())),
        }
    }
}
