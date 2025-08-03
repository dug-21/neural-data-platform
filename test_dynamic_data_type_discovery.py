"""
Comprehensive Test Suite for Dynamic Data Type Discovery System

Tests the runtime data type discovery, characteristic-based matching,
and integration with neural model requirements.

Author: Data-Pipeline-Dev2
Date: 2025-08-02
"""

import pytest
import asyncio
import json
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, AsyncMock
import sys
import os

# Add the module path
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

from phase3_data_pipeline_type_discovery import (
    DataFrequency, DataScope, DataNature, DataQuality,
    DataCharacteristics, DiscoveredDataType, ModelDataRequirement,
    DynamicDataTypeRegistry, HeuristicDiscoveryStrategy,
    ModelDataMatcher, DataIngestionAdapter
)


# =============================================================================
# Test Data Fixtures
# =============================================================================

@pytest.fixture
def sample_price_data():
    """Sample OHLCV price data"""
    return {
        "timestamp": "2025-08-02T13:00:00Z",
        "symbol": "AAPL", 
        "open": 150.0,
        "high": 152.0,
        "low": 149.5,
        "close": 151.5,
        "volume": 1000000,
        "vwap": 151.2
    }

@pytest.fixture
def sample_sentiment_data():
    """Sample sentiment data"""
    return {
        "timestamp": "2025-08-02T13:00:00Z",
        "symbol": "AAPL",
        "news_sentiment": 0.65,
        "social_sentiment": 0.42,
        "analyst_sentiment": 0.78,
        "sentiment_momentum": 0.12,
        "confidence": 0.85
    }

@pytest.fixture
def sample_economic_data():
    """Sample economic indicators"""
    return {
        "timestamp": "2025-08-02T13:00:00Z",
        "region": "US",
        "gdp_growth": 2.1,
        "inflation_rate": 3.2,
        "unemployment_rate": 3.5,
        "interest_rate": 4.5,
        "pmi": 52.3
    }

@pytest.fixture
def sample_technical_data():
    """Sample technical indicators"""
    return {
        "timestamp": "2025-08-02T13:00:00Z",
        "symbol": "AAPL",
        "sma_20": 148.5,
        "ema_12": 150.2,
        "rsi_14": 65.3,
        "macd": 0.82,
        "bollinger_upper": 155.0,
        "bollinger_lower": 145.0,
        "atr": 2.15
    }

@pytest.fixture
def mock_redis_client():
    """Mock Redis client"""
    mock_redis = AsyncMock()
    mock_redis.pubsub.return_value = AsyncMock()
    return mock_redis

@pytest.fixture
def registry(mock_redis_client):
    """DynamicDataTypeRegistry instance"""
    return DynamicDataTypeRegistry(mock_redis_client)

@pytest.fixture
def model_requirements():
    """Sample model requirements"""
    return ModelDataRequirement(
        model_id="test_lstm_model",
        required_data=[
            DataCharacteristics(
                frequency=DataFrequency.MINUTE,
                scope=DataScope.SYMBOL,
                nature=DataNature.PRICE,
                quality=DataQuality.REQUIRED,
                feature_count=6
            )
        ],
        optional_data=[
            DataCharacteristics(
                frequency=DataFrequency.HOUR,
                scope=DataScope.SYMBOL,
                nature=DataNature.SENTIMENT,
                quality=DataQuality.OPTIONAL,
                feature_count=5
            )
        ],
        min_feature_count=5,
        max_latency_ms=500,
        min_reliability=0.8
    )


# =============================================================================
# Data Characteristics Tests
# =============================================================================

