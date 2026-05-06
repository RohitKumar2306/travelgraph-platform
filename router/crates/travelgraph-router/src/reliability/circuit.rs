//! Sliding-window circuit breaker per subgraph (Phase 4.1).

use crate::error::SubgraphError;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Sample {
    at: Instant,
    success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub window: Duration,
    pub min_samples: usize,
    pub failure_ratio: f64,
    pub recovery: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            window: Duration::from_secs(60),
            min_samples: 8,
            failure_ratio: 0.5,
            recovery: Duration::from_secs(15),
        }
    }
}

pub struct CircuitBreaker {
    cfg: CircuitBreakerConfig,
    inner: Mutex<Inner>,
}

struct Inner {
    samples: VecDeque<Sample>,
    state: State,
}

impl CircuitBreaker {
    pub fn new(cfg: CircuitBreakerConfig) -> Self {
        Self {
            cfg,
            inner: Mutex::new(Inner {
                samples: VecDeque::new(),
                state: State::Closed,
            }),
        }
    }

    pub fn into_shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn check(&self) -> Result<(), SubgraphError> {
        let mut g = self.inner.lock();
        let now = Instant::now();
        Self::prune_static(&self.cfg, &mut g.samples, now);

        match g.state {
            State::Open { until } if now < until => Err(SubgraphError::CircuitOpen {
                retry_after: until.saturating_duration_since(now),
            }),
            State::Open { .. } => {
                g.state = State::HalfOpen;
                Ok(())
            }
            State::HalfOpen | State::Closed => Ok(()),
        }
    }

    pub fn record(&self, success: bool) {
        let mut g = self.inner.lock();
        let now = Instant::now();
        Self::prune_static(&self.cfg, &mut g.samples, now);
        g.samples.push_back(Sample { at: now, success });

        match g.state {
            State::HalfOpen => {
                if success {
                    g.state = State::Closed;
                } else {
                    g.state = State::Open {
                        until: now + self.cfg.recovery,
                    };
                }
            }
            State::Closed => {
                let n = g.samples.len();
                if n >= self.cfg.min_samples {
                    let fails = g.samples.iter().filter(|s| !s.success).count();
                    let ratio = fails as f64 / n as f64;
                    if ratio >= self.cfg.failure_ratio {
                        g.state = State::Open {
                            until: now + self.cfg.recovery,
                        };
                    }
                }
            }
            State::Open { .. } => { /* should not record while open (no requests) */ }
        }
    }

    fn prune_static(cfg: &CircuitBreakerConfig, samples: &mut VecDeque<Sample>, now: Instant) {
        let cutoff = now - cfg.window;
        while let Some(front) = samples.front() {
            if front.at < cutoff {
                samples.pop_front();
            } else {
                break;
            }
        }
    }
}

pub fn breakers_for_subgraphs(
    names: impl Iterator<Item = String>,
    cfg: CircuitBreakerConfig,
) -> HashMap<String, Arc<CircuitBreaker>> {
    names
        .map(|n| (n.clone(), CircuitBreaker::new(cfg.clone()).into_shared()))
        .collect()
}
