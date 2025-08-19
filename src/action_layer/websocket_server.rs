//! WebSocket Server for Real-time Trading Updates
//!
//! Provides real-time streaming of order status, position changes, and system events

use crate::action_layer::{ActionLayer, Order, Position};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::{
    sync::{broadcast, RwLock},
    time::{interval, Duration},
};
use uuid::Uuid;

pub struct WebSocketServer {
    port: u16,
    action_layer: Arc<ActionLayer>,
    broadcaster: broadcast::Sender<WsMessage>,
    connected_clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
}

#[derive(Debug, Clone)]
struct ClientConnection {
    id: String,
    subscriptions: Vec<SubscriptionType>,
    connected_at: chrono::DateTime<chrono::Utc>,
    last_heartbeat: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionType {
    Orders,
    Positions,
    Account,
    System,
    PnL,
    All,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    OrderUpdate {
        order_id: String,
        symbol: String,
        side: String,
        status: String,
        filled_quantity: f64,
        avg_fill_price: Option<f64>,
        timestamp: String,
    },
    PositionUpdate {
        symbol: String,
        quantity: f64,
        avg_entry_price: f64,
        current_price: f64,
        unrealized_pnl: f64,
        realized_pnl: f64,
        side: String,
        timestamp: String,
    },
    AccountUpdate {
        equity: f64,
        buying_power: f64,
        cash: f64,
        portfolio_value: f64,
        daily_pnl: f64,
        unrealized_pnl: f64,
        realized_pnl: f64,
        timestamp: String,
    },
    SystemEvent {
        event_type: String,
        message: String,
        emergency_stop: bool,
        timestamp: String,
    },
    PnLUpdate {
        symbol: String,
        unrealized_pnl: f64,
        realized_pnl: f64,
        total_pnl: f64,
        percentage_change: f64,
        timestamp: String,
    },
    Heartbeat {
        timestamp: String,
        active_connections: usize,
    },
    Error {
        message: String,
        code: String,
        timestamp: String,
    },
    Welcome {
        client_id: String,
        server_time: String,
        available_subscriptions: Vec<SubscriptionType>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    Subscribe {
        subscriptions: Vec<SubscriptionType>,
    },
    Unsubscribe {
        subscriptions: Vec<SubscriptionType>,
    },
    Heartbeat,
    GetSnapshot {
        data_type: SnapshotType,
    },
}

#[derive(Debug, Deserialize)]
enum SnapshotType {
    Orders,
    Positions,
    Account,
    All,
}

impl WebSocketServer {
    pub fn new(port: u16, action_layer: Arc<ActionLayer>) -> Self {
        let (broadcaster, _) = broadcast::channel(1000);
        
        Self {
            port,
            action_layer,
            broadcaster,
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn start(&self) -> Result<(), crate::action_layer::ActionLayerError> {
        // Start background tasks
        self.start_heartbeat_task().await;
        self.start_data_monitoring_task().await;
        
        // Create router
        let app = Router::new()
            .route("/ws", get(websocket_handler))
            .route("/ws/health", get(websocket_health))
            .with_state((self.action_layer.clone(), self.broadcaster.clone(), self.connected_clients.clone()));
        
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| crate::action_layer::ActionLayerError::Position(
                format!("Failed to bind WebSocket server to port {}: {}", self.port, e)
            ))?;
        
        tracing::info!("🚀 WebSocket server starting on port {}", self.port);
        
        axum::serve(listener, app)
            .await
            .map_err(|e| crate::action_layer::ActionLayerError::Position(
                format!("WebSocket server error: {}", e)
            ))?;
        
        Ok(())
    }
    
    async fn start_heartbeat_task(&self) {
        let broadcaster = self.broadcaster.clone();
        let clients = self.connected_clients.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // Heartbeat every 30 seconds
            
            loop {
                interval.tick().await;
                
                let client_count = clients.read().await.len();
                let heartbeat = WsMessage::Heartbeat {
                    timestamp: Utc::now().to_rfc3339(),
                    active_connections: client_count,
                };
                
                let _ = broadcaster.send(heartbeat);
            }
        });
    }
    
    async fn start_data_monitoring_task(&self) {
        let action_layer = self.action_layer.clone();
        let broadcaster = self.broadcaster.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1)); // Monitor every second
            let mut last_positions: HashMap<String, Position> = HashMap::new();
            let mut last_account_hash = 0u64;
            
