"""
Example usage of neural prediction metrics integration.

This file demonstrates how to integrate the neural prediction metrics
into existing neural trader components for comprehensive monitoring.
"""

import asyncio
import time
from typing import List, Dict, Any
from datetime import datetime, timedelta
from dataclasses import dataclass
import logging

# Import the metrics integration
from utils.neural_metrics_integration import (
    neural_metrics_collector,
    track_enhanced_predictions,
    track_retraining_decision,
    track_ensemble_stats,
    track_regime_change,
    neural_prediction_tracker,
    MetricsReporter
)

logger = logging.getLogger(__name__)

# Mock data structures to simulate neural prediction results
@dataclass
class MockConfidenceBreakdown:
    base_confidence: float = 0.8
    ensemble_agreement: float = 0.15
    historical_accuracy: float = 0.05
    market_regime_adjustment: float = 0.02
    data_quality_factor: float = 0.95
    volatility_penalty: float = -0.03
    temporal_distance_penalty: float = -0.02

@dataclass
class MockEnhancedPredictionResult:
    timestamp: datetime
    value: float
    confidence: float
    confidence_breakdown: MockConfidenceBreakdown
    models_agree: bool = True
    model_agreement_score: float = 0.9
    interval_low: float = 0.0
    interval_high: float = 0.0
    ensemble_size: int = 5
    market_regime: str = "bullish"
    volatility_adjustment: float = 1.1

@dataclass
class MockRetrainingMetrics:
    should_retrain: bool = False
    current_accuracy: float = 0.85
    accuracy_threshold: float = 0.75
    hours_since_training: int = 12
    hours_threshold: int = 24
    new_samples: int = 5000
    sample_threshold: int = 10000
    primary_trigger: str = "none"
    urgency_score: float = 0.0

class ExampleNeuralPredictor:
    """Example neural predictor showing metrics integration."""
    
    def __init__(self, model_name: str = "example_predictor"):
        self.model_name = model_name
        self.last_regime = "unknown"
        self.prediction_count = 0
        
    @neural_prediction_tracker("example_predictor", "enhanced_ensemble")
    async def predict_with_enhanced_confidence(self, data: List[Any], horizon: int = 5) -> List[MockEnhancedPredictionResult]:
        """Example prediction method with automatic metrics tracking."""
        logger.info(f"Generating {horizon} predictions with enhanced confidence")
        
        predictions = []
        base_time = datetime.utcnow()
        
        for i in range(horizon):
            # Simulate prediction generation
            await asyncio.sleep(0.01)  # Simulate computation time
            
            # Create mock prediction with varying confidence
            confidence_base = 0.8 - (i * 0.05)  # Confidence decreases with horizon
            breakdown = MockConfidenceBreakdown(
                base_confidence=confidence_base,
                ensemble_agreement=0.15 if i < 3 else 0.10,  # Lower agreement for distant predictions
                temporal_distance_penalty=-0.01 * i
            )
            
            prediction = MockEnhancedPredictionResult(
                timestamp=base_time + timedelta(minutes=i+1),
                value=100.0 + (i * 0.5),  # Mock price progression
                confidence=confidence_base + breakdown.ensemble_agreement + breakdown.temporal_distance_penalty,
                confidence_breakdown=breakdown,
                interval_low=99.0 + (i * 0.5),
                interval_high=101.0 + (i * 0.5),
                ensemble_size=5,
                market_regime="bullish" if i < 3 else "sideways"
            )
            
            predictions.append(prediction)
        
        self.prediction_count += len(predictions)
        
        # Metrics are automatically tracked by the decorator
        # But we can also track additional custom metrics
        await self._track_custom_metrics(predictions)
        
        return predictions
    
    async def _track_custom_metrics(self, predictions: List[MockEnhancedPredictionResult]):
        """Track additional custom metrics beyond the automatic tracking."""
        
        # Track regime changes
        current_regime = predictions[-1].market_regime if predictions else "unknown"
        if current_regime != self.last_regime and self.last_regime != "unknown":
            await track_regime_change(self.last_regime, current_regime, confidence=0.85)
        self.last_regime = current_regime
        
        # Track custom accuracy metrics
        accuracy_metrics = {
            "short_term": 0.92,
            "medium_term": 0.87,
            "long_term": 0.81
        }
        await neural_metrics_collector.track_prediction_accuracy_update(
            self.model_name, accuracy_metrics
        )
        
        # Track data quality impact
        quality_metrics = {
            "completeness": 0.95,
            "freshness": 0.88,
            "outlier_detection": 0.92,
            "volume_consistency": 0.90
        }
        await neural_metrics_collector.track_data_quality_impact(quality_metrics)
    
    async def check_retraining_needs(self) -> MockRetrainingMetrics:
        """Check if model needs retraining and track the decision."""
        
        # Simulate retraining decision logic
        current_accuracy = 0.75 - (self.prediction_count * 0.001)  # Gradual degradation
        should_retrain = current_accuracy < 0.70 or self.prediction_count > 1000
        
        retraining_metrics = MockRetrainingMetrics(
            should_retrain=should_retrain,
            current_accuracy=current_accuracy,
            hours_since_training=24 if should_retrain else 12,
            new_samples=self.prediction_count * 10,
            primary_trigger="accuracy_degradation" if current_accuracy < 0.70 else "data_volume" if self.prediction_count > 1000 else "none",
            urgency_score=2.5 if should_retrain else 0.5
        )
        
        # Track the retraining decision
        await track_retraining_decision(retraining_metrics, self.model_name)
        
        return retraining_metrics
    
    async def get_ensemble_performance(self) -> Dict[str, Any]:
        """Get ensemble performance statistics and track them."""
        
        # Simulate ensemble statistics
        ensemble_stats = {
            "current_regime": self.last_regime,
            "dynamic_weights": {
                "DeepAR": 1.5,
                "LSTM": 1.2,
                "Transformer": 1.3,
                "GRU": 1.1,
                "NHITS": 1.0
            },
            "model_performances": {
                "DeepAR": {
                    "recent_accuracy": 0.89,
                    "confidence_score": 0.85,
                    "stability_score": 0.92,
                    "prediction_count": self.prediction_count // 5
                },
                "LSTM": {
                    "recent_accuracy": 0.86,
                    "confidence_score": 0.83,
                    "stability_score": 0.88,
                    "prediction_count": self.prediction_count // 5
                }
            },
            "diversity_metrics": {
                "DeepAR": 0.75,
                "LSTM": 0.68,
                "Transformer": 0.82
            },
            "volatility_adjustments": {
                "DeepAR": 1.1,
                "LSTM": 0.95,
                "Transformer": 1.05
            }
        }
        
        # Track ensemble performance
        await track_ensemble_stats(ensemble_stats)
        
        return ensemble_stats

