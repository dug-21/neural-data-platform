"""
Tests for neural prediction metrics integration.

This test suite validates that the neural prediction metrics are properly
integrated and can track all required aspects of the prediction system.
"""

import pytest
import asyncio
from unittest.mock import Mock, patch, MagicMock
from datetime import datetime, timedelta
import sys
import os

# Add parent directory to path for imports
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from utils.neural_metrics_integration import (
    NeuralMetricsCollector,
    track_enhanced_predictions,
    track_retraining_decision,
    track_ensemble_stats,
    neural_prediction_tracker,
    MetricsReporter
)

# Mock data classes for testing
class MockConfidenceBreakdown:
    def __init__(self):
        self.base_confidence = 0.8
        self.ensemble_agreement = 0.15
        self.historical_accuracy = 0.05
        self.market_regime_adjustment = 0.02
        self.data_quality_factor = 0.95
        self.volatility_penalty = -0.03
        self.temporal_distance_penalty = -0.02

class MockEnhancedPredictionResult:
    def __init__(self, confidence=0.85):
        self.timestamp = datetime.utcnow()
        self.value = 100.0
        self.confidence = confidence
        self.confidence_breakdown = MockConfidenceBreakdown()
        self.models_agree = True
        self.model_agreement_score = 0.9
        self.interval_low = 98.0
        self.interval_high = 102.0
        self.ensemble_size = 5
        self.market_regime = "bullish"
        self.volatility_adjustment = 1.1

class MockRetrainingMetrics:
    def __init__(self, should_retrain=False):
        self.should_retrain = should_retrain
        self.current_accuracy = 0.75
        self.accuracy_threshold = 0.7
        self.hours_since_training = 12
        self.hours_threshold = 24
        self.new_samples = 5000
        self.sample_threshold = 10000
        self.primary_trigger = "accuracy_degradation" if should_retrain else "none"
        self.urgency_score = 2.0 if should_retrain else 0.5

class TestNeuralMetricsCollector:
    """Test the NeuralMetricsCollector class."""
    
    @pytest.fixture
    def collector(self):
        return NeuralMetricsCollector()
    
    @pytest.fixture
    def mock_metrics(self):
        with patch('utils.neural_metrics_integration.metrics') as mock:
            mock.track_neural_prediction_with_enhanced_confidence = Mock()
            mock.track_neural_prediction_confidence = Mock()
            mock.track_neural_retraining_decision = Mock()
            mock.track_neural_ensemble_performance = Mock()
            mock.track_neural_market_regime_detection = Mock()
            mock.update_neural_prediction_accuracy = Mock()
            mock.update_neural_data_quality_impact = Mock()
            yield mock
    
    @pytest.mark.asyncio
    async def test_track_prediction_batch_enhanced(self, collector, mock_metrics):
        """Test tracking a batch of enhanced predictions."""
        predictions = [
            MockEnhancedPredictionResult(confidence=0.8),
            MockEnhancedPredictionResult(confidence=0.75),
            MockEnhancedPredictionResult(confidence=0.9)
        ]
        
        await collector.track_prediction_batch(predictions, "test_model")
        
        # Should call enhanced tracking for each prediction
        assert mock_metrics.track_neural_prediction_with_enhanced_confidence.call_count == 3
    
    @pytest.mark.asyncio
    async def test_track_retraining_event(self, collector, mock_metrics):
        """Test tracking retraining events."""
        retraining_metrics = MockRetrainingMetrics(should_retrain=True)
        
        await collector.track_retraining_event(retraining_metrics, "test_model")
        
        mock_metrics.track_neural_retraining_decision.assert_called_once_with(retraining_metrics)
    
    @pytest.mark.asyncio
    async def test_track_ensemble_performance(self, collector, mock_metrics):
        """Test tracking ensemble performance statistics."""
        ensemble_stats = {
            "current_regime": "bullish",
            "dynamic_weights": {"model1": 1.2, "model2": 0.8},
            "model_performances": {
                "model1": {"recent_accuracy": 0.85}
            }
        }
        
        await collector.track_ensemble_performance(ensemble_stats)
        
        mock_metrics.track_neural_ensemble_performance.assert_called_once_with(ensemble_stats)
    
    @pytest.mark.asyncio
    async def test_track_market_regime_change(self, collector, mock_metrics):
        """Test tracking market regime changes."""
        await collector.track_market_regime_change("bullish", "bearish", confidence=0.9)
        
        mock_metrics.track_neural_market_regime_detection.assert_called_once_with(
            previous_regime="bullish",
            detected_regime="bearish", 
            confidence_level="high"
        )
    
    @pytest.mark.asyncio
    async def test_track_prediction_accuracy_update(self, collector, mock_metrics):
        """Test tracking prediction accuracy updates."""
        accuracy_metrics = {
            "short_term": 0.9,
            "medium_term": 0.85,
            "long_term": 0.8
        }
        
        await collector.track_prediction_accuracy_update("test_model", accuracy_metrics)
        
        # Should be called once for each accuracy type
        assert mock_metrics.update_neural_prediction_accuracy.call_count == 3
    
    @pytest.mark.asyncio
    async def test_track_data_quality_impact(self, collector, mock_metrics):
        """Test tracking data quality impact on predictions."""
        quality_metrics = {
            "completeness": 0.95,
            "freshness": 0.8,
            "outliers": 0.6  # Low score indicates high impact
        }
        
        await collector.track_data_quality_impact(quality_metrics)
        
        # Should be called once for each quality component
        assert mock_metrics.update_neural_data_quality_impact.call_count == 3
    
    def test_create_prediction_tracker_decorator(self, collector, mock_metrics):
        """Test creating prediction tracker decorator."""
        decorator = collector.create_prediction_tracker_decorator("test_model", "ensemble")
        
        # Should return a callable decorator
        assert callable(decorator)
        
        # Test decorator application
        @decorator
        async def dummy_prediction_function():
            return [MockEnhancedPredictionResult()]
        
        assert callable(dummy_prediction_function)

