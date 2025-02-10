use std::sync::Arc;
use tokio::signal;
use tracing::{info, error};
use clap::{Parser, Subcommand};

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_strategy::prelude::*;
use arbfinder_execution::prelude::*;
use arbfinder_monitoring::prelude::*;

// Exchange adapters
use arbfinder_binance::BinanceClient;
use arbfinder_coinbase::CoinbaseClient;
use arbfinder_kraken::KrakenClient;

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
            HealthStatus::Healthy,
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
            let binance_client = Arc::new(BinanceClient::new(
                binance_config.api_key.clone(),
                binance_config.api_secret.clone(),
                binance_config.sandbox,
            ));
            
            self.execution_engine.add_exchange("binance".to_string(), binance_client.clone());
            self.health_checker.register_component("exchange_binance").await;
            
            info!("Binance exchange configured");
        }

        // Setup Coinbase
        if let Some(coinbase_config) = &self.config.exchanges.coinbase {
            let coinbase_client = Arc::new(CoinbaseClient::new(
                coinbase_config.api_key.clone(),
                coinbase_config.api_secret.clone(),
                coinbase_config.passphrase.clone().unwrap_or_default(),
                coinbase_config.sandbox,
            ));
            
            self.execution_engine.add_exchange("coinbase".to_string(), coinbase_client.clone());
            self.health_checker.register_component("exchange_coinbase").await;
            
            info!("Coinbase exchange configured");
        }

        // Setup Kraken
        if let Some(kraken_config) = &self.config.exchanges.kraken {
            let kraken_client = Arc::new(KrakenClient::new(
                kraken_config.api_key.clone(),
                kraken_config.api_secret.clone(),
            ));
            
            self.execution_engine.add_exchange("kraken".to_string(), kraken_client.clone());
            self.health_checker.register_component("exchange_kraken").await;
            
            info!("Kraken exchange configured");
        }

        Ok(())
    }

    async fn setup_strategies(&mut self) -> Result<()> {
        info!("Setting up trading strategies");

        // Add triangular arbitrage strategy
        let triangular_strategy = Box::new(TriangularArbitrage::new());
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
    // Simplified config loading - in production, use a proper config library like config-rs
    info!("Loading configuration from: {}", config_path);
    
    // For now, return default config
    // In production, you would parse TOML/YAML/JSON config file
    Ok(AppConfig::default())
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