use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::types::{VenueCredentials, VenueId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbFinderConfig {
    pub venues: HashMap<VenueId, VenueConfig>,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub database: DatabaseConfig,
    pub messaging: MessagingConfig,
    pub monitoring: MonitoringConfig,
    pub execution: ExecutionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueConfig {
    pub enabled: bool,
    pub credentials: Option<VenueCredentials>,
    pub symbols: Vec<String>,
    pub rate_limit_buffer: f64,
    pub reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub order_book_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub enabled_strategies: Vec<String>,
    pub min_spread_bps: i32,
    pub max_position_size: rust_decimal::Decimal,
    pub position_timeout_ms: u64,
    pub confidence_threshold: f64,
    pub signal_strength_threshold: f64,
    pub max_opportunities_per_second: u32,
    pub min_volume_threshold: rust_decimal::Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_daily_loss: rust_decimal::Decimal,
    pub max_position_size: rust_decimal::Decimal,
    pub max_exposure_per_venue: rust_decimal::Decimal,
    pub max_correlation_exposure: rust_decimal::Decimal,
    pub stop_loss_percentage: rust_decimal::Decimal,
    pub max_leverage: rust_decimal::Decimal,
    pub var_limit: rust_decimal::Decimal,
    pub stress_test_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub clickhouse_url: String,
    pub clickhouse_database: String,
    pub postgres_url: String,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
    pub query_timeout_ms: u64,
    pub batch_size: u32,
    pub flush_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingConfig {
    pub nats_url: String,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
    pub request_timeout_ms: u64,
    pub max_payload_size: usize,
    pub enable_jetstream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub prometheus_address: String,
    pub grafana_url: Option<String>,
    pub log_level: String,
    pub metrics_interval_ms: u64,
    pub alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub latency_p99_ms: u64,
    pub error_rate_percentage: f64,
    pub memory_usage_percentage: f64,
    pub cpu_usage_percentage: f64,
    pub connection_failure_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub dry_run: bool,
    pub max_concurrent_orders: u32,
    pub order_timeout_ms: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub slippage_tolerance_bps: i32,
    pub partial_fill_threshold: rust_decimal::Decimal,
}

impl ArbFinderConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(File::from(path.as_ref()))
            .add_source(Environment::with_prefix("ARBFINDER"))
            .build()?;

        settings.try_deserialize()
    }

    pub fn from_files<P: AsRef<Path>>(paths: &[P]) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        for path in paths {
            builder = builder.add_source(File::from(path.as_ref()).required(false));
        }

        let settings = builder
            .add_source(Environment::with_prefix("ARBFINDER"))
            .build()?;

        settings.try_deserialize()
    }

    pub fn development() -> Self {
        Self {
            venues: Self::default_venues(),
            strategy: StrategyConfig::development(),
            risk: RiskConfig::development(),
            database: DatabaseConfig::development(),
            messaging: MessagingConfig::development(),
            monitoring: MonitoringConfig::development(),
            execution: ExecutionConfig::development(),
        }
    }

    pub fn production() -> Self {
        Self {
            venues: Self::default_venues(),
            strategy: StrategyConfig::production(),
            risk: RiskConfig::production(),
            database: DatabaseConfig::production(),
            messaging: MessagingConfig::production(),
            monitoring: MonitoringConfig::production(),
            execution: ExecutionConfig::production(),
        }
    }

    fn default_venues() -> HashMap<VenueId, VenueConfig> {
        let mut venues = HashMap::new();

        venues.insert(
            VenueId::Binance,
            VenueConfig {
                enabled: true,
                credentials: None,
                symbols: vec!["BTC/USDT".to_string(), "ETH/USDT".to_string()],
                rate_limit_buffer: 0.8,
                reconnect_attempts: 10,
                reconnect_delay_ms: 5000,
                heartbeat_interval_ms: 30000,
                order_book_depth: 20,
            },
        );

        venues.insert(
            VenueId::Coinbase,
            VenueConfig {
                enabled: true,
                credentials: None,
                symbols: vec!["BTC/USD".to_string(), "ETH/USD".to_string()],
                rate_limit_buffer: 0.8,
                reconnect_attempts: 10,
                reconnect_delay_ms: 5000,
                heartbeat_interval_ms: 30000,
                order_book_depth: 20,
            },
        );

        venues
    }
}

impl StrategyConfig {
    fn development() -> Self {
        Self {
            enabled_strategies: vec!["simple".to_string(), "cross_exchange".to_string()],
            min_spread_bps: 50,
            max_position_size: rust_decimal::Decimal::from(1000),
            position_timeout_ms: 30000,
            confidence_threshold: 0.7,
            signal_strength_threshold: 0.6,
            max_opportunities_per_second: 100,
            min_volume_threshold: rust_decimal::Decimal::from(100),
        }
    }

