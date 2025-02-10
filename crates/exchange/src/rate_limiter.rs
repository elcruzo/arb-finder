use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, Instant};
use tracing::debug;

#[derive(Debug)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    permits_per_window: u32,
    window_duration: Duration,
    last_reset: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    pub fn new(permits_per_window: u32, window_duration: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(permits_per_window as usize)),
            permits_per_window,
            window_duration,
            last_reset: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub async fn acquire(&self) {
        self.maybe_reset_window().await;
        
        let _permit = self.semaphore.acquire().await.unwrap();
        debug!("Rate limiter permit acquired");
        
        // Permit is automatically released when dropped
    }

    pub async fn try_acquire(&self) -> bool {
        self.maybe_reset_window().await;
        
        match self.semaphore.try_acquire() {
            Ok(_permit) => {
                debug!("Rate limiter permit acquired (non-blocking)");
                true
            }
            Err(_) => {
                debug!("Rate limiter permit unavailable");
                false
            }
        }
    }

    pub fn available_permits(&self) -> u32 {
        self.semaphore.available_permits() as u32
    }

    async fn maybe_reset_window(&self) {
        let mut last_reset = self.last_reset.lock().await;
        let now = Instant::now();
        
        if now.duration_since(*last_reset) >= self.window_duration {
            // Reset the window
            let used_permits = self.permits_per_window - self.semaphore.available_permits() as u32;
            if used_permits > 0 {
                self.semaphore.add_permits(used_permits as usize);
            }
            *last_reset = now;
            debug!("Rate limiter window reset, {} permits restored", used_permits);
        }
    }
}

#[derive(Debug)]
pub struct TokenBucket {
    tokens: Arc<Mutex<f64>>,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Arc<Mutex<Instant>>,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(capacity)),
            capacity,
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub async fn acquire(&self, tokens_needed: f64) -> bool {
        self.refill().await;
        
        let mut tokens = self.tokens.lock().await;
        if *tokens >= tokens_needed {
            *tokens -= tokens_needed;
            debug!("Token bucket: {} tokens consumed, {} remaining", tokens_needed, *tokens);
            true
        } else {
            debug!("Token bucket: insufficient tokens ({} needed, {} available)", tokens_needed, *tokens);
            false
        }
    }

    pub async fn acquire_blocking(&self, tokens_needed: f64) {
        loop {
            if self.acquire(tokens_needed).await {
                break;
            }
            
            // Calculate wait time
            let wait_time = Duration::from_secs_f64(tokens_needed / self.refill_rate);
            debug!("Token bucket: waiting {:?} for tokens", wait_time);
            sleep(wait_time).await;
        }
    }

    pub async fn available_tokens(&self) -> f64 {
        self.refill().await;
        *self.tokens.lock().await
    }

    async fn refill(&self) {
        let mut last_refill = self.last_refill.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();
        
        if elapsed > 0.0 {
            let mut tokens = self.tokens.lock().await;
            let new_tokens = elapsed * self.refill_rate;
            *tokens = (*tokens + new_tokens).min(self.capacity);
            *last_refill = now;
            
            if new_tokens > 0.0 {
                debug!("Token bucket: refilled {} tokens, {} total", new_tokens, *tokens);
            }
        }
    }
}

#[derive(Debug)]
pub struct AdaptiveRateLimiter {
    base_permits: u32,
    current_permits: Arc<Mutex<u32>>,
    window_duration: Duration,
    semaphore: Arc<Mutex<Arc<Semaphore>>>,
    success_count: Arc<Mutex<u32>>,
    error_count: Arc<Mutex<u32>>,
    last_reset: Arc<Mutex<Instant>>,
    adjustment_factor: f64,
}

impl AdaptiveRateLimiter {
    pub fn new(base_permits: u32, window_duration: Duration) -> Self {
        Self {
            base_permits,
            current_permits: Arc::new(Mutex::new(base_permits)),
            window_duration,
            semaphore: Arc::new(Mutex::new(Arc::new(Semaphore::new(base_permits as usize)))),
            success_count: Arc::new(Mutex::new(0)),
            error_count: Arc::new(Mutex::new(0)),
            last_reset: Arc::new(Mutex::new(Instant::now())),
            adjustment_factor: 0.1,
        }
    }

