use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// Represents an arbitrage opportunity for CSV export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityRecord {
    pub timestamp: String,
    pub exchange_a: String,
    pub exchange_b: String,
    pub symbol: String,
    pub price_a: f64,
    pub price_b: f64,
    pub price_difference: f64,
    pub price_difference_percent: f64,
    pub profit_margin: f64,
    pub volume_available: f64,
    pub status: String,
}

/// Represents a trade execution record for CSV export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub timestamp: String,
    pub trade_id: String,
    pub exchange_a: String,
    pub exchange_b: String,
    pub symbol: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub quantity: f64,
    pub trade_status: String,
    pub estimated_profit: f64,
    pub actual_profit: Option<f64>,
    pub fee_a: f64,
    pub fee_b: f64,
}

/// Represents execution results for CSV export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub timestamp: String,
    pub execution_id: String,
    pub trade_id: String,
    pub phase: String, // "buy", "sell", "complete"
    pub exchange: String,
    pub symbol: String,
    pub quantity: f64,
    pub price: f64,
    pub total_value: f64,
    pub status: String, // "pending", "completed", "failed"
    pub error_message: Option<String>,
    pub duration_ms: u64,
}

/// CSV data export manager with async support
pub struct DataExporter {
    opportunities_file: Arc<Mutex<Option<File>>>,
    trades_file: Arc<Mutex<Option<File>>>,
    executions_file: Arc<Mutex<Option<File>>>,
}

impl DataExporter {
    /// Create a new DataExporter instance
    pub fn new() -> Self {
        Self {
            opportunities_file: Arc::new(Mutex::new(None)),
            trades_file: Arc::new(Mutex::new(None)),
            executions_file: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize CSV files with headers
    pub async fn initialize<P: AsRef<Path>>(&self, output_dir: P) -> Result<(), Box<dyn Error>> {
        let output_dir = output_dir.as_ref();

        // Create output directory if it doesn't exist
        tokio::fs::create_dir_all(output_dir).await?;

        // Initialize opportunities file
        let opp_path = output_dir.join("arbitrage_opportunities.csv");
        self.initialize_opportunities_file(&opp_path).await?;

        // Initialize trades file
        let trades_path = output_dir.join("trades.csv");
        self.initialize_trades_file(&trades_path).await?;

        // Initialize executions file
        let exec_path = output_dir.join("executions.csv");
        self.initialize_executions_file(&exec_path).await?;

        Ok(())
    }

    /// Initialize opportunities CSV file with headers
    async fn initialize_opportunities_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path).await?;
        let header = "timestamp,exchange_a,exchange_b,symbol,price_a,price_b,price_difference,price_difference_percent,profit_margin,volume_available,status\n";
        file.write_all(header.as_bytes()).await?;
        file.sync_all().await?;

        *self.opportunities_file.lock().await = Some(file);
        Ok(())
    }

