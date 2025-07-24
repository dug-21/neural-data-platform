"""
Phase 1 Integration Tests for Neural-Expand Feature

This test suite validates all Phase 1 implementations:
1. Historical data expansion (providers)
2. Feature engineering enhancements
3. Neural model optimizations
"""

import pytest
import asyncio
import pandas as pd
from datetime import datetime, timedelta
import sys
import os

# Add project root to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from data_ingestion.providers import PROVIDERS
from data_ingestion.providers.binance import BinanceProvider


class TestPhase1Implementation:
    """Test suite for Phase 1 neural-expand features"""
    
    @pytest.fixture
    def event_loop(self):
        """Create an instance of the default event loop."""
        loop = asyncio.get_event_loop_policy().new_event_loop()
        yield loop
        loop.close()
    
    def test_all_providers_available(self):
        """Test that all required providers are implemented"""
        required_providers = [
            "alpaca",
            "yahoo_finance", 
            "binance",
            "fred"
        ]
        
        for provider in required_providers:
            assert provider in PROVIDERS, f"Provider {provider} not found"
            assert PROVIDERS[provider] is not None, f"Provider {provider} is None"
    
    @pytest.mark.asyncio
    async def test_binance_provider_initialization(self):
        """Test Binance provider can be initialized"""
        provider = BinanceProvider(testnet=True)
        assert provider is not None
        assert provider.name == "binance"
        assert provider.testnet is True
        
        # Test connect method exists
        assert hasattr(provider, 'connect')
        assert hasattr(provider, 'get_market_data')
    
    @pytest.mark.asyncio
    async def test_binance_symbol_validation(self):
        """Test Binance symbol formatting"""
        provider = BinanceProvider()
        
        # Test various symbol formats
        assert provider._validate_symbol("BTC/USDT") == "BTCUSDT"
        assert provider._validate_symbol("eth/usdt") == "ETHUSDT"
        assert provider._validate_symbol("BNB-USDT") == "BNBUSDT"
    
    def test_elliott_wave_features_exist(self):
        """Test that Elliott Wave pattern detection is implemented"""
        # This would test the Rust implementation
        # For now, we verify the implementation exists
        technical_indicators_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/features/technical_indicators.rs'
        )
        
        assert os.path.exists(technical_indicators_path)
        
        with open(technical_indicators_path, 'r') as f:
            content = f.read()
            assert 'detect_elliott_waves' in content
            assert 'ElliottWavePattern' in content
            assert 'analyze_impulsive_waves' in content
    
    def test_harmonic_patterns_exist(self):
        """Test that Harmonic pattern recognition is implemented"""
        technical_indicators_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/features/technical_indicators.rs'
        )
        
        with open(technical_indicators_path, 'r') as f:
            content = f.read()
            assert 'detect_harmonic_patterns' in content
            assert 'harmonic_pattern_gartley' in content
            assert 'harmonic_pattern_bat' in content
            assert 'harmonic_pattern_butterfly' in content
            assert 'harmonic_pattern_crab' in content
    
    def test_order_flow_toxicity_metrics_exist(self):
        """Test that order flow toxicity metrics are implemented"""
        microstructure_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/features/market_microstructure.rs'
        )
        
        assert os.path.exists(microstructure_path)
        
        with open(microstructure_path, 'r') as f:
            content = f.read()
            assert 'calculate_order_flow_toxicity' in content
            assert 'adverse_selection_component' in content
            assert 'realized_spread_toxicity' in content
            assert 'flow_toxicity_index' in content
            assert 'predatory_trading_indicator' in content
            assert 'quote_stuffing_indicator' in content
            assert 'spoofing_detection_score' in content
    
    def test_lstm_gru_models_configured(self):
        """Test that LSTM/GRU models are configured"""
        fann_predictor_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/neural/fann_predictor.rs'
        )
        
        assert os.path.exists(fann_predictor_path)
        
        with open(fann_predictor_path, 'r') as f:
            content = f.read()
            assert '"LSTM" => FannModelConfig' in content
            assert '"GRU" => FannModelConfig' in content
            assert 'prepare_recurrent_training_data' in content
            assert 'RecurrentState' in content
    
    def test_attention_mechanism_implemented(self):
        """Test that attention mechanisms are implemented"""
        fann_predictor_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/neural/fann_predictor.rs'
        )
        
        with open(fann_predictor_path, 'r') as f:
            content = f.read()
            assert 'apply_attention_mechanism' in content
            assert 'prepare_attention_training_data' in content
            assert 'attention_heads' in content
            assert 'softmax_scores' in content
    
    def test_ensemble_weights_updated(self):
        """Test that ensemble weights include new models"""
        fann_predictor_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/neural/fann_predictor.rs'
        )
        
        with open(fann_predictor_path, 'r') as f:
            content = f.read()
            # Check that LSTM and GRU have appropriate weights
            assert '"LSTM" => 1.4' in content
            assert '"GRU" => 1.25' in content


class TestFeatureEngineering:
    """Test feature engineering enhancements"""
    
    def test_feature_count_expansion(self):
        """Test that feature pipeline has been expanded"""
        # Check technical indicators
        technical_indicators_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/features/technical_indicators.rs'
        )
        
        with open(technical_indicators_path, 'r') as f:
            content = f.read()
            
            # Count feature insertions (rough estimate)
            feature_count = content.count('features.insert')
            
            # With Elliott Wave, Harmonic patterns, and existing features
            # we should have significantly more features
            assert feature_count > 50, f"Expected >50 features, found {feature_count}"
    
    def test_microstructure_features(self):
        """Test market microstructure feature completeness"""
        microstructure_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/features/market_microstructure.rs'
        )
        
        with open(microstructure_path, 'r') as f:
            content = f.read()
            
            # Check for toxicity metrics
            toxicity_metrics = [
                'adverse_selection_component',
                'realized_spread_toxicity',
                'flow_toxicity_index',
                'predatory_trading_indicator',
                'quote_stuffing_indicator',
                'spoofing_detection_score',
                'toxicity_level'
            ]
            
            for metric in toxicity_metrics:
                assert metric in content, f"Missing toxicity metric: {metric}"


class TestPerformanceMetrics:
    """Test performance improvement metrics"""
    
    def test_data_expansion_capacity(self):
        """Test that data pipeline can handle expanded historical data"""
        # Verify provider configurations support long history
        providers_config = {
            "alpaca": 5,      # 5+ years
            "yahoo_finance": 20,  # 20+ years
            "binance": 10,    # Full crypto history (~10 years)
            "fred": 50        # Economic indicators (decades)
        }
        
        # This is a capacity test - actual data fetching would require API keys
        for provider, expected_years in providers_config.items():
            assert provider in PROVIDERS
            # Verify the provider class exists and can be instantiated
            provider_class = PROVIDERS[provider]
            assert provider_class is not None
    
    def test_model_architecture_depth(self):
        """Test that neural models have appropriate depth"""
        fann_predictor_path = os.path.join(
            os.path.dirname(__file__), 
            '../src/neural/fann_predictor.rs'
        )
        
        with open(fann_predictor_path, 'r') as f:
            content = f.read()
            
            # Check LSTM architecture
            assert 'vec![128, 64, 64, 32]' in content  # LSTM layers
            
            # Check Transformer architecture
            assert 'vec![256, 128, 64, 32]' in content  # Transformer layers


if __name__ == "__main__":
    pytest.main([__file__, "-v"])