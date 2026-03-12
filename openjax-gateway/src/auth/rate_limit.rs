use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SlidingWindowRateLimiter {
    limit: usize,
    window: Duration,
    buckets: HashMap<String, VecDeque<Instant>>,
}

impl SlidingWindowRateLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            buckets: HashMap::new(),
        }
    }

    pub fn allow(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let bucket = self.buckets.entry(key.to_string()).or_default();
        while let Some(ts) = bucket.front() {
            if now.duration_since(*ts) <= self.window {
                break;
            }
            let _ = bucket.pop_front();
        }
        if bucket.len() >= self.limit {
            return false;
        }
        bucket.push_back(now);
        true
    }
}