    /// Initialize trades CSV file with headers
    async fn initialize_trades_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path).await?;
        let header = "timestamp,trade_id,exchange_a,exchange_b,symbol,buy_exchange,sell_exchange,buy_price,sell_price,quantity,trade_status,estimated_profit,actual_profit,fee_a,fee_b\n";
        file.write_all(header.as_bytes()).await?;
        file.sync_all().await?;

        *self.trades_file.lock().await = Some(file);
        Ok(())
    }

    /// Initialize executions CSV file with headers
    async fn initialize_executions_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path).await?;
        let header = "timestamp,execution_id,trade_id,phase,exchange,symbol,quantity,price,total_value,status,error_message,duration_ms\n";
        file.write_all(header.as_bytes()).await?;
        file.sync_all().await?;

        *self.executions_file.lock().await = Some(file);
        Ok(())
    }

    /// Log an arbitrage opportunity
    pub async fn log_opportunity(&self, record: OpportunityRecord) -> Result<(), Box<dyn Error>> {
        let mut file_guard = self.opportunities_file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            let csv_line = format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                record.timestamp,
                record.exchange_a,
                record.exchange_b,
                record.symbol,
                record.price_a,
                record.price_b,
                record.price_difference,
                record.price_difference_percent,
                record.profit_margin,
                record.volume_available,
                record.status
            );

            file.write_all(csv_line.as_bytes()).await?;
            file.sync_all().await?;
            Ok(())
        } else {
            Err("Opportunities file not initialized".into())
        }
    }

    /// Log a trade execution
    pub async fn log_trade(&self, record: TradeRecord) -> Result<(), Box<dyn Error>> {
        let mut file_guard = self.trades_file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            let actual_profit_str = record
                .actual_profit
                .map(|p| p.to_string())
                .unwrap_or_else(|| "".to_string());

            let csv_line = format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                record.timestamp,
                record.trade_id,
                record.exchange_a,
                record.exchange_b,
                record.symbol,
                record.buy_exchange,
                record.sell_exchange,
                record.buy_price,
                record.sell_price,
                record.quantity,
                record.trade_status,
                record.estimated_profit,
                actual_profit_str,
                record.fee_a,
                record.fee_b
            );

            file.write_all(csv_line.as_bytes()).await?;
            file.sync_all().await?;
            Ok(())
        } else {
            Err("Trades file not initialized".into())
        }
    }

    /// Log execution results
    pub async fn log_execution(&self, record: ExecutionResult) -> Result<(), Box<dyn Error>> {
        let mut file_guard = self.executions_file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            let error_msg = record
                .error_message
                .as_deref()
                .unwrap_or("")
                .replace(",", ";");

            let csv_line = format!(
                "{},{},{},{},{},{},{},{},{},{},{},{}\n",
                record.timestamp,
                record.execution_id,
                record.trade_id,
                record.phase,
                record.exchange,
                record.symbol,
                record.quantity,
                record.price,
                record.total_value,
                record.status,
                error_msg,
                record.duration_ms
            );

            file.write_all(csv_line.as_bytes()).await?;
            file.sync_all().await?;
            Ok(())
        } else {
            Err("Executions file not initialized".into())
        }
    }

    /// Log multiple opportunities in batch
    pub async fn log_opportunities_batch(
        &self,
        records: Vec<OpportunityRecord>,
    ) -> Result<(), Box<dyn Error>> {
        let mut file_guard = self.opportunities_file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            let mut csv_data = String::new();

            for record in records {
                csv_data.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{}\n",
                    record.timestamp,
                    record.exchange_a,
                    record.exchange_b,
                    record.symbol,
                    record.price_a,
                    record.price_b,
                    record.price_difference,
                    record.price_difference_percent,
                    record.profit_margin,
                    record.volume_available,
                    record.status
                ));
            }

            file.write_all(csv_data.as_bytes()).await?;
            file.sync_all().await?;
            Ok(())
        } else {
            Err("Opportunities file not initialized".into())
        }
    }

    /// Log multiple trades in batch
    pub async fn log_trades_batch(
        &self,
        records: Vec<TradeRecord>,
    ) -> Result<(), Box<dyn Error>> {
        let mut file_guard = self.trades_file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            let mut csv_data = String::new();

            for record in records {
                let actual_profit_str = record
                    .actual_profit
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "".to_string());

                csv_data.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    record.timestamp,
                    record.trade_id,
                    record.exchange_a,
                    record.exchange_b,
                    record.symbol,
                    record.buy_exchange,
                    record.sell_exchange,
                    record.buy_price,
                    record.sell_price,
                    record.quantity,
                    record.trade_status,
                    record.estimated_profit,
                    actual_profit_str,
                    record.fee_a,
                    record.fee_b
                ));
            }

            file.write_all(csv_data.as_bytes()).await?;
            file.sync_all().await?;
            Ok(())
        } else {
            Err("Trades file not initialized".into())
        }
    }

    /// Log multiple execution results in batch
    pub async fn log_executions_batch(
        &self,
        records: Vec<ExecutionResult>,
    ) -> Result<(), Box<dyn Error>> {
        let mut file_guard = self.executions_file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            let mut csv_data = String::new();

            for record in records {
                let error_msg = record
                    .error_message
                    .as_deref()
                    .unwrap_or("")
                    .replace(",", ";");

                csv_data.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    record.timestamp,
                    record.execution_id,
                    record.trade_id,
                    record.phase,
                    record.exchange,
                    record.symbol,
                    record.quantity,
                    record.price,
                    record.total_value,
                    record.status,
                    error_msg,
                    record.duration_ms
                ));
            }

            file.write_all(csv_data.as_bytes()).await?;
            file.sync_all().await?;
            Ok(())
        } else {
            Err("Executions file not initialized".into())
        }
    }

    /// Flush all open files to ensure data is written
    pub async fn flush_all(&self) -> Result<(), Box<dyn Error>> {
        if let Some(file) = self.opportunities_file.lock().await.as_mut() {
            file.sync_all().await?;
        }

        if let Some(file) = self.trades_file.lock().await.as_mut() {
            file.sync_all().await?;
        }

        if let Some(file) = self.executions_file.lock().await.as_mut() {
            file.sync_all().await?;
        }

        Ok(())
    }

    /// Close all open files
    pub async fn close(&self) -> Result<(), Box<dyn Error>> {
        *self.opportunities_file.lock().await = None;
        *self.trades_file.lock().await = None;
        *self.executions_file.lock().await = None;

        Ok(())
    }
}

