//! OrderBook Cache
//!
//! Provides caching mechanisms for order book data

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use arbfinder_core::{Symbol, VenueId};
use chrono::{DateTime, Utc, Duration};

use crate::{FastOrderBook, OrderBookSnapshot};

/// Cache entry for order book
#[derive(Debug, Clone)]
struct CacheEntry {
    book: FastOrderBook,
    last_accessed: DateTime<Utc>,
    access_count: u64,
}

/// OrderBook cache with LRU eviction
pub struct OrderBookCache {
    cache: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    max_size: usize,
    ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    venue_id: VenueId,
    symbol: Symbol,
}

impl OrderBookCache {
    pub fn new(max_size: usize, ttl_seconds: i64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            ttl: Duration::seconds(ttl_seconds),
        }
    }

    pub async fn get(&self, venue_id: &VenueId, symbol: &Symbol) -> Option<FastOrderBook> {
        let mut cache = self.cache.write().await;
        let key = CacheKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };

        if let Some(entry) = cache.get_mut(&key) {
            // Check if entry is still valid
            if Utc::now().signed_duration_since(entry.last_accessed) < self.ttl {
                entry.last_accessed = Utc::now();
                entry.access_count += 1;
                return Some(entry.book.clone());
            } else {
                // Entry expired, remove it
                cache.remove(&key);
            }
        }

        None
    }

    pub async fn put(&self, venue_id: VenueId, book: FastOrderBook) {
        let mut cache = self.cache.write().await;
        let key = CacheKey {
            venue_id,
            symbol: book.symbol.clone(),
        };

        // Evict if cache is full
        if cache.len() >= self.max_size && !cache.contains_key(&key) {
            self.evict_lru(&mut cache);
        }

        let entry = CacheEntry {
            book,
            last_accessed: Utc::now(),
            access_count: 1,
        };

        cache.insert(key, entry);
    }

    pub async fn invalidate(&self, venue_id: &VenueId, symbol: &Symbol) {
        let mut cache = self.cache.write().await;
        let key = CacheKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };
        cache.remove(&key);
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub async fn size(&self) -> usize {
        self.cache.read().await.len()
    }

    pub async fn get_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total_accesses: u64 = cache.values().map(|e| e.access_count).sum();
        let oldest = cache
            .values()
            .map(|e| e.last_accessed)
            .min();
        let newest = cache
            .values()
            .map(|e| e.last_accessed)
            .max();

        CacheStats {
            size: cache.len(),
            max_size: self.max_size,
            total_accesses,
            oldest_entry: oldest,
            newest_entry: newest,
        }
    }

    fn evict_lru(&self, cache: &mut HashMap<CacheKey, CacheEntry>) {
        // Find least recently used entry
        if let Some(lru_key) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&lru_key);
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub total_accesses: u64,
    pub oldest_entry: Option<DateTime<Utc>>,
    pub newest_entry: Option<DateTime<Utc>>,
}

/// Snapshot cache for storing historical orderbook states
pub struct SnapshotCache {
    snapshots: Arc<RwLock<HashMap<SnapshotKey, Vec<OrderBookSnapshot>>>>,
    max_snapshots_per_key: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SnapshotKey {
    venue_id: VenueId,
    symbol: Symbol,
}

impl SnapshotCache {
    pub fn new(max_snapshots_per_key: usize) -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            max_snapshots_per_key,
        }
    }

    pub async fn add_snapshot(&self, venue_id: VenueId, snapshot: OrderBookSnapshot) {
        let mut snapshots = self.snapshots.write().await;
        let key = SnapshotKey {
            venue_id,
            symbol: snapshot.symbol.clone(),
        };

        let entry = snapshots.entry(key).or_insert_with(Vec::new);
        entry.push(snapshot);

        // Keep only the most recent snapshots
        if entry.len() > self.max_snapshots_per_key {
            entry.remove(0);
        }
    }

    pub async fn get_latest(&self, venue_id: &VenueId, symbol: &Symbol) -> Option<OrderBookSnapshot> {
        let snapshots = self.snapshots.read().await;
        let key = SnapshotKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };

        snapshots.get(&key)?.last().cloned()
    }

    pub async fn get_all(&self, venue_id: &VenueId, symbol: &Symbol) -> Vec<OrderBookSnapshot> {
        let snapshots = self.snapshots.read().await;
        let key = SnapshotKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };

        snapshots.get(&key).cloned().unwrap_or_default()
    }

    pub async fn clear(&self, venue_id: &VenueId, symbol: &Symbol) {
        let mut snapshots = self.snapshots.write().await;
        let key = SnapshotKey {
            venue_id: venue_id.clone(),
            symbol: symbol.clone(),
        };
        snapshots.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic() {
        let cache = OrderBookCache::new(10, 60);
        let symbol = Symbol::new("BTC", "USDT");
        let book = FastOrderBook::new(symbol.clone(), None);

        cache.put(VenueId::Binance, book.clone()).await;
        assert_eq!(cache.size().await, 1);

        let retrieved = cache.get(&VenueId::Binance, &symbol).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = OrderBookCache::new(10, 60);
        let symbol = Symbol::new("BTC", "USDT");
        let book = FastOrderBook::new(symbol.clone(), None);

        cache.put(VenueId::Binance, book).await;
        cache.invalidate(&VenueId::Binance, &symbol).await;

        assert_eq!(cache.size().await, 0);
    }
}
