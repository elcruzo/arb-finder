use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{info, warn, error};

use arbfinder_core::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub components: HashMap<String, ComponentHealth>,
    pub system_metrics: SystemMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthState,
    pub message: String,
    pub last_check: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub network_connections: u32,
    pub uptime_seconds: u64,
}

pub struct HealthChecker {
    components: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    system_start_time: DateTime<Utc>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
            system_start_time: Utc::now(),
        }
    }

    pub async fn register_component(&self, name: &str) {
        let component = ComponentHealth {
            name: name.to_string(),
            status: HealthState::Unknown,
            message: "Component registered".to_string(),
            last_check: Utc::now(),
            uptime_seconds: 0,
            error_count: 0,
        };

        self.components.write().await.insert(name.to_string(), component);
        info!("Health checker: Registered component {}", name);
    }

    pub async fn update_component_health(
        &self,
        name: &str,
        status: HealthState,
        message: &str,
    ) {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(name) {
            let now = Utc::now();
            let uptime = now.signed_duration_since(self.system_start_time);
            
            component.status = status;
            component.message = message.to_string();
            component.last_check = now;
            component.uptime_seconds = uptime.num_seconds() as u64;
            
            match component.status {
                HealthState::Unhealthy => component.error_count += 1,
                _ => {}
            }
        }
    }

    pub async fn increment_error_count(&self, component_name: &str) {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(component_name) {
            component.error_count += 1;
            
            // Auto-update status based on error count
            if component.error_count > 10 {
                component.status = HealthState::Unhealthy;
                component.message = format!("High error count: {}", component.error_count);
            } else if component.error_count > 5 {
                component.status = HealthState::Degraded;
                component.message = format!("Elevated error count: {}", component.error_count);
            }
        }
    }

    pub async fn check_health(&self) -> HealthStatus {
        let components = self.components.read().await.clone();
        let system_metrics = self.get_system_metrics().await;
        
        let overall_healthy = self.calculate_overall_health(&components, &system_metrics);
        let message = if overall_healthy {
            "All systems operational".to_string()
        } else {
            "Some components are unhealthy".to_string()
        };

        HealthStatus {
            is_healthy: overall_healthy,
            message,
            timestamp: Utc::now(),
            components,
            system_metrics,
        }
    }

    pub async fn get_status(&self) -> HealthStatus {
        self.check_health().await
    }

    async fn get_system_metrics(&self) -> SystemMetrics {
        // In a real implementation, you would use system monitoring libraries
        // like sysinfo or procfs to get actual system metrics
        
        SystemMetrics {
            memory_usage_mb: self.get_memory_usage().await,
            cpu_usage_percent: self.get_cpu_usage().await,
            disk_usage_percent: self.get_disk_usage().await,
            network_connections: self.get_network_connections().await,
            uptime_seconds: Utc::now()
                .signed_duration_since(self.system_start_time)
                .num_seconds() as u64,
        }
    }

    async fn get_memory_usage(&self) -> f64 {
        // Simplified memory usage calculation
        // In production, use sysinfo crate or similar
        #[cfg(target_os = "macos")]
        {
            // macOS-specific memory check (simplified)
            100.0 // Placeholder
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Linux/Windows memory check
            100.0 // Placeholder
        }
    }

    async fn get_cpu_usage(&self) -> f64 {
        // Simplified CPU usage calculation
        // In production, use sysinfo crate or similar
        25.0 // Placeholder
    }

    async fn get_disk_usage(&self) -> f64 {
        // Simplified disk usage calculation
        // In production, use sysinfo crate or similar
        45.0 // Placeholder
    }

    async fn get_network_connections(&self) -> u32 {
        // Simplified network connections count
        // In production, parse /proc/net/tcp or use system APIs
        10 // Placeholder
    }

    fn calculate_overall_health(
        &self,
        components: &HashMap<String, ComponentHealth>,
        system_metrics: &SystemMetrics,
    ) -> bool {
        // Check component health
        let unhealthy_components = components.values()
            .filter(|c| matches!(c.status, HealthState::Unhealthy))
            .count();

        if unhealthy_components > 0 {
            return false;
        }

        // Check system metrics thresholds
        if system_metrics.memory_usage_mb > 1000.0 {  // > 1GB
            warn!("High memory usage: {:.2} MB", system_metrics.memory_usage_mb);
        }

        if system_metrics.cpu_usage_percent > 80.0 {
            warn!("High CPU usage: {:.2}%", system_metrics.cpu_usage_percent);
            return false;
        }

        if system_metrics.disk_usage_percent > 90.0 {
            error!("Critical disk usage: {:.2}%", system_metrics.disk_usage_percent);
            return false;
        }

        true
    }

    // Health check methods for specific components
    pub async fn check_exchange_health(&self, exchange_name: &str) -> HealthState {
        // Simplified exchange health check
        // In production, this would ping the exchange API
        
        match exchange_name {
            "binance" | "coinbase" | "kraken" => {
                self.update_component_health(
                    exchange_name,
                    HealthState::Healthy,
                    "Exchange API responding normally"
                ).await;
                HealthState::Healthy
            }
            _ => {
                self.update_component_health(
                    exchange_name,
                    HealthState::Unknown,
                    "Unknown exchange"
                ).await;
                HealthState::Unknown
            }
        }
    }

    pub async fn check_database_health(&self) -> HealthState {
        // Simplified database health check
        // In production, this would test database connectivity
        
        self.update_component_health(
            "database",
            HealthState::Healthy,
            "Database connection active"
        ).await;
        
        HealthState::Healthy
    }

    pub async fn check_strategy_health(&self, strategy_name: &str) -> HealthState {
        // Simplified strategy health check
        // In production, this would check strategy performance metrics
        
        self.update_component_health(
            &format!("strategy_{}", strategy_name),
            HealthState::Healthy,
            "Strategy running normally"
        ).await;
        
        HealthState::Healthy
    }

    pub async fn run_comprehensive_health_check(&self) -> HealthStatus {
        info!("Running comprehensive health check");

        // Check all registered components
        let component_names: Vec<String> = {
            let components = self.components.read().await;
            components.keys().cloned().collect()
        };

        for component_name in component_names {
            match component_name.as_str() {
                name if name.starts_with("exchange_") => {
                    let exchange = &name[9..]; // Remove "exchange_" prefix
                    self.check_exchange_health(exchange).await;
                }
                "database" => {
                    self.check_database_health().await;
                }
                name if name.starts_with("strategy_") => {
                    let strategy = &name[9..]; // Remove "strategy_" prefix
                    self.check_strategy_health(strategy).await;
                }
                _ => {
                    // Generic component check
                    self.update_component_health(
                        &component_name,
                        HealthState::Healthy,
                        "Component operational"
                    ).await;
                }
            }
        }

        self.check_health().await
    }

    pub async fn get_component_status(&self, component_name: &str) -> Option<ComponentHealth> {
        self.components.read().await.get(component_name).cloned()
    }

    pub async fn reset_error_count(&self, component_name: &str) {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(component_name) {
            component.error_count = 0;
            component.status = HealthState::Healthy;
            component.message = "Error count reset".to_string();
            component.last_check = Utc::now();
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}