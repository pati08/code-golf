use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use moka::future::Cache;

#[derive(Clone)]
pub struct RateLimiter {
    cache: Cache<String, Arc<AtomicU32>>,
    limit: u32,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(window)
            .build();
        Self { cache, limit }
    }

    /// Returns true if the request is allowed, false if it exceeds the limit.
    pub async fn check(&self, key: impl Into<String>) -> bool {
        let counter = self
            .cache
            .get_with(key.into(), async { Arc::new(AtomicU32::new(0)) })
            .await;
        let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
        count <= self.limit
    }
}

#[derive(Clone)]
pub struct RateLimiters {
    pub login: RateLimiter,
    pub register: RateLimiter,
    pub submit: RateLimiter,
    pub feedback: RateLimiter,
}

impl RateLimiters {
    pub fn new() -> Self {
        Self {
            login: RateLimiter::new(5, Duration::from_secs(60)),
            register: RateLimiter::new(3, Duration::from_secs(3600)),
            submit: RateLimiter::new(10, Duration::from_secs(60)),
            feedback: RateLimiter::new(5, Duration::from_secs(3600)),
        }
    }
}
