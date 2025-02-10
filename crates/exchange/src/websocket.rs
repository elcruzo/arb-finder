use arbfinder_core::{ArbFinderError, Result};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

use crate::traits::{ExchangeConfig, WebSocketHandler};

pub type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug)]
pub struct WebSocketConnection {
    url: String,
    stream: Option<WsStream>,
    is_connected: Arc<RwLock<bool>>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
    last_ping: Arc<Mutex<Option<Instant>>>,
    last_pong: Arc<Mutex<Option<Instant>>>,
    message_tx: Option<mpsc::UnboundedSender<String>>,
    close_tx: Option<mpsc::UnboundedSender<()>>,
}

impl WebSocketConnection {
    pub fn new<C: ExchangeConfig>(config: &C) -> Self {
        Self {
            url: config.websocket_url().to_string(),
            stream: None,
            is_connected: Arc::new(RwLock::new(false)),
            reconnect_attempts: 0,
            max_reconnect_attempts: config.reconnect_attempts(),
            reconnect_delay: Duration::from_millis(config.reconnect_delay_ms()),
            last_ping: Arc::new(Mutex::new(None)),
            last_pong: Arc::new(Mutex::new(None)),
            message_tx: None,
            close_tx: None,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to WebSocket: {}", self.url);

        let url = Url::parse(&self.url)
            .map_err(|e| ArbFinderError::WebSocket(format!("Invalid WebSocket URL: {}", e)))?;

        match connect_async(url).await {
            Ok((ws_stream, response)) => {
                info!("WebSocket connected. Response: {:?}", response.status());
                self.stream = Some(ws_stream);
                *self.is_connected.write().await = true;
                self.reconnect_attempts = 0;
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to WebSocket: {}", e);
                Err(ArbFinderError::WebSocket(format!("Connection failed: {}", e)))
            }
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting WebSocket");

        if let Some(close_tx) = &self.close_tx {
            let _ = close_tx.send(());
        }

        if let Some(mut stream) = self.stream.take() {
            let _ = stream.close(None).await;
        }

        *self.is_connected.write().await = false;
        self.reconnect_attempts = 0;

        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    pub async fn send_message(&mut self, message: &str) -> Result<()> {
        if let Some(stream) = &mut self.stream {
            stream
                .send(Message::Text(message.to_string()))
                .await
                .map_err(|e| ArbFinderError::WebSocket(format!("Failed to send message: {}", e)))?;
            debug!("Sent WebSocket message: {}", message);
            Ok(())
        } else {
            Err(ArbFinderError::WebSocket("Not connected".to_string()))
        }
    }

    pub async fn send_ping(&mut self) -> Result<()> {
        if let Some(stream) = &mut self.stream {
            let ping_data = Vec::new();
            stream
                .send(Message::Ping(ping_data))
                .await
                .map_err(|e| ArbFinderError::WebSocket(format!("Failed to send ping: {}", e)))?;
            
            *self.last_ping.lock().await = Some(Instant::now());
            debug!("Sent WebSocket ping");
            Ok(())
        } else {
            Err(ArbFinderError::WebSocket("Not connected".to_string()))
        }
    }

    pub async fn get_last_pong_latency(&self) -> Option<Duration> {
        let last_ping = *self.last_ping.lock().await;
        let last_pong = *self.last_pong.lock().await;

        match (last_ping, last_pong) {
            (Some(ping), Some(pong)) if pong > ping => Some(pong - ping),
            _ => None,
        }
    }

    pub async fn run_with_handler<H>(&mut self, handler: Arc<Mutex<H>>) -> Result<()>
    where
        H: WebSocketHandler + 'static,
    {
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<String>();
        let (close_tx, mut close_rx) = mpsc::unbounded_channel::<()>();
        
        self.message_tx = Some(message_tx);
        self.close_tx = Some(close_tx);

        loop {
            if !self.is_connected().await {
                if let Err(e) = self.reconnect(handler.clone()).await {
                    error!("Failed to reconnect: {}", e);
                    if self.reconnect_attempts >= self.max_reconnect_attempts {
                        return Err(ArbFinderError::WebSocket(
                            "Max reconnection attempts reached".to_string(),
                        ));
                    }
                    sleep(self.reconnect_delay).await;
                    continue;
                }
            }

            if let Some(stream) = &mut self.stream {
                tokio::select! {
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(message)) => {
                                if let Err(e) = self.handle_message(message, handler.clone()).await {
                                    error!("Error handling message: {}", e);
                                }
                            }
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                *self.is_connected.write().await = false;
                                let error = ArbFinderError::WebSocket(e.to_string());
                                if let Err(handler_err) = handler.lock().await.on_error(&error).await {
                                    error!("Handler error: {}", handler_err);
                                }
                            }
                            None => {
                                warn!("WebSocket stream ended");
                                *self.is_connected.write().await = false;
                            }
                        }
                    }
                    
                    message = message_rx.recv() => {
                        if let Some(msg) = message {
                            if let Err(e) = self.send_message(&msg).await {
                                error!("Failed to send queued message: {}", e);
                            }
                        }
                    }
                    
                    _ = close_rx.recv() => {
                        info!("Received close signal");
                        break;
                    }
                }
            } else {
                sleep(Duration::from_millis(100)).await;
            }
        }

