use rust_decimal::Decimal;
use crate::types::{Symbol, OrderSide, OrderType};
use crate::error::{ArbFinderError, Result};

pub fn validate_symbol(symbol: &Symbol) -> Result<()> {
    if symbol.base.is_empty() {
        return Err(ArbFinderError::InvalidData("Base asset cannot be empty".to_string()));
    }
    
    if symbol.quote.is_empty() {
        return Err(ArbFinderError::InvalidData("Quote asset cannot be empty".to_string()));
    }
    
    if symbol.base == symbol.quote {
        return Err(ArbFinderError::InvalidData("Base and quote assets cannot be the same".to_string()));
    }
    
    if !is_valid_asset_name(&symbol.base) {
        return Err(ArbFinderError::InvalidData(format!("Invalid base asset name: {}", symbol.base)));
    }
    
    if !is_valid_asset_name(&symbol.quote) {
        return Err(ArbFinderError::InvalidData(format!("Invalid quote asset name: {}", symbol.quote)));
    }
    
    Ok(())
}

pub fn validate_price(price: Decimal) -> Result<()> {
    if price <= Decimal::ZERO {
        return Err(ArbFinderError::InvalidData("Price must be positive".to_string()));
    }
    
    if price > Decimal::from(1_000_000_000) {
        return Err(ArbFinderError::InvalidData("Price is too large".to_string()));
    }
    
    Ok(())
}

pub fn validate_quantity(quantity: Decimal) -> Result<()> {
    if quantity <= Decimal::ZERO {
        return Err(ArbFinderError::InvalidData("Quantity must be positive".to_string()));
    }
    
    if quantity > Decimal::from(1_000_000_000) {
        return Err(ArbFinderError::InvalidData("Quantity is too large".to_string()));
    }
    
    Ok(())
}

pub fn validate_order_request(
    symbol: &Symbol,
    _side: OrderSide,
    order_type: OrderType,
    quantity: Decimal,
    price: Option<Decimal>,
) -> Result<()> {
    validate_symbol(symbol)?;
    validate_quantity(quantity)?;
    
    match order_type {
        OrderType::Market => {
            if price.is_some() {
                return Err(ArbFinderError::InvalidOrder("Market orders should not have a price".to_string()));
            }
        }
        OrderType::Limit | OrderType::StopLimit | OrderType::PostOnly => {
            match price {
                Some(p) => validate_price(p)?,
                None => return Err(ArbFinderError::InvalidOrder(format!("{} orders must have a price", order_type))),
            }
        }
        OrderType::StopMarket => {
            if price.is_some() {
                return Err(ArbFinderError::InvalidOrder("Stop market orders should not have a limit price".to_string()));
            }
        }
        _ => {}
    }
    
    Ok(())
}

pub fn validate_spread_bps(spread_bps: i32) -> Result<()> {
    if spread_bps < 0 {
        return Err(ArbFinderError::InvalidData("Spread cannot be negative".to_string()));
    }
    
    if spread_bps > 10000 {
        return Err(ArbFinderError::InvalidData("Spread is too large (>100%)".to_string()));
    }
    
    Ok(())
}

pub fn validate_confidence(confidence: f64) -> Result<()> {
    if !(0.0..=1.0).contains(&confidence) {
        return Err(ArbFinderError::InvalidData("Confidence must be between 0 and 1".to_string()));
    }
    
    Ok(())
}

pub fn validate_percentage(percentage: f64, name: &str) -> Result<()> {
    if !(0.0..=100.0).contains(&percentage) {
        return Err(ArbFinderError::InvalidData(format!("{} must be between 0 and 100", name)));
    }
    
    Ok(())
}

pub fn validate_api_key(api_key: &str) -> Result<()> {
    if api_key.is_empty() {
        return Err(ArbFinderError::Authentication("API key cannot be empty".to_string()));
    }
    
    if api_key.len() < 16 {
        return Err(ArbFinderError::Authentication("API key is too short".to_string()));
    }
    
    if api_key.len() > 256 {
        return Err(ArbFinderError::Authentication("API key is too long".to_string()));
    }
    
    Ok(())
}

pub fn validate_secret_key(secret_key: &str) -> Result<()> {
    if secret_key.is_empty() {
        return Err(ArbFinderError::Authentication("Secret key cannot be empty".to_string()));
    }
    
    if secret_key.len() < 16 {
        return Err(ArbFinderError::Authentication("Secret key is too short".to_string()));
    }
    
    if secret_key.len() > 256 {
        return Err(ArbFinderError::Authentication("Secret key is too long".to_string()));
    }
    
    Ok(())
}

pub fn validate_url(url: &str) -> Result<()> {
    url::Url::parse(url)
        .map_err(|e| ArbFinderError::InvalidData(format!("Invalid URL: {}", e)))?;
    
    Ok(())
}

pub fn validate_symbol_pair(pair: &str) -> Result<Symbol> {
    let symbol = Symbol::from_pair(pair)
        .ok_or_else(|| ArbFinderError::InvalidData(format!("Invalid symbol pair format: {}", pair)))?;
    
    validate_symbol(&symbol)?;
    Ok(symbol)
}

pub fn is_valid_asset_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 20 {
        return false;
    }
    
    name.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn is_valid_order_id(order_id: &str) -> bool {
    !order_id.is_empty() && order_id.len() <= 100 && order_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn normalize_symbol_pair(pair: &str) -> String {
    pair.to_uppercase().replace('_', "/")
}

pub fn sanitize_string(input: &str, max_length: usize) -> String {
    input.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || *c == '-' || *c == '_')
        .take(max_length)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_symbol() {
        let valid_symbol = Symbol::new("BTC", "USDT");
        assert!(validate_symbol(&valid_symbol).is_ok());
        
        let invalid_symbol = Symbol::new("", "USDT");
        assert!(validate_symbol(&invalid_symbol).is_err());
        
        let same_assets = Symbol::new("BTC", "BTC");
        assert!(validate_symbol(&same_assets).is_err());
    }

    #[test]
    fn test_validate_price() {
        assert!(validate_price(Decimal::from(100)).is_ok());
        assert!(validate_price(Decimal::ZERO).is_err());
        assert!(validate_price(Decimal::from(-10)).is_err());
    }

    #[test]
    fn test_validate_quantity() {
        assert!(validate_quantity(Decimal::from(1)).is_ok());
        assert!(validate_quantity(Decimal::ZERO).is_err());
        assert!(validate_quantity(Decimal::from(-1)).is_err());
    }

    #[test]
    fn test_validate_confidence() {
        assert!(validate_confidence(0.5).is_ok());
        assert!(validate_confidence(0.0).is_ok());
        assert!(validate_confidence(1.0).is_ok());
        assert!(validate_confidence(-0.1).is_err());
        assert!(validate_confidence(1.1).is_err());
    }

    #[test]
    fn test_is_valid_asset_name() {
        assert!(is_valid_asset_name("BTC"));
        assert!(is_valid_asset_name("USDT"));
        assert!(!is_valid_asset_name(""));
        assert!(!is_valid_asset_name("BTC-USD"));
        assert!(!is_valid_asset_name("verylongassetnamethatexceedsthelimit"));
    }

    #[test]
    fn test_normalize_symbol_pair() {
        assert_eq!(normalize_symbol_pair("btc_usdt"), "BTC/USDT");
        assert_eq!(normalize_symbol_pair("eth/usd"), "ETH/USD");
        assert_eq!(normalize_symbol_pair("BTC/USDT"), "BTC/USDT");
    }
}