class TestDataCharacteristics:
    """Test the data characteristics framework"""
    
    def test_frequency_enum_properties(self):
        """Test frequency enum values and properties"""
        assert DataFrequency.MINUTE.seconds == 60
        assert DataFrequency.HOUR.seconds == 3600
        assert DataFrequency.DAILY.seconds == 86400
        assert DataFrequency.REAL_TIME.value == "1s"
    
    def test_data_nature_enum(self):
        """Test data nature enumeration"""
        assert DataNature.PRICE.value == "price"
        assert DataNature.SENTIMENT.value == "sentiment"
        assert DataNature.ECONOMIC.value == "economic"
    
    def test_characteristics_matching_exact(self):
        """Test exact matching of data characteristics"""
        chars1 = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        chars2 = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        matches, score = chars1.matches_requirements(chars2)
        assert matches is True
        assert score > 0.5
    
    def test_characteristics_matching_frequency_tolerance(self):
        """Test frequency tolerance in matching"""
        higher_freq = DataCharacteristics(
            frequency=DataFrequency.REAL_TIME,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        lower_freq = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        # Higher frequency should satisfy lower frequency requirement
        matches, score = higher_freq.matches_requirements(lower_freq)
        assert matches is True
        assert score > 0.0
    
    def test_characteristics_scope_compatibility(self):
        """Test scope compatibility logic"""
        symbol_scope = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        sector_scope = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SECTOR,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        # Symbol data should work for sector models
        matches, score = symbol_scope.matches_requirements(sector_scope)
        assert matches is True or score > 0.0  # Should have some compatibility
    
    def test_characteristics_nature_mismatch(self):
        """Test that different natures don't match"""
        price_nature = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        sentiment_nature = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.SENTIMENT,
            quality=DataQuality.REQUIRED
        )
        
        matches, score = price_nature.matches_requirements(sentiment_nature)
        assert matches is False


# =============================================================================
# Heuristic Discovery Strategy Tests
# =============================================================================

class TestHeuristicDiscoveryStrategy:
    """Test the heuristic data type discovery strategy"""
    
    @pytest.fixture
    def strategy(self):
        return HeuristicDiscoveryStrategy()
    
    @pytest.mark.asyncio
    async def test_discover_price_data(self, strategy, sample_price_data):
        """Test discovery of price data"""
        characteristics = await strategy.discover_from_sample(
            sample_price_data, "market_data:1min", {}
        )
        
        assert characteristics is not None
        assert characteristics.nature == DataNature.PRICE
        assert characteristics.frequency == DataFrequency.MINUTE
        assert characteristics.scope == DataScope.SYMBOL
    
    @pytest.mark.asyncio
    async def test_discover_sentiment_data(self, strategy, sample_sentiment_data):
        """Test discovery of sentiment data"""
        characteristics = await strategy.discover_from_sample(
            sample_sentiment_data, "sentiment:hourly", {}
        )
        
        assert characteristics is not None
        assert characteristics.nature == DataNature.SENTIMENT
        assert characteristics.frequency == DataFrequency.HOUR
        assert characteristics.scope == DataScope.SYMBOL
    
    @pytest.mark.asyncio
    async def test_discover_economic_data(self, strategy, sample_economic_data):
        """Test discovery of economic data"""
        characteristics = await strategy.discover_from_sample(
            sample_economic_data, "economic:daily", {}
        )
        
        assert characteristics is not None
        assert characteristics.nature == DataNature.ECONOMIC
        assert characteristics.frequency == DataFrequency.DAILY
        assert characteristics.scope == DataScope.GEOGRAPHIC
    
    @pytest.mark.asyncio
    async def test_discover_technical_data(self, strategy, sample_technical_data):
        """Test discovery of technical indicators"""
        characteristics = await strategy.discover_from_sample(
            sample_technical_data, "indicators:5min", {}
        )
        
        assert characteristics is not None
        assert characteristics.nature == DataNature.TECHNICAL
        assert characteristics.frequency == DataFrequency.FIVE_MINUTE
        assert characteristics.scope == DataScope.SYMBOL
    
    @pytest.mark.asyncio
    async def test_discover_unknown_data(self, strategy):
        """Test handling of unknown data patterns"""
        unknown_data = {
            "random_field": "random_value",
            "another_field": 42
        }
        
        characteristics = await strategy.discover_from_sample(
            unknown_data, "unknown:channel", {}
        )
        
        # Should return None for unrecognizable data
        assert characteristics is None
    
    def test_detect_price_patterns(self, strategy):
        """Test price pattern detection"""
        price_fields = {"open", "high", "low", "close", "volume"}
        price_data = {"open": 100, "high": 102, "low": 99, "close": 101}
        
        assert strategy._detect_price_data(price_fields, price_data) is True
        
        non_price_fields = {"random", "field"}
        assert strategy._detect_price_data(non_price_fields, {}) is False
    
    def test_detect_sentiment_patterns(self, strategy):
        """Test sentiment pattern detection"""
        sentiment_fields = {"sentiment", "news_sentiment", "bullish"}
        sentiment_data = {"sentiment": 0.5}
        
        assert strategy._detect_sentiment_data(sentiment_fields, sentiment_data) is True
        
        non_sentiment_fields = {"price", "volume"}
        assert strategy._detect_sentiment_data(non_sentiment_fields, {}) is False
    
    def test_frequency_detection_from_channel(self, strategy):
        """Test frequency detection from channel names"""
        test_cases = [
            ("market_data:1min", DataFrequency.MINUTE),
            ("prices:5min", DataFrequency.FIVE_MINUTE),
            ("sentiment:hourly", DataFrequency.HOUR),
            ("economic:daily", DataFrequency.DAILY),
            ("realtime:ticks", DataFrequency.REAL_TIME),
        ]
        
        for channel, expected_freq in test_cases:
            detected_freq = strategy._detect_data_frequency(channel, {}, {})
            assert detected_freq == expected_freq
    
    def test_scope_detection_from_channel(self, strategy):
        """Test scope detection from channel names"""
        test_cases = [
            ("market:SPY", DataScope.MARKET),
            ("sector:technology", DataScope.SECTOR),
            ("symbol:AAPL", DataScope.SYMBOL),
            ("region:US", DataScope.GEOGRAPHIC),
            ("global:indicators", DataScope.GLOBAL),
        ]
        
        for channel, expected_scope in test_cases:
            detected_scope = strategy._detect_data_scope(channel, set(), {})
            assert detected_scope == expected_scope


