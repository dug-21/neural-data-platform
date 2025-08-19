//! REST API Server for Trading Operations
//!
//! Provides HTTP endpoints for order submission, position queries, and system control

use crate::action_layer::{ActionLayer, ActionLayerError, Order, OrderSide, OrderType, TimeInForce, OrderSource};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, IntoResponse},
    routing::{get, post, delete},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

pub struct ApiServer {
    port: u16,
    action_layer: Arc<ActionLayer>,
}

impl ApiServer {
    pub fn new(port: u16, action_layer: Arc<ActionLayer>) -> Self {
        Self {
            port,
            action_layer,
        }
    }
    
    pub async fn start(&self) -> Result<(), ActionLayerError> {
        let app = self.create_router().await;
        
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| ActionLayerError::Position(format!("Failed to bind to port {}: {}", self.port, e)))?;
        
        tracing::info!("🚀 Trading API server starting on port {}", self.port);
        
        axum::serve(listener, app)
            .await
            .map_err(|e| ActionLayerError::Position(format!("Server error: {}", e)))?;
        
        Ok(())
    }
    
    async fn create_router(&self) -> Router {
        Router::new()
            // Order management endpoints
            .route("/api/v1/orders", post(submit_order))
            .route("/api/v1/orders/:order_id", get(get_order))
            .route("/api/v1/orders/:order_id", delete(cancel_order))
            .route("/api/v1/orders", get(list_orders))
            
            // Position endpoints
            .route("/api/v1/positions", get(get_positions))
            .route("/api/v1/positions/:symbol", get(get_position))
            .route("/api/v1/positions/summary", get(get_position_summary))
            
            // Account endpoints
            .route("/api/v1/account", get(get_account))
            .route("/api/v1/account/pnl", get(get_pnl))
            
            // System control endpoints
            .route("/api/v1/system/status", get(get_system_status))
            .route("/api/v1/system/emergency_stop", post(emergency_stop))
            .route("/api/v1/system/resume", post(resume_trading))
            .route("/api/v1/system/health", get(health_check))
            
            // Risk endpoints
            .route("/api/v1/risk/validate", post(validate_order_risk))
            .route("/api/v1/risk/limits", get(get_risk_limits))
            
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    .into_inner(),
            )
            .with_state(self.action_layer.clone())
    }
}

