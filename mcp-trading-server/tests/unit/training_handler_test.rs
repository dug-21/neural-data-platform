//! Unit tests for the training handler module
//! Tests the MCP handler functions for autonomous training

use mcp_trading_server::handlers::training_handler;
use serde_json::json;

#[cfg(test)]
mod training_handler_tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize_training_system() {
        // Test basic initialization
        let params = json!({});
        let result = training_handler::initialize_training_system(params).await.unwrap();
        
        assert_eq!(result["status"], "initialized");
        assert_eq!(result["default_triggers"], 3);
        assert_eq!(result["daa_integration"], false);
        assert_eq!(result["auto_training"], false);
        
        // Test with DAA integration enabled
        let params_with_daa = json!({
            "enable_daa_integration": true,
            "auto_training": true
        });
        let result_daa = training_handler::initialize_training_system(params_with_daa).await.unwrap();
        
        assert_eq!(result_daa["status"], "initialized");
        assert_eq!(result_daa["daa_integration"], true);
        assert_eq!(result_daa["auto_training"], true);
    }

    #[tokio::test]
    async fn test_add_training_trigger() {
        // Initialize system first
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Add a custom trigger
        let trigger_params = json!({
            "id": "test_trigger",
            "name": "Test Trigger",
            "description": "A test trigger for unit testing",
            "min_accuracy_threshold": 0.75,
            "max_mae_threshold": 45.0,
            "priority": 8,
            "enabled": true,
            "cooldown_hours": 6
        });
        
        let result = training_handler::add_training_trigger(trigger_params).await.unwrap();
        
        assert_eq!(result["status"], "added");
        assert_eq!(result["trigger_id"], "test_trigger");
        assert_eq!(result["priority"], 8);
    }

    #[tokio::test]
    async fn test_add_training_trigger_with_all_fields() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Add a comprehensive trigger
        let trigger_params = json!({
            "id": "comprehensive_trigger",
            "name": "Comprehensive Trigger",
            "description": "Tests all trigger fields",
            "min_accuracy_threshold": 0.7,
            "max_accuracy_threshold": 0.95,
            "min_sharpe_ratio_threshold": 0.5,
            "max_sharpe_ratio_threshold": 2.0,
            "max_mae_threshold": 50.0,
            "max_rmse_threshold": 75.0,
            "max_drawdown_threshold": 0.15,
            "min_confidence_threshold": 0.65,
            "max_consecutive_failures_threshold": 5,
            "min_prediction_count": 30,
            "priority": 9,
            "enabled": true,
            "cooldown_hours": 12
        });
        
        let result = training_handler::add_training_trigger(trigger_params).await.unwrap();
        
        assert_eq!(result["status"], "added");
        assert_eq!(result["trigger_id"], "comprehensive_trigger");
    }

    #[tokio::test]
    async fn test_remove_training_trigger() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Add a trigger first
        let add_params = json!({
            "id": "removable_trigger",
            "name": "Removable Trigger",
            "priority": 5
        });
        training_handler::add_training_trigger(add_params).await.unwrap();
        
        // Remove the trigger
        let remove_params = json!({
            "trigger_id": "removable_trigger"
        });
        let result = training_handler::remove_training_trigger(remove_params).await.unwrap();
        
        assert_eq!(result["status"], "removed");
        assert_eq!(result["trigger_id"], "removable_trigger");
    }

    #[tokio::test]
    async fn test_list_training_triggers() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // List triggers (should have 3 default triggers)
        let params = json!({});
        let result = training_handler::list_training_triggers(params).await.unwrap();
        
        assert_eq!(result["status"], "success");
        assert_eq!(result["count"], 3);
        assert!(result["triggers"].is_array());
        
        let triggers = result["triggers"].as_array().unwrap();
        assert_eq!(triggers.len(), 3);
        
        // Verify default triggers are present
        let trigger_ids: Vec<&str> = triggers.iter()
            .map(|t| t["id"].as_str().unwrap())
            .collect();
        assert!(trigger_ids.contains(&"performance_degradation"));
        assert!(trigger_ids.contains(&"market_volatility"));
        assert!(trigger_ids.contains(&"confidence_drop"));
    }

    #[tokio::test]
    async fn test_record_performance_metrics() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Record good performance
        let good_metrics = json!({
            "symbol": "BTC/USD",
            "accuracy_metrics": {
                "directional_accuracy": 0.78,
                "price_mae": 35.0,
                "price_rmse": 55.0,
                "sharpe_ratio": 0.8,
                "max_drawdown": 0.08
            },
            "prediction_count": 100,
            "avg_confidence": 0.82,
            "consecutive_failures": 1,
            "trading_performance": {
                "realized_pnl": 500.0,
                "unrealized_pnl": 200.0,
                "win_rate": 0.68,
                "avg_trade_duration_minutes": 75.0,
                "risk_adjusted_return": 0.12
            }
        });
        
        let result = training_handler::record_performance_metrics(good_metrics).await.unwrap();
        
        assert_eq!(result["status"], "recorded");
        assert_eq!(result["symbol"], "BTC/USD");
        assert!(result["snapshot_count"] >= 1);
        assert_eq!(result["decision_triggered"], false);
    }

    #[tokio::test]
    async fn test_record_performance_triggers_training() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Record poor performance that should trigger training
        let poor_metrics = json!({
            "symbol": "ETH/USD",
            "accuracy_metrics": {
                "directional_accuracy": 0.45,
                "price_mae": 85.0,
                "price_rmse": 120.0,
                "sharpe_ratio": 0.2,
                "max_drawdown": 0.25
            },
            "prediction_count": 150,
            "avg_confidence": 0.48,
            "consecutive_failures": 8,
            "trading_performance": {
                "realized_pnl": -800.0,
                "unrealized_pnl": -400.0,
                "win_rate": 0.32,
                "avg_trade_duration_minutes": 180.0,
                "risk_adjusted_return": -0.18
            }
        });
        
        let result = training_handler::record_performance_metrics(poor_metrics).await.unwrap();
        
        assert_eq!(result["status"], "recorded");
        assert_eq!(result["symbol"], "ETH/USD");
        assert_eq!(result["decision_triggered"], true);
        assert!(result["decision"].is_object());
        
        let decision = &result["decision"];
        assert!(decision["decision_id"].is_string());
        assert_eq!(decision["decision_type"], "EmergencyRetraining");
        assert!(decision["confidence"].as_f64().unwrap() > 0.5);
    }

    #[tokio::test]
    async fn test_get_training_status() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Add some performance data
        let metrics = json!({
            "symbol": "BTC/USD",
            "accuracy_metrics": {
                "directional_accuracy": 0.65,
                "price_mae": 45.0,
                "price_rmse": 65.0,
                "sharpe_ratio": 0.6,
                "max_drawdown": 0.12
            },
            "prediction_count": 75,
            "avg_confidence": 0.7,
            "consecutive_failures": 3,
            "trading_performance": {
                "realized_pnl": 200.0,
                "unrealized_pnl": 100.0,
                "win_rate": 0.58,
                "avg_trade_duration_minutes": 90.0,
                "risk_adjusted_return": 0.05
            }
        });
        
        training_handler::record_performance_metrics(metrics).await.unwrap();
        
        // Get status
        let status_params = json!({ "hours": 24 });
        let result = training_handler::get_training_status(status_params).await.unwrap();
        
        assert_eq!(result["status"], "active");
        assert!(result["stats"].is_object());
        assert!(result["recent_decisions"].is_array());
        assert_eq!(result["triggers_count"], 3);
        
        let stats = &result["stats"];
        assert!(stats["total_snapshots"].as_u64().unwrap() >= 1);
        assert!(stats["recent_24h_snapshots"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn test_mark_decision_executed() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // First trigger a decision
        let poor_metrics = json!({
            "symbol": "BTC/USD",
            "accuracy_metrics": {
                "directional_accuracy": 0.4,
                "price_mae": 90.0,
                "price_rmse": 130.0,
                "sharpe_ratio": 0.1,
                "max_drawdown": 0.3
            },
            "prediction_count": 100,
            "avg_confidence": 0.4,
            "consecutive_failures": 10,
            "trading_performance": {
                "realized_pnl": -1000.0,
                "unrealized_pnl": -500.0,
                "win_rate": 0.25,
                "avg_trade_duration_minutes": 200.0,
                "risk_adjusted_return": -0.25
            }
        });
        
        let record_result = training_handler::record_performance_metrics(poor_metrics).await.unwrap();
        assert_eq!(record_result["decision_triggered"], true);
        
        let decision_id = record_result["decision"]["decision_id"].as_str().unwrap();
        
        // Mark as executed
        let execute_params = json!({
            "decision_id": decision_id
        });
        let result = training_handler::mark_decision_executed(execute_params).await.unwrap();
        
        assert_eq!(result["status"], "marked_executed");
        assert_eq!(result["decision_id"], decision_id);
    }

    #[tokio::test]
    async fn test_error_handling_uninitialized() {
        // Try to add trigger without initialization
        let trigger_params = json!({
            "id": "test",
            "name": "Test"
        });
        
        let result = training_handler::add_training_trigger(trigger_params).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(error.to_string().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_error_handling_missing_fields() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Try to add trigger without required fields
        let incomplete_trigger = json!({
            "name": "Incomplete"
        });
        
        let result = training_handler::add_training_trigger(incomplete_trigger).await;
        assert!(result.is_err());
        
        // Try to record metrics without required fields
        let incomplete_metrics = json!({
            "symbol": "BTC/USD"
        });
        
        let result = training_handler::record_performance_metrics(incomplete_metrics).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        let mut handles = Vec::new();
        
        // Spawn multiple concurrent metric recordings
        for i in 0..10 {
            let handle = tokio::spawn(async move {
                let metrics = json!({
                    "symbol": format!("TEST{}/USD", i),
                    "accuracy_metrics": {
                        "directional_accuracy": 0.6 + (i as f64 * 0.01),
                        "price_mae": 40.0 + (i as f64),
                        "price_rmse": 60.0 + (i as f64),
                        "sharpe_ratio": 0.5 + (i as f64 * 0.01),
                        "max_drawdown": 0.1 + (i as f64 * 0.01)
                    },
                    "prediction_count": 50 + i,
                    "avg_confidence": 0.7,
                    "consecutive_failures": i % 5,
                    "trading_performance": {
                        "realized_pnl": 100.0 * i as f64,
                        "unrealized_pnl": 50.0 * i as f64,
                        "win_rate": 0.6,
                        "avg_trade_duration_minutes": 90.0,
                        "risk_adjusted_return": 0.05
                    }
                });
                
                training_handler::record_performance_metrics(metrics).await
            });
            handles.push(handle);
        }
        
        // All operations should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        // Verify all were recorded
        let status = training_handler::get_training_status(json!({ "hours": 1 })).await.unwrap();
        assert!(status["stats"]["total_snapshots"].as_u64().unwrap() >= 10);
    }

    #[tokio::test]
    async fn test_training_status_with_decisions() {
        training_handler::initialize_training_system(json!({})).await.unwrap();
        
        // Generate multiple decisions
        for i in 0..3 {
            let metrics = json!({
                "symbol": format!("TEST{}/USD", i),
                "accuracy_metrics": {
                    "directional_accuracy": 0.4 - (i as f64 * 0.05),
                    "price_mae": 80.0 + (i as f64 * 10.0),
                    "price_rmse": 110.0 + (i as f64 * 10.0),
                    "sharpe_ratio": 0.2 - (i as f64 * 0.05),
                    "max_drawdown": 0.25 + (i as f64 * 0.05)
                },
                "prediction_count": 100,
                "avg_confidence": 0.4,
                "consecutive_failures": 10 + i,
                "trading_performance": {
                    "realized_pnl": -500.0 - (i as f64 * 200.0),
                    "unrealized_pnl": -200.0 - (i as f64 * 100.0),
                    "win_rate": 0.3 - (i as f64 * 0.05),
                    "avg_trade_duration_minutes": 180.0,
                    "risk_adjusted_return": -0.15 - (i as f64 * 0.05)
                }
            });
            
            training_handler::record_performance_metrics(metrics).await.unwrap();
        }
        
        // Get status with recent decisions
        let status = training_handler::get_training_status(json!({ "hours": 1 })).await.unwrap();
        
        assert!(status["recent_decisions"].as_array().unwrap().len() >= 1);
        assert!(status["stats"]["pending_training_decisions"].as_u64().unwrap() >= 1);
    }
}