# =============================================================================
# Dynamic Data Type Registry Tests
# =============================================================================

class TestDynamicDataTypeRegistry:
    """Test the main registry functionality"""
    
    @pytest.mark.asyncio
    async def test_register_new_type(self, registry, sample_price_data):
        """Test registration of a new data type"""
        type_id = await registry.register_type(
            sample_price_data, "market_data:1min", {"source": "test"}
        )
        
        assert type_id is not None
        assert type_id in registry.discovered_types
        
        discovered_type = registry.discovered_types[type_id]
        assert discovered_type.characteristics.nature == DataNature.PRICE
        assert discovered_type.seen_count == 1
    
    @pytest.mark.asyncio
    async def test_register_existing_type_updates(self, registry, sample_price_data):
        """Test that registering the same type again updates it"""
        # Register once
        type_id1 = await registry.register_type(
            sample_price_data, "market_data:1min", {"source": "test"}
        )
        
        # Register again
        type_id2 = await registry.register_type(
            sample_price_data, "market_data:1min", {"source": "test"}
        )
        
        assert type_id1 == type_id2
        assert registry.discovered_types[type_id1].seen_count == 2
    
    @pytest.mark.asyncio
    async def test_discover_type_without_registration(self, registry, sample_sentiment_data):
        """Test type discovery without registration"""
        characteristics = await registry.discover_type(
            sample_sentiment_data, "sentiment:hourly"
        )
        
        assert characteristics is not None
        assert characteristics.nature == DataNature.SENTIMENT
        # Should not be in registry
        assert len(registry.discovered_types) == 0
    
    @pytest.mark.asyncio
    async def test_match_available_with_requirements(self, registry, model_requirements, sample_price_data):
        """Test matching available types with model requirements"""
        # Register model requirements
        registry.register_model_requirements("test_model", model_requirements)
        
        # Register compatible data type
        await registry.register_type(sample_price_data, "market_data:1min", {})
        
        # Find matches
        matches = await registry.match_available("test_model")
        
        assert len(matches) > 0
        type_id, score = matches[0]
        assert score > 0.5  # Should have good compatibility
    
    @pytest.mark.asyncio
    async def test_match_available_no_matches(self, registry, model_requirements):
        """Test matching when no compatible types are available"""
        # Register model requirements but no data types
        registry.register_model_requirements("test_model", model_requirements)
        
        matches = await registry.match_available("test_model")
        assert len(matches) == 0
    
    @pytest.mark.asyncio
    async def test_match_available_unknown_model(self, registry):
        """Test matching for unknown model"""
        matches = await registry.match_available("unknown_model")
        assert len(matches) == 0
    
    def test_get_available_types(self, registry):
        """Test getting all available types"""
        types = registry.get_available_types()
        assert isinstance(types, dict)
        assert len(types) == len(registry.discovered_types)
    
    def test_get_type_statistics(self, registry):
        """Test getting registry statistics"""
        stats = registry.get_type_statistics()
        
        required_keys = [
            'total_discoveries', 'successful_matches', 'failed_matches',
            'total_types', 'monitored_channels', 'cache_size'
        ]
        
        for key in required_keys:
            assert key in stats
        
        assert isinstance(stats['types_by_nature'], dict)
    
    @pytest.mark.asyncio
    async def test_optimize_registry(self, registry, sample_price_data):
        """Test registry optimization functionality"""
        # Add some data
        await registry.register_type(sample_price_data, "test_channel", {})
        
        # Run optimization
        recommendations = await registry.optimize_registry()
        
        assert 'actions_taken' in recommendations
        assert 'suggestions' in recommendations
        assert isinstance(recommendations['actions_taken'], list)
    
    def test_generate_type_id_uniqueness(self, registry):
        """Test that type IDs are generated uniquely"""
        chars1 = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED
        )
        
        chars2 = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.SENTIMENT,
            quality=DataQuality.REQUIRED
        )
        
        id1 = registry._generate_type_id(chars1, "channel1")
        id2 = registry._generate_type_id(chars2, "channel1")
        id3 = registry._generate_type_id(chars1, "channel2")
        
        # Different natures should give different IDs
        assert id1 != id2
        # Same characteristics, different channels should give different IDs
        assert id1 != id3
    
    def test_infer_schema(self, registry, sample_price_data):
        """Test schema inference from data"""
        schema = registry._infer_schema(sample_price_data)
        
        assert schema['timestamp'] == 'string'
        assert schema['symbol'] == 'string'
        assert schema['open'] == 'float'
        assert schema['volume'] == 'integer'
    
    def test_identify_required_fields(self, registry, sample_price_data):
        """Test identification of required fields"""
        required = registry._identify_required_fields(sample_price_data)
        
        expected_required = {'timestamp', 'symbol', 'close', 'volume'}
        assert expected_required.issubset(required)
    
    def test_check_model_constraints(self, registry, model_requirements):
        """Test model constraint checking"""
        # Create a discovered type that meets constraints
        good_type = DiscoveredDataType(
            type_id="test",
            characteristics=DataCharacteristics(
                frequency=DataFrequency.MINUTE,
                scope=DataScope.SYMBOL,
                nature=DataNature.PRICE,
                quality=DataQuality.REQUIRED,
                feature_count=10,
                latency_ms=100,
                reliability_score=0.9,
                coverage_ratio=0.95
            )
        )
        
        assert registry._check_model_constraints(good_type, model_requirements) is True
        
        # Create a type that violates constraints
        bad_type = DiscoveredDataType(
            type_id="test",
            characteristics=DataCharacteristics(
                frequency=DataFrequency.MINUTE,
                scope=DataScope.SYMBOL,
                nature=DataNature.PRICE,
                quality=DataQuality.REQUIRED,
                feature_count=2,  # Too few features
                latency_ms=2000,  # Too high latency
                reliability_score=0.5,  # Too low reliability
                coverage_ratio=0.5  # Too low coverage
            )
        )
        
        assert registry._check_model_constraints(bad_type, model_requirements) is False


