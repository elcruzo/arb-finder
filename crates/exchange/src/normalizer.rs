use arbfinder_core::{ArbFinderError, Result, Symbol, OrderSide, OrderType};
use serde_json::Value;
use std::collections::HashMap;

use crate::traits::SymbolNormalizer;

#[derive(Debug, Clone)]
pub struct DefaultSymbolNormalizer {
    symbol_mappings: HashMap<String, Symbol>,
    reverse_symbol_mappings: HashMap<Symbol, String>,
    side_mappings: HashMap<String, OrderSide>,
    reverse_side_mappings: HashMap<OrderSide, String>,
    type_mappings: HashMap<String, OrderType>,
    reverse_type_mappings: HashMap<OrderType, String>,
}

impl DefaultSymbolNormalizer {
    pub fn new() -> Self {
        let mut normalizer = Self {
            symbol_mappings: HashMap::new(),
            reverse_symbol_mappings: HashMap::new(),
            side_mappings: HashMap::new(),
            reverse_side_mappings: HashMap::new(),
            type_mappings: HashMap::new(),
            reverse_type_mappings: HashMap::new(),
        };

        normalizer.init_default_mappings();
        normalizer
    }

    fn init_default_mappings(&mut self) {
        // Default side mappings
        self.add_side_mapping("buy", OrderSide::Buy);
        self.add_side_mapping("BUY", OrderSide::Buy);
        self.add_side_mapping("bid", OrderSide::Buy);
        self.add_side_mapping("BID", OrderSide::Buy);
        self.add_side_mapping("sell", OrderSide::Sell);
        self.add_side_mapping("SELL", OrderSide::Sell);
        self.add_side_mapping("ask", OrderSide::Sell);
        self.add_side_mapping("ASK", OrderSide::Sell);

        // Default order type mappings
        self.add_type_mapping("market", OrderType::Market);
        self.add_type_mapping("MARKET", OrderType::Market);
        self.add_type_mapping("limit", OrderType::Limit);
        self.add_type_mapping("LIMIT", OrderType::Limit);
        self.add_type_mapping("stop", OrderType::StopMarket);
        self.add_type_mapping("STOP", OrderType::StopMarket);
        self.add_type_mapping("stop_market", OrderType::StopMarket);
        self.add_type_mapping("STOP_MARKET", OrderType::StopMarket);
        self.add_type_mapping("stop_limit", OrderType::StopLimit);
        self.add_type_mapping("STOP_LIMIT", OrderType::StopLimit);
        self.add_type_mapping("post_only", OrderType::PostOnly);
        self.add_type_mapping("POST_ONLY", OrderType::PostOnly);
        self.add_type_mapping("fill_or_kill", OrderType::FillOrKill);
        self.add_type_mapping("FILL_OR_KILL", OrderType::FillOrKill);
        self.add_type_mapping("fok", OrderType::FillOrKill);
        self.add_type_mapping("FOK", OrderType::FillOrKill);
        self.add_type_mapping("immediate_or_cancel", OrderType::ImmediateOrCancel);
        self.add_type_mapping("IMMEDIATE_OR_CANCEL", OrderType::ImmediateOrCancel);
        self.add_type_mapping("ioc", OrderType::ImmediateOrCancel);
        self.add_type_mapping("IOC", OrderType::ImmediateOrCancel);
    }

    pub fn add_symbol_mapping(&mut self, exchange_symbol: String, normalized_symbol: Symbol) {
        self.reverse_symbol_mappings.insert(normalized_symbol.clone(), exchange_symbol.clone());
        self.symbol_mappings.insert(exchange_symbol, normalized_symbol);
    }

    pub fn add_side_mapping(&mut self, exchange_side: &str, normalized_side: OrderSide) {
        self.reverse_side_mappings.insert(normalized_side, exchange_side.to_string());
        self.side_mappings.insert(exchange_side.to_string(), normalized_side);
    }

    pub fn add_type_mapping(&mut self, exchange_type: &str, normalized_type: OrderType) {
        self.reverse_type_mappings.insert(normalized_type, exchange_type.to_string());
        self.type_mappings.insert(exchange_type.to_string(), normalized_type);
    }

    pub fn parse_symbol_from_parts(base: &str, quote: &str) -> Symbol {
        Symbol::new(base.to_uppercase(), quote.to_uppercase())
    }