class TestNeuralPredictionTracker:
    """Test the neural prediction tracker decorator."""
    
    @pytest.fixture
    def mock_metrics(self):
        with patch('utils.neural_metrics_integration.metrics') as mock:
            mock.track_neural_prediction_latency = Mock()
            mock.track_neural_prediction_with_enhanced_confidence = Mock()
            mock.track_neural_prediction_confidence = Mock()
            yield mock
    
    @pytest.mark.asyncio
    async def test_decorator_tracks_latency(self, mock_metrics):
        """Test that decorator tracks prediction latency."""
        
        @neural_prediction_tracker("test_model", "test_type")
        async def test_prediction():
            await asyncio.sleep(0.01)  # Simulate work
            return [MockEnhancedPredictionResult()]
        
        result = await test_prediction()
        
        # Should track latency
        mock_metrics.track_neural_prediction_latency.assert_called()
        
        # Should return results
        assert len(result) == 1
        assert isinstance(result[0], MockEnhancedPredictionResult)
    
    @pytest.mark.asyncio
    async def test_decorator_tracks_enhanced_predictions(self, mock_metrics):
        """Test that decorator automatically tracks enhanced prediction results."""
        
        @neural_prediction_tracker("test_model", "enhanced")
        async def test_prediction():
            return [
                MockEnhancedPredictionResult(confidence=0.8),
                MockEnhancedPredictionResult(confidence=0.9)
            ]
        
        await test_prediction()
        
        # Should track each enhanced prediction
        assert mock_metrics.track_neural_prediction_with_enhanced_confidence.call_count == 2
    
    @pytest.mark.asyncio
    async def test_decorator_handles_errors(self, mock_metrics):
        """Test that decorator handles errors gracefully."""
        
        @neural_prediction_tracker("test_model", "error_test")
        async def failing_prediction():
            raise ValueError("Test error")
        
        with pytest.raises(ValueError):
            await failing_prediction()
        
        # Should still track latency for failed predictions
        mock_metrics.track_neural_prediction_latency.assert_called()
        call_args = mock_metrics.track_neural_prediction_latency.call_args
        assert "error_test_failed" in str(call_args)

