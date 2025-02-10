use std::sync::Arc;
use std::collections::HashMap;
use prometheus::{
    Registry, Counter, Gauge, Histogram, HistogramOpts, Opts,
    Encoder, TextEncoder, gather,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::net::TcpListener;
use tracing::{info, error};

use arbfinder_core::prelude::*;

pub struct MetricsCollector {
    registry: Registry,
    
    // Trading metrics
    pub trades_total: Counter,
    pub orders_total: Counter,
    pub arbitrage_opportunities: Counter,
    pub profit_total: Gauge,
    pub portfolio_value: Gauge,
    
    // Exchange metrics
    pub exchange_requests: Counter,
    pub exchange_errors: Counter,
    pub exchange_latency: Histogram,
    
    // System metrics
    pub system_uptime: Gauge,
    pub memory_usage: Gauge,
    pub cpu_usage: Gauge,
    
    // Custom metrics
    custom_counters: HashMap<String, Counter>,
    custom_gauges: HashMap<String, Gauge>,
    custom_histograms: HashMap<String, Histogram>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        // Trading metrics
        let trades_total = Counter::with_opts(Opts::new(
            "arbfinder_trades_total",
            "Total number of trades executed"
        )).unwrap();
        
        let orders_total = Counter::with_opts(Opts::new(
            "arbfinder_orders_total",
            "Total number of orders placed"
        )).unwrap();
        
        let arbitrage_opportunities = Counter::with_opts(Opts::new(
            "arbfinder_arbitrage_opportunities_total",
            "Total number of arbitrage opportunities detected"
        )).unwrap();
        
        let profit_total = Gauge::with_opts(Opts::new(
            "arbfinder_profit_total",
            "Total profit in USD"
        )).unwrap();
        
        let portfolio_value = Gauge::with_opts(Opts::new(
            "arbfinder_portfolio_value",
            "Current portfolio value in USD"
        )).unwrap();
        
        // Exchange metrics
        let exchange_requests = Counter::with_opts(Opts::new(
            "arbfinder_exchange_requests_total",
            "Total number of exchange API requests"
        )).unwrap();
        
        let exchange_errors = Counter::with_opts(Opts::new(
            "arbfinder_exchange_errors_total",
            "Total number of exchange API errors"
        )).unwrap();
        
        let exchange_latency = Histogram::with_opts(HistogramOpts::new(
            "arbfinder_exchange_latency_seconds",
            "Exchange API request latency in seconds"
        )).unwrap();
        
        // System metrics
        let system_uptime = Gauge::with_opts(Opts::new(
            "arbfinder_system_uptime_seconds",
            "System uptime in seconds"
        )).unwrap();
        
        let memory_usage = Gauge::with_opts(Opts::new(
            "arbfinder_memory_usage_bytes",
            "Memory usage in bytes"
        )).unwrap();
        
        let cpu_usage = Gauge::with_opts(Opts::new(
            "arbfinder_cpu_usage_percent",
            "CPU usage percentage"
        )).unwrap();
        
        // Register metrics
        registry.register(Box::new(trades_total.clone())).unwrap();
        registry.register(Box::new(orders_total.clone())).unwrap();
        registry.register(Box::new(arbitrage_opportunities.clone())).unwrap();
        registry.register(Box::new(profit_total.clone())).unwrap();
        registry.register(Box::new(portfolio_value.clone())).unwrap();
        registry.register(Box::new(exchange_requests.clone())).unwrap();
        registry.register(Box::new(exchange_errors.clone())).unwrap();
        registry.register(Box::new(exchange_latency.clone())).unwrap();
        registry.register(Box::new(system_uptime.clone())).unwrap();
        registry.register(Box::new(memory_usage.clone())).unwrap();
        registry.register(Box::new(cpu_usage.clone())).unwrap();
        
        Self {
            registry,
            trades_total,
            orders_total,
            arbitrage_opportunities,
            profit_total,
            portfolio_value,
            exchange_requests,
            exchange_errors,
            exchange_latency,
            system_uptime,
            memory_usage,
            cpu_usage,
            custom_counters: HashMap::new(),
            custom_gauges: HashMap::new(),
            custom_histograms: HashMap::new(),
        }
    }
    
    pub fn record_trade(&self, exchange: &str, symbol: &str, side: &str, amount: f64, price: f64) {
        self.trades_total
            .with_label_values(&[exchange, symbol, side])
            .inc();
    }
    
    pub fn record_order(&self, exchange: &str, symbol: &str, side: &str) {
        self.orders_total
            .with_label_values(&[exchange, symbol, side])
            .inc();
    }
    
    pub fn record_arbitrage_opportunity(&self, exchange_a: &str, exchange_b: &str, symbol: &str) {
        self.arbitrage_opportunities
            .with_label_values(&[exchange_a, exchange_b, symbol])
            .inc();
    }
    
    pub fn update_profit(&self, profit: f64) {
        self.profit_total.set(profit);
    }
    
    pub fn update_portfolio_value(&self, value: f64) {
        self.portfolio_value.set(value);
    }
    
    pub fn record_exchange_request(&self, exchange: &str, endpoint: &str) {
        self.exchange_requests
            .with_label_values(&[exchange, endpoint])
            .inc();
    }
    
    pub fn record_exchange_error(&self, exchange: &str, endpoint: &str, error_type: &str) {
        self.exchange_errors
            .with_label_values(&[exchange, endpoint, error_type])
            .inc();
    }
    
    pub fn record_exchange_latency(&self, exchange: &str, endpoint: &str, duration: f64) {
        self.exchange_latency
            .with_label_values(&[exchange, endpoint])
            .observe(duration);
    }
    
    pub fn update_system_uptime(&self, uptime: f64) {
        self.system_uptime.set(uptime);
    }
    
    pub fn update_memory_usage(&self, usage: f64) {
        self.memory_usage.set(usage);
    }
    
    pub fn update_cpu_usage(&self, usage: f64) {
        self.cpu_usage.set(usage);
    }
    
    pub fn create_custom_counter(&mut self, name: &str, help: &str) -> Result<()> {
        let counter = Counter::with_opts(Opts::new(name, help))
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        self.registry.register(Box::new(counter.clone()))
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        self.custom_counters.insert(name.to_string(), counter);
        Ok(())
    }
    
    pub fn create_custom_gauge(&mut self, name: &str, help: &str) -> Result<()> {
        let gauge = Gauge::with_opts(Opts::new(name, help))
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        self.registry.register(Box::new(gauge.clone()))
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        self.custom_gauges.insert(name.to_string(), gauge);
        Ok(())
    }
    
    pub fn create_custom_histogram(&mut self, name: &str, help: &str) -> Result<()> {
        let histogram = Histogram::with_opts(HistogramOpts::new(name, help))
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        self.registry.register(Box::new(histogram.clone()))
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        self.custom_histograms.insert(name.to_string(), histogram);
        Ok(())
    }
    
    pub fn increment_custom_counter(&self, name: &str) {
        if let Some(counter) = self.custom_counters.get(name) {
            counter.inc();
        }
    }
    
    pub fn set_custom_gauge(&self, name: &str, value: f64) {
        if let Some(gauge) = self.custom_gauges.get(name) {
            gauge.set(value);
        }
    }
    
    pub fn observe_custom_histogram(&self, name: &str, value: f64) {
        if let Some(histogram) = self.custom_histograms.get(name) {
            histogram.observe(value);
        }
    }
    
    pub fn gather_metrics(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        String::from_utf8(buffer)
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))
    }
}

pub struct MetricsServer {
    port: u16,
    metrics_collector: Arc<MetricsCollector>,
}

impl MetricsServer {
    pub fn new(port: u16, metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            port,
            metrics_collector,
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler))
            .with_state(Arc::clone(&self.metrics_collector));
        
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await
            .map_err(|e| ArbFinderError::InternalError(e.to_string()))?;
        
        info!("Metrics server starting on port {}", self.port);
        
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Metrics server error: {}", e);
            }
        });
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<()> {
        // In a real implementation, you'd store the server handle and gracefully shut it down
        info!("Metrics server stopped");
        Ok(())
    }
}

async fn metrics_handler(
    State(metrics_collector): State<Arc<MetricsCollector>>,
) -> impl IntoResponse {
    match metrics_collector.gather_metrics() {
        Ok(metrics) => (StatusCode::OK, metrics),
        Err(e) => {
            error!("Failed to gather metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to gather metrics".to_string())
        }
    }
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}