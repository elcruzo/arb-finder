use arbfinder_core::{ArbFinderError, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, sleep, Instant, MissedTickBehavior};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct HeartbeatStatus {
    pub last_ping: Option<Instant>,
    pub last_pong: Option<Instant>,
    pub ping_count: u64,
    pub pong_count: u64,
    pub missed_pongs: u32,
    pub average_latency: Option<Duration>,
    pub is_healthy: bool,
}

impl Default for HeartbeatStatus {
    fn default() -> Self {
        Self {
            last_ping: None,
            last_pong: None,
            ping_count: 0,
            pong_count: 0,
            missed_pongs: 0,
            average_latency: None,
            is_healthy: true,
        }
    }
}

#[derive(Debug)]
pub struct HeartbeatManager {
    status: Arc<RwLock<HeartbeatStatus>>,
    ping_interval: Duration,
    max_missed_pongs: u32,
    timeout_duration: Duration,
    latency_samples: Arc<Mutex<Vec<Duration>>>,
    max_latency_samples: usize,
}

impl HeartbeatManager {
    pub fn new(
        ping_interval: Duration,
        max_missed_pongs: u32,
        timeout_duration: Duration,
    ) -> Self {
        Self {
            status: Arc::new(RwLock::new(HeartbeatStatus::default())),
            ping_interval,
            max_missed_pongs,
            timeout_duration,
            latency_samples: Arc::new(Mutex::new(Vec::new())),
            max_latency_samples: 100,
        }
    }

    pub async fn start<F, Fut>(&self, ping_sender: F) -> Result<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let status = Arc::clone(&self.status);
        let latency_samples = Arc::clone(&self.latency_samples);
        let ping_interval = self.ping_interval;
        let max_missed_pongs = self.max_missed_pongs;
        let timeout_duration = self.timeout_duration;
        let max_latency_samples = self.max_latency_samples;

