use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use chrono::Local;
use csv::Writer;
use serde::Serialize;

/// Represents exportable data from the arbitrage finder
#[derive(Serialize, Clone, Debug)]
pub struct ExportData {
    pub timestamp: String,
    pub exchange1: String,
    pub exchange2: String,
    pub asset_pair: String,
    pub price1: f64,
    pub price2: f64,
    pub spread_percentage: f64,
    pub volume: f64,
}

/// Data export module for handling various export formats
pub struct DataExporter {
    output_dir: PathBuf,
}

impl DataExporter {
    /// Create a new DataExporter with the specified output directory
    pub fn new<P: AsRef<Path>>(output_dir: P) -> io::Result<Self> {
        let dir_path = output_dir.as_ref();
        fs::create_dir_all(dir_path)?;
        Ok(DataExporter {
            output_dir: dir_path.to_path_buf(),
        })
    }

    /// Export data to CSV format
    pub fn export_csv(&self, filename: &str, data: &[ExportData]) -> io::Result<PathBuf> {
        let file_path = self.output_dir.join(filename);
        let file = File::create(&file_path)?;
        let mut writer = Writer::from_writer(file);

        for record in data {
            writer
                .serialize(record)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }

        writer
            .flush()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(file_path)
    }

    /// Export data to JSON format
    pub fn export_json(&self, filename: &str, data: &[ExportData]) -> io::Result<PathBuf> {
        let file_path = self.output_dir.join(filename);
        let json_data =
            serde_json::to_string_pretty(data)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut file = File::create(&file_path)?;
        file.write_all(json_data.as_bytes())?;

        Ok(file_path)
    }

    /// Export data to JSON Lines format (one JSON object per line)
    pub fn export_jsonl(&self, filename: &str, data: &[ExportData]) -> io::Result<PathBuf> {
        let file_path = self.output_dir.join(filename);
        let mut file = File::create(&file_path)?;

        for record in data {
            let json_line =
                serde_json::to_string(record)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            writeln!(file, "{}", json_line)?;
        }

        Ok(file_path)
    }

    /// Generate a timestamped filename for exports
    pub fn generate_filename(&self, prefix: &str, extension: &str) -> String {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        format!("{}_{}.{}", prefix, timestamp, extension)
    }

    /// Get the output directory path
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_test_data() -> Vec<ExportData> {
        vec![
            ExportData {
                timestamp: "2024-01-01 12:00:00".to_string(),
                exchange1: "Binance".to_string(),
                exchange2: "Kraken".to_string(),
                asset_pair: "BTC/USD".to_string(),
                price1: 45000.0,
                price2: 45100.0,
                spread_percentage: 0.22,
                volume: 1.5,
            },
            ExportData {
                timestamp: "2024-01-01 12:05:00".to_string(),
                exchange1: "Coinbase".to_string(),
                exchange2: "Binance".to_string(),
                asset_pair: "ETH/USD".to_string(),
                price1: 2500.0,
                price2: 2480.0,
                spread_percentage: -0.80,
                volume: 10.0,
            },
        ]
    }

    #[test]
    fn test_exporter_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter = DataExporter::new(temp_dir.path());
        assert!(exporter.is_ok());
    }

    #[test]
    fn test_export_csv() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let result = exporter.export_csv("test_export.csv", &data);
        assert!(result.is_ok());

        let file_path = result.unwrap();
        assert!(file_path.exists());
        assert!(Path::new(&file_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("test_export.csv"));
    }

    #[test]
    fn test_export_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let result = exporter.export_json("test_export.json", &data);
        assert!(result.is_ok());

        let file_path = result.unwrap();
        assert!(file_path.exists());
        assert!(Path::new(&file_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("test_export.json"));
    }

    #[test]
    fn test_export_jsonl() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let result = exporter.export_jsonl("test_export.jsonl", &data);
        assert!(result.is_ok());

        let file_path = result.unwrap();
        assert!(file_path.exists());
        assert!(Path::new(&file_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("test_export.jsonl"));
    }

    #[test]
    fn test_export_data_serialization() {
        let data = create_test_data();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].exchange1, "Binance");
        assert_eq!(data[1].asset_pair, "ETH/USD");
    }

    #[test]
    fn test_generate_filename() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");

        let filename = exporter.generate_filename("arb_data", "csv");
        assert!(filename.contains("arb_data"));
        assert!(filename.contains(".csv"));
        assert!(filename.contains("_")); // Contains timestamp separator
    }

    #[test]
    fn test_output_dir_persistence() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let temp_path = temp_dir.path().to_path_buf();
        let exporter =
            DataExporter::new(&temp_path).expect("Failed to create DataExporter");

        assert_eq!(exporter.output_dir(), &temp_path);
    }

    #[test]
    fn test_multiple_exports() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let csv_result = exporter.export_csv("data.csv", &data);
        let json_result = exporter.export_json("data.json", &data);
        let jsonl_result = exporter.export_jsonl("data.jsonl", &data);

        assert!(csv_result.is_ok());
        assert!(json_result.is_ok());
        assert!(jsonl_result.is_ok());

        let dir_contents = fs::read_dir(temp_dir.path()).expect("Failed to read directory");
        let file_count = dir_contents.count();
        assert_eq!(file_count, 3);
    }

    #[test]
    fn test_csv_content_integrity() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let file_path = exporter
            .export_csv("integrity_test.csv", &data)
            .expect("Failed to export CSV");

        let content = fs::read_to_string(&file_path).expect("Failed to read CSV file");
        assert!(content.contains("Binance"));
        assert!(content.contains("BTC/USD"));
        assert!(content.contains("45000"));
    }

    #[test]
    fn test_json_content_integrity() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let file_path = exporter
            .export_json("integrity_test.json", &data)
            .expect("Failed to export JSON");

        let content = fs::read_to_string(&file_path).expect("Failed to read JSON file");
        assert!(content.contains("Binance"));
        assert!(content.contains("BTC/USD"));
        assert!(content.contains("45000"));
    }

    #[test]
    fn test_jsonl_content_integrity() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let data = create_test_data();

        let file_path = exporter
            .export_jsonl("integrity_test.jsonl", &data)
            .expect("Failed to export JSONL");

        let content = fs::read_to_string(&file_path).expect("Failed to read JSONL file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2); // Should have 2 lines for 2 records
        assert!(content.contains("Binance"));
        assert!(content.contains("Kraken"));
    }

    #[test]
    fn test_nonexistent_directory_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let nested_path = temp_dir.path().join("nested/deep/directory");

        let result = DataExporter::new(&nested_path);
        assert!(result.is_ok());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_export_empty_data() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let exporter =
            DataExporter::new(temp_dir.path()).expect("Failed to create DataExporter");
        let empty_data: Vec<ExportData> = vec![];

        let csv_result = exporter.export_csv("empty.csv", &empty_data);
        let json_result = exporter.export_json("empty.json", &empty_data);
        let jsonl_result = exporter.export_jsonl("empty.jsonl", &empty_data);

        assert!(csv_result.is_ok());
        assert!(json_result.is_ok());
        assert!(jsonl_result.is_ok());
    }
}
