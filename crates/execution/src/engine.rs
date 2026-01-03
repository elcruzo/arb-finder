use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Mutex};
use tokio::time::{Duration, Instant};
use rust_decimal::Decimal;
use tracing::{info, warn};

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_strategy::prelude::*;

use crate::{ExecutionConfig, ExecutionEvent, Portfolio, RiskManager};

pub struct ExecutionEngine {
    config: ExecutionConfig,
    exchanges: HashMap<String, Arc<dyn ExchangeAdapter>>,
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
            strategies: Vec::new(),
            portfolio: Arc::new(RwLock::new(Portfolio::new())),
            risk_manager: Arc::new(RiskManager::new()),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            order_rate_limiter: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_exchange(&mut self, name: String, exchange: Arc<dyn ExchangeAdapter>) {
        self.exchanges.insert(name, exchange);
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
            let symbols = exchange.get_symbols().await?;
            
            for symbol in symbols {
                let exchange_clone = Arc::clone(exchange);
                let symbol_clone = symbol.clone();
                let event_sender = self.event_sender.clone();
                
                // Process market data for each symbol
                // In a real implementation, this would subscribe to websocket feeds
                info!("Would start market data processing for {}", symbol_clone.to_pair());
            }
        }
        
        Ok(())
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
            ExecutionEvent::StrategySignal { strategy, symbol, signal } => {
                info!("Strategy signal from {} for {}: {:?}", strategy, symbol.to_pair(), signal);
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
        venue_id: VenueId,
        symbol: Symbol,
        side: OrderSide,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> Result<OrderId> {
        // Check rate limits
        let exchange_str = format!("{:?}", venue_id);
        if !self.check_rate_limit(&exchange_str).await {
            return Err(ArbFinderError::RateLimit("Rate limit exceeded".to_string()));
        }

        // Check risk limits
        if !self.risk_manager.check_order_risk(&symbol.to_pair(), side, price.unwrap_or_default(), quantity).await {
            return Err(ArbFinderError::InvalidOrder("Risk limits exceeded".to_string()));
        }

        if self.config.enable_paper_trading {
            // Paper trading mode
            let order = if let Some(p) = price {
                Order::new_limit(venue_id, symbol, side, quantity, p)
            } else {
                Order::new_market(venue_id, symbol, side, quantity)
            };

            let order_id = order.id.clone();
            self.event_sender.send(ExecutionEvent::OrderPlaced(order))
                .map_err(|e| ArbFinderError::Internal(e.to_string()))?;

            Ok(order_id)
        } else {
            // Real trading mode would use adapter methods here
            Err(ArbFinderError::Exchange("Real trading not implemented yet".to_string()))
        }
    }

    pub async fn cancel_order(&self, order_id: &OrderId) -> Result<()> {
        if self.config.enable_paper_trading {
            // Paper trading mode - just mark as canceled
            info!("Paper trading: Canceling order {}", order_id);
            Ok(())
        } else {
            // Real trading mode would use adapter methods here
            Err(ArbFinderError::Exchange("Real trading not implemented yet".to_string()))
        }
    }

    pub async fn get_portfolio(&self) -> Portfolio {
        self.portfolio.read().await.clone()
    }
}