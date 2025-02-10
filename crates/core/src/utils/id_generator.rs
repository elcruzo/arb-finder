use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

pub struct IdGenerator {
    counter: Arc<AtomicU64>,
    node_id: u16,
}

impl IdGenerator {
    pub fn new(node_id: u16) -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(0)),
            node_id,
        }
    }

    pub fn generate_uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn generate_order_id(&self) -> String {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("ORD-{}-{}-{:06}", timestamp, self.node_id, counter % 1_000_000)
    }

    pub fn generate_trade_id(&self) -> String {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("TRD-{}-{}-{:06}", timestamp, self.node_id, counter % 1_000_000)
    }

    pub fn generate_opportunity_id(&self) -> String {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("ARB-{}-{}-{:06}", timestamp, self.node_id, counter % 1_000_000)
    }

    pub fn generate_execution_id(&self) -> String {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("EXE-{}-{}-{:06}", timestamp, self.node_id, counter % 1_000_000)
    }

    pub fn generate_session_id(&self) -> String {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("SES-{}-{}-{:06}", timestamp, self.node_id, counter % 1_000_000)
    }

    pub fn generate_snowflake_id(&self) -> u64 {
        let timestamp = (Utc::now().timestamp_millis() as u64) << 22;
        let node = (self.node_id as u64) << 12;
        let sequence = self.counter.fetch_add(1, Ordering::SeqCst) & 0xFFF;
        
        timestamp | node | sequence
    }

    pub fn generate_client_order_id(&self, venue: &str) -> String {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("{}-{}-{:06}", venue.to_uppercase(), timestamp, counter % 1_000_000)
    }

    pub fn generate_nonce(&self) -> u64 {
        Utc::now().timestamp_millis() as u64 * 1000 + (self.counter.fetch_add(1, Ordering::SeqCst) % 1000)
    }

    pub fn reset_counter(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }

    pub fn get_counter(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl Clone for IdGenerator {
    fn clone(&self) -> Self {
        Self {
            counter: Arc::clone(&self.counter),
            node_id: self.node_id,
        }
    }
}

lazy_static::lazy_static! {
    static ref GLOBAL_ID_GENERATOR: IdGenerator = IdGenerator::new(
        std::env::var("ARBFINDER_NODE_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
    );
}

pub fn generate_uuid() -> String {
    GLOBAL_ID_GENERATOR.generate_uuid()
}

pub fn generate_order_id() -> String {
    GLOBAL_ID_GENERATOR.generate_order_id()
}

pub fn generate_trade_id() -> String {
    GLOBAL_ID_GENERATOR.generate_trade_id()
}

pub fn generate_opportunity_id() -> String {
    GLOBAL_ID_GENERATOR.generate_opportunity_id()
}

pub fn generate_execution_id() -> String {
    GLOBAL_ID_GENERATOR.generate_execution_id()
}

pub fn generate_session_id() -> String {
    GLOBAL_ID_GENERATOR.generate_session_id()
}

pub fn generate_snowflake_id() -> u64 {
    GLOBAL_ID_GENERATOR.generate_snowflake_id()
}

pub fn generate_client_order_id(venue: &str) -> String {
    GLOBAL_ID_GENERATOR.generate_client_order_id(venue)
}

pub fn generate_nonce() -> u64 {
    GLOBAL_ID_GENERATOR.generate_nonce()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generation() {
        let generator = IdGenerator::new(1);
        
        let uuid1 = generator.generate_uuid();
        let uuid2 = generator.generate_uuid();
        assert_ne!(uuid1, uuid2);
        assert_eq!(uuid1.len(), 36);
        
        let order_id1 = generator.generate_order_id();
        let order_id2 = generator.generate_order_id();
        assert_ne!(order_id1, order_id2);
        assert!(order_id1.starts_with("ORD-"));
        
        let trade_id = generator.generate_trade_id();
        assert!(trade_id.starts_with("TRD-"));
        
        let arb_id = generator.generate_opportunity_id();
        assert!(arb_id.starts_with("ARB-"));
    }

    #[test]
    fn test_snowflake_id() {
        let generator = IdGenerator::new(1);
        
        let id1 = generator.generate_snowflake_id();
        let id2 = generator.generate_snowflake_id();
        
        assert_ne!(id1, id2);
        assert!(id1 > 0);
        assert!(id2 > 0);
    }

    #[test]
    fn test_client_order_id() {
        let generator = IdGenerator::new(1);
        
        let binance_id = generator.generate_client_order_id("binance");
        let coinbase_id = generator.generate_client_order_id("coinbase");
        
        assert!(binance_id.starts_with("BINANCE-"));
        assert!(coinbase_id.starts_with("COINBASE-"));
        assert_ne!(binance_id, coinbase_id);
    }

    #[test]
    fn test_nonce_generation() {
        let generator = IdGenerator::new(1);
        
        let nonce1 = generator.generate_nonce();
        let nonce2 = generator.generate_nonce();
        
        assert_ne!(nonce1, nonce2);
        assert!(nonce2 > nonce1);
    }

    #[test]
    fn test_counter_operations() {
        let generator = IdGenerator::new(1);
        
        let initial = generator.get_counter();
        generator.generate_order_id();
        let after_gen = generator.get_counter();
        
        assert!(after_gen > initial);
        
        generator.reset_counter();
        let after_reset = generator.get_counter();
        assert_eq!(after_reset, 0);
    }

    #[test]
    fn test_global_functions() {
        let uuid = generate_uuid();
        let order_id = generate_order_id();
        let trade_id = generate_trade_id();
        
        assert_eq!(uuid.len(), 36);
        assert!(order_id.starts_with("ORD-"));
        assert!(trade_id.starts_with("TRD-"));
    }
}