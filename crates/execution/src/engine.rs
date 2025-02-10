use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Mutex};
use tokio::time::{Duration, Instant};
use rust_decimal::Decimal;
use tracing::{info, warn, error};

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_orderbook::OrderBook;
use arbfinder_strategy::prelude::*;

use crate::{ExecutionConfig, ExecutionEvent, TradingSignal, Portfolio, RiskManager};

pub struct ExecutionEngine {
    config: ExecutionConfig,
    exchanges: HashMap<String, Arc<dyn Exchange>>,
    trading_exchanges: HashMap<String, Arc<dyn Trading>>,
    strategies: Vec<Box<dyn Strategy>>,
    portfolio: Arc<RwLock<Portfolio>>,
    risk_manager: Arc<RiskManager>,
    event_sender: mpsc::UnboundedSender<ExecutionEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ExecutionEvent>>>,
    order_rate_limiter: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl ExecutionEngine {
    pub fn new(config: ExecutionConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            config,
            exchanges: HashMap::new(),
            trading_exchanges: HashMap::new(),
            strategies: Vec::new(),
            portfolio: Arc::new(RwLock::new(Portfolio::new())),
            risk_manager: Arc::new(RiskManager::new()),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            order_rate_limiter: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_exchange(&mut self, name: String, exchange: Arc<dyn Exchange>) {
        self.exchanges.insert(name, exchange);
    }

    pub fn add_trading_exchange(&mut self, name: String, exchange: Arc<dyn Trading>) {
        self.trading_exchanges.insert(name, exchange);
    }

    pub fn add_strategy(&mut self, strategy: Box<dyn Strategy>) {
        self.strategies.push(strategy);
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting execution engine");
        
        // Start event processing loop
        let event_receiver = Arc::clone(&self.event_receiver);
        let portfolio = Arc::clone(&self.portfolio);
        let risk_manager = Arc::clone(&self.risk_manager);
        
        tokio::spawn(async move {
            let mut receiver = event_receiver.lock().await;
            while let Some(event) = receiver.recv().await {
                Self::handle_event(event, &portfolio, &risk_manager).await;
            }
        });

        // Start market data processing
        self.start_market_data_processing().await?;
        
        Ok(())
    }

    async fn start_market_data_processing(&mut self) -> Result<()> {
        for (exchange_name, exchange) in &self.exchanges {
            let markets = exchange.get_markets().await
                .map_err(|e| ArbFinderError::ExchangeError(e.to_string()))?;
            
            for market in markets {
                let exchange_clone = Arc::clone(exchange);
                let market_clone = market.clone();
                let event_sender = self.event_sender.clone();
                let strategies = &mut self.strategies;
                
                // Process market data for each market
                tokio::spawn(async move {
                    loop {
                        match Self::process_market_tick(&exchange_clone, &market_clone).await {
                            Ok((ticker, orderbook)) => {
                                // Send to strategies (simplified - in real implementation would need better strategy management)
                                info!("Market tick: {} - Bid: {}, Ask: {}", 
                                    market_clone.symbol, ticker.bid, ticker.ask);
                            }
                            Err(e) => {
                                error!("Error processing market tick for {}: {}", market_clone.symbol, e);
                            }
                        }
                        
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                });
            }
        }
        
        Ok(())
    }

    async fn process_market_tick(
        exchange: &Arc<dyn Exchange>,
        market: &Market,
    ) -> Result<(arbfinder_core::types::Ticker, OrderBookSnapshot)> {
        let ticker = exchange.get_ticker(&market.symbol).await
            .map_err(|e| ArbFinderError::ExchangeError(e.to_string()))?;
        
        let orderbook = exchange.get_orderbook(&market.symbol).await
            .map_err(|e| ArbFinderError::ExchangeError(e.to_string()))?;
        
        Ok((ticker, orderbook))
    }

    async fn handle_event(
        event: ExecutionEvent,
        portfolio: &Arc<RwLock<Portfolio>>,
        risk_manager: &Arc<RiskManager>,
    ) {
        match event {
            ExecutionEvent::OrderPlaced(order) => {
                info!("Order placed: {:?}", order);
                portfolio.write().await.add_pending_order(order);
            }
            ExecutionEvent::OrderFilled(order) => {
                info!("Order filled: {:?}", order);
                portfolio.write().await.update_order(order);
            }
            ExecutionEvent::OrderCanceled(order) => {
                info!("Order canceled: {:?}", order);
                portfolio.write().await.remove_pending_order(&order.id);
            }
            ExecutionEvent::TradeExecuted(trade) => {
                info!("Trade executed: {:?}", trade);
                portfolio.write().await.add_trade(trade);
            }
            ExecutionEvent::RiskLimitHit(reason) => {
                warn!("Risk limit hit: {}", reason);
                // Implement risk management actions
            }
            ExecutionEvent::StrategySignal { strategy, market, signal } => {
                info!("Strategy signal from {}: {:?}", strategy, signal);
                // Process trading signal
            }
        }
    }

    async fn check_rate_limit(&self, exchange: &str) -> bool {
        let mut rate_limiter = self.order_rate_limiter.write().await;
        let now = Instant::now();
        let window_start = now - Duration::from_secs(1);
        
        let orders = rate_limiter.entry(exchange.to_string()).or_insert_with(Vec::new);
        orders.retain(|&time| time > window_start);
        
        if orders.len() >= self.config.max_orders_per_second as usize {
            false
        } else {
            orders.push(now);
            true
        }
    }

    pub async fn place_order(
        &self,
        exchange: &str,
        market: &str,
        side: Side,
        price: Decimal,
        amount: Decimal,
    ) -> Result<String> {
        // Check rate limits
        if !self.check_rate_limit(exchange).await {
            return Err(ArbFinderError::OrderError("Rate limit exceeded".to_string()));
        }

        // Check risk limits
        if !self.risk_manager.check_order_risk(market, side, price, amount).await {
            return Err(ArbFinderError::OrderError("Risk limits exceeded".to_string()));
        }

        if self.config.enable_paper_trading {
            // Paper trading mode
            let order_id = uuid::Uuid::new_v4().to_string();
            let order = Order {
                exchange: exchange.to_string(),
                symbol: market.to_string(),
                id: order_id.clone(),
                client_id: None,
                side,
                price,
                amount,
                filled_amount: Decimal::ZERO,
                status: OrderStatus::New,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            self.event_sender.send(ExecutionEvent::OrderPlaced(order))
                .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;

            Ok(order_id)
        } else {
            // Real trading mode
            if let Some(trading_exchange) = self.trading_exchanges.get(exchange) {
                let order = trading_exchange.place_order(market, side, price, amount).await
                    .map_err(|e| ArbFinderError::OrderError(e.to_string()))?;

                self.event_sender.send(ExecutionEvent::OrderPlaced(order.clone()))
                    .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;

                Ok(order.id)
            } else {
                Err(ArbFinderError::ExchangeError(format!("Trading not supported for exchange: {}", exchange)))
            }
        }
    }

    pub async fn cancel_order(&self, exchange: &str, market: &str, order_id: &str) -> Result<()> {
        if self.config.enable_paper_trading {
            // Paper trading mode - just mark as canceled
            info!("Paper trading: Canceling order {}", order_id);
            Ok(())
        } else {
            // Real trading mode
            if let Some(trading_exchange) = self.trading_exchanges.get(exchange) {
                trading_exchange.cancel_order(market, order_id).await
                    .map_err(|e| ArbFinderError::OrderError(e.to_string()))?;
                Ok(())
            } else {
                Err(ArbFinderError::ExchangeError(format!("Trading not supported for exchange: {}", exchange)))
            }
        }
    }

    pub async fn get_portfolio(&self) -> Portfolio {
        self.portfolio.read().await.clone()
    }
}