impl Default for DataExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_data_exporter_initialization() {
        let exporter = DataExporter::new();
        let temp_dir = "/tmp/arb_finder_test";

        let result = exporter.initialize(temp_dir).await;
        assert!(result.is_ok());

        // Verify files exist
        assert!(Path::new(&format!("{}/arbitrage_opportunities.csv", temp_dir)).exists());
        assert!(Path::new(&format!("{}/trades.csv", temp_dir)).exists());
        assert!(Path::new(&format!("{}/executions.csv", temp_dir)).exists());

        let _ = exporter.close().await;
    }

    #[tokio::test]
    async fn test_log_opportunity() {
        let exporter = DataExporter::new();
        let temp_dir = "/tmp/arb_finder_test_opp";

        let _ = exporter.initialize(temp_dir).await;

        let record = OpportunityRecord {
            timestamp: Utc::now().to_rfc3339(),
            exchange_a: "binance".to_string(),
            exchange_b: "kraken".to_string(),
            symbol: "BTC/USD".to_string(),
            price_a: 45000.0,
            price_b: 45500.0,
            price_difference: 500.0,
            price_difference_percent: 1.11,
            profit_margin: 0.95,
            volume_available: 10.0,
            status: "identified".to_string(),
        };

        let result = exporter.log_opportunity(record).await;
        assert!(result.is_ok());

        let _ = exporter.close().await;
    }

    #[tokio::test]
    async fn test_log_trade() {
        let exporter = DataExporter::new();
        let temp_dir = "/tmp/arb_finder_test_trade";

        let _ = exporter.initialize(temp_dir).await;

        let record = TradeRecord {
            timestamp: Utc::now().to_rfc3339(),
            trade_id: "trade_001".to_string(),
            exchange_a: "binance".to_string(),
            exchange_b: "kraken".to_string(),
            symbol: "BTC/USD".to_string(),
            buy_exchange: "binance".to_string(),
            sell_exchange: "kraken".to_string(),
            buy_price: 45000.0,
            sell_price: 45500.0,
            quantity: 1.0,
            trade_status: "pending".to_string(),
            estimated_profit: 450.0,
            actual_profit: None,
            fee_a: 45.0,
            fee_b: 45.5,
        };

        let result = exporter.log_trade(record).await;
        assert!(result.is_ok());

        let _ = exporter.close().await;
    }

    #[tokio::test]
    async fn test_log_execution() {
        let exporter = DataExporter::new();
        let temp_dir = "/tmp/arb_finder_test_exec";

        let _ = exporter.initialize(temp_dir).await;

        let record = ExecutionResult {
            timestamp: Utc::now().to_rfc3339(),
            execution_id: "exec_001".to_string(),
            trade_id: "trade_001".to_string(),
            phase: "buy".to_string(),
            exchange: "binance".to_string(),
            symbol: "BTC/USD".to_string(),
            quantity: 1.0,
            price: 45000.0,
            total_value: 45000.0,
            status: "completed".to_string(),
            error_message: None,
            duration_ms: 1500,
        };

        let result = exporter.log_execution(record).await;
        assert!(result.is_ok());

        let _ = exporter.close().await;
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let exporter = DataExporter::new();
        let temp_dir = "/tmp/arb_finder_test_batch";

        let _ = exporter.initialize(temp_dir).await;

        let opportunities = vec![
            OpportunityRecord {
                timestamp: Utc::now().to_rfc3339(),
                exchange_a: "binance".to_string(),
                exchange_b: "kraken".to_string(),
                symbol: "BTC/USD".to_string(),
                price_a: 45000.0,
                price_b: 45500.0,
                price_difference: 500.0,
                price_difference_percent: 1.11,
                profit_margin: 0.95,
                volume_available: 10.0,
                status: "identified".to_string(),
            },
            OpportunityRecord {
                timestamp: Utc::now().to_rfc3339(),
                exchange_a: "coinbase".to_string(),
                exchange_b: "kraken".to_string(),
                symbol: "ETH/USD".to_string(),
                price_a: 2500.0,
                price_b: 2550.0,
                price_difference: 50.0,
                price_difference_percent: 2.0,
                profit_margin: 1.5,
                volume_available: 100.0,
                status: "identified".to_string(),
            },
        ];

        let result = exporter.log_opportunities_batch(opportunities).await;
        assert!(result.is_ok());

        let _ = exporter.close().await;
    }
}
