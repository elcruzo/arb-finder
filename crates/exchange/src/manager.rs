use arbfinder_core::{ArbFinderError, Result, Symbol, VenueId};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::traits::{ExchangeAdapter, ConnectionStatus, SubscriptionInfo};

#[derive(Debug)]
pub struct ExchangeManager {
    adapters: Arc<RwLock<HashMap<VenueId, Arc<Mutex<Box<dyn ExchangeAdapter>>>>>>,
    connections: Arc<RwLock<HashMap<VenueId, ConnectionStatus>>>,
    subscriptions: Arc<RwLock<HashMap<VenueId, Vec<SubscriptionInfo>>>>,
}

impl ExchangeManager {
    pub fn new() -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_adapter(&self, adapter: Box<dyn ExchangeAdapter>) -> Result<()> {
        let venue_id = adapter.venue_id();
        info!("Adding adapter for venue: {}", venue_id);

        let mut adapters = self.adapters.write().await;
        let mut connections = self.connections.write().await;
        let mut subscriptions = self.subscriptions.write().await;

        adapters.insert(venue_id.clone(), Arc::new(Mutex::new(adapter)));
        connections.insert(venue_id.clone(), ConnectionStatus {
            connected: false,
            last_ping: None,
            reconnect_count: 0,
            error_count: 0,
            last_error: None,
        });
        subscriptions.insert(venue_id, Vec::new());

        Ok(())
    }

    pub async fn remove_adapter(&self, venue_id: &VenueId) -> Result<()> {
        info!("Removing adapter for venue: {}", venue_id);

        // Disconnect first if connected
        if let Err(e) = self.disconnect(venue_id).await {
            warn!("Failed to disconnect before removal: {}", e);
        }

        let mut adapters = self.adapters.write().await;
        let mut connections = self.connections.write().await;
        let mut subscriptions = self.subscriptions.write().await;

        adapters.remove(venue_id);
        connections.remove(venue_id);
        subscriptions.remove(venue_id);

        Ok(())
    }

    pub async fn connect(&self, venue_id: &VenueId) -> Result<()> {
        info!("Connecting to venue: {}", venue_id);

        let adapters = self.adapters.read().await;
        let adapter = adapters
            .get(venue_id)
            .ok_or_else(|| ArbFinderError::Exchange(format!("Adapter not found for venue: {}", venue_id)))?;

        let mut adapter_guard = adapter.lock().await;
        match adapter_guard.connect().await {
            Ok(_) => {
                drop(adapter_guard);
                drop(adapters);

                let mut connections = self.connections.write().await;
                if let Some(status) = connections.get_mut(venue_id) {
                    status.connected = true;
                    status.last_ping = None;
                    status.last_error = None;
                }

                info!("Successfully connected to venue: {}", venue_id);
                Ok(())
            }
            Err(e) => {
                drop(adapter_guard);
                drop(adapters);

                let mut connections = self.connections.write().await;
                if let Some(status) = connections.get_mut(venue_id) {
                    status.connected = false;
                    status.error_count += 1;
                    status.last_error = Some(e.to_string());
                }

                error!("Failed to connect to venue {}: {}", venue_id, e);
                Err(e)
            }
        }
    }