class TestConvenienceFunctions:
    """Test the convenience functions for direct metric tracking."""
    
    @pytest.fixture
    def mock_collector(self):
        with patch('utils.neural_metrics_integration.neural_metrics_collector') as mock:
            mock.track_prediction_batch = Mock()
            mock.track_retraining_event = Mock()
            mock.track_ensemble_performance = Mock()
            mock.track_market_regime_change = Mock()
            yield mock
    
    @pytest.mark.asyncio
    async def test_track_enhanced_predictions(self, mock_collector):
        """Test convenience function for tracking enhanced predictions."""
        predictions = [MockEnhancedPredictionResult()]
        
        await track_enhanced_predictions(predictions, "test_model")
        
        mock_collector.track_prediction_batch.assert_called_once_with(predictions, "test_model")
    
    @pytest.mark.asyncio
    async def test_track_retraining_decision(self, mock_collector):
        """Test convenience function for tracking retraining decisions."""
        metrics = MockRetrainingMetrics(should_retrain=True)
        
        await track_retraining_decision(metrics, "test_model")
        
        mock_collector.track_retraining_event.assert_called_once_with(metrics, "test_model")
    
    @pytest.mark.asyncio
    async def test_track_ensemble_stats(self, mock_collector):
        """Test convenience function for tracking ensemble statistics."""
        stats = {"current_regime": "bullish"}
        
        await track_ensemble_stats(stats)
        
        mock_collector.track_ensemble_performance.assert_called_once_with(stats)

class TestMetricsReporter:
    """Test the MetricsReporter class."""
    
    @pytest.mark.asyncio
    async def test_generate_neural_performance_summary(self):
        """Test generating neural performance summary."""
        summary = await MetricsReporter.generate_neural_performance_summary()
        
        assert isinstance(summary, dict)
        assert "neural_metrics_summary" in summary
        assert "timestamp" in summary
        assert "health_status" in summary
        
        # Check that all expected metric categories are included
        metrics_summary = summary["neural_metrics_summary"]
        expected_categories = [
            "prediction_confidence",
            "retraining_events", 
            "ensemble_agreement",
            "prediction_accuracy",
            "market_regime_tracking"
        ]
        
        for category in expected_categories:
            assert category in metrics_summary
            assert "metric_name" in metrics_summary[category]
    
    @pytest.mark.asyncio
    async def test_check_neural_metrics_health(self):
        """Test checking neural metrics health."""
        health = await MetricsReporter.check_neural_metrics_health()
        
        assert isinstance(health, dict)
        assert "metrics_enabled" in health
        assert "collector_status" in health
        assert "recommendations" in health
        assert isinstance(health["recommendations"], list)

class TestMetricsIntegrationDisabled:
    """Test behavior when metrics system is not available."""
    
    @pytest.fixture
    def collector_no_metrics(self):
        with patch('utils.neural_metrics_integration.metrics', None):
            collector = NeuralMetricsCollector()
            assert not collector.metrics_enabled
            return collector
    
    @pytest.mark.asyncio
    async def test_graceful_degradation(self, collector_no_metrics):
        """Test that system degrades gracefully when metrics unavailable."""
        predictions = [MockEnhancedPredictionResult()]
        
        # Should not raise errors
        await collector_no_metrics.track_prediction_batch(predictions, "test_model")
        await collector_no_metrics.track_retraining_event(MockRetrainingMetrics(), "test_model")
        await collector_no_metrics.track_ensemble_performance({})
        await collector_no_metrics.track_market_regime_change("bullish", "bearish")
    
    def test_no_op_decorator(self, collector_no_metrics):
        """Test that decorator becomes no-op when metrics unavailable."""
        decorator = collector_no_metrics.create_prediction_tracker_decorator("test_model")
        
        @decorator
        async def test_function():
            return "test_result"
        
        # Function should work normally but without metrics tracking
        result = asyncio.run(test_function())
        assert result == "test_result"

if __name__ == "__main__":
    # Run tests with pytest
    pytest.main([__file__, "-v"])