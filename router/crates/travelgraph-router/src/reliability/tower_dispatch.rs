use crate::config::ReliabilityConfig;
use crate::error::SubgraphError;
use crate::graphql::types::GraphQLResponse;
use crate::plan::OperationKind;
use crate::reliability::circuit::CircuitBreaker;
use futures::future::BoxFuture;
use reqwest::Url;
use std::collections::HashMap;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::Service;
use tower::ServiceExt;

/// One HTTP POST to a subgraph (`/graphql`).
#[derive(Debug, Clone)]
pub struct SubgraphHttpCall {
    pub subgraph: String,
    pub url: String,
    pub body: Vec<u8>,
    pub timeout: Duration,
    pub operation: OperationKind,
}

#[derive(Clone)]
pub struct SubgraphClient {
    http: reqwest::Client,
    breakers: Arc<HashMap<String, Arc<CircuitBreaker>>>,
    policy: RetryPolicy,
}

#[derive(Clone)]
struct RetryPolicy {
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl SubgraphClient {
    pub fn new(
        http: reqwest::Client,
        breakers: Arc<HashMap<String, Arc<CircuitBreaker>>>,
        cfg: &ReliabilityConfig,
    ) -> Self {
        Self {
            http,
            breakers,
            policy: RetryPolicy {
                max_retries: cfg.retry_max_attempts,
                initial_backoff: Duration::from_millis(cfg.retry_initial_backoff_ms),
                max_backoff: Duration::from_millis(cfg.retry_max_backoff_ms),
            },
        }
    }

    /// Tower entrypoint — identical to [`Service::call`].
    pub async fn send(&self, call: SubgraphHttpCall) -> Result<GraphQLResponse, SubgraphError> {
        self.clone().oneshot(call).await
    }
}

impl Service<SubgraphHttpCall> for SubgraphClient {
    type Response = GraphQLResponse;
    type Error = SubgraphError;
    type Future = BoxFuture<'static, Result<GraphQLResponse, SubgraphError>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, call: SubgraphHttpCall) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.dispatch_with_reliability(call).await })
    }
}

impl SubgraphClient {
    async fn dispatch_with_reliability(
        &self,
        call: SubgraphHttpCall,
    ) -> Result<GraphQLResponse, SubgraphError> {
        let breaker = self
            .breakers
            .get(&call.subgraph)
            .cloned()
            .ok_or_else(|| {
                SubgraphError::Transport(format!(
                    "no circuit breaker for subgraph \"{}\"",
                    call.subgraph
                ))
            })?;

        let max_attempts = 1 + self.policy.max_retries;

        for attempt in 0..max_attempts {
            breaker.check()?;

            let outcome = Self::single_attempt(&self.http, &call).await;

            match outcome {
                Ok(resp) => {
                    // Successful HTTP + JSON parse: count as breaker success.
                    breaker.record(true);
                    return Ok(resp);
                }
                Err(e) => {
                    breaker.record(false);
                    let can_retry = matches!(call.operation, OperationKind::Query)
                        && attempt + 1 < max_attempts
                        && retryable_error(&e);
                    if can_retry {
                        let backoff =
                            Self::backoff(attempt, self.policy.initial_backoff, self.policy.max_backoff);
                        tracing::debug!(
                            subgraph = %call.subgraph,
                            attempt,
                            ?backoff,
                            "subgraph retry after transient error"
                        );
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(SubgraphError::Transport(
            "exhausted subgraph retries without success".into(),
        ))
    }

    fn backoff(attempt: u32, initial: Duration, cap: Duration) -> Duration {
        let factor = 2u32.saturating_pow(attempt);
        let ms = initial.as_millis().saturating_mul(factor as u128);
        Duration::from_millis(ms.min(cap.as_millis()) as u64)
    }

    async fn single_attempt(
        http: &reqwest::Client,
        call: &SubgraphHttpCall,
    ) -> Result<GraphQLResponse, SubgraphError> {
        let url = Url::parse(&call.url)
            .map_err(|e| SubgraphError::Transport(format!("invalid subgraph URL: {e}")))?;
        let request = http
            .post(url)
            .header("content-type", "application/json")
            .body(call.body.clone());

        let result = tokio::time::timeout(call.timeout, request.send()).await;

        let response = match result {
            Err(_elapsed) => return Err(SubgraphError::Timeout(call.timeout)),
            Ok(Err(e)) => return Err(SubgraphError::Transport(e.to_string())),
            Ok(Ok(r)) => r,
        };

        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| SubgraphError::Transport(e.to_string()))?;

        if !status.is_success() {
            return Err(SubgraphError::BadStatus {
                status: status.as_u16(),
                body: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }

        serde_json::from_slice::<GraphQLResponse>(&bytes)
            .map_err(|e| SubgraphError::Decode(e.to_string()))
    }
}

fn retryable_error(e: &SubgraphError) -> bool {
    match e {
        SubgraphError::Timeout(_) => true,
        SubgraphError::Transport(_) => true,
        SubgraphError::BadStatus { status, .. } => {
            matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
        }
        SubgraphError::Decode(_) | SubgraphError::CircuitOpen { .. } => false,
    }
}