    pub fn parse_symbol_from_string(symbol_str: &str) -> Result<Symbol> {
        // Try different separators
        for separator in &["/", "-", "_", ""] {
            if separator.is_empty() {
                // For concatenated symbols, try common quote currencies
                for quote in &["USDT", "USDC", "USD", "BTC", "ETH", "BNB"] {
                    if symbol_str.ends_with(quote) && symbol_str.len() > quote.len() {
                        let base = &symbol_str[..symbol_str.len() - quote.len()];
                        return Ok(Symbol::new(base, quote));
                    }
                }
            } else if let Some(pos) = symbol_str.find(separator) {
                let base = &symbol_str[..pos];
                let quote = &symbol_str[pos + separator.len()..];
                return Ok(Symbol::new(base, quote));
            }
        }

        Err(ArbFinderError::InvalidData(format!("Unable to parse symbol: {}", symbol_str)))
    }

    pub fn format_symbol_for_exchange(&self, symbol: &Symbol, format: SymbolFormat) -> String {
        match format {
            SymbolFormat::Slash => format!("{}/{}", symbol.base, symbol.quote),
            SymbolFormat::Dash => format!("{}-{}", symbol.base, symbol.quote),
            SymbolFormat::Underscore => format!("{}_{}", symbol.base, symbol.quote),
            SymbolFormat::Concatenated => format!("{}{}", symbol.base, symbol.quote),
            SymbolFormat::Lower => format!("{}{}", symbol.base.to_lowercase(), symbol.quote.to_lowercase()),
            SymbolFormat::Upper => format!("{}{}", symbol.base.to_uppercase(), symbol.quote.to_uppercase()),
        }
    }
}

impl Default for DefaultSymbolNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolNormalizer for DefaultSymbolNormalizer {
    fn normalize_symbol(&self, exchange_symbol: &str) -> Result<Symbol> {
        // Check direct mapping first
        if let Some(symbol) = self.symbol_mappings.get(exchange_symbol) {
            return Ok(symbol.clone());
        }

        // Try to parse the symbol
        Self::parse_symbol_from_string(exchange_symbol)
    }

    fn denormalize_symbol(&self, symbol: &Symbol) -> Result<String> {
        // Check reverse mapping first
        if let Some(exchange_symbol) = self.reverse_symbol_mappings.get(symbol) {
            return Ok(exchange_symbol.clone());
        }

        // Default to slash format
        Ok(self.format_symbol_for_exchange(symbol, SymbolFormat::Slash))
    }

    fn normalize_side(&self, exchange_side: &str) -> Result<OrderSide> {
        self.side_mappings
            .get(exchange_side)
            .copied()
            .ok_or_else(|| ArbFinderError::InvalidData(format!("Unknown side: {}", exchange_side)))
    }

    fn denormalize_side(&self, side: OrderSide) -> String {
        self.reverse_side_mappings
            .get(&side)
            .cloned()
            .unwrap_or_else(|| side.to_string())
    }

    fn normalize_order_type(&self, exchange_type: &str) -> Result<OrderType> {
        self.type_mappings
            .get(exchange_type)
            .copied()
            .ok_or_else(|| ArbFinderError::InvalidData(format!("Unknown order type: {}", exchange_type)))
    }