// Request/Response types
#[derive(Debug, Deserialize)]
struct SubmitOrderRequest {
    symbol: String,
    side: String,
    quantity: f64,
    order_type: String,
    price: Option<f64>,
    time_in_force: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct SubmitOrderResponse {
    order_id: String,
    status: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct OrderResponse {
    id: String,
    symbol: String,
    side: String,
    quantity: f64,
    order_type: String,
    price: Option<f64>,
    time_in_force: String,
    status: String,
    filled_quantity: f64,
    avg_fill_price: Option<f64>,
    created_at: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct PositionResponse {
    symbol: String,
    quantity: f64,
    avg_entry_price: f64,
    current_price: f64,
    unrealized_pnl: f64,
    realized_pnl: f64,
    side: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct AccountResponse {
    equity: f64,
    buying_power: f64,
    cash: f64,
    day_trading_buying_power: f64,
    portfolio_value: f64,
    daily_pnl: f64,
    unrealized_pnl: f64,
    realized_pnl: f64,
    positions_count: usize,
}

#[derive(Debug, Serialize)]
struct SystemStatusResponse {
    status: String,
    emergency_stop_active: bool,
    paper_trading: bool,
    uptime_seconds: i64,
    session_id: String,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct EmergencyStopRequest {
    reason: String,
}

#[derive(Debug, Deserialize)]
struct OrderQuery {
    symbol: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
}

// API Handlers
async fn submit_order(
    State(action_layer): State<Arc<ActionLayer>>,
    Json(request): Json<SubmitOrderRequest>,
) -> Result<Json<SubmitOrderResponse>, ApiError> {
    // Parse order side
    let side = match request.side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => return Err(ApiError::BadRequest("Invalid order side".to_string())),
    };
    
    // Parse order type
    let order_type = match request.order_type.to_lowercase().as_str() {
        "market" => OrderType::Market,
        "limit" => OrderType::Limit,
        "stop" | "stop_loss" => OrderType::StopLoss,
        _ => return Err(ApiError::BadRequest("Invalid order type".to_string())),
    };
    
    // Parse time in force
    let time_in_force = match request.time_in_force.as_deref().unwrap_or("day") {
        "day" => TimeInForce::Day,
        "gtc" => TimeInForce::GoodTilCancelled,
        "ioc" => TimeInForce::ImmediateOrCancel,
        "fok" => TimeInForce::FillOrKill,
        _ => return Err(ApiError::BadRequest("Invalid time in force".to_string())),
    };
    
    // Parse source
    let source = match request.source.as_deref().unwrap_or("manual") {
        "neural" => OrderSource::Neural,
        "manual" => OrderSource::Manual,
        "risk" => OrderSource::Risk,
        "emergency" => OrderSource::Emergency,
        _ => return Err(ApiError::BadRequest("Invalid order source".to_string())),
    };
    
    // Validate required fields
    if request.quantity <= 0.0 {
        return Err(ApiError::BadRequest("Quantity must be positive".to_string()));
    }
    
    if matches!(order_type, OrderType::Limit | OrderType::StopLoss) && request.price.is_none() {
        return Err(ApiError::BadRequest("Price required for limit/stop orders".to_string()));
    }
    
    // Create order
    let order = Order {
        id: Uuid::new_v4(),
        symbol: request.symbol.to_uppercase(),
        side,
        quantity: request.quantity,
        order_type,
        price: request.price,
        time_in_force,
        created_at: Utc::now(),
        status: crate::action_layer::OrderStatus::Pending,
        filled_quantity: 0.0,
        avg_fill_price: None,
        source,
    };
    
    // Submit order
    match action_layer.submit_order(order).await {
        Ok(order_id) => Ok(Json(SubmitOrderResponse {
            order_id: order_id.to_string(),
            status: "submitted".to_string(),
            message: "Order submitted successfully".to_string(),
        })),
        Err(e) => Err(ApiError::from(e)),
    }
}

async fn get_order(
    State(action_layer): State<Arc<ActionLayer>>,
    Path(order_id): Path<String>,
) -> Result<Json<OrderResponse>, ApiError> {
    let uuid = Uuid::parse_str(&order_id)
        .map_err(|_| ApiError::BadRequest("Invalid order ID format".to_string()))?;
    
    match action_layer.get_order(uuid).await? {
        Some(order) => Ok(Json(OrderResponse {
            id: order.id.to_string(),
            symbol: order.symbol,
            side: format!("{:?}", order.side),
            quantity: order.quantity,
            order_type: format!("{:?}", order.order_type),
            price: order.price,
            time_in_force: format!("{:?}", order.time_in_force),
            status: format!("{:?}", order.status),
            filled_quantity: order.filled_quantity,
            avg_fill_price: order.avg_fill_price,
            created_at: order.created_at.to_rfc3339(),
            source: format!("{:?}", order.source),
        })),
        None => Err(ApiError::NotFound("Order not found".to_string())),
    }
}

async fn cancel_order(
    State(action_layer): State<Arc<ActionLayer>>,
    Path(order_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let uuid = Uuid::parse_str(&order_id)
        .map_err(|_| ApiError::BadRequest("Invalid order ID format".to_string()))?;
    
    action_layer.cancel_order(uuid).await?;
    
    Ok(Json(serde_json::json!({
        "order_id": order_id,
        "status": "cancelled",
        "message": "Order cancelled successfully"
    })))
}

async fn list_orders(
    State(action_layer): State<Arc<ActionLayer>>,
    Query(query): Query<OrderQuery>,
) -> Result<Json<Vec<OrderResponse>>, ApiError> {
    // In a real implementation, this would filter orders based on query parameters
    // For MVP, we'll return empty list as orders are not stored long-term
    Ok(Json(vec![]))
}

async fn get_positions(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<HashMap<String, PositionResponse>>, ApiError> {
    let positions = action_layer.get_positions().await?;
    
    let response: HashMap<String, PositionResponse> = positions
        .into_iter()
        .map(|(symbol, pos)| {
            (symbol.clone(), PositionResponse {
                symbol,
                quantity: pos.quantity,
                avg_entry_price: pos.avg_entry_price,
                current_price: pos.current_price,
                unrealized_pnl: pos.unrealized_pnl,
                realized_pnl: pos.realized_pnl,
                side: format!("{:?}", pos.side),
                created_at: pos.created_at.to_rfc3339(),
                updated_at: pos.updated_at.to_rfc3339(),
            })
        })
        .collect();
    
    Ok(Json(response))
}

async fn get_position(
    State(action_layer): State<Arc<ActionLayer>>,
    Path(symbol): Path<String>,
) -> Result<Json<PositionResponse>, ApiError> {
    let positions = action_layer.get_positions().await?;
    
    match positions.get(&symbol.to_uppercase()) {
        Some(pos) => Ok(Json(PositionResponse {
            symbol: pos.symbol.clone(),
            quantity: pos.quantity,
            avg_entry_price: pos.avg_entry_price,
            current_price: pos.current_price,
            unrealized_pnl: pos.unrealized_pnl,
            realized_pnl: pos.realized_pnl,
            side: format!("{:?}", pos.side),
            created_at: pos.created_at.to_rfc3339(),
            updated_at: pos.updated_at.to_rfc3339(),
        })),
        None => Err(ApiError::NotFound("Position not found".to_string())),
    }
}

async fn get_position_summary(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let positions = action_layer.get_positions().await?;
    
    let total_unrealized_pnl: f64 = positions.values().map(|p| p.unrealized_pnl).sum();
    let total_realized_pnl: f64 = positions.values().map(|p| p.realized_pnl).sum();
    let total_exposure: f64 = positions.values().map(|p| p.quantity * p.current_price).sum();
    
    Ok(Json(serde_json::json!({
        "total_positions": positions.len(),
        "total_unrealized_pnl": total_unrealized_pnl,
        "total_realized_pnl": total_realized_pnl,
        "total_pnl": total_unrealized_pnl + total_realized_pnl,
        "total_exposure": total_exposure,
    })))
}

async fn get_account(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<AccountResponse>, ApiError> {
    let account = action_layer.get_account().await?;
    
    Ok(Json(AccountResponse {
        equity: account.equity,
        buying_power: account.buying_power,
        cash: account.cash,
        day_trading_buying_power: account.day_trading_buying_power,
        portfolio_value: account.portfolio_value,
        daily_pnl: account.daily_pnl,
        unrealized_pnl: account.unrealized_pnl,
        realized_pnl: account.realized_pnl,
        positions_count: account.positions.len(),
    }))
}

async fn get_pnl(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let account = action_layer.get_account().await?;
    
    Ok(Json(serde_json::json!({
        "daily_pnl": account.daily_pnl,
        "unrealized_pnl": account.unrealized_pnl,
        "realized_pnl": account.realized_pnl,
        "total_pnl": account.unrealized_pnl + account.realized_pnl,
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

async fn get_system_status(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<SystemStatusResponse>, ApiError> {
    let emergency_status = action_layer.emergency_controls.get_state().await;
    
    Ok(Json(SystemStatusResponse {
        status: if emergency_status.is_stopped { "stopped".to_string() } else { "running".to_string() },
        emergency_stop_active: emergency_status.is_stopped,
        paper_trading: action_layer.config.paper_trading,
        uptime_seconds: emergency_status.uptime_seconds,
        session_id: "api_session".to_string(), // Would be actual session ID
        timestamp: Utc::now().to_rfc3339(),
    }))
}

async fn emergency_stop(
    State(action_layer): State<Arc<ActionLayer>>,
    Json(request): Json<EmergencyStopRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    action_layer.emergency_stop(&request.reason).await?;
    
    Ok(Json(serde_json::json!({
        "status": "stopped",
        "reason": request.reason,
        "timestamp": Utc::now().to_rfc3339(),
        "message": "Emergency stop activated"
    })))
}

async fn resume_trading(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (can_resume, issues) = action_layer.emergency_controls.can_resume().await;
    
    if !can_resume {
        return Err(ApiError::BadRequest(format!("Cannot resume: {}", issues.join(", "))));
    }
    
    action_layer.resume_trading().await?;
    
    Ok(Json(serde_json::json!({
        "status": "running",
        "timestamp": Utc::now().to_rfc3339(),
        "message": "Trading resumed"
    })))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now().to_rfc3339(),
        "service": "neural-trader-action-layer"
    }))
}

async fn validate_order_risk(
    State(action_layer): State<Arc<ActionLayer>>,
    Json(request): Json<SubmitOrderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // This would validate order against risk rules without submitting
    // For MVP, return a simple validation result
    Ok(Json(serde_json::json!({
        "valid": true,
        "risk_score": 0.3,
        "warnings": [],
        "recommendations": []
    })))
}

async fn get_risk_limits(
    State(action_layer): State<Arc<ActionLayer>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limits = &action_layer.config.risk;
    
    Ok(Json(serde_json::json!({
        "max_position_size": limits.max_position_size,
        "max_daily_loss": limits.max_daily_loss,
        "max_portfolio_risk": limits.max_portfolio_risk,
        "max_drawdown": limits.max_drawdown,
        "max_correlation_exposure": limits.max_correlation_exposure,
        "stop_loss_percentage": limits.stop_loss_percentage,
    })))
}

// Error handling
#[derive(Debug)]
enum ApiError {
    ActionLayer(ActionLayerError),
    BadRequest(String),
    NotFound(String),
    InternalServer(String),
}

impl From<ActionLayerError> for ApiError {
    fn from(err: ActionLayerError) -> Self {
        ApiError::ActionLayer(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::ActionLayer(e) => match e {
                ActionLayerError::RiskLimitExceeded(msg) => (StatusCode::FORBIDDEN, msg),
                ActionLayerError::EmergencyStop => (StatusCode::SERVICE_UNAVAILABLE, "Emergency stop active".to_string()),
                ActionLayerError::PaperTradingViolation(msg) => (StatusCode::FORBIDDEN, msg),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            },
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InternalServer(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        
        let error_response = serde_json::json!({
            "error": message,
            "timestamp": Utc::now().to_rfc3339(),
        });
        
        (status, Json(error_response)).into_response()
    }
}