        Ok(())
    }

    async fn handle_message<H>(&mut self, message: Message, handler: Arc<Mutex<H>>) -> Result<()>
    where
        H: WebSocketHandler,
    {
        match message {
            Message::Text(text) => {
                debug!("Received WebSocket message: {}", text);
                handler.lock().await.on_message(&text).await?;
            }
            Message::Binary(data) => {
                let text = String::from_utf8_lossy(&data);
                debug!("Received binary WebSocket message: {}", text);
                handler.lock().await.on_message(&text).await?;
            }
            Message::Ping(data) => {
                debug!("Received WebSocket ping");
                if let Some(stream) = &mut self.stream {
                    stream.send(Message::Pong(data)).await.map_err(|e| {
                        ArbFinderError::WebSocket(format!("Failed to send pong: {}", e))
                    })?;
                }
                handler.lock().await.on_ping().await?;
            }
            Message::Pong(_) => {
                debug!("Received WebSocket pong");
                *self.last_pong.lock().await = Some(Instant::now());
                handler.lock().await.on_pong().await?;
            }
            Message::Close(frame) => {
                warn!("Received WebSocket close: {:?}", frame);
                *self.is_connected.write().await = false;
                handler.lock().await.on_disconnect().await?;
            }
            Message::Frame(_) => {
                debug!("Received WebSocket frame");
            }
        }
        Ok(())
    }

    async fn reconnect<H>(&mut self, handler: Arc<Mutex<H>>) -> Result<()>
    where
        H: WebSocketHandler,
    {
        self.reconnect_attempts += 1;
        warn!(
            "Attempting to reconnect ({}/{})",
            self.reconnect_attempts, self.max_reconnect_attempts
        );

        if let Some(mut stream) = self.stream.take() {
            let _ = stream.close(None).await;
        }

        match self.connect().await {
            Ok(_) => {
                info!("Reconnected successfully");
                handler.lock().await.on_connect().await?;
                Ok(())
            }
            Err(e) => {
                error!("Reconnection failed: {}", e);
                Err(e)
            }
        }
    }

    pub fn get_message_sender(&self) -> Option<mpsc::UnboundedSender<String>> {
        self.message_tx.clone()
    }
}

#[derive(Debug)]
pub struct WebSocketManager {
    connections: std::collections::HashMap<String, WebSocketConnection>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: std::collections::HashMap::new(),
        }
    }

    pub async fn add_connection<C: ExchangeConfig>(
        &mut self,
        name: String,
        config: &C,
    ) -> Result<()> {
        let connection = WebSocketConnection::new(config);
        self.connections.insert(name, connection);
        Ok(())
    }

    pub async fn connect(&mut self, name: &str) -> Result<()> {
        if let Some(connection) = self.connections.get_mut(name) {
            connection.connect().await
        } else {
            Err(ArbFinderError::WebSocket(format!(
                "Connection '{}' not found",
                name
            )))
        }
    }

    pub async fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(connection) = self.connections.get_mut(name) {
            connection.disconnect().await
        } else {
            Err(ArbFinderError::WebSocket(format!(
                "Connection '{}' not found",
                name
            )))
        }
    }

    pub async fn send_message(&mut self, name: &str, message: &str) -> Result<()> {
        if let Some(connection) = self.connections.get_mut(name) {
            connection.send_message(message).await
        } else {
            Err(ArbFinderError::WebSocket(format!(
                "Connection '{}' not found",
                name
            )))
        }
    }

    pub async fn is_connected(&self, name: &str) -> bool {
        if let Some(connection) = self.connections.get(name) {
            connection.is_connected().await
        } else {
            false
        }
    }

    pub fn get_connection(&mut self, name: &str) -> Option<&mut WebSocketConnection> {
        self.connections.get_mut(name)
    }

    pub async fn disconnect_all(&mut self) -> Result<()> {
        for (name, connection) in &mut self.connections {
            if let Err(e) = connection.disconnect().await {
                error!("Failed to disconnect {}: {}", name, e);
            }
        }
        Ok(())
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}