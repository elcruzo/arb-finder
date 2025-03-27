use std::sync::Arc;
use tokio::signal;
use tracing::{info, error};
use clap::{Parser, Subcommand};

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_strategy::prelude::*;
use arbfinder_execution::prelude::*;
use arbfinder_monitoring::prelude::*;
use rust_decimal::Decimal;

// Exchange adapters
use arbfinder_binance::BinanceAdapter;
use arbfinder_coinbase::CoinbaseAdapter;
use arbfinder_kraken::KrakenAdapter;

#[derive(Parser)]
#[command(name = "arbfinder")]
#[command(about = "A cryptocurrency arbitrage finder and trading bot")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the arbitrage finder
    Run {
        /// Configuration file path
        #[arg(short, long, default_value = "config.toml")]
        config: String,
        
        /// Enable paper trading mode
        #[arg(long)]
        paper_trading: bool,
        
        /// Log level
        #[arg(long, default_value = "info")]
        log_level: String,
    },
    /// Check system health
    Health,
    /// Show version information
    Version,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub execution: ExecutionConfig,
    pub monitoring: MonitoringConfig,
    pub exchanges: ExchangeConfigs,
}

#[derive(Debug, Clone)]
pub struct ExchangeConfigs {
    pub binance: Option<ExchangeCredentials>,
    pub coinbase: Option<ExchangeCredentials>,
    pub kraken: Option<ExchangeCredentials>,
}

#[derive(Debug, Clone)]
pub struct ExchangeCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>, // For Coinbase
    pub sandbox: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            execution: ExecutionConfig::default(),
            monitoring: MonitoringConfig::default(),
            exchanges: ExchangeConfigs {
                binance: None,
                coinbase: None,
                kraken: None,
            },
        }
    }
}

pub struct ArbFinderApp {
    config: AppConfig,
    execution_engine: ExecutionEngine,
    monitoring_system: MonitoringSystem,
    health_checker: Arc<HealthChecker>,
}

impl ArbFinderApp {
    pub fn new(config: AppConfig) -> Result<Self> {
        let execution_engine = ExecutionEngine::new(config.execution.clone());
        let monitoring_system = MonitoringSystem::new(config.monitoring.clone())?;
        let health_checker = Arc::new(HealthChecker::new());

        Ok(Self {
            config,
            execution_engine,
            monitoring_system,
            health_checker,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting ArbFinder application");

        // Start monitoring system
        self.monitoring_system.start().await?;
        
        // Register health check components
        self.health_checker.register_component("execution_engine").await;
        self.health_checker.register_component("monitoring_system").await;

        // Setup exchanges
        self.setup_exchanges().await?;

        // Setup strategies
        self.setup_strategies().await?;

        // Start execution engine
        self.execution_engine.start().await?;

        // Update health status
        self.health_checker.update_component_health(
            "execution_engine",
            HealthState::Healthy,
            "Execution engine started successfully"
        ).await;

        info!("ArbFinder application started successfully");

        // Wait for shutdown signal
        self.wait_for_shutdown().await;

        // Graceful shutdown
        self.shutdown().await?;

        Ok(())
    }

    async fn setup_exchanges(&mut self) -> Result<()> {
        info!("Setting up exchange connections");

        // Setup Binance
        if let Some(binance_config) = &self.config.exchanges.binance {
            let binance_adapter = Arc::new(BinanceAdapter::with_credentials(
                binance_config.api_key.clone(),
                binance_config.api_secret.clone(),
            ));
            
            self.execution_engine.add_exchange("binance".to_string(), binance_adapter);
            self.health_checker.register_component("exchange_binance").await;
            
            info!("Binance exchange configured");
        }

        // Setup Coinbase
        if let Some(coinbase_config) = &self.config.exchanges.coinbase {
            let coinbase_adapter = Arc::new(CoinbaseAdapter::with_credentials(
                coinbase_config.api_key.clone(),
                coinbase_config.api_secret.clone(),
                coinbase_config.passphrase.clone().unwrap_or_default(),
            ));
            
            self.execution_engine.add_exchange("coinbase".to_string(), coinbase_adapter);
            self.health_checker.register_component("exchange_coinbase").await;
            
            info!("Coinbase exchange configured");
        }

        // Setup Kraken
        if let Some(kraken_config) = &self.config.exchanges.kraken {
            let kraken_adapter = Arc::new(KrakenAdapter::with_credentials(
                kraken_config.api_key.clone(),
                kraken_config.api_secret.clone(),
            ));
            
            self.execution_engine.add_exchange("kraken".to_string(), kraken_adapter);
            self.health_checker.register_component("exchange_kraken").await;
            
            info!("Kraken exchange configured");
        }

        Ok(())
    }

    async fn setup_strategies(&mut self) -> Result<()> {
        info!("Setting up trading strategies");

        // Add triangular arbitrage strategy with proper parameters
        let triangular_strategy = Box::new(TriangularArbitrage::new(
            "binance".to_string(),  // Default exchange
            "USDT".to_string(),     // Base currency
            Decimal::new(1, 1), // 0.1% minimum profit threshold
        ));
        self.execution_engine.add_strategy(triangular_strategy);
        
        self.health_checker.register_component("strategy_triangular").await;
        
        info!("Triangular arbitrage strategy configured");

        Ok(())
    }

    async fn wait_for_shutdown(&self) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C signal");
            },
            _ = terminate => {
                info!("Received terminate signal");
            },
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down ArbFinder application");

        // Stop monitoring system
        self.monitoring_system.stop().await?;

        info!("ArbFinder application shut down successfully");
        Ok(())
    }