    pub async fn disconnect(&self, venue_id: &VenueId) -> Result<()> {
        info!("Disconnecting from venue: {}", venue_id);

        let adapters = self.adapters.read().await;
        let adapter = adapters
            .get(venue_id)
            .ok_or_else(|| ArbFinderError::Exchange(format!("Adapter not found for venue: {}", venue_id)))?;

        let mut adapter_guard = adapter.lock().await;
        match adapter_guard.disconnect().await {
            Ok(_) => {
                drop(adapter_guard);
                drop(adapters);

                let mut connections = self.connections.write().await;
                if let Some(status) = connections.get_mut(venue_id) {
                    status.connected = false;
                    status.last_ping = None;
                }

                // Clear subscriptions
                let mut subscriptions = self.subscriptions.write().await;
                if let Some(subs) = subscriptions.get_mut(venue_id) {
                    subs.clear();
                }

                info!("Successfully disconnected from venue: {}", venue_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to disconnect from venue {}: {}", venue_id, e);
                Err(e)
            }
        }
    }

    pub async fn connect_all(&self) -> Result<()> {
        info!("Connecting to all venues");

        let adapters = self.adapters.read().await;
        let venue_ids: Vec<VenueId> = adapters.keys().cloned().collect();
        drop(adapters);

        let mut results = Vec::new();
        for venue_id in venue_ids {
            match self.connect(&venue_id).await {
                Ok(_) => results.push(Ok(())),
                Err(e) => {
                    warn!("Failed to connect to {}: {}", venue_id, e);
                    results.push(Err(e));
                }
            }
        }

        let failed_count = results.iter().filter(|r| r.is_err()).count();
        if failed_count > 0 {
            warn!("{} venues failed to connect", failed_count);
        }

        Ok(())
    }

    pub async fn disconnect_all(&self) -> Result<()> {
        info!("Disconnecting from all venues");

        let adapters = self.adapters.read().await;
        let venue_ids: Vec<VenueId> = adapters.keys().cloned().collect();
        drop(adapters);

        for venue_id in venue_ids {
            if let Err(e) = self.disconnect(&venue_id).await {
                warn!("Failed to disconnect from {}: {}", venue_id, e);
            }
        }

        Ok(())
    }

    pub async fn is_connected(&self, venue_id: &VenueId) -> bool {
        let connections = self.connections.read().await;
        connections
            .get(venue_id)
            .map(|status| status.connected)
            .unwrap_or(false)
    }

    pub async fn get_connection_status(&self, venue_id: &VenueId) -> Option<ConnectionStatus> {
        let connections = self.connections.read().await;
        connections.get(venue_id).cloned()
    }

    pub async fn get_all_connection_statuses(&self) -> HashMap<VenueId, ConnectionStatus> {
        self.connections.read().await.clone()
    }

    pub async fn subscribe_orderbook(&self, venue_id: &VenueId, symbol: &Symbol, depth: Option<u32>) -> Result<()> {
        debug!("Subscribing to orderbook for {} on {}", symbol, venue_id);

        let adapters = self.adapters.read().await;
        let adapter = adapters
            .get(venue_id)
            .ok_or_else(|| ArbFinderError::Exchange(format!("Adapter not found for venue: {}", venue_id)))?;

        let mut adapter_guard = adapter.lock().await;
        match adapter_guard.subscribe_orderbook(symbol, depth).await {
            Ok(_) => {
                drop(adapter_guard);
                drop(adapters);

                let mut subscriptions = self.subscriptions.write().await;
                if let Some(subs) = subscriptions.get_mut(venue_id) {
                    subs.push(SubscriptionInfo {
                        symbol: symbol.clone(),
                        data_type: "orderbook".to_string(),
                        subscribed_at: chrono::Utc::now(),
                        message_count: 0,
                        last_message: None,
                    });
                }

                debug!("Successfully subscribed to orderbook for {} on {}", symbol, venue_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to subscribe to orderbook for {} on {}: {}", symbol, venue_id, e);
                Err(e)
            }
        }
    }

    pub async fn subscribe_trades(&self, venue_id: &VenueId, symbol: &Symbol) -> Result<()> {
        debug!("Subscribing to trades for {} on {}", symbol, venue_id);

        let adapters = self.adapters.read().await;
        let adapter = adapters
            .get(venue_id)
            .ok_or_else(|| ArbFinderError::Exchange(format!("Adapter not found for venue: {}", venue_id)))?;

        let mut adapter_guard = adapter.lock().await;
        match adapter_guard.subscribe_trades(symbol).await {
            Ok(_) => {
                drop(adapter_guard);
                drop(adapters);

                let mut subscriptions = self.subscriptions.write().await;
                if let Some(subs) = subscriptions.get_mut(venue_id) {
                    subs.push(SubscriptionInfo {
                        symbol: symbol.clone(),
                        data_type: "trades".to_string(),
                        subscribed_at: chrono::Utc::now(),
                        message_count: 0,
                        last_message: None,
                    });
                }

                debug!("Successfully subscribed to trades for {} on {}", symbol, venue_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to subscribe to trades for {} on {}: {}", symbol, venue_id, e);
                Err(e)
            }
        }
    }

    pub async fn unsubscribe_orderbook(&self, venue_id: &VenueId, symbol: &Symbol) -> Result<()> {
        debug!("Unsubscribing from orderbook for {} on {}", symbol, venue_id);

        let adapters = self.adapters.read().await;
        let adapter = adapters
            .get(venue_id)
            .ok_or_else(|| ArbFinderError::Exchange(format!("Adapter not found for venue: {}", venue_id)))?;

        let mut adapter_guard = adapter.lock().await;
        match adapter_guard.unsubscribe_orderbook(symbol).await {
            Ok(_) => {
                drop(adapter_guard);
                drop(adapters);

                let mut subscriptions = self.subscriptions.write().await;
                if let Some(subs) = subscriptions.get_mut(venue_id) {
                    subs.retain(|sub| !(sub.symbol == *symbol && sub.data_type == "orderbook"));
                }

                debug!("Successfully unsubscribed from orderbook for {} on {}", symbol, venue_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to unsubscribe from orderbook for {} on {}: {}", symbol, venue_id, e);
                Err(e)
            }
        }
    }

    pub async fn get_subscriptions(&self, venue_id: &VenueId) -> Vec<SubscriptionInfo> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(venue_id).cloned().unwrap_or_default()
    }

    pub async fn get_all_subscriptions(&self) -> HashMap<VenueId, Vec<SubscriptionInfo>> {
        self.subscriptions.read().await.clone()
    }

    pub async fn get_adapter(&self, venue_id: &VenueId) -> Option<Arc<Mutex<Box<dyn ExchangeAdapter>>>> {
        let adapters = self.adapters.read().await;
        adapters.get(venue_id).cloned()
    }

    pub async fn get_available_venues(&self) -> Vec<VenueId> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    pub async fn get_connected_venues(&self) -> Vec<VenueId> {
        let connections = self.connections.read().await;
        connections
            .iter()
            .filter(|(_, status)| status.connected)
            .map(|(venue_id, _)| venue_id.clone())
            .collect()
    }

    pub async fn record_message(&self, venue_id: &VenueId, symbol: &Symbol, data_type: &str) {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(subs) = subscriptions.get_mut(venue_id) {
            for sub in subs.iter_mut() {
                if sub.symbol == *symbol && sub.data_type == data_type {
                    sub.message_count += 1;
                    sub.last_message = Some(chrono::Utc::now());
                    break;
                }
            }
        }
    }

    pub async fn record_error(&self, venue_id: &VenueId, error: &str) {
        let mut connections = self.connections.write().await;
        if let Some(status) = connections.get_mut(venue_id) {
            status.error_count += 1;
            status.last_error = Some(error.to_string());
        }
    }

    pub async fn record_ping(&self, venue_id: &VenueId) {
        let mut connections = self.connections.write().await;
        if let Some(status) = connections.get_mut(venue_id) {
            status.last_ping = Some(chrono::Utc::now());
        }
    }

    pub async fn health_check(&self) -> HashMap<VenueId, bool> {
        let mut health_status = HashMap::new();
        
        let adapters = self.adapters.read().await;
        for venue_id in adapters.keys() {
            let is_healthy = match adapters.get(venue_id) {
                Some(adapter) => {
                    let adapter_guard = adapter.lock().await;
                    adapter_guard.is_connected().await
                }
                None => false,
            };
            health_status.insert(venue_id.clone(), is_healthy);
        }

        health_status
    }

    pub async fn restart_adapter(&self, venue_id: &VenueId) -> Result<()> {
        info!("Restarting adapter for venue: {}", venue_id);

        // Disconnect first
        if let Err(e) = self.disconnect(venue_id).await {
            warn!("Error during disconnect before restart: {}", e);
        }

        // Wait a moment
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        // Reconnect
        self.connect(venue_id).await?;

        // Increment reconnect count
        let mut connections = self.connections.write().await;
        if let Some(status) = connections.get_mut(venue_id) {
            status.reconnect_count += 1;
        }

        info!("Successfully restarted adapter for venue: {}", venue_id);
        Ok(())
    }
}

impl Default for ExchangeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait ExchangeManagerExt {
    async fn subscribe_to_symbols(&self, venue_id: &VenueId, symbols: &[Symbol]) -> Result<()>;
    async fn unsubscribe_from_symbols(&self, venue_id: &VenueId, symbols: &[Symbol]) -> Result<()>;
    async fn get_market_data_stats(&self) -> HashMap<VenueId, MarketDataStats>;
}

#[derive(Debug, Clone)]
pub struct MarketDataStats {
    pub total_messages: u64,
    pub messages_per_second: f64,
    pub last_message_time: Option<chrono::DateTime<chrono::Utc>>,
    pub symbols_subscribed: usize,
    pub uptime_percentage: f64,
}

#[async_trait]
impl ExchangeManagerExt for ExchangeManager {
    async fn subscribe_to_symbols(&self, venue_id: &VenueId, symbols: &[Symbol]) -> Result<()> {
        info!("Subscribing to {} symbols on {}", symbols.len(), venue_id);

        for symbol in symbols {
            if let Err(e) = self.subscribe_orderbook(venue_id, symbol, Some(20)).await {
                warn!("Failed to subscribe to orderbook for {} on {}: {}", symbol, venue_id, e);
            }
            
            if let Err(e) = self.subscribe_trades(venue_id, symbol).await {
                warn!("Failed to subscribe to trades for {} on {}: {}", symbol, venue_id, e);
            }
        }

        Ok(())
    }

    async fn unsubscribe_from_symbols(&self, venue_id: &VenueId, symbols: &[Symbol]) -> Result<()> {
        info!("Unsubscribing from {} symbols on {}", symbols.len(), venue_id);

        for symbol in symbols {
            if let Err(e) = self.unsubscribe_orderbook(venue_id, symbol).await {
                warn!("Failed to unsubscribe from orderbook for {} on {}: {}", symbol, venue_id, e);
            }
        }

        Ok(())
    }

    async fn get_market_data_stats(&self) -> HashMap<VenueId, MarketDataStats> {
        let subscriptions = self.subscriptions.read().await;
        let mut stats = HashMap::new();

        for (venue_id, subs) in subscriptions.iter() {
            let total_messages: u64 = subs.iter().map(|s| s.message_count).sum();
            let last_message_time = subs.iter()
                .filter_map(|s| s.last_message)
                .max();
            
            let symbols_subscribed = subs.len();
            
            // Calculate messages per second (rough estimate)
            let messages_per_second = if let Some(last_msg) = last_message_time {
                let elapsed = chrono::Utc::now().signed_duration_since(last_msg);
                if elapsed.num_seconds() > 0 {
                    total_messages as f64 / elapsed.num_seconds() as f64
                } else {
                    0.0
                }
            } else {
                0.0
            };

            stats.insert(venue_id.clone(), MarketDataStats {
                total_messages,
                messages_per_second,
                last_message_time,
                symbols_subscribed,
                uptime_percentage: 100.0, // TODO: Calculate actual uptime
            });
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbfinder_core::{Balance, MarketData, Order, OrderFill, OrderId, OrderRequest, OrderUpdate, VenueCredentials};
    use async_trait::async_trait;
    use futures::Stream;
    use std::pin::Pin;

    // Mock adapter for testing
    struct MockAdapter {
        venue_id: VenueId,
        connected: bool,
    }

    impl MockAdapter {
        fn new(venue_id: VenueId) -> Self {
            Self {
                venue_id,
                connected: false,
            }
        }
    }

    #[async_trait]
    impl ExchangeAdapter for MockAdapter {
        fn venue_id(&self) -> VenueId {
            self.venue_id.clone()
        }

        async fn connect(&mut self) -> Result<()> {
            self.connected = true;
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<()> {
            self.connected = false;
            Ok(())
        }

        async fn is_connected(&self) -> bool {
            self.connected
        }

        // Implement other required methods with minimal functionality for testing
        async fn get_server_time(&self) -> Result<chrono::DateTime<chrono::Utc>> {
            Ok(chrono::Utc::now())
        }

        async fn ping(&self) -> Result<u64> {
            Ok(100)
        }

        async fn get_symbols(&self) -> Result<Vec<Symbol>> {
            Ok(vec![Symbol::new("BTC", "USDT")])
        }

        async fn get_symbol_info(&self, _symbol: &Symbol) -> Result<crate::traits::SymbolInfo> {
            unimplemented!()
        }

        async fn subscribe_orderbook(&mut self, _symbol: &Symbol, _depth: Option<u32>) -> Result<()> {
            Ok(())
        }

        async fn subscribe_trades(&mut self, _symbol: &Symbol) -> Result<()> {
            Ok(())
        }

        async fn subscribe_ticker(&mut self, _symbol: &Symbol) -> Result<()> {
            Ok(())
        }

        async fn unsubscribe_orderbook(&mut self, _symbol: &Symbol) -> Result<()> {
            Ok(())
        }

        async fn unsubscribe_trades(&mut self, _symbol: &Symbol) -> Result<()> {
            Ok(())
        }

        async fn unsubscribe_ticker(&mut self, _symbol: &Symbol) -> Result<()> {
            Ok(())
        }

        async fn market_data_stream(&self) -> Result<Pin<Box<dyn Stream<Item = Result<MarketData>> + Send>>> {
            unimplemented!()
        }

        async fn order_update_stream(&self) -> Result<Pin<Box<dyn Stream<Item = Result<OrderUpdate>> + Send>>> {
            unimplemented!()
        }

        async fn place_order(&mut self, _request: &OrderRequest) -> Result<Order> {
            unimplemented!()
        }

        async fn cancel_order(&mut self, _order_id: &OrderId) -> Result<()> {
            unimplemented!()
        }

        async fn cancel_all_orders(&mut self, _symbol: Option<&Symbol>) -> Result<Vec<OrderId>> {
            unimplemented!()
        }

        async fn get_order(&self, _order_id: &OrderId) -> Result<Option<Order>> {
            unimplemented!()
        }

        async fn get_open_orders(&self, _symbol: Option<&Symbol>) -> Result<Vec<Order>> {
            unimplemented!()
        }

        async fn get_order_history(&self, _symbol: Option<&Symbol>, _limit: Option<u32>) -> Result<Vec<Order>> {
            unimplemented!()
        }

        async fn get_balances(&self) -> Result<Vec<Balance>> {
            unimplemented!()
        }

        async fn get_balance(&self, _asset: &str) -> Result<Option<Balance>> {
            unimplemented!()
        }

        async fn get_trade_history(&self, _symbol: Option<&Symbol>, _limit: Option<u32>) -> Result<Vec<OrderFill>> {
            unimplemented!()
        }

        async fn get_account_info(&self) -> Result<crate::traits::AccountInfo> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_exchange_manager() {
        let manager = ExchangeManager::new();
        let venue_id = VenueId::Binance;

        // Add adapter
        let adapter = Box::new(MockAdapter::new(venue_id.clone()));
        manager.add_adapter(adapter).await.unwrap();

        // Check that it's not connected initially
        assert!(!manager.is_connected(&venue_id).await);

        // Connect
        manager.connect(&venue_id).await.unwrap();
        assert!(manager.is_connected(&venue_id).await);

        // Disconnect
        manager.disconnect(&venue_id).await.unwrap();
        assert!(!manager.is_connected(&venue_id).await);
    }

    #[tokio::test]
    async fn test_subscription_management() {
        let manager = ExchangeManager::new();
        let venue_id = VenueId::Binance;
        let symbol = Symbol::new("BTC", "USDT");

        // Add and connect adapter
        let adapter = Box::new(MockAdapter::new(venue_id.clone()));
        manager.add_adapter(adapter).await.unwrap();
        manager.connect(&venue_id).await.unwrap();

        // Subscribe
        manager.subscribe_orderbook(&venue_id, &symbol, Some(20)).await.unwrap();
        manager.subscribe_trades(&venue_id, &symbol).await.unwrap();

        // Check subscriptions
        let subscriptions = manager.get_subscriptions(&venue_id).await;
        assert_eq!(subscriptions.len(), 2);

        // Unsubscribe
        manager.unsubscribe_orderbook(&venue_id, &symbol).await.unwrap();
        let subscriptions = manager.get_subscriptions(&venue_id).await;
        assert_eq!(subscriptions.len(), 1);
    }
}