    fn production() -> Self {
        Self {
            enabled_strategies: vec!["simple".to_string(), "cross_exchange".to_string()],
            min_spread_bps: 20,
            max_position_size: rust_decimal::Decimal::from(10000),
            position_timeout_ms: 15000,
            confidence_threshold: 0.8,
            signal_strength_threshold: 0.7,
            max_opportunities_per_second: 1000,
            min_volume_threshold: rust_decimal::Decimal::from(1000),
        }
    }
}

impl RiskConfig {
    fn development() -> Self {
        Self {
            max_daily_loss: rust_decimal::Decimal::from(1000),
            max_position_size: rust_decimal::Decimal::from(5000),
            max_exposure_per_venue: rust_decimal::Decimal::from(10000),
            max_correlation_exposure: rust_decimal::Decimal::from(20000),
            stop_loss_percentage: rust_decimal::Decimal::from(5),
            max_leverage: rust_decimal::Decimal::from(1),
            var_limit: rust_decimal::Decimal::from(500),
            stress_test_enabled: false,
        }
    }

    fn production() -> Self {
        Self {
            max_daily_loss: rust_decimal::Decimal::from(10000),
            max_position_size: rust_decimal::Decimal::from(50000),
            max_exposure_per_venue: rust_decimal::Decimal::from(100000),
            max_correlation_exposure: rust_decimal::Decimal::from(200000),
            stop_loss_percentage: rust_decimal::Decimal::from(2),
            max_leverage: rust_decimal::Decimal::from(3),
            var_limit: rust_decimal::Decimal::from(5000),
            stress_test_enabled: true,
        }
    }
}

impl DatabaseConfig {
    fn development() -> Self {
        Self {
            clickhouse_url: "tcp://localhost:9000".to_string(),
            clickhouse_database: "arbfinder_dev".to_string(),
            postgres_url: "postgresql://localhost/arbfinder_dev".to_string(),
            max_connections: 10,
            connection_timeout_ms: 5000,
            query_timeout_ms: 30000,
            batch_size: 1000,
            flush_interval_ms: 1000,
        }
    }

    fn production() -> Self {
        Self {
            clickhouse_url: "tcp://clickhouse:9000".to_string(),
            clickhouse_database: "arbfinder".to_string(),
            postgres_url: "postgresql://postgres:5432/arbfinder".to_string(),
            max_connections: 50,
            connection_timeout_ms: 10000,
            query_timeout_ms: 60000,
            batch_size: 10000,
            flush_interval_ms: 500,
        }
    }
}

impl MessagingConfig {
    fn development() -> Self {
        Self {
            nats_url: "nats://localhost:4222".to_string(),
            max_reconnect_attempts: 10,
            reconnect_delay_ms: 2000,
            request_timeout_ms: 5000,
            max_payload_size: 1024 * 1024,
            enable_jetstream: false,
        }
    }

    fn production() -> Self {
        Self {
            nats_url: "nats://nats:4222".to_string(),
            max_reconnect_attempts: 100,
            reconnect_delay_ms: 1000,
            request_timeout_ms: 10000,
            max_payload_size: 8 * 1024 * 1024,
            enable_jetstream: true,
        }
    }
}

impl MonitoringConfig {
    fn development() -> Self {
        Self {
            prometheus_address: "127.0.0.1:9090".to_string(),
            grafana_url: Some("http://localhost:3000".to_string()),
            log_level: "debug".to_string(),
            metrics_interval_ms: 5000,
            alert_thresholds: AlertThresholds {
                latency_p99_ms: 100,
                error_rate_percentage: 5.0,
                memory_usage_percentage: 80.0,
                cpu_usage_percentage: 80.0,
                connection_failure_rate: 10.0,
            },
        }
    }

    fn production() -> Self {
        Self {
            prometheus_address: "0.0.0.0:9090".to_string(),
            grafana_url: Some("http://grafana:3000".to_string()),
            log_level: "info".to_string(),
            metrics_interval_ms: 1000,
            alert_thresholds: AlertThresholds {
                latency_p99_ms: 50,
                error_rate_percentage: 1.0,
                memory_usage_percentage: 70.0,
                cpu_usage_percentage: 70.0,
                connection_failure_rate: 2.0,
            },
        }
    }
}

impl ExecutionConfig {
    fn development() -> Self {
        Self {
            dry_run: true,
            max_concurrent_orders: 10,
            order_timeout_ms: 30000,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            slippage_tolerance_bps: 100,
            partial_fill_threshold: "0.1".parse().unwrap(),
        }
    }

    fn production() -> Self {
        Self {
            dry_run: false,
            max_concurrent_orders: 100,
            order_timeout_ms: 10000,
            retry_attempts: 5,
            retry_delay_ms: 500,
            slippage_tolerance_bps: 50,
            partial_fill_threshold: "0.05".parse().unwrap(),
        }
    }
}