    fn denormalize_order_type(&self, order_type: OrderType) -> String {
        self.reverse_type_mappings
            .get(&order_type)
            .cloned()
            .unwrap_or_else(|| order_type.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SymbolFormat {
    Slash,       // BTC/USDT
    Dash,        // BTC-USDT
    Underscore,  // BTC_USDT
    Concatenated, // BTCUSDT
    Lower,       // btcusdt
    Upper,       // BTCUSDT
}

pub fn normalize_price_precision(price: f64, precision: u32) -> f64 {
    let factor = 10.0_f64.powi(precision as i32);
    (price * factor).round() / factor
}

pub fn normalize_quantity_precision(quantity: f64, precision: u32) -> f64 {
    let factor = 10.0_f64.powi(precision as i32);
    (quantity * factor).floor() / factor
}

pub fn extract_string_field(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Missing or invalid field: {}", field)))
}

pub fn extract_f64_field(value: &Value, field: &str) -> Result<f64> {
    value
        .get(field)
        .and_then(|v| match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        })
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Missing or invalid numeric field: {}", field)))
}

pub fn extract_u64_field(value: &Value, field: &str) -> Result<u64> {
    value
        .get(field)
        .and_then(|v| match v {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        })
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Missing or invalid integer field: {}", field)))
}

pub fn extract_bool_field(value: &Value, field: &str) -> Result<bool> {
    value
        .get(field)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Missing or invalid boolean field: {}", field)))
}

pub fn parse_timestamp_ms(value: &Value, field: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    let timestamp = extract_u64_field(value, field)?;
    chrono::DateTime::from_timestamp_millis(timestamp as i64)
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Invalid timestamp: {}", timestamp)))
}

pub fn parse_timestamp_s(value: &Value, field: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    let timestamp = extract_u64_field(value, field)?;
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Invalid timestamp: {}", timestamp)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_parsing() {
        // Test slash format
        let symbol = DefaultSymbolNormalizer::parse_symbol_from_string("BTC/USDT").unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");

        // Test dash format
        let symbol = DefaultSymbolNormalizer::parse_symbol_from_string("ETH-USD").unwrap();
        assert_eq!(symbol.base, "ETH");
        assert_eq!(symbol.quote, "USD");

        // Test concatenated format
        let symbol = DefaultSymbolNormalizer::parse_symbol_from_string("ADAUSDT").unwrap();
        assert_eq!(symbol.base, "ADA");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_symbol_formatting() {
        let normalizer = DefaultSymbolNormalizer::new();
        let symbol = Symbol::new("BTC", "USDT");

        assert_eq!(normalizer.format_symbol_for_exchange(&symbol, SymbolFormat::Slash), "BTC/USDT");
        assert_eq!(normalizer.format_symbol_for_exchange(&symbol, SymbolFormat::Dash), "BTC-USDT");
        assert_eq!(normalizer.format_symbol_for_exchange(&symbol, SymbolFormat::Underscore), "BTC_USDT");
        assert_eq!(normalizer.format_symbol_for_exchange(&symbol, SymbolFormat::Concatenated), "BTCUSDT");
        assert_eq!(normalizer.format_symbol_for_exchange(&symbol, SymbolFormat::Lower), "btcusdt");
        assert_eq!(normalizer.format_symbol_for_exchange(&symbol, SymbolFormat::Upper), "BTCUSDT");
    }

    #[test]
    fn test_side_normalization() {
        let normalizer = DefaultSymbolNormalizer::new();

        assert_eq!(normalizer.normalize_side("buy").unwrap(), OrderSide::Buy);
        assert_eq!(normalizer.normalize_side("BUY").unwrap(), OrderSide::Buy);
        assert_eq!(normalizer.normalize_side("sell").unwrap(), OrderSide::Sell);
        assert_eq!(normalizer.normalize_side("SELL").unwrap(), OrderSide::Sell);

        assert_eq!(normalizer.denormalize_side(OrderSide::Buy), "buy");
        assert_eq!(normalizer.denormalize_side(OrderSide::Sell), "sell");
    }

    #[test]
    fn test_order_type_normalization() {
        let normalizer = DefaultSymbolNormalizer::new();

        assert_eq!(normalizer.normalize_order_type("market").unwrap(), OrderType::Market);
        assert_eq!(normalizer.normalize_order_type("LIMIT").unwrap(), OrderType::Limit);
        assert_eq!(normalizer.normalize_order_type("fok").unwrap(), OrderType::FillOrKill);

        assert_eq!(normalizer.denormalize_order_type(OrderType::Market), "market");
        assert_eq!(normalizer.denormalize_order_type(OrderType::Limit), "limit");
    }

    #[test]
    fn test_precision_normalization() {
        assert_eq!(normalize_price_precision(123.456789, 2), 123.46);
        assert_eq!(normalize_price_precision(123.456789, 4), 123.4568);
        
        assert_eq!(normalize_quantity_precision(123.456789, 2), 123.45);
        assert_eq!(normalize_quantity_precision(123.456789, 0), 123.0);
    }

    #[test]
    fn test_field_extraction() {
        use serde_json::json;

        let data = json!({
            "string_field": "test",
            "number_field": 123.45,
            "string_number": "678.90",
            "bool_field": true,
            "timestamp": 1640995200000u64
        });

        assert_eq!(extract_string_field(&data, "string_field").unwrap(), "test");
        assert_eq!(extract_f64_field(&data, "number_field").unwrap(), 123.45);
        assert_eq!(extract_f64_field(&data, "string_number").unwrap(), 678.90);
        assert_eq!(extract_bool_field(&data, "bool_field").unwrap(), true);
        assert_eq!(extract_u64_field(&data, "timestamp").unwrap(), 1640995200000);
    }
}