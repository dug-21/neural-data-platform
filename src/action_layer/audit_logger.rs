//! Audit Logging System
//!
//! Provides comprehensive logging of all trading activities for compliance and debugging

use crate::action_layer::{ActionLayerError, Order};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

pub struct AuditLogger {
    log_file: Mutex<tokio::fs::File>,
    session_id: String,
}

impl AuditLogger {
    pub fn new(log_path: &str) -> Result<Self, ActionLayerError> {
        // Ensure log directory exists
        if let Some(parent) = Path::new(log_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ActionLayerError::Position(format!("Failed to create log directory: {}", e)))?;
        }
        
        let session_id = uuid::Uuid::new_v4().to_string();
        
        // Open log file in append mode
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| ActionLayerError::Position(format!("Failed to open log file: {}", e)))?;
        
        Ok(Self {
            log_file: Mutex::new(tokio::fs::File::from_std(file)),
            session_id,
        })
    }
    
    async fn write_log_entry(&self, entry: &AuditLogEntry) -> Result<(), ActionLayerError> {
        let mut file = self.log_file.lock().await;
        let log_line = format!("{}\n", serde_json::to_string(entry).unwrap());
        
        file.write_all(log_line.as_bytes()).await
            .map_err(|e| ActionLayerError::Position(format!("Failed to write log: {}", e)))?;
        
        file.flush().await
            .map_err(|e| ActionLayerError::Position(format!("Failed to flush log: {}", e)))?;
        
        Ok(())
    }
    
    pub async fn log_order_submit(&self, order: &Order) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "ORDER_SUBMIT".to_string(),
            order_id: Some(order.id.to_string()),
            symbol: Some(order.symbol.clone()),
            side: Some(format!("{:?}", order.side)),
            quantity: Some(order.quantity),
            price: order.price,
            order_type: Some(format!("{:?}", order.order_type)),
            status: Some(format!("{:?}", order.status)),
            source: Some(format!("{:?}", order.source)),
            message: format!("Order submitted: {} {} @ {}", 
                order.quantity, 
                order.symbol, 
                order.price.map(|p| p.to_string()).unwrap_or("MARKET".to_string())
            ),
            metadata: Some(json!({
                "time_in_force": format!("{:?}", order.time_in_force),
                "created_at": order.created_at.to_rfc3339()
            })),
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_order_accepted(&self, order: &Order, broker_order_id: &str) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "ORDER_ACCEPTED".to_string(),
            order_id: Some(order.id.to_string()),
            symbol: Some(order.symbol.clone()),
            side: Some(format!("{:?}", order.side)),
            quantity: Some(order.quantity),
            price: order.price,
            order_type: Some(format!("{:?}", order.order_type)),
            status: Some(format!("{:?}", order.status)),
            source: Some(format!("{:?}", order.source)),
            message: format!("Order accepted by broker with ID: {}", broker_order_id),
            metadata: Some(json!({
                "broker_order_id": broker_order_id
            })),
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_order_fill(&self, order: &Order, fill_price: f64, fill_quantity: f64) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "ORDER_FILL".to_string(),
            order_id: Some(order.id.to_string()),
            symbol: Some(order.symbol.clone()),
            side: Some(format!("{:?}", order.side)),
            quantity: Some(fill_quantity),
            price: Some(fill_price),
            order_type: Some(format!("{:?}", order.order_type)),
            status: Some(format!("{:?}", order.status)),
            source: Some(format!("{:?}", order.source)),
            message: format!("Order filled: {} {} @ {}", fill_quantity, order.symbol, fill_price),
            metadata: Some(json!({
                "original_quantity": order.quantity,
                "remaining_quantity": order.quantity - fill_quantity
            })),
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_order_cancel(&self, order: &Order) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "ORDER_CANCEL".to_string(),
            order_id: Some(order.id.to_string()),
            symbol: Some(order.symbol.clone()),
            side: Some(format!("{:?}", order.side)),
            quantity: Some(order.quantity),
            price: order.price,
            order_type: Some(format!("{:?}", order.order_type)),
            status: Some(format!("{:?}", order.status)),
            source: Some(format!("{:?}", order.source)),
            message: format!("Order cancelled: {}", order.id),
            metadata: Some(json!({
                "filled_quantity": order.filled_quantity
            })),
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_order_reject(&self, order: &Order, reason: &str) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "ORDER_REJECT".to_string(),
            order_id: Some(order.id.to_string()),
            symbol: Some(order.symbol.clone()),
            side: Some(format!("{:?}", order.side)),
            quantity: Some(order.quantity),
            price: order.price,
            order_type: Some(format!("{:?}", order.order_type)),
            status: Some(format!("{:?}", order.status)),
            source: Some(format!("{:?}", order.source)),
            message: format!("Order rejected: {}", reason),
            metadata: None,
            error: Some(reason.to_string()),
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_risk_violation(&self, order: &Order, violation: &str) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "RISK_VIOLATION".to_string(),
            order_id: Some(order.id.to_string()),
            symbol: Some(order.symbol.clone()),
            side: Some(format!("{:?}", order.side)),
            quantity: Some(order.quantity),
            price: order.price,
            order_type: Some(format!("{:?}", order.order_type)),
            status: Some(format!("{:?}", order.status)),
            source: Some(format!("{:?}", order.source)),
            message: format!("Risk violation: {}", violation),
            metadata: None,
            error: Some(violation.to_string()),
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_emergency_stop(&self, reason: &str) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "EMERGENCY_STOP".to_string(),
            order_id: None,
            symbol: None,
            side: None,
            quantity: None,
            price: None,
            order_type: None,
            status: None,
            source: None,
            message: format!("EMERGENCY STOP ACTIVATED: {}", reason),
            metadata: None,
            error: Some(reason.to_string()),
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_trading_resume(&self) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "TRADING_RESUME".to_string(),
            order_id: None,
            symbol: None,
            side: None,
            quantity: None,
            price: None,
            order_type: None,
            status: None,
            source: None,
            message: "Trading resumed after emergency stop".to_string(),
            metadata: None,
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_position_update(&self, symbol: &str, old_quantity: f64, new_quantity: f64, price: f64) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "POSITION_UPDATE".to_string(),
            order_id: None,
            symbol: Some(symbol.to_string()),
            side: None,
            quantity: Some(new_quantity),
            price: Some(price),
            order_type: None,
            status: None,
            source: None,
            message: format!("Position updated: {} {} -> {}", symbol, old_quantity, new_quantity),
            metadata: Some(json!({
                "old_quantity": old_quantity,
                "new_quantity": new_quantity,
                "change": new_quantity - old_quantity
            })),
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_pnl_update(&self, symbol: &str, realized_pnl: f64, unrealized_pnl: f64) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "PNL_UPDATE".to_string(),
            order_id: None,
            symbol: Some(symbol.to_string()),
            side: None,
            quantity: None,
            price: None,
            order_type: None,
            status: None,
            source: None,
            message: format!("P&L update: {} realized: {:.2}, unrealized: {:.2}", 
                symbol, realized_pnl, unrealized_pnl),
            metadata: Some(json!({
                "realized_pnl": realized_pnl,
                "unrealized_pnl": unrealized_pnl,
                "total_pnl": realized_pnl + unrealized_pnl
            })),
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_system_event(&self, event: &str, details: Option<serde_json::Value>) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "SYSTEM_EVENT".to_string(),
            order_id: None,
            symbol: None,
            side: None,
            quantity: None,
            price: None,
            order_type: None,
            status: None,
            source: None,
            message: event.to_string(),
            metadata: details,
            error: None,
        };
        
        self.write_log_entry(&entry).await
    }
    
    pub async fn log_error(&self, error: &str, context: Option<serde_json::Value>) -> Result<(), ActionLayerError> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            event_type: "ERROR".to_string(),
            order_id: None,
            symbol: None,
            side: None,
            quantity: None,
            price: None,
            order_type: None,
            status: None,
            source: None,
            message: format!("Error: {}", error),
            metadata: context,
            error: Some(error.to_string()),
        };
        
        self.write_log_entry(&entry).await
    }
}

#[derive(Debug, serde::Serialize)]
struct AuditLogEntry {
    timestamp: DateTime<Utc>,
    session_id: String,
    event_type: String,
    order_id: Option<String>,
    symbol: Option<String>,
    side: Option<String>,
    quantity: Option<f64>,
    price: Option<f64>,
    order_type: Option<String>,
    status: Option<String>,
    source: Option<String>,
    message: String,
    metadata: Option<serde_json::Value>,
    error: Option<String>,
}