    pub async fn health_check(&self) -> HealthStatus {
        self.health_checker.run_comprehensive_health_check().await
    }
}

fn load_config(config_path: &str) -> Result<AppConfig> {
    use std::fs;
    info!("Loading configuration from: {}", config_path);
    
    // Try to read and parse the config file
    match fs::read_to_string(config_path) {
        Ok(contents) => {
            // Parse TOML config
            // Note: In production, add toml = "0.8" to Cargo.toml and use proper parsing:
            // let config: toml::Value = toml::from_str(&contents)
            //     .map_err(|e| ArbFinderError::Config(format!("Failed to parse config: {}", e)))?;
            
            // For now, return default config since we don't have toml dependency
            // This is marked for future enhancement
            info!("Config file found but using default configuration (TOML parsing not yet implemented)");
            info!("To enable config parsing, add 'toml = \"0.8\"' to Cargo.toml dependencies");
            Ok(AppConfig::default())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("Config file not found at {}, using default configuration", config_path);
            Ok(AppConfig::default())
        }
        Err(e) => {
            error!("Failed to read config file: {}", e);
            Err(ArbFinderError::Io(e))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { config, paper_trading, log_level } => {
            // Load configuration
            let mut app_config = load_config(&config)?;
            
            // Override with CLI options
            app_config.execution.enable_paper_trading = paper_trading;
            app_config.monitoring.log_level = log_level;

            // Create and run application
            let mut app = ArbFinderApp::new(app_config)?;
            app.run().await?;
        }
        Commands::Health => {
            // Quick health check
            let config = AppConfig::default();
            let app = ArbFinderApp::new(config)?;
            let health_status = app.health_check().await;
            
            println!("Health Status: {}", if health_status.is_healthy { "Healthy" } else { "Unhealthy" });
            println!("Message: {}", health_status.message);
            println!("Timestamp: {}", health_status.timestamp);
            
            for (name, component) in health_status.components {
                println!("  {}: {:?} - {}", name, component.status, component.message);
            }
        }
        Commands::Version => {
            println!("ArbFinder v{}", env!("CARGO_PKG_VERSION"));
            println!("A cryptocurrency arbitrage finder and trading bot");
        }
    }

    Ok(())
}