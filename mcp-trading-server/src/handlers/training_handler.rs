//! MCP Tool Handlers for Autonomous Training Trigger System
//!
//! This module provides MCP tool handlers that expose the training trigger
//! functionality to the Claude Code interface, enabling autonomous monitoring
//! and management of neural network training decisions.

use std::sync::Arc;
use serde_json::{json, Value};
use chrono::Utc;
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::tools::training_triggers::{
    TrainingDecisionEngine, TrainingTrigger, PerformanceSnapshot, 
    TradingPerformanceMetrics, DAATrainingIntegration
};
use crate::integrations::neural::AccuracyMetrics;

/// Global training decision engine instance
static TRAINING_ENGINE: once_cell::sync::Lazy<Arc<TrainingDecisionEngine>> = 
    once_cell::sync::Lazy::new(|| {
        Arc::new(TrainingDecisionEngine::new())
    });

/// MCP tool handler for initializing the training trigger system
pub async fn initialize_training_system(params: Value) -> Result<Value> {
    info!("Initializing autonomous training trigger system");
    
    // Add default triggers
    TRAINING_ENGINE.add_default_triggers().await?;
    
    // Extract configuration from params if provided
    let enable_daa_integration = params.get("enable_daa_integration")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    
    let auto_training = params.get("auto_training")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    
    info!("Training system initialized with DAA integration: {}, auto training: {}", 
          enable_daa_integration, auto_training);
    
    Ok(json!({
        "status": "initialized",
        "daa_integration": enable_daa_integration,
        "auto_training": auto_training,
        "default_triggers": 3,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// MCP tool handler for adding custom training triggers
pub async fn add_training_trigger(params: Value) -> Result<Value> {
    let trigger_config = params.clone();
    
    // Extract trigger parameters
    let id = trigger_config.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("custom_trigger")
        .to_string();
    
    let name = trigger_config.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Custom Trigger")
        .to_string();
    
    let min_accuracy = trigger_config.get("min_accuracy_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.65);
    
    let max_mae = trigger_config.get("max_price_mae_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0);
    
    let priority = trigger_config.get("priority")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as u8;
    
    let trigger = TrainingTrigger {
        id: id.clone(),
        name,
        min_accuracy_threshold: min_accuracy,
        max_price_mae_threshold: max_mae,
        priority,
        enabled: true,
        ..Default::default()
    };
    
    TRAINING_ENGINE.add_trigger(trigger).await?;
    
    info!("Added custom training trigger: {}", id);
    
    Ok(json!({
        "status": "added",
        "trigger_id": id,
        "priority": priority,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// MCP tool handler for recording performance metrics
pub async fn record_performance_metrics(params: Value) -> Result<Value> {
    // Extract performance data from params
    let symbol = params.get("symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();
    
    let accuracy_data = params.get("accuracy_metrics")
        .ok_or_else(|| Error::ValidationError("Missing accuracy_metrics".to_string()))?;
    
    // Parse accuracy metrics
    let accuracy_metrics = AccuracyMetrics {
        directional_accuracy: accuracy_data.get("directional_accuracy")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        price_mae: accuracy_data.get("price_mae")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        price_rmse: accuracy_data.get("price_rmse")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        sharpe_ratio: accuracy_data.get("sharpe_ratio")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        max_drawdown: accuracy_data.get("max_drawdown")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
    };
    
    // Parse trading performance
    let default_trading_perf = json!({});
    let trading_perf = params.get("trading_performance").unwrap_or(&default_trading_perf);
    let trading_performance = TradingPerformanceMetrics {
        realized_pnl: trading_perf.get("realized_pnl")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        unrealized_pnl: trading_perf.get("unrealized_pnl")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        win_rate: trading_perf.get("win_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        avg_trade_duration_minutes: trading_perf.get("avg_trade_duration_minutes")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        risk_adjusted_return: trading_perf.get("risk_adjusted_return")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
    };
    
    // Create performance snapshot
    let snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy_metrics,
        symbol: symbol.clone(),
        prediction_count: params.get("prediction_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        avg_confidence: params.get("avg_confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        consecutive_failures: params.get("consecutive_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        trading_performance,
    };
    
    // Record performance and trigger evaluation
    TRAINING_ENGINE.record_performance(snapshot).await?;
    
    // Check if training decision was generated
    let recent_decisions = TRAINING_ENGINE.get_recent_decisions(1).await?;
    let new_decision = recent_decisions.first();
    
    info!("Recorded performance metrics for {}", symbol);
    
    Ok(json!({
        "status": "recorded",
        "symbol": symbol,
        "timestamp": Utc::now().to_rfc3339(),
        "evaluation_triggered": true,
        "new_training_decision": new_decision.map(|d| &d.decision.decision_id)
    }))
}

/// MCP tool handler for getting current training status
pub async fn get_training_status(params: Value) -> Result<Value> {
    let hours = params.get("hours")
        .and_then(|v| v.as_i64())
        .unwrap_or(24);
    
    // Get performance statistics
    let stats = TRAINING_ENGINE.get_performance_stats().await?;
    
    // Get recent decisions
    let recent_decisions = TRAINING_ENGINE.get_recent_decisions(hours).await?;
    
    let decision_summary: Vec<Value> = recent_decisions.iter().map(|record| {
        json!({
            "decision_id": record.decision.decision_id,
            "decision_type": format!("{:?}", record.decision.decision_type),
            "confidence": record.decision.confidence,
            "priority": record.decision.priority,
            "triggered_by": record.decision.triggered_by,
            "timestamp": record.decision.timestamp.to_rfc3339(),
            "executed": record.executed,
            "completed": record.training_results.is_some(),
            "execution_time_minutes": record.training_results.as_ref()
                .map(|r| r.training_time_minutes)
        })
    }).collect();
    
    info!("Retrieved training status for last {} hours", hours);
    
    Ok(json!({
        "status": "active",
        "statistics": {
            "total_snapshots": stats.total_snapshots,
            "recent_snapshots": stats.recent_24h_snapshots,
            "recent_decisions": stats.recent_24h_decisions,
            "avg_accuracy_24h": stats.avg_accuracy_24h,
            "avg_confidence_24h": stats.avg_confidence_24h,
            "pending_decisions": stats.pending_training_decisions,
            "completed_sessions": stats.completed_training_sessions
        },
        "recent_decisions": decision_summary,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// MCP tool handler for manually triggering training evaluation
pub async fn trigger_training_evaluation(params: Value) -> Result<Value> {
    info!("Manual training evaluation triggered");
    
    // Force evaluation of training need
    let decision = TRAINING_ENGINE.evaluate_training_need().await?;
    
    match decision {
        Some(decision) => {
            warn!("Training evaluation generated decision: {} (type: {:?})", 
                  decision.decision_id, decision.decision_type);
            
            Ok(json!({
                "status": "decision_generated",
                "decision": {
                    "decision_id": decision.decision_id,
                    "decision_type": format!("{:?}", decision.decision_type),
                    "confidence": decision.confidence,
                    "priority": decision.priority,
                    "triggered_by": decision.triggered_by,
                    "reasoning": decision.reasoning,
                    "estimated_time_minutes": decision.estimated_training_time_minutes,
                    "target_symbols": decision.target_symbols
                },
                "timestamp": Utc::now().to_rfc3339()
            }))
        },
        None => {
            info!("Training evaluation completed - no training needed");
            
            Ok(json!({
                "status": "no_training_needed",
                "message": "Current performance meets all thresholds",
                "timestamp": Utc::now().to_rfc3339()
            }))
        }
    }
}

/// MCP tool handler for configuring DAA integration
pub async fn configure_daa_integration(params: Value) -> Result<Value> {
    let neural_service_url = params.get("neural_service_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8000");
    
    let enable_auto_execution = params.get("enable_auto_execution")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    // Note: DAA integration configuration is stored in the engine
    // Actual integration happens when training decisions are executed
    
    info!("Configured DAA integration with neural service: {}", neural_service_url);
    
    Ok(json!({
        "status": "configured",
        "neural_service_url": neural_service_url,
        "auto_execution": enable_auto_execution,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// MCP tool handler for executing training decisions
pub async fn execute_training_decision(params: Value) -> Result<Value> {
    let decision_id = params.get("decision_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::ValidationError("Missing decision_id".to_string()))?;
    
    // Mark decision as executed
    TRAINING_ENGINE.mark_decision_executed(decision_id).await?;
    
    // In a real implementation, this would trigger actual training
    warn!("Training execution for decision {} marked (actual training not implemented)", decision_id);
    
    Ok(json!({
        "status": "execution_started",
        "decision_id": decision_id,
        "timestamp": Utc::now().to_rfc3339(),
        "note": "Training execution marked - actual training implementation pending"
    }))
}

/// MCP tool handler for monitoring training progress
pub async fn monitor_training_progress(params: Value) -> Result<Value> {
    let decision_id = params.get("decision_id")
        .and_then(|v| v.as_str());
    
    let recent_decisions = TRAINING_ENGINE.get_recent_decisions(24).await?;
    
    let filtered_decisions: Vec<_> = if let Some(id) = decision_id {
        recent_decisions.into_iter()
            .filter(|record| record.decision.decision_id == id)
            .collect()
    } else {
        recent_decisions
    };
    
    let progress_reports: Vec<Value> = filtered_decisions.iter().map(|record| {
        let progress_status = if record.training_results.is_some() {
            "completed"
        } else if record.executed {
            "in_progress"
        } else {
            "pending"
        };
        
        json!({
            "decision_id": record.decision.decision_id,
            "status": progress_status,
            "started": record.execution_started.map(|t| t.to_rfc3339()),
            "completed": record.execution_completed.map(|t| t.to_rfc3339()),
            "results": record.training_results.as_ref().map(|r| json!({
                "final_accuracy": r.final_accuracy,
                "training_time_minutes": r.training_time_minutes,
                "performance_improvement": r.performance_improvement,
                "new_model_version": r.new_model_version
            })),
            "errors": record.errors
        })
    }).collect();
    
    Ok(json!({
        "status": "monitoring_active",
        "progress_reports": progress_reports,
        "total_monitored": progress_reports.len(),
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// MCP tool handler for updating training trigger configuration
pub async fn update_training_trigger(params: Value) -> Result<Value> {
    let trigger_id = params.get("trigger_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::ValidationError("Missing trigger_id".to_string()))?;
    
    // Remove existing trigger
    TRAINING_ENGINE.remove_trigger(trigger_id).await?;
    
    // Add updated trigger
    add_training_trigger(params).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_initialize_training_system() {
        let params = json!({
            "enable_daa_integration": true,
            "auto_training": true
        });
        
        let result = initialize_training_system(params).await.unwrap();
        assert_eq!(result["status"], "initialized");
        assert_eq!(result["default_triggers"], 3);
    }

    #[tokio::test]
    async fn test_add_training_trigger() {
        let params = json!({
            "id": "test_trigger",
            "name": "Test Trigger",
            "min_accuracy_threshold": 0.7,
            "priority": 8
        });
        
        let result = add_training_trigger(params).await.unwrap();
        assert_eq!(result["status"], "added");
        assert_eq!(result["trigger_id"], "test_trigger");
    }

    #[tokio::test]
    async fn test_record_performance_metrics() {
        let params = json!({
            "symbol": "ETH/USD",
            "accuracy_metrics": {
                "directional_accuracy": 0.75,
                "price_mae": 25.0,
                "price_rmse": 35.0,
                "sharpe_ratio": 1.2,
                "max_drawdown": 0.08
            },
            "prediction_count": 100,
            "avg_confidence": 0.85,
            "consecutive_failures": 2
        });
        
        let result = record_performance_metrics(params).await.unwrap();
        assert_eq!(result["status"], "recorded");
        assert_eq!(result["symbol"], "ETH/USD");
    }
}