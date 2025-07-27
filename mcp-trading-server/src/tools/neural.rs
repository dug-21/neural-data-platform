use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::Result;
use crate::integrations::neural::NeuralClient;
use crate::models::{PricePrediction, TrendAnalysis, ChartPattern};

#[derive(Debug, Clone)]
pub struct NeuralPredictionTool {
    neural_client: Arc<NeuralClient>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum NeuralRequest {
    PredictPrice { 
        symbol: String, 
        horizon: String,
        confidence_threshold: Option<f64>,
    },
    AnalyzeTrend { 
        symbol: String,
    },
    RecognizePatterns { 
        symbol: String, 
        timeframe: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NeuralResponse {
    PricePrediction(PricePrediction),
    TrendAnalysis(TrendAnalysis),
    PatternRecognition { patterns: Vec<ChartPattern> },
}

impl NeuralPredictionTool {
    pub fn new(neural_client: Arc<NeuralClient>) -> Self {
        Self { neural_client }
    }

    pub async fn execute(&self, request: NeuralRequest) -> Result<NeuralResponse> {
        match request {
            NeuralRequest::PredictPrice { symbol, horizon, confidence_threshold } => {
                info!("Predicting price for {} (horizon: {})", symbol, horizon);
                // Convert horizon to timeframe and periods
                let (timeframe, periods) = match horizon.as_str() {
                    "1h" => ("5m", 12),
                    "4h" => ("15m", 16),
                    "1d" => ("1h", 24),
                    "1w" => ("4h", 42),
                    _ => ("1h", 24), // default
                };
                let prediction = self.neural_client
                    .predict_price(&symbol, timeframe, periods)
                    .await?;
                Ok(NeuralResponse::PricePrediction(prediction))
            }
            NeuralRequest::AnalyzeTrend { symbol } => {
                info!("Analyzing trend for {}", symbol);
                let analysis = self.neural_client
                    .analyze_trend(&symbol)
                    .await?;
                Ok(NeuralResponse::TrendAnalysis(analysis))
            }
            NeuralRequest::RecognizePatterns { symbol, timeframe } => {
                info!("Recognizing patterns for {} ({})", symbol, timeframe);
                let patterns = self.neural_client
                    .recognize_patterns(&symbol, &timeframe)
                    .await?;
                Ok(NeuralResponse::PatternRecognition { patterns })
            }
        }
    }
}