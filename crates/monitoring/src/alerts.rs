use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use reqwest::Client;

use arbfinder_core::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub level: AlertLevel,
    pub title: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub webhook_url: Option<String>,
    pub email_config: Option<EmailConfig>,
    pub slack_config: Option<SlackConfig>,
    pub enable_console_alerts: bool,
    pub rate_limit_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub to_addresses: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SlackConfig {
    pub webhook_url: String,
    pub channel: String,
    pub username: String,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            webhook_url: None,
            email_config: None,
            slack_config: None,
            enable_console_alerts: true,
            rate_limit_seconds: 60,
        }
    }
}

pub struct AlertManager {
    config: AlertConfig,
    sender: mpsc::UnboundedSender<Alert>,
    receiver: Option<mpsc::UnboundedReceiver<Alert>>,
    http_client: Client,
    last_alert_times: HashMap<String, DateTime<Utc>>,
}

impl AlertManager {
    pub fn new(config: AlertConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        Self {
            config,
            sender,
            receiver: Some(receiver),
            http_client: Client::new(),
            last_alert_times: HashMap::new(),
        }
    }

    pub async fn start(&mut self) {
        if let Some(mut receiver) = self.receiver.take() {
            let config = self.config.clone();
            let http_client = self.http_client.clone();
            let mut last_alert_times = self.last_alert_times.clone();

            tokio::spawn(async move {
                while let Some(alert) = receiver.recv().await {
                    Self::process_alert(alert, &config, &http_client, &mut last_alert_times).await;
                }
            });
        }
    }

    pub async fn send_alert(&self, alert: Alert) {
        if let Err(e) = self.sender.send(alert) {
            error!("Failed to send alert: {}", e);
        }
    }

    async fn process_alert(
        alert: Alert,
        config: &AlertConfig,
        http_client: &Client,
        last_alert_times: &mut HashMap<String, DateTime<Utc>>,
    ) {
        // Rate limiting
        let alert_key = format!("{}:{}", alert.level as u8, alert.title);
        let now = Utc::now();
        
        if let Some(last_time) = last_alert_times.get(&alert_key) {
            let duration = now.signed_duration_since(*last_time);
            if duration.num_seconds() < config.rate_limit_seconds as i64 {
                return; // Skip this alert due to rate limiting
            }
        }
        
        last_alert_times.insert(alert_key, now);

        // Console alerts
        if config.enable_console_alerts {
            Self::send_console_alert(&alert);
        }

        // Webhook alerts
        if let Some(webhook_url) = &config.webhook_url {
            Self::send_webhook_alert(&alert, webhook_url, http_client).await;
        }

        // Slack alerts
        if let Some(slack_config) = &config.slack_config {
            Self::send_slack_alert(&alert, slack_config, http_client).await;
        }

        // Email alerts (simplified - would need actual SMTP implementation)
        if let Some(email_config) = &config.email_config {
            Self::send_email_alert(&alert, email_config).await;
        }
    }

    fn send_console_alert(alert: &Alert) {
        match alert.level {
            AlertLevel::Info => {
                info!(
                    alert_id = %alert.id,
                    title = %alert.title,
                    message = %alert.message,
                    "Alert: {}", alert.title
                );
            }
            AlertLevel::Warning => {
                warn!(
                    alert_id = %alert.id,
                    title = %alert.title,
                    message = %alert.message,
                    "Alert: {}", alert.title
                );
            }
            AlertLevel::Critical => {
                error!(
                    alert_id = %alert.id,
                    title = %alert.title,
                    message = %alert.message,
                    "CRITICAL Alert: {}", alert.title
                );
            }
        }
    }