    pub async fn acquire(&self) -> Result<(), ()> {
        self.maybe_adjust_rate().await;
        
        let semaphore = {
            let guard = self.semaphore.lock().await;
            Arc::clone(&*guard)
        };
        
        let _permit = semaphore.acquire().await.map_err(|_| ())?;
        debug!("Adaptive rate limiter permit acquired");
        Ok(())
    }

    pub async fn record_success(&self) {
        let mut success_count = self.success_count.lock().await;
        *success_count += 1;
    }

    pub async fn record_error(&self) {
        let mut error_count = self.error_count.lock().await;
        *error_count += 1;
    }

    async fn maybe_adjust_rate(&self) {
        let mut last_reset = self.last_reset.lock().await;
        let now = Instant::now();
        
        if now.duration_since(*last_reset) >= self.window_duration {
            let success_count = {
                let mut count = self.success_count.lock().await;
                let value = *count;
                *count = 0;
                value
            };
            
            let error_count = {
                let mut count = self.error_count.lock().await;
                let value = *count;
                *count = 0;
                value
            };
            
            let total_requests = success_count + error_count;
            if total_requests > 0 {
                let error_rate = error_count as f64 / total_requests as f64;
                
                let mut current_permits = self.current_permits.lock().await;
                let new_permits = if error_rate > 0.1 {
                    // Too many errors, reduce rate
                    ((*current_permits as f64) * (1.0 - self.adjustment_factor)).max(1.0) as u32
                } else if error_rate < 0.05 && success_count > *current_permits / 2 {
                    // Low error rate and good utilization, increase rate
                    ((*current_permits as f64) * (1.0 + self.adjustment_factor))
                        .min(self.base_permits as f64 * 2.0) as u32
                } else {
                    *current_permits
                };
                
                if new_permits != *current_permits {
                    *current_permits = new_permits;
                    let mut semaphore_guard = self.semaphore.lock().await;
                    *semaphore_guard = Arc::new(Semaphore::new(new_permits as usize));
                    debug!("Adaptive rate limiter adjusted to {} permits/window", new_permits);
                }
            }
            
            *last_reset = now;
        }
    }

    pub async fn current_rate(&self) -> u32 {
        *self.current_permits.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));
        
        // Should acquire first permit immediately
        limiter.acquire().await;
        assert_eq!(limiter.available_permits(), 1);
        
        // Should acquire second permit immediately
        limiter.acquire().await;
        assert_eq!(limiter.available_permits(), 0);
        
        // Should not acquire third permit immediately
        assert!(!limiter.try_acquire().await);
        
        // Wait for window reset
        sleep(Duration::from_millis(150)).await;
        
        // Should be able to acquire again
        assert!(limiter.try_acquire().await);
    }

    #[tokio::test]
    async fn test_token_bucket() {
        let bucket = TokenBucket::new(5.0, 2.0); // 5 tokens capacity, 2 tokens/sec refill
        
        // Should consume tokens
        assert!(bucket.acquire(3.0).await);
        assert_eq!(bucket.available_tokens().await, 2.0);
        
        // Should not have enough tokens
        assert!(!bucket.acquire(3.0).await);
        
        // Wait for refill
        sleep(Duration::from_millis(1000)).await;
        assert!(bucket.available_tokens().await >= 4.0);
    }

    #[tokio::test]
    async fn test_adaptive_rate_limiter() {
        let limiter = AdaptiveRateLimiter::new(5, Duration::from_millis(100));
        
        assert_eq!(limiter.current_rate().await, 5);
        
        // Simulate errors
        for _ in 0..10 {
            limiter.record_error().await;
        }
        
        // Wait for adjustment
        sleep(Duration::from_millis(150)).await;
        limiter.acquire().await.ok(); // Trigger adjustment
        
        // Rate should be reduced
        assert!(limiter.current_rate().await < 5);
    }
}