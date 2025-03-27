//! OrderBook Manager
//!
//! Manages multiple order books across venues and symbols

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use arbfinder_core::{Symbol, VenueId};
use crate::{FastOrderBook, OrderBookSnapshot, OrderBookUpdate, OrderBookCache};

/// Manages order books for multiple venues and symbols
pub struct OrderBookManager {
    books: Arc<RwLock<HashMap<BookKey, Arc<RwLock<FastOrderBook>>>>>,
    cache: Option<OrderBookCache>,
    max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BookKey {
    venue_id: VenueId,
    symbol: Symbol,
}

impl OrderBookManager {
    pub fn new(max_depth: usize) -> Self {
        Self {
            books: Arc::new(RwLock::new(HashMap::new())),
            cache: None,
            max_depth,
        }
    }

    pub fn with_cache(mut self, cache: OrderBookCache) -> Self {
        self.cache = Some(cache);
        self
    }

    pub async fn get_or_create_book(
        &self,
        venue_id: VenueId,
        symbol: Symbol,
    ) -> Arc<RwLock<FastOrderBook>> {
        let key = BookKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };

        let books = self.books.read().await;
        if let Some(book) = books.get(&key) {
            return Arc::clone(book);
        }
        drop(books);

        // Create new book if it doesn't exist
        let mut books = self.books.write().await;
        
        // Double-check after acquiring write lock
        if let Some(book) = books.get(&key) {
            return Arc::clone(book);
        }

        let new_book = Arc::new(RwLock::new(FastOrderBook::new(symbol, Some(self.max_depth))));
        books.insert(key, Arc::clone(&new_book));
        
        info!("Created new orderbook for {} on {}", new_book.read().await.symbol, venue_id);
        new_book
    }

    pub async fn get_book(&self, venue_id: &VenueId, symbol: &Symbol) -> Option<Arc<RwLock<FastOrderBook>>> {
        let key = BookKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };
        
        let books = self.books.read().await;
        books.get(&key).cloned()
    }

    pub async fn apply_snapshot(&self, venue_id: VenueId, snapshot: OrderBookSnapshot) {
        let book = self.get_or_create_book(venue_id.clone(), snapshot.symbol.clone()).await;
        let mut book_guard = book.write().await;
        snapshot.apply_to_book(&mut book_guard);
        
        debug!(
            "Applied snapshot for {} on {} (sequence: {})",
            snapshot.symbol, venue_id, snapshot.sequence
        );

        // Update cache if available
        if let Some(cache) = &self.cache {
            cache.put(venue_id, book_guard.clone()).await;
        }
    }

    pub async fn apply_updates(&self, venue_id: VenueId, symbol: Symbol, updates: Vec<OrderBookUpdate>) {
        let book = self.get_or_create_book(venue_id.clone(), symbol.clone()).await;
        let mut book_guard = book.write().await;
        book_guard.batch_update(updates.clone());
        
        debug!(
            "Applied {} updates for {} on {}",
            updates.len(),
            symbol,
            venue_id
        );

        // Update cache if available
        if let Some(cache) = &self.cache {
            cache.put(venue_id, book_guard.clone()).await;
        }
    }

    pub async fn remove_book(&self, venue_id: &VenueId, symbol: &Symbol) -> Option<Arc<RwLock<FastOrderBook>>> {
        let key = BookKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };

        let mut books = self.books.write().await;
        let result = books.remove(&key);

        if result.is_some() {
            info!("Removed orderbook for {} on {}", symbol, venue_id);
            
            // Invalidate cache if available
            if let Some(cache) = &self.cache {
                cache.invalidate(venue_id, symbol).await;
            }
        }

        result
    }

    pub async fn clear_venue(&self, venue_id: &VenueId) {
        let mut books = self.books.write().await;
        books.retain(|key, _| &key.venue_id != venue_id);
        info!("Cleared all orderbooks for venue: {}", venue_id);
    }

    pub async fn clear_all(&self) {
        let mut books = self.books.write().await;
        books.clear();
        
        if let Some(cache) = &self.cache {
            cache.clear().await;
        }
        
        info!("Cleared all orderbooks");
    }

    pub async fn get_book_count(&self) -> usize {
        self.books.read().await.len()
    }

    pub async fn get_venues(&self) -> Vec<VenueId> {
        let books = self.books.read().await;
        books
            .keys()
            .map(|key| key.venue_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    pub async fn get_symbols_for_venue(&self, venue_id: &VenueId) -> Vec<Symbol> {
        let books = self.books.read().await;
        books
            .keys()
            .filter(|key| &key.venue_id == venue_id)
            .map(|key| key.symbol.clone())
            .collect()
    }

    pub async fn has_book(&self, venue_id: &VenueId, symbol: &Symbol) -> bool {
        let key = BookKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };
        self.books.read().await.contains_key(&key)
    }

    pub async fn get_all_books(&self) -> Vec<(VenueId, Symbol, Arc<RwLock<FastOrderBook>>)> {
        let books = self.books.read().await;
        books
            .iter()
            .map(|(key, book)| (key.venue_id.clone(), key.symbol.clone(), Arc::clone(book)))
            .collect()
    }

    pub async fn get_snapshot(&self, venue_id: &VenueId, symbol: &Symbol) -> Option<OrderBookSnapshot> {
        let book = self.get_book(venue_id, symbol).await?;
        let book_guard = book.read().await;
        Some(OrderBookSnapshot::from_fast_orderbook(&book_guard))
    }

    pub async fn health_check(&self) -> ManagerHealthStatus {
        let books = self.books.read().await;
        let total_books = books.len();
        let mut empty_books = 0;
        let mut crossed_books = 0;

        for book in books.values() {
            let book_guard = book.read().await;
            if book_guard.is_empty() {
                empty_books += 1;
            }
            if book_guard.is_crossed() {
                crossed_books += 1;
                warn!("Crossed orderbook detected: {}", book_guard.symbol);
            }
        }

        ManagerHealthStatus {
            total_books,
            empty_books,
            crossed_books,
            healthy: crossed_books == 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagerHealthStatus {
    pub total_books: usize,
    pub empty_books: usize,
    pub crossed_books: usize,
    pub healthy: bool,
}

impl Default for OrderBookManager {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_basic() {
        let manager = OrderBookManager::new(100);
        let symbol = Symbol::new("BTC", "USDT");
        
        assert!(!manager.has_book(&VenueId::Binance, &symbol).await);
        
        let _book = manager.get_or_create_book(VenueId::Binance, symbol.clone()).await;
        
        assert!(manager.has_book(&VenueId::Binance, &symbol).await);
        assert_eq!(manager.get_book_count().await, 1);
    }

    #[tokio::test]
    async fn test_manager_remove() {
        let manager = OrderBookManager::new(100);
        let symbol = Symbol::new("BTC", "USDT");
        
        let _book = manager.get_or_create_book(VenueId::Binance, symbol.clone()).await;
        assert_eq!(manager.get_book_count().await, 1);
        
        manager.remove_book(&VenueId::Binance, &symbol).await;
        assert_eq!(manager.get_book_count().await, 0);
    }
}
