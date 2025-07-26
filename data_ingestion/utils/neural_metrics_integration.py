"""
Neural Prediction Metrics Integration Module

This module provides integration between the neural prediction system 
and the Prometheus metrics collection system. It includes utilities 
for tracking neural prediction performance, confidence scores, 
retraining events, and ensemble coordination.
"""

import asyncio
import time
from typing import List, Dict, Any, Optional
from datetime import datetime
import logging

try:
    from .metrics import metrics
except ImportError:
    # Fallback if metrics module not available
    metrics = None

logger = logging.getLogger(__name__)

class NeuralMetricsCollector:
    """Collector for neural prediction metrics with batching and async support."""
    
    def __init__(self):
        self.metrics_enabled = metrics is not None
        self.batch_metrics = []
        self.batch_size = 100
        self.last_flush = time.time()
        self.flush_interval = 30  # seconds
        
        if not self.metrics_enabled:
            logger.warning("Metrics system not available - neural metrics will be logged only")
    
    async def track_prediction_batch(self, predictions: List[Any], model_name: str = "unknown"):
        """Track a batch of predictions with automatic metrics extraction."""
        if not self.metrics_enabled:
            logger.info(f"Would track {len(predictions)} predictions for model {model_name}")
            return
        
        for prediction in predictions:
            try:
                if hasattr(prediction, 'confidence_breakdown'):
                    # Enhanced prediction result
                    metrics.track_neural_prediction_with_enhanced_confidence(prediction)
                elif hasattr(prediction, 'confidence'):
                    # Standard prediction result
                    market_regime = getattr(prediction, 'market_regime', 'unknown')
                    model_name_actual = getattr(prediction, 'model_name', model_name)
                    metrics.track_neural_prediction_confidence(
                        model_name=model_name_actual,
                        market_regime=market_regime,
                        confidence_score=prediction.confidence
                    )
            except Exception as e:
                logger.error(f"Error tracking prediction metrics: {e}")
    
    async def track_retraining_event(self, retraining_metrics: Any, model_name: str = "ensemble"):
        """Track retraining decision and metrics."""
        if not self.metrics_enabled:
            logger.info(f"Would track retraining event for model {model_name}")
            return
        
        try:
            metrics.track_neural_retraining_decision(retraining_metrics)
            logger.info(f"Tracked retraining metrics for {model_name}: should_retrain={retraining_metrics.should_retrain}")
        except Exception as e:
            logger.error(f"Error tracking retraining metrics: {e}")
    
    async def track_ensemble_performance(self, ensemble_stats: Dict[str, Any]):
        """Track comprehensive ensemble performance statistics."""
        if not self.metrics_enabled:
            logger.info(f"Would track ensemble performance with {len(ensemble_stats)} metrics")
            return
        
        try:
            metrics.track_neural_ensemble_performance(ensemble_stats)
            logger.debug("Tracked ensemble performance metrics")
        except Exception as e:
            logger.error(f"Error tracking ensemble performance: {e}")
    
    async def track_market_regime_change(self, previous_regime: str, new_regime: str, confidence: float = 0.8):
        """Track market regime detection events."""
        if not self.metrics_enabled:
            logger.info(f"Would track regime change: {previous_regime} -> {new_regime}")
            return
        
        try:
            confidence_level = "high" if confidence > 0.8 else "medium" if confidence > 0.6 else "low"
            metrics.track_neural_market_regime_detection(
                previous_regime=previous_regime,
                detected_regime=new_regime,
                confidence_level=confidence_level
            )
            logger.info(f"Tracked market regime change: {previous_regime} -> {new_regime} (confidence: {confidence:.2f})")
        except Exception as e:
            logger.error(f"Error tracking market regime change: {e}")
    
    async def track_prediction_accuracy_update(self, model_name: str, accuracy_metrics: Dict[str, float]):
        """Track prediction accuracy updates for specific models."""
        if not self.metrics_enabled:
            logger.info(f"Would track accuracy update for model {model_name}")
            return
        
        try:
            for accuracy_type, accuracy_value in accuracy_metrics.items():
                metrics.update_neural_prediction_accuracy(
                    model_name=model_name,
                    time_horizon="recent",
                    accuracy_type=accuracy_type,
                    accuracy_value=accuracy_value
                )
            logger.debug(f"Updated accuracy metrics for {model_name}: {accuracy_metrics}")
        except Exception as e:
            logger.error(f"Error tracking accuracy metrics: {e}")
    
    async def track_data_quality_impact(self, quality_metrics: Dict[str, float]):
        """Track data quality impact on prediction confidence."""
        if not self.metrics_enabled:
            logger.info(f"Would track data quality impact with {len(quality_metrics)} metrics")
            return
        
        try:
            for component, impact_value in quality_metrics.items():
                severity = "high" if impact_value < 0.7 else "medium" if impact_value < 0.9 else "low"
                metrics.update_neural_data_quality_impact(
                    quality_component=component,
                    severity_level=severity,
                    impact_value=impact_value
                )
            logger.debug(f"Tracked data quality impact: {quality_metrics}")
        except Exception as e:
            logger.error(f"Error tracking data quality metrics: {e}")
    
    def create_prediction_tracker_decorator(self, model_name: str, prediction_type: str = "standard"):
        """Create a decorator for automatic prediction tracking."""
        if not self.metrics_enabled:
            # Return a no-op decorator if metrics not available
            def no_op_decorator(func):
                return func
            return no_op_decorator
        
        return metrics.track_neural_prediction_operation(model_name, prediction_type)
    
    async def flush_batch_metrics(self):
        """Flush any batched metrics (if implemented in the future)."""
        if self.batch_metrics:
            logger.info(f"Flushing {len(self.batch_metrics)} batched metrics")
            self.batch_metrics.clear()
            self.last_flush = time.time()