async def run_neural_metrics_example():
    """Run a comprehensive example of neural metrics integration."""
    
    logger.info("Starting neural metrics integration example")
    
    # Create example predictor
    predictor = ExampleNeuralPredictor("example_model")
    
    # Simulate prediction cycles
    for cycle in range(5):
        logger.info(f"--- Prediction Cycle {cycle + 1} ---")
        
        # Generate predictions with automatic metrics tracking
        predictions = await predictor.predict_with_enhanced_confidence(
            data=[],  # Mock data
            horizon=5
        )
        
        logger.info(f"Generated {len(predictions)} predictions")
        
        # Check retraining needs
        retraining_metrics = await predictor.check_retraining_needs()
        if retraining_metrics.should_retrain:
            logger.info(f"Retraining recommended: {retraining_metrics.primary_trigger}")
        
        # Get ensemble performance
        ensemble_stats = await predictor.get_ensemble_performance()
        logger.info(f"Ensemble performance tracked for regime: {ensemble_stats['current_regime']}")
        
        # Simulate time passing
        await asyncio.sleep(0.1)
    
    # Generate performance summary
    logger.info("--- Performance Summary ---")
    summary = await MetricsReporter.generate_neural_performance_summary()
    logger.info(f"Performance summary: {summary}")
    
    # Check metrics health
    health = await MetricsReporter.check_neural_metrics_health()
    logger.info(f"Metrics health: {health}")
    
    logger.info("Neural metrics integration example completed")

def demonstrate_decorator_usage():
    """Demonstrate different ways to use the metrics decorators."""
    
    class ExampleModel:
        
        @neural_prediction_tracker("lstm_model", "sequence")
        async def lstm_predict(self, data):
            """LSTM prediction with automatic tracking."""
            await asyncio.sleep(0.05)  # Simulate computation
            return [MockEnhancedPredictionResult(
                timestamp=datetime.utcnow(),
                value=100.0,
                confidence=0.85,
                confidence_breakdown=MockConfidenceBreakdown(),
                interval_low=98.0,
                interval_high=102.0
            )]
        
        @neural_prediction_tracker("transformer_model", "attention")
        async def transformer_predict(self, data):
            """Transformer prediction with automatic tracking."""
            await asyncio.sleep(0.08)  # Simulate computation
            return [MockEnhancedPredictionResult(
                timestamp=datetime.utcnow(),
                value=101.5,
                confidence=0.88,
                confidence_breakdown=MockConfidenceBreakdown(),
                interval_low=99.0,
                interval_high=104.0
            )]
    
    return ExampleModel()

async def run_decorator_example():
    """Run example showing decorator usage."""
    
    logger.info("--- Decorator Usage Example ---")
    
    model = demonstrate_decorator_usage()
    
    # Test LSTM prediction with automatic metrics
    lstm_results = await model.lstm_predict([])
    logger.info(f"LSTM prediction: {lstm_results[0].value} (confidence: {lstm_results[0].confidence})")
    
    # Test Transformer prediction with automatic metrics
    transformer_results = await model.transformer_predict([])
    logger.info(f"Transformer prediction: {transformer_results[0].value} (confidence: {transformer_results[0].confidence})")

if __name__ == "__main__":
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Run examples
    asyncio.run(run_neural_metrics_example())
    asyncio.run(run_decorator_example())