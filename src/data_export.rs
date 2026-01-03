use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use chrono::Utc;

/// Represents an arbitrage opportunity for logging
#[derive(Clone, Debug)]
pub struct ArbitrageOpportunity {
    pub symbol: String,
    pub spread_percentage: f64,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub status: String,
}

/// CSV exporter for arbitrage opportunities
pub struct DataExporter {
    file_path: String,
}

impl DataExporter {
    /// Creates a new DataExporter instance
    pub fn new(file_path: &str) -> Self {
        DataExporter {
            file_path: file_path.to_string(),
        }
    }

    /// Initializes the CSV file with headers if it doesn't exist
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&self.file_path);
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Check if file already exists
        if !path.exists() {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&self.file_path)?;

            let header = "timestamp,symbol,spread_percentage,buy_exchange,sell_exchange,status\n";
            file.write_all(header.as_bytes())?;
        }

        Ok(())
    }

    /// Logs a single arbitrage opportunity to CSV
    pub fn log_opportunity(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let csv_line = format!(
            "{},{},{:.4},{},{},{}\n",
            timestamp,
            opportunity.symbol,
            opportunity.spread_percentage,
            opportunity.buy_exchange,
            opportunity.sell_exchange,
            opportunity.status,
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;

        file.write_all(csv_line.as_bytes())?;

        Ok(())
    }

    /// Logs multiple arbitrage opportunities to CSV
    pub fn log_opportunities(
        &self,
        opportunities: &[ArbitrageOpportunity],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for opportunity in opportunities {
            self.log_opportunity(opportunity)?;
        }
        Ok(())
    }

    /// Gets the file path of the CSV export
    pub fn get_file_path(&self) -> &str {
        &self.file_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_initialize_creates_file() {
        let test_file = "/tmp/test_arb_export.csv";
        let exporter = DataExporter::new(test_file);
        
        // Clean up if file exists
        let _ = fs::remove_file(test_file);
        
        assert!(exporter.initialize().is_ok());
        assert!(Path::new(test_file).exists());
        
        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_log_opportunity() {
        let test_file = "/tmp/test_arb_log.csv";
        let exporter = DataExporter::new(test_file);
        
        // Clean up if file exists
        let _ = fs::remove_file(test_file);
        
        exporter.initialize().unwrap();

        let opportunity = ArbitrageOpportunity {
            symbol: "BTC/USD".to_string(),
            spread_percentage: 2.5,
            buy_exchange: "Binance".to_string(),
            sell_exchange: "Kraken".to_string(),
            status: "ACTIVE".to_string(),
        };

        assert!(exporter.log_opportunity(&opportunity).is_ok());

        let content = fs::read_to_string(test_file).unwrap();
        assert!(content.contains("BTC/USD"));
        assert!(content.contains("2.5000"));
        assert!(content.contains("Binance"));
        assert!(content.contains("Kraken"));
        assert!(content.contains("ACTIVE"));

        // Clean up
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_log_multiple_opportunities() {
        let test_file = "/tmp/test_arb_multiple.csv";
        let exporter = DataExporter::new(test_file);
        
        // Clean up if file exists
        let _ = fs::remove_file(test_file);
        
        exporter.initialize().unwrap();

        let opportunities = vec![
            ArbitrageOpportunity {
                symbol: "ETH/USD".to_string(),
                spread_percentage: 1.8,
                buy_exchange: "Coinbase".to_string(),
                sell_exchange: "Gemini".to_string(),
                status: "PENDING".to_string(),
            },
            ArbitrageOpportunity {
                symbol: "LTC/USD".to_string(),
                spread_percentage: 3.2,
                buy_exchange: "Kraken".to_string(),
                sell_exchange: "Binance".to_string(),
                status: "ACTIVE".to_string(),
            },
        ];

        assert!(exporter.log_opportunities(&opportunities).is_ok());

        let content = fs::read_to_string(test_file).unwrap();
        assert!(content.contains("ETH/USD"));
        assert!(content.contains("LTC/USD"));

        // Clean up
        let _ = fs::remove_file(test_file);
    }
}