# Global collector instance
neural_metrics_collector = NeuralMetricsCollector()

# Convenience functions for direct use
async def track_enhanced_predictions(predictions: List[Any], model_name: str = "ensemble"):
    """Convenience function to track enhanced prediction results."""
    await neural_metrics_collector.track_prediction_batch(predictions, model_name)

async def track_retraining_decision(retraining_metrics: Any, model_name: str = "ensemble"):
    """Convenience function to track retraining decisions."""
    await neural_metrics_collector.track_retraining_event(retraining_metrics, model_name)

async def track_ensemble_stats(ensemble_stats: Dict[str, Any]):
    """Convenience function to track ensemble statistics."""
    await neural_metrics_collector.track_ensemble_performance(ensemble_stats)

async def track_regime_change(previous: str, current: str, confidence: float = 0.8):
    """Convenience function to track market regime changes."""
    await neural_metrics_collector.track_market_regime_change(previous, current, confidence)

def neural_prediction_tracker(model_name: str, prediction_type: str = "standard"):
    """Decorator for automatic neural prediction tracking."""
    return neural_metrics_collector.create_prediction_tracker_decorator(model_name, prediction_type)

class MetricsReporter:
    """Reporter for generating metrics summaries and health reports."""
    
    @staticmethod
    async def generate_neural_performance_summary() -> Dict[str, Any]:
        """Generate a summary of neural prediction performance metrics."""
        if not metrics:
            return {"error": "Metrics system not available"}
        
        try:
            # This would typically query Prometheus for current metric values
            # For now, we return a structure that could be populated
            return {
                "timestamp": datetime.utcnow().isoformat(),
                "neural_metrics_summary": {
                    "prediction_confidence": {
                        "description": "Confidence score distribution across models",
                        "metric_name": "neural_trader_prediction_confidence_score"
                    },
                    "retraining_events": {
                        "description": "Recent retraining trigger events",
                        "metric_name": "neural_trader_retraining_triggers_total"
                    },
                    "ensemble_agreement": {
                        "description": "Model agreement in ensemble predictions",
                        "metric_name": "neural_trader_ensemble_agreement_score"
                    },
                    "prediction_accuracy": {
                        "description": "Current prediction accuracy by model",
                        "metric_name": "neural_trader_prediction_accuracy"
                    },
                    "market_regime_tracking": {
                        "description": "Market regime detection events",
                        "metric_name": "neural_trader_market_regime_detection_total"
                    }
                },
                "health_status": "operational",
                "metrics_collection_enabled": True
            }
        except Exception as e:
            logger.error(f"Error generating performance summary: {e}")
            return {"error": str(e)}
    
    @staticmethod
    async def check_neural_metrics_health() -> Dict[str, Any]:
        """Check the health of neural metrics collection."""
        return {
            "metrics_enabled": neural_metrics_collector.metrics_enabled,
            "collector_status": "healthy" if neural_metrics_collector.metrics_enabled else "degraded",
            "last_flush": neural_metrics_collector.last_flush,
            "batch_size": len(neural_metrics_collector.batch_metrics),
            "recommendations": [
                "Neural metrics collection is operational" if neural_metrics_collector.metrics_enabled
                else "Consider enabling Prometheus metrics collection for better observability"
            ]
        }

# Example usage in neural prediction code:
"""
from data_ingestion.utils.neural_metrics_integration import (
    track_enhanced_predictions, 
    track_retraining_decision,
    neural_prediction_tracker
)

# In your neural predictor code:
@neural_prediction_tracker("enhanced_predictor", "ensemble")
async def predict_with_confidence(self, data, horizon):
    # Your prediction logic here
    predictions = await self.generate_predictions(data, horizon)
    
    # Metrics are automatically tracked by the decorator
    return predictions

# For retraining events:
async def check_and_retrain(self):
    retraining_metrics = await self.should_retrain()
    await track_retraining_decision(retraining_metrics, "my_model")
    
    if retraining_metrics.should_retrain:
        await self.perform_retraining()

# For ensemble performance:
async def evaluate_ensemble(self):
    ensemble_stats = await self.get_ensemble_stats()
    await track_ensemble_stats(ensemble_stats)
"""