# =============================================================================
# Model Data Matcher Tests
# =============================================================================

class TestModelDataMatcher:
    """Test the model data matching functionality"""
    
    @pytest.fixture
    def matcher(self, registry):
        return ModelDataMatcher(registry)
    
    @pytest.mark.asyncio
    async def test_find_optimal_configuration(self, matcher, registry, model_requirements,
                                            sample_price_data, sample_sentiment_data):
        """Test finding optimal data configuration for a model"""
        # Register requirements
        registry.register_model_requirements("test_model", model_requirements)
        
        # Register compatible data types
        await registry.register_type(sample_price_data, "market_data:1min", {})
        await registry.register_type(sample_sentiment_data, "sentiment:hourly", {})
        
        # Find optimal configuration
        config = await matcher.find_optimal_configuration("test_model")
        
        assert config is not None
        assert config['model_id'] == "test_model"
        assert len(config['primary_data']) > 0  # Should have required data
        assert config['completeness'] > 0.0
        assert config['total_score'] > 0.0
    
    @pytest.mark.asyncio
    async def test_find_optimal_configuration_no_requirements(self, matcher, registry):
        """Test configuration finding with no model requirements"""
        config = await matcher.find_optimal_configuration("unknown_model")
        assert config is None
    
    @pytest.mark.asyncio
    async def test_find_optimal_configuration_no_data(self, matcher, registry, model_requirements):
        """Test configuration finding with no available data"""
        registry.register_model_requirements("test_model", model_requirements)
        
        config = await matcher.find_optimal_configuration("test_model")
        # Should return None when no data is available
        assert config is None


# =============================================================================
# Data Ingestion Adapter Tests
# =============================================================================