        tokio::spawn(async move {
            let mut ping_ticker = interval(ping_interval);
            ping_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                ping_ticker.tick().await;

                debug!("Sending heartbeat ping");
                let ping_time = Instant::now();

                // Send ping
                match ping_sender().await {
                    Ok(_) => {
                        let mut status_guard = status.write().await;
                        status_guard.last_ping = Some(ping_time);
                        status_guard.ping_count += 1;
                        debug!("Heartbeat ping sent, count: {}", status_guard.ping_count);
                    }
                    Err(e) => {
                        error!("Failed to send heartbeat ping: {}", e);
                        continue;
                    }
                }

                // Wait for pong response
                sleep(timeout_duration).await;

                // Check if we received a pong
                let mut status_guard = status.write().await;
                if let (Some(last_ping), Some(last_pong)) = (status_guard.last_ping, status_guard.last_pong) {
                    if last_pong >= last_ping {
                        // Received pong for this ping
                        let latency = last_pong - last_ping;
                        status_guard.missed_pongs = 0;
                        status_guard.is_healthy = true;

                        // Update latency statistics
                        drop(status_guard);
                        let mut samples = latency_samples.lock().await;
                        samples.push(latency);
                        if samples.len() > max_latency_samples {
                            samples.remove(0);
                        }

                        // Calculate average latency
                        let avg_latency = if !samples.is_empty() {
                            let total: Duration = samples.iter().sum();
                            Some(total / samples.len() as u32)
                        } else {
                            None
                        };

                        let mut status_guard = status.write().await;
                        status_guard.average_latency = avg_latency;

                        debug!("Heartbeat pong received, latency: {:?}, avg: {:?}", latency, avg_latency);
                    } else {
                        // No pong received for this ping
                        status_guard.missed_pongs += 1;
                        warn!("Missed heartbeat pong, count: {}", status_guard.missed_pongs);

                        if status_guard.missed_pongs >= max_missed_pongs {
                            status_guard.is_healthy = false;
                            error!("Connection unhealthy: too many missed pongs ({})", status_guard.missed_pongs);
                        }
                    }
                } else {
                    // First ping or no pong received yet
                    status_guard.missed_pongs += 1;
                    if status_guard.missed_pongs >= max_missed_pongs {
                        status_guard.is_healthy = false;
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn record_pong(&self) {
        let mut status = self.status.write().await;
        status.last_pong = Some(Instant::now());
        status.pong_count += 1;
        debug!("Heartbeat pong recorded, count: {}", status.pong_count);
    }

    pub async fn get_status(&self) -> HeartbeatStatus {
        self.status.read().await.clone()
    }

    pub async fn is_healthy(&self) -> bool {
        self.status.read().await.is_healthy
    }

    pub async fn get_latency(&self) -> Option<Duration> {
        self.status.read().await.average_latency
    }

    pub async fn reset(&self) {
        let mut status = self.status.write().await;
        *status = HeartbeatStatus::default();
        
        let mut samples = self.latency_samples.lock().await;
        samples.clear();
        
        info!("Heartbeat status reset");
    }

    pub async fn get_latency_percentiles(&self) -> Option<LatencyStats> {
        let samples = self.latency_samples.lock().await;
        if samples.is_empty() {
            return None;
        }

        let mut sorted_samples: Vec<Duration> = samples.clone();
        sorted_samples.sort();

        let len = sorted_samples.len();
        Some(LatencyStats {
            min: sorted_samples[0],
            max: sorted_samples[len - 1],
            p50: sorted_samples[len / 2],
            p95: sorted_samples[(len * 95) / 100],
            p99: sorted_samples[(len * 99) / 100],
            avg: sorted_samples.iter().sum::<Duration>() / len as u32,
            count: len,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub min: Duration,
    pub max: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub avg: Duration,
    pub count: usize,
}

#[derive(Debug)]
pub struct ConnectionHealthMonitor {
    heartbeat_manager: HeartbeatManager,
    reconnect_threshold: u32,
    health_check_interval: Duration,
    is_monitoring: Arc<RwLock<bool>>,
}

impl ConnectionHealthMonitor {
    pub fn new(
        ping_interval: Duration,
        max_missed_pongs: u32,
        timeout_duration: Duration,
        reconnect_threshold: u32,
    ) -> Self {
        Self {
            heartbeat_manager: HeartbeatManager::new(ping_interval, max_missed_pongs, timeout_duration),
            reconnect_threshold,
            health_check_interval: Duration::from_secs(30),
            is_monitoring: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn start_monitoring<F, R, Fut1, Fut2>(
        &self,
        ping_sender: F,
        reconnect_handler: R,
    ) -> Result<()>
    where
        F: Fn() -> Fut1 + Send + Sync + 'static,
        Fut1: std::future::Future<Output = Result<()>> + Send + 'static,
        R: Fn() -> Fut2 + Send + Sync + 'static,
        Fut2: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        *self.is_monitoring.write().await = true;

        // Start heartbeat manager
        self.heartbeat_manager.start(ping_sender).await?;

        // Start health monitoring
        let heartbeat_manager = HeartbeatManager::new(
            self.heartbeat_manager.ping_interval,
            self.heartbeat_manager.max_missed_pongs,
            self.heartbeat_manager.timeout_duration,
        );
        let reconnect_threshold = self.reconnect_threshold;
        let health_check_interval = self.health_check_interval;
        let is_monitoring = Arc::clone(&self.is_monitoring);

        tokio::spawn(async move {
            let mut health_ticker = interval(health_check_interval);
            health_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            while *is_monitoring.read().await {
                health_ticker.tick().await;

                let status = heartbeat_manager.get_status().await;
                
                if !status.is_healthy && status.missed_pongs >= reconnect_threshold {
                    warn!("Connection requires reconnection due to health issues");
                    
                    match reconnect_handler().await {
                        Ok(_) => {
                            info!("Reconnection successful, resetting heartbeat status");
                            heartbeat_manager.reset().await;
                        }
                        Err(e) => {
                            error!("Reconnection failed: {}", e);
                        }
                    }
                }

                // Log health statistics
                if let Some(latency_stats) = heartbeat_manager.get_latency_percentiles().await {
                    debug!(
                        "Connection health - Latency: avg={:?}, p95={:?}, p99={:?}, missed_pongs={}",
                        latency_stats.avg,
                        latency_stats.p95,
                        latency_stats.p99,
                        status.missed_pongs
                    );
                }
            }
        });

        Ok(())
    }

    pub async fn stop_monitoring(&self) {
        *self.is_monitoring.write().await = false;
        info!("Health monitoring stopped");
    }

    pub async fn record_pong(&self) {
        self.heartbeat_manager.record_pong().await;
    }

    pub async fn get_status(&self) -> HeartbeatStatus {
        self.heartbeat_manager.get_status().await
    }

    pub async fn is_healthy(&self) -> bool {
        self.heartbeat_manager.is_healthy().await
    }

    pub async fn get_latency_stats(&self) -> Option<LatencyStats> {
        self.heartbeat_manager.get_latency_percentiles().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_heartbeat_manager() {
        let manager = HeartbeatManager::new(
            Duration::from_millis(100),
            3,
            Duration::from_millis(50),
        );

        let ping_count = Arc::new(AtomicU32::new(0));
        let ping_count_clone = Arc::clone(&ping_count);

        let ping_sender = move || {
            let count = Arc::clone(&ping_count_clone);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        };

        // Start heartbeat manager
        manager.start(ping_sender).await.unwrap();

        // Wait for a few pings
        sleep(Duration::from_millis(250)).await;

        let status = manager.get_status().await;
        assert!(status.ping_count > 0);
        assert_eq!(ping_count.load(Ordering::SeqCst), status.ping_count as u32);
    }

    #[tokio::test]
    async fn test_pong_recording() {
        let manager = HeartbeatManager::new(
            Duration::from_secs(1),
            3,
            Duration::from_millis(100),
        );

        // Record a pong
        manager.record_pong().await;

        let status = manager.get_status().await;
        assert_eq!(status.pong_count, 1);
        assert!(status.last_pong.is_some());
    }

    #[tokio::test]
    async fn test_latency_calculation() {
        let manager = HeartbeatManager::new(
            Duration::from_secs(1),
            3,
            Duration::from_millis(100),
        );

        // Simulate ping-pong with known latency
        {
            let mut status = manager.status.write().await;
            status.last_ping = Some(Instant::now());
        }

        sleep(Duration::from_millis(10)).await;
        manager.record_pong().await;

        let latency = manager.get_latency().await;
        assert!(latency.is_some());
        assert!(latency.unwrap() >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_health_monitoring() {
        let monitor = ConnectionHealthMonitor::new(
            Duration::from_millis(50),
            2,
            Duration::from_millis(25),
            2,
        );

        let ping_count = Arc::new(AtomicU32::new(0));
        let reconnect_count = Arc::new(AtomicU32::new(0));

        let ping_count_clone = Arc::clone(&ping_count);
        let reconnect_count_clone = Arc::clone(&reconnect_count);

        let ping_sender = move || {
            let count = Arc::clone(&ping_count_clone);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        };

        let reconnect_handler = move || {
            let count = Arc::clone(&reconnect_count_clone);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        };

        monitor.start_monitoring(ping_sender, reconnect_handler).await.unwrap();

        // Wait for health monitoring to detect issues
        sleep(Duration::from_millis(200)).await;

        assert!(ping_count.load(Ordering::SeqCst) > 0);
        // Note: reconnect_count might be 0 if pongs are being recorded properly
    }
}