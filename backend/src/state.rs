use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::Client;
use sqlx::PgPool;
use tokio::sync::Mutex;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pool: PgPool,
    pub http: Client,
    pub rate_limiter: Arc<RateLimiter>,
    pub started_at: Instant,
}

impl AppState {
    pub fn new(config: AppConfig, pool: PgPool) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client configuration is valid");

        Self {
            config: Arc::new(config),
            pool,
            http,
            rate_limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(15 * 60))),
            started_at: Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(_max_requests: usize, _window: Duration) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub async fn check_with_limit(
        &self,
        key: impl Into<String>,
        max_requests: usize,
        window: Duration,
    ) -> bool {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().await;
        let bucket = buckets.entry(key.into()).or_default();
        bucket.retain(|seen_at| now.duration_since(*seen_at) < window);
        if bucket.len() >= max_requests {
            return false;
        }
        bucket.push(now);

        if buckets.len() > 10_000 {
            buckets.retain(|_, seen| seen.iter().any(|t| now.duration_since(*t) < window));
        }

        true
    }
}