class TestDataIngestionAdapter:
    """Test the data ingestion adapter"""
    
    @pytest.fixture
    def adapter(self, mock_redis_client, registry):
        return DataIngestionAdapter(mock_redis_client, registry)
    
    @pytest.mark.asyncio
    async def test_process_message(self, adapter, sample_price_data):
        """Test processing of Redis messages"""
        # Mock message
        message = {
            'type': 'pmessage',
            'channel': b'market_data:1min',
            'data': json.dumps(sample_price_data).encode('utf-8')
        }
        
        # Process message
        await adapter._process_message(message)
        
        # Check that type was registered
        assert len(adapter.registry.discovered_types) > 0
    
    @pytest.mark.asyncio
    async def test_process_invalid_message(self, adapter):
        """Test handling of invalid messages"""
        # Invalid JSON message
        message = {
            'type': 'pmessage',
            'channel': b'test_channel',
            'data': b'invalid json'
        }
        
        # Should not raise exception
        await adapter._process_message(message)
        
        # No types should be registered
        assert len(adapter.registry.discovered_types) == 0


# =============================================================================
# Integration Tests
# =============================================================================

class TestIntegration:
    """Integration tests for the complete system"""
    
    @pytest.mark.asyncio
    async def test_end_to_end_workflow(self, registry, model_requirements, 
                                     sample_price_data, sample_sentiment_data):
        """Test complete end-to-end workflow"""
        # 1. Register model requirements
        registry.register_model_requirements("integration_test_model", model_requirements)
        
        # 2. Register data types
        price_type_id = await registry.register_type(
            sample_price_data, "market_data:1min", {"source": "test"}
        )
        sentiment_type_id = await registry.register_type(
            sample_sentiment_data, "sentiment:hourly", {"source": "test"}
        )
        
        assert price_type_id is not None
        assert sentiment_type_id is not None
        
        # 3. Find matches
        matches = await registry.match_available("integration_test_model")
        assert len(matches) > 0
        
        # 4. Get optimal configuration
        matcher = ModelDataMatcher(registry)
        config = await matcher.find_optimal_configuration("integration_test_model")
        
        assert config is not None
        assert config['completeness'] >= 0.5  # Should have reasonable data coverage
        
        # 5. Get statistics
        stats = registry.get_type_statistics()
        assert stats['total_types'] == 2
        assert stats['successful_matches'] > 0
    
    @pytest.mark.asyncio
    async def test_multiple_models_same_data(self, registry, sample_price_data):
        """Test multiple models using the same data type"""
        # Create requirements for multiple models
        model1_req = ModelDataRequirement(
            model_id="model1",
            required_data=[
                DataCharacteristics(
                    frequency=DataFrequency.MINUTE,
                    scope=DataScope.SYMBOL,
                    nature=DataNature.PRICE,
                    quality=DataQuality.REQUIRED
                )
            ]
        )
        
        model2_req = ModelDataRequirement(
            model_id="model2",
            required_data=[
                DataCharacteristics(
                    frequency=DataFrequency.MINUTE,
                    scope=DataScope.SYMBOL,
                    nature=DataNature.PRICE,
                    quality=DataQuality.PREFERRED
                )
            ]
        )
        
        # Register both models
        registry.register_model_requirements("model1", model1_req)
        registry.register_model_requirements("model2", model2_req)
        
        # Register data type
        await registry.register_type(sample_price_data, "market_data:1min", {})
        
        # Both models should find matches
        matches1 = await registry.match_available("model1")
        matches2 = await registry.match_available("model2")
        
        assert len(matches1) > 0
        assert len(matches2) > 0
        
        # Should be using the same data type
        assert matches1[0][0] == matches2[0][0]
    
    @pytest.mark.asyncio
    async def test_data_type_evolution(self, registry, sample_price_data):
        """Test how data types evolve as more samples are seen"""
        # Register initial type
        type_id1 = await registry.register_type(
            sample_price_data, "market_data:1min", {"source": "test"}
        )
        
        initial_type = registry.discovered_types[type_id1]
        initial_seen_count = initial_type.seen_count
        initial_channels = len(initial_type.characteristics.source_channels)
        
        # Add more data with additional fields
        enhanced_data = sample_price_data.copy()
        enhanced_data['new_field'] = 42.0
        
        type_id2 = await registry.register_type(
            enhanced_data, "market_data:1min", {"source": "test"}
        )
        
        # Should be the same type ID
        assert type_id1 == type_id2
        
        # But seen count should increase
        updated_type = registry.discovered_types[type_id1]
        assert updated_type.seen_count > initial_seen_count
        
        # Schema should be updated to include new field
        assert 'new_field' in updated_type.schema
        assert 'new_field' in updated_type.optional_fields


