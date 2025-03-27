use std::path::Path;
use tracing::{info, error};
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
    EnvFilter,
};
use tracing_appender::{non_blocking, rolling};

use arbfinder_core::prelude::*;
use crate::MonitoringConfig;

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub enable_json: bool,
    pub enable_console: bool,
    pub max_files: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: Some("logs/arbfinder.log".to_string()),
            enable_json: true,
            enable_console: true,
            max_files: 10,
        }
    }
}

pub fn setup_logging(config: &MonitoringConfig) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let mut layers = Vec::new();

    // Console logging layer
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    if config.enable_json_logs {
        let json_console_layer = console_layer.json();
        layers.push(json_console_layer.boxed());
    } else {
        layers.push(console_layer.boxed());
    }

    // File logging layer
    if let Some(log_file) = &config.log_file {
        let log_path = Path::new(log_file);
        
        // Create log directory if it doesn't exist
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ArbFinderError::Internal(format!("Failed to create log directory: {}", e)))?;
        }

        let file_appender = rolling::daily(
            log_path.parent().unwrap_or_else(|| Path::new(".")),
            log_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("arbfinder.log"))
        );
        
        let (non_blocking_appender, _guard) = non_blocking(file_appender);
        
        let file_layer = fmt::layer()
            .with_writer(non_blocking_appender)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true);

        if config.enable_json_logs {
            let json_file_layer = file_layer.json();
            layers.push(json_file_layer.boxed());
        } else {
            layers.push(file_layer.boxed());
        }

        // Store the guard to prevent it from being dropped
        // In a real implementation, you'd want to store this somewhere
        std::mem::forget(_guard);
    }

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(layers)
        .init();

    info!("Logging initialized with level: {}", config.log_level);
    
    Ok(())
}

pub struct StructuredLogger {
    component: String,
}

impl StructuredLogger {
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
        }
    }

    pub fn log_trade(&self, exchange: &str, symbol: &str, side: &str, amount: f64, price: f64) {
        info!(
            component = %self.component,
            event = "trade_executed",
            exchange = %exchange,
            symbol = %symbol,
            side = %side,
            amount = %amount,
            price = %price,
            "Trade executed"
        );
    }

    pub fn log_order(&self, exchange: &str, symbol: &str, side: &str, amount: f64, price: f64, order_id: &str) {
        info!(
            component = %self.component,
            event = "order_placed",
            exchange = %exchange,
            symbol = %symbol,
            side = %side,
            amount = %amount,
            price = %price,
            order_id = %order_id,
            "Order placed"
        );
    }

    pub fn log_arbitrage_opportunity(&self, exchange_a: &str, exchange_b: &str, symbol: &str, profit: f64) {
        info!(
            component = %self.component,
            event = "arbitrage_opportunity",
            exchange_a = %exchange_a,
            exchange_b = %exchange_b,
            symbol = %symbol,
            profit = %profit,
            "Arbitrage opportunity detected"
        );
    }

    pub fn log_error(&self, error: &str, context: Option<&str>) {
        error!(
            component = %self.component,
            event = "error",
            error = %error,
            context = ?context,
            "Error occurred"
        );
    }

    pub fn log_warning(&self, message: &str, context: Option<&str>) {
        tracing::warn!(
            component = %self.component,
            event = "warning",
            message = %message,
            context = ?context,
            "Warning"
        );
    }

    pub fn log_system_event(&self, event: &str, details: &serde_json::Value) {
        info!(
            component = %self.component,
            event = %event,
            details = %details,
            "System event"
        );
    }

    pub fn log_performance(&self, operation: &str, duration_ms: f64, success: bool) {
        info!(
            component = %self.component,
            event = "performance",
            operation = %operation,
            duration_ms = %duration_ms,
            success = %success,
            "Performance metric"
        );
    }

    pub fn log_exchange_request(&self, exchange: &str, endpoint: &str, method: &str, status_code: Option<u16>) {
        info!(
            component = %self.component,
            event = "exchange_request",
            exchange = %exchange,
            endpoint = %endpoint,
            method = %method,
            status_code = ?status_code,
            "Exchange API request"
        );
    }

    pub fn log_portfolio_update(&self, total_value: f64, pnl: f64, positions: usize) {
        info!(
            component = %self.component,
            event = "portfolio_update",
            total_value = %total_value,
            pnl = %pnl,
            positions = %positions,
            "Portfolio updated"
        );
    }

    pub fn log_risk_event(&self, event_type: &str, symbol: Option<&str>, details: &str) {
        tracing::warn!(
            component = %self.component,
            event = "risk_event",
            event_type = %event_type,
            symbol = ?symbol,
            details = %details,
            "Risk management event"
        );
    }
}