    async fn send_webhook_alert(alert: &Alert, webhook_url: &str, http_client: &Client) {
        let payload = serde_json::json!({
            "id": alert.id,
            "level": format!("{:?}", alert.level),
            "title": alert.title,
            "message": alert.message,
            "timestamp": alert.timestamp.to_rfc3339(),
            "metadata": alert.metadata
        });

        match http_client.post(webhook_url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Webhook alert sent successfully: {}", alert.id);
                } else {
                    error!("Webhook alert failed with status: {}", response.status());
                }
            }
            Err(e) => {
                error!("Failed to send webhook alert: {}", e);
            }
        }
    }

    async fn send_slack_alert(alert: &Alert, slack_config: &SlackConfig, http_client: &Client) {
        let color = match alert.level {
            AlertLevel::Info => "good",
            AlertLevel::Warning => "warning",
            AlertLevel::Critical => "danger",
        };

        let emoji = match alert.level {
            AlertLevel::Info => ":information_source:",
            AlertLevel::Warning => ":warning:",
            AlertLevel::Critical => ":rotating_light:",
        };

        let payload = serde_json::json!({
            "channel": slack_config.channel,
            "username": slack_config.username,
            "attachments": [{
                "color": color,
                "title": format!("{} {}", emoji, alert.title),
                "text": alert.message,
                "fields": [
                    {
                        "title": "Level",
                        "value": format!("{:?}", alert.level),
                        "short": true
                    },
                    {
                        "title": "Time",
                        "value": alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                        "short": true
                    }
                ],
                "footer": "ArbFinder Alert System",
                "ts": alert.timestamp.timestamp()
            }]
        });

        match http_client.post(&slack_config.webhook_url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Slack alert sent successfully: {}", alert.id);
                } else {
                    error!("Slack alert failed with status: {}", response.status());
                }
            }
            Err(e) => {
                error!("Failed to send Slack alert: {}", e);
            }
        }
    }

    async fn send_email_alert(alert: &Alert, _email_config: &EmailConfig) {
        // Simplified email implementation
        // In a real implementation, you would use an SMTP library like lettre
        info!("Email alert would be sent: {} - {}", alert.title, alert.message);
    }

    // Predefined alert creators
    pub fn create_profit_alert(profit: f64, threshold: f64) -> Alert {
        Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level: if profit > threshold { AlertLevel::Info } else { AlertLevel::Warning },
            title: "Profit Threshold".to_string(),
            message: format!("Current profit: ${:.2}, Threshold: ${:.2}", profit, threshold),
            timestamp: Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("profit".to_string(), profit.to_string());
                map.insert("threshold".to_string(), threshold.to_string());
                map
            },
        }
    }

    pub fn create_loss_alert(loss: f64, max_loss: f64) -> Alert {
        Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level: AlertLevel::Critical,
            title: "Loss Limit Exceeded".to_string(),
            message: format!("Current loss: ${:.2}, Max allowed: ${:.2}", loss, max_loss),
            timestamp: Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("loss".to_string(), loss.to_string());
                map.insert("max_loss".to_string(), max_loss.to_string());
                map
            },
        }
    }

    pub fn create_exchange_error_alert(exchange: &str, error: &str) -> Alert {
        Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level: AlertLevel::Warning,
            title: format!("Exchange Error: {}", exchange),
            message: format!("Error occurred on {}: {}", exchange, error),
            timestamp: Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("exchange".to_string(), exchange.to_string());
                map.insert("error".to_string(), error.to_string());
                map
            },
        }
    }

    pub fn create_arbitrage_alert(
        exchange_a: &str,
        exchange_b: &str,
        symbol: &str,
        profit_percentage: f64,
    ) -> Alert {
        Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level: AlertLevel::Info,
            title: "Arbitrage Opportunity".to_string(),
            message: format!(
                "Arbitrage opportunity: {} between {} and {} with {:.2}% profit",
                symbol, exchange_a, exchange_b, profit_percentage
            ),
            timestamp: Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("exchange_a".to_string(), exchange_a.to_string());
                map.insert("exchange_b".to_string(), exchange_b.to_string());
                map.insert("symbol".to_string(), symbol.to_string());
                map.insert("profit_percentage".to_string(), profit_percentage.to_string());
                map
            },
        }
    }

    pub fn create_system_alert(component: &str, message: &str, level: AlertLevel) -> Alert {
        Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level,
            title: format!("System Alert: {}", component),
            message: message.to_string(),
            timestamp: Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("component".to_string(), component.to_string());
                map
            },
        }
    }
}