# =============================================================================
# Performance Tests
# =============================================================================

class TestPerformance:
    """Performance tests for the discovery system"""
    
    @pytest.mark.asyncio
    async def test_discovery_latency(self, registry, sample_price_data):
        """Test that discovery operations complete within reasonable time"""
        import time
        
        start_time = time.time()
        
        # Register 100 data types
        for i in range(100):
            data = sample_price_data.copy()
            data['sequence'] = i
            await registry.register_type(data, f"channel_{i}", {})
        
        end_time = time.time()
        total_time = end_time - start_time
        
        # Should complete within 5 seconds for 100 registrations
        assert total_time < 5.0
        
        # Average time per registration should be reasonable
        avg_time_ms = (total_time / 100) * 1000
        assert avg_time_ms < 50  # Less than 50ms per registration
    
    @pytest.mark.asyncio
    async def test_matching_performance(self, registry, model_requirements, sample_price_data):
        """Test matching performance with many data types"""
        # Register model
        registry.register_model_requirements("test_model", model_requirements)
        
        # Register many similar data types
        for i in range(50):
            data = sample_price_data.copy()
            data['variant'] = i
            await registry.register_type(data, f"variant_channel_{i}", {})
        
        import time
        start_time = time.time()
        
        # Perform matching
        matches = await registry.match_available("test_model")
        
        end_time = time.time()
        match_time = end_time - start_time
        
        # Should complete matching quickly even with many types
        assert match_time < 1.0  # Less than 1 second
        assert len(matches) > 0  # Should find matches
    
    def test_memory_usage(self, registry, sample_price_data):
        """Test that memory usage is reasonable"""
        import sys
        
        initial_size = sys.getsizeof(registry.discovered_types)
        
        # Add many data types
        for i in range(1000):
            # Use asyncio.run for individual operations to avoid async fixture issues
            asyncio.run(registry.register_type(
                sample_price_data.copy(), f"mem_test_{i}", {}
            ))
        
        final_size = sys.getsizeof(registry.discovered_types)
        size_per_type = (final_size - initial_size) / 1000
        
        # Each type should use reasonable memory (less than 1KB overhead)
        assert size_per_type < 1024


# =============================================================================
# Error Handling Tests
# =============================================================================

class TestErrorHandling:
    """Test error handling and edge cases"""
    
    @pytest.mark.asyncio
    async def test_register_none_data(self, registry):
        """Test registration with None data"""
        result = await registry.register_type(None, "test_channel", {})
        assert result is None
    
    @pytest.mark.asyncio
    async def test_register_empty_data(self, registry):
        """Test registration with empty data"""
        result = await registry.register_type({}, "test_channel", {})
        assert result is None
    
    @pytest.mark.asyncio
    async def test_register_invalid_data(self, registry):
        """Test registration with invalid data types"""
        invalid_data = {"field": object()}  # Non-serializable object
        
        # Should handle gracefully
        result = await registry.register_type(invalid_data, "test_channel", {})
        # May return None or handle the object conversion
        # The key is it shouldn't crash
    
    @pytest.mark.asyncio
    async def test_match_with_corrupted_cache(self, registry, model_requirements):
        """Test matching with corrupted cache entries"""
        registry.register_model_requirements("test_model", model_requirements)
        
        # Corrupt the cache
        registry._compatibility_cache[("test_model", "fake_type")] = ("invalid", "data")
        
        # Should still work
        matches = await registry.match_available("test_model")
        assert isinstance(matches, list)
    
    def test_characteristics_with_invalid_values(self):
        """Test characteristics creation with invalid values"""
        # Test with extreme values
        chars = DataCharacteristics(
            frequency=DataFrequency.MINUTE,
            scope=DataScope.SYMBOL,
            nature=DataNature.PRICE,
            quality=DataQuality.REQUIRED,
            reliability_score=1.5,  # Invalid - should be 0-1
            coverage_ratio=-0.5     # Invalid - should be 0-1
        )
        
        # Object should still be created (validation could be added later)
        assert chars is not None


if __name__ == "__main__":
    # Run tests with pytest
    pytest.main([__file__, "-v", "--tb=short"])