            loop {
                interval.tick().await;
                
                // Check for position updates
                if let Ok(current_positions) = action_layer.get_positions().await {
                    for (symbol, position) in &current_positions {
                        if let Some(last_position) = last_positions.get(symbol) {
                            // Check if position has changed
                            if position.unrealized_pnl != last_position.unrealized_pnl ||
                               position.current_price != last_position.current_price ||
                               position.quantity != last_position.quantity {
                                
                                let update = WsMessage::PositionUpdate {
                                    symbol: position.symbol.clone(),
                                    quantity: position.quantity,
                                    avg_entry_price: position.avg_entry_price,
                                    current_price: position.current_price,
                                    unrealized_pnl: position.unrealized_pnl,
                                    realized_pnl: position.realized_pnl,
                                    side: format!("{:?}", position.side),
                                    timestamp: Utc::now().to_rfc3339(),
                                };
                                
                                let _ = broadcaster.send(update);
                            }
                        } else {
                            // New position
                            let update = WsMessage::PositionUpdate {
                                symbol: position.symbol.clone(),
                                quantity: position.quantity,
                                avg_entry_price: position.avg_entry_price,
                                current_price: position.current_price,
                                unrealized_pnl: position.unrealized_pnl,
                                realized_pnl: position.realized_pnl,
                                side: format!("{:?}", position.side),
                                timestamp: Utc::now().to_rfc3339(),
                            };
                            
                            let _ = broadcaster.send(update);
                        }
                    }
                    last_positions = current_positions;
                }
                
                // Check for account updates
                if let Ok(account) = action_layer.get_account().await {
                    use std::hash::{Hash, Hasher};
                    use std::collections::hash_map::DefaultHasher;
                    
                    let mut hasher = DefaultHasher::new();
                    (account.equity as i64).hash(&mut hasher);
                    (account.daily_pnl as i64).hash(&mut hasher);
                    (account.unrealized_pnl as i64).hash(&mut hasher);
                    let account_hash = hasher.finish();
                    
                    if account_hash != last_account_hash {
                        let update = WsMessage::AccountUpdate {
                            equity: account.equity,
                            buying_power: account.buying_power,
                            cash: account.cash,
                            portfolio_value: account.portfolio_value,
                            daily_pnl: account.daily_pnl,
                            unrealized_pnl: account.unrealized_pnl,
                            realized_pnl: account.realized_pnl,
                            timestamp: Utc::now().to_rfc3339(),
                        };
                        
                        let _ = broadcaster.send(update);
                        last_account_hash = account_hash;
                    }
                }
            }
        });
    }
    
    /// Send order update to WebSocket clients
    pub async fn send_order_update(&self, order: &Order) {
        let message = WsMessage::OrderUpdate {
            order_id: order.id.to_string(),
            symbol: order.symbol.clone(),
            side: format!("{:?}", order.side),
            status: format!("{:?}", order.status),
            filled_quantity: order.filled_quantity,
            avg_fill_price: order.avg_fill_price,
            timestamp: Utc::now().to_rfc3339(),
        };
        
        let _ = self.broadcaster.send(message);
    }
    
    /// Send system event to WebSocket clients
    pub async fn send_system_event(&self, event_type: &str, message: &str, emergency_stop: bool) {
        let system_event = WsMessage::SystemEvent {
            event_type: event_type.to_string(),
            message: message.to_string(),
            emergency_stop,
            timestamp: Utc::now().to_rfc3339(),
        };
        
        let _ = self.broadcaster.send(system_event);
    }
}

// WebSocket handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((action_layer, broadcaster, clients)): State<(
        Arc<ActionLayer>,
        broadcast::Sender<WsMessage>,
        Arc<RwLock<HashMap<String, ClientConnection>>>,
    )>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, action_layer, broadcaster, clients))
}

async fn handle_socket(
    socket: WebSocket,
    action_layer: Arc<ActionLayer>,
    broadcaster: broadcast::Sender<WsMessage>,
    clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
) {
    let client_id = Uuid::new_v4().to_string();
    
    // Register client
    let connection = ClientConnection {
        id: client_id.clone(),
        subscriptions: vec![SubscriptionType::All], // Default subscription
        connected_at: Utc::now(),
        last_heartbeat: Utc::now(),
    };
    
    clients.write().await.insert(client_id.clone(), connection);
    
    // Send welcome message
    let welcome = WsMessage::Welcome {
        client_id: client_id.clone(),
        server_time: Utc::now().to_rfc3339(),
        available_subscriptions: vec![
            SubscriptionType::Orders,
            SubscriptionType::Positions,
            SubscriptionType::Account,
            SubscriptionType::System,
            SubscriptionType::PnL,
            SubscriptionType::All,
        ],
    };
    
    let (mut sender, mut receiver) = socket.split();
    
    // Send welcome message
    if let Ok(welcome_json) = serde_json::to_string(&welcome) {
        let _ = sender.send(Message::Text(welcome_json)).await;
    }
    
    // Start receiver for client messages
    let clients_clone = clients.clone();
    let client_id_clone = client_id.clone();
    tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if let Ok(msg) = msg {
                if let Err(e) = handle_client_message(msg, &client_id_clone, &clients_clone, &action_layer).await {
                    tracing::warn!("Error handling client message: {}", e);
                    break;
                }
            } else {
                break;
            }
        }
        
        // Clean up client on disconnect
        clients_clone.write().await.remove(&client_id_clone);
        tracing::info!("WebSocket client {} disconnected", client_id_clone);
    });
    
    // Start broadcaster receiver
    let mut broadcast_receiver = broadcaster.subscribe();
    let clients_for_send = clients.clone();
    let client_id_for_send = client_id.clone();
    
    tokio::spawn(async move {
        while let Ok(message) = broadcast_receiver.recv().await {
            // Check if client is still connected and should receive this message
            if let Some(client) = clients_for_send.read().await.get(&client_id_for_send) {
                if should_send_message(&message, &client.subscriptions) {
                    if let Ok(json) = serde_json::to_string(&message) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
    });
    
    tracing::info!("WebSocket client {} connected", client_id);
}

async fn handle_client_message(
    msg: Message,
    client_id: &str,
    clients: &Arc<RwLock<HashMap<String, ClientConnection>>>,
    action_layer: &Arc<ActionLayer>,
) -> Result<(), Box<dyn std::error::Error>> {
    use futures_util::StreamExt;
    
    match msg {
        Message::Text(text) => {
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                match client_msg {
                    ClientMessage::Subscribe { subscriptions } => {
                        if let Some(client) = clients.write().await.get_mut(client_id) {
                            client.subscriptions = subscriptions;
                        }
                    }
                    ClientMessage::Unsubscribe { subscriptions } => {
                        if let Some(client) = clients.write().await.get_mut(client_id) {
                            client.subscriptions.retain(|sub| !subscriptions.contains(sub));
                        }
                    }
                    ClientMessage::Heartbeat => {
                        if let Some(client) = clients.write().await.get_mut(client_id) {
                            client.last_heartbeat = Utc::now();
                        }
                    }
                    ClientMessage::GetSnapshot { data_type } => {
                        // Send current snapshot of requested data
                        match data_type {
                            SnapshotType::Positions => {
                                if let Ok(positions) = action_layer.get_positions().await {
                                    for (_, position) in positions {
                                        // Send position as update message
                                        // Implementation would send each position
                                    }
                                }
                            }
                            SnapshotType::Account => {
                                if let Ok(account) = action_layer.get_account().await {
                                    // Send account snapshot
                                    // Implementation would send account data
                                }
                            }
                            _ => {} // Other snapshot types
                        }
                    }
                }
            }
        }
        Message::Pong(_) => {
            // Update heartbeat
            if let Some(client) = clients.write().await.get_mut(client_id) {
                client.last_heartbeat = Utc::now();
            }
        }
        _ => {} // Ignore other message types
    }
    
    Ok(())
}

fn should_send_message(message: &WsMessage, subscriptions: &[SubscriptionType]) -> bool {
    if subscriptions.contains(&SubscriptionType::All) {
        return true;
    }
    
    match message {
        WsMessage::OrderUpdate { .. } => subscriptions.contains(&SubscriptionType::Orders),
        WsMessage::PositionUpdate { .. } => subscriptions.contains(&SubscriptionType::Positions),
        WsMessage::AccountUpdate { .. } => subscriptions.contains(&SubscriptionType::Account),
        WsMessage::SystemEvent { .. } => subscriptions.contains(&SubscriptionType::System),
        WsMessage::PnLUpdate { .. } => subscriptions.contains(&SubscriptionType::PnL),
        WsMessage::Heartbeat { .. } => true, // Always send heartbeats
        WsMessage::Error { .. } => true, // Always send errors
        WsMessage::Welcome { .. } => true, // Always send welcome
    }
}

// Health check endpoint for WebSocket server
async fn websocket_health() -> &'static str {
    "WebSocket server healthy"
}

// Re-export for easier access
pub use WebSocketServer as WsServer;