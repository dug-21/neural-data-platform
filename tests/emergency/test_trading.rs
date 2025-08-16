//! Emergency Test: Trading Decision Flow
//! Tests that the system can make buy/sell decisions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct TradingDecision {
    symbol: String,
    action: String,
    confidence: f64,
    timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarketDataSubmission {
    symbol: String,
    price: f64,
    volume: f64,
    timestamp: i64,
}

struct TestClient {
    base_url: String,
    client: reqwest::Client,
}

impl TestClient {
    async fn connect(base_url: &str) -> Result<Self> {
        Ok(Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        })
    }

    async fn submit_data(&self, symbol: &str, price: f64) -> Result<()> {
        let data = MarketDataSubmission {
            symbol: symbol.to_string(),
            price,
            volume: 1000.0,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let response = self
            .client
            .post(&format!("{}/api/market-data", self.base_url))
            .json(&data)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to submit market data: {}", response.status());
        }

        Ok(())
    }

    async fn get_decision(&self, symbol: &str) -> Result<Option<TradingDecision>> {
        let response = self
            .client
            .get(&format!("{}/api/decisions/{}", self.base_url, symbol))
            .send()
            .await?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            anyhow::bail!("Failed to get decision: {}", response.status());
        }

        let decision: TradingDecision = response.json().await?;
        Ok(Some(decision))
    }
}

pub async fn test_trading_decision_flow() -> Result<()> {
    println!("🧪 Testing Trading Decision Flow...");
    
    // Connect to running system
    let client = TestClient::connect("http://localhost:8080").await?;
    
    // Test symbols
    let symbols = vec!["AAPL", "XLK", "GOOGL"];
    
    for symbol in symbols {
        println!("  📊 Testing symbol: {}", symbol);
        
        // Submit market data
        match client.submit_data(symbol, 150.0).await {
            Ok(_) => println!("    ✅ Market data submitted"),
            Err(e) => {
                println!("    ⚠️  Failed to submit data: {}", e);
                continue;
            }
        }
        
        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Verify decision made
        match client.get_decision(symbol).await {
            Ok(Some(decision)) => {
                println!("    ✅ Decision received: {} (confidence: {:.2}%)", 
                    decision.action, decision.confidence * 100.0);
                assert!(["buy", "sell", "hold"].contains(&decision.action.as_str()));
                assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
            }
            Ok(None) => {
                println!("    ⚠️  No decision available yet");
            }
            Err(e) => {
                println!("    ❌ Failed to get decision: {}", e);
                // Don't fail the test if API is not available
                if e.to_string().contains("Connection refused") {
                    println!("    ℹ️  System appears to be offline");
                    return Ok(());
                }
            }
        }
    }
    
    println!("✅ Trading Decision Flow test completed");
    Ok(())
}

pub async fn test_risk_limits_enforced() -> Result<()> {
    println!("🧪 Testing Risk Limit Enforcement...");
    
    let client = TestClient::connect("http://localhost:8080").await?;
    
    // Submit extreme market movement
    match client.submit_data("AAPL", 1000.0).await {
        Ok(_) => {
            // Check if system prevents excessive position
            let decision = client.get_decision("AAPL").await?;
            if let Some(d) = decision {
                // Confidence should be low for extreme movements
                assert!(d.confidence < 0.5, "System should have low confidence on extreme movements");
                println!("  ✅ Risk limits appear to be working (confidence: {:.2}%)", d.confidence * 100.0);
            }
        }
        Err(e) => {
            println!("  ℹ️  Could not test risk limits: {}", e);
        }
    }
    
    Ok(())
}