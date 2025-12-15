//! Rate limiting and other middleware.

use crate::error::ProxyError;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{num::NonZeroU32, sync::Arc};
use tracing::{debug, warn};

/// Global rate limiter (not keyed by IP).
pub type GlobalLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Rate limiter state shared across requests.
#[derive(Clone)]
pub struct RateLimitState {
    /// Global rate limiter for all requests
    pub global: Arc<GlobalLimiter>,
}

impl RateLimitState {
    /// Create a new rate limit state with the specified limits.
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(
            NonZeroU32::new(requests_per_minute).unwrap_or(NonZeroU32::new(10).unwrap()),
        );

        Self {
            global: Arc::new(RateLimiter::direct(quota)),
        }
    }

    /// Create a permissive rate limiter for testing.
    pub fn permissive() -> Self {
        Self::new(1000)
    }
}

/// Rate limiting middleware.
///
/// Checks the global rate limit and returns 429 Too Many Requests if exceeded.
pub async fn rate_limit_middleware(
    State(rate_limit): State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, ProxyError> {
    // Check global rate limit
    if rate_limit.global.check().is_err() {
        warn!("Global rate limit exceeded");
        return Err(ProxyError::RateLimitExceeded);
    }

    debug!("Rate limit check passed");
    Ok(next.run(request).await)
}

/// Logging middleware for requests.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    debug!(%method, %uri, "Request started");

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    if status.is_success() {
        debug!(%method, %uri, %status, ?duration, "Request completed");
    } else {
        warn!(%method, %uri, %status, ?duration, "Request failed");
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_state_creation() {
        let state = RateLimitState::new(10);
        // Should allow first request
        assert!(state.global.check().is_ok());
    }

    #[test]
    fn test_rate_limit_exhaustion() {
        // Very low limit for testing
        let state = RateLimitState::new(1);

        // First request should succeed
        assert!(state.global.check().is_ok());

        // Second request should fail (exceeded 1 per minute)
        assert!(state.global.check().is_err());
    }

    #[test]
    fn test_permissive_rate_limit() {
        let state = RateLimitState::permissive();
        // Should allow many requests
        for _ in 0..100 {
            assert!(state.global.check().is_ok());
        }
    }
}
