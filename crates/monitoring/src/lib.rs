use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use arbfinder_core::prelude::*;

pub mod metrics;
pub mod logging;
pub mod alerts;
pub mod health;

pub use metrics::{MetricsCollector, MetricsServer};
pub use logging::{LoggingConfig, setup_logging};
pub use alerts::{AlertManager, AlertConfig, Alert, AlertLevel};
pub use health::{HealthChecker, HealthStatus, HealthState, ComponentHealth, SystemMetrics};

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub metrics_port: u16,
    pub log_level: String,
    pub log_file: Option<String>,
    pub enable_json_logs: bool,
    pub alert_config: AlertConfig,
    pub health_check_interval_secs: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_port: 9090,
            log_level: "info".to_string(),
            log_file: Some("arbfinder.log".to_string()),
            enable_json_logs: true,
            alert_config: AlertConfig::default(),
            health_check_interval_secs: 30,
        }
    }
}

pub struct MonitoringSystem {
    config: MonitoringConfig,
    metrics_collector: Arc<MetricsCollector>,
    metrics_server: Option<MetricsServer>,
    alert_manager: Arc<RwLock<AlertManager>>,
    health_checker: Arc<HealthChecker>,
}

impl MonitoringSystem {
    pub fn new(config: MonitoringConfig) -> Result<Self> {
        let metrics_collector = Arc::new(MetricsCollector::new());
        let alert_manager = Arc::new(RwLock::new(AlertManager::new(config.alert_config.clone())));
        let health_checker = Arc::new(HealthChecker::new());

        Ok(Self {
            config,
            metrics_collector,
            metrics_server: None,
            alert_manager,
            health_checker,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting monitoring system");

        // Setup logging
        setup_logging(&self.config)?;

        // Start metrics server
        let metrics_server = MetricsServer::new(
            self.config.metrics_port,
            Arc::clone(&self.metrics_collector),
        );
        metrics_server.start().await?;
        self.metrics_server = Some(metrics_server);

        // Start health checker
        self.start_health_checker().await;

        // Start alert manager
        self.start_alert_manager().await;

        info!("Monitoring system started successfully");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping monitoring system");

        if let Some(server) = &mut self.metrics_server {
            server.stop().await?;
        }

        Ok(())
    }

    pub fn get_metrics_collector(&self) -> Arc<MetricsCollector> {
        Arc::clone(&self.metrics_collector)
    }

    pub async fn send_alert(&self, alert: Alert) {
        self.alert_manager.write().await.send_alert(alert).await;
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        self.health_checker.get_status().await
    }

    async fn start_health_checker(&self) {
        let health_checker = Arc::clone(&self.health_checker);
        let interval = self.config.health_check_interval_secs;
        let alert_manager = Arc::clone(&self.alert_manager);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval)
            );

            loop {
                interval.tick().await;
                
                let status = health_checker.check_health().await;
                if !status.is_healthy {
                    let alert = Alert {
                        id: uuid::Uuid::new_v4().to_string(),
                        level: AlertLevel::Critical,
                        title: "System Health Check Failed".to_string(),
                        message: format!("Health check failed: {}", status.message),
                        timestamp: chrono::Utc::now(),
                        metadata: std::collections::HashMap::new(),
                    };

                    alert_manager.write().await.send_alert(alert).await;
                }
            }
        });
    }

    async fn start_alert_manager(&self) {
        let alert_manager = Arc::clone(&self.alert_manager);
        
        tokio::spawn(async move {
            alert_manager.write().await.start().await;
        });
    }
}

pub mod prelude {
    pub use super::{
        MonitoringSystem, MonitoringConfig,
        MetricsCollector, AlertManager, Alert, AlertLevel,
        HealthChecker, HealthStatus, HealthState, ComponentHealth, SystemMetrics,
        setup_logging,
    };
}
