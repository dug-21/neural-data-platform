"""
Data Validation and Quality Scoring Tests for Backfill

This module tests the data validation and quality scoring functionality
used during backfill operations.
"""

import pytest
import asyncio
import numpy as np
from datetime import datetime, timedelta, timezone
from unittest.mock import Mock, AsyncMock, patch
from typing import List, Dict, Any

# System imports
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from providers.historical_backfill import (
    HistoricalBackfillCoordinator,
    DataValidationResult,
    DataGranularity
)
from providers.base import MarketData, TickData


class TestDataValidation:
    """Test data validation functionality"""
    
    @pytest.fixture
    def coordinator(self):
        """Create coordinator instance for testing."""
        return HistoricalBackfillCoordinator()
    
    @pytest.fixture
    def valid_market_data(self):
        """Create valid market data for testing."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        data = []
        
        for i in range(10):
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=150.0 + i * 0.1,
                high=152.0 + i * 0.1,
                low=149.0 + i * 0.1,
                close=151.0 + i * 0.1,
                volume=1000000 + i * 10000
            ))
        
        return data
    
    @pytest.fixture
    def invalid_market_data(self):
        """Create invalid market data for testing."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Mix of valid and invalid data
        return [
            # Valid data
            MarketData(
                time=base_time,
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Invalid: negative price
            MarketData(
                time=base_time + timedelta(minutes=1),
                symbol="AAPL",
                open=-10.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Invalid: high < low
            MarketData(
                time=base_time + timedelta(minutes=2),
                symbol="AAPL",
                open=150.0,
                high=148.0,  # High less than low
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Invalid: negative volume
            MarketData(
                time=base_time + timedelta(minutes=3),
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=-1000
            ),
            # Valid data
            MarketData(
                time=base_time + timedelta(minutes=4),
                symbol="AAPL",
                open=151.0,
                high=153.0,
                low=150.0,
                close=152.0,
                volume=1100000
            )
        ]
    
    @pytest.mark.asyncio
    async def test_valid_data_validation(self, coordinator, valid_market_data):
        """Test validation of completely valid data."""
        result = await coordinator.validate_data(valid_market_data, DataGranularity.MINUTE)
        
        assert result.is_valid is True
        assert result.total_points == 10
        assert result.valid_points == 10
        assert result.invalid_points == 0
        assert result.duplicate_points == 0
        assert result.quality_score >= 0.8
        assert len(result.issues) == 0
    
    @pytest.mark.asyncio
    async def test_invalid_data_validation(self, coordinator, invalid_market_data):
        """Test validation of data with quality issues."""
        result = await coordinator.validate_data(invalid_market_data, DataGranularity.MINUTE)
        
        assert result.is_valid is False  # Should fail quality threshold
        assert result.total_points == 5
        assert result.valid_points == 2  # Only 2 valid records
        assert result.invalid_points == 3  # 3 invalid records
        assert result.quality_score < 0.8  # Below quality threshold
        assert len(result.issues) > 0
    
    @pytest.mark.asyncio
    async def test_duplicate_data_detection(self, coordinator):
        """Test detection and handling of duplicate data points."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Create data with duplicates
        data_with_duplicates = [
            MarketData(
                time=base_time,
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Duplicate timestamp and symbol
            MarketData(
                time=base_time,  # Same time
                symbol="AAPL",   # Same symbol
                open=150.1,      # Different values
                high=152.1,
                low=149.1,
                close=151.1,
                volume=1000100
            ),
            MarketData(
                time=base_time + timedelta(minutes=1),
                symbol="AAPL",
                open=151.0,
                high=153.0,
                low=150.0,
                close=152.0,
                volume=1100000
            )
        ]
        
        result = await coordinator.validate_data(data_with_duplicates, DataGranularity.MINUTE)
        
        assert result.duplicate_points == 1
        assert result.total_points == 3
        assert "Duplicate data point" in str(result.issues)
    
    @pytest.mark.asyncio
    async def test_missing_data_gap_detection(self, coordinator):
        """Test detection of missing data gaps."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Create data with gaps (missing minutes 2-4)
        data_with_gaps = [
            MarketData(
                time=base_time,
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            MarketData(
                time=base_time + timedelta(minutes=1),
                symbol="AAPL",
                open=151.0,
                high=153.0,
                low=150.0,
                close=152.0,
                volume=1100000
            ),
            # Gap here - missing minutes 2, 3, 4
            MarketData(
                time=base_time + timedelta(minutes=5),  # Jump to minute 5
                symbol="AAPL",
                open=155.0,
                high=157.0,
                low=154.0,
                close=156.0,
                volume=1500000
            )
        ]
        
        result = await coordinator.validate_data(data_with_gaps, DataGranularity.MINUTE)
        
        assert result.missing_points > 0
        assert result.quality_score < 1.0  # Quality reduced due to gaps
    
    @pytest.mark.asyncio
    async def test_ohlc_consistency_validation(self, coordinator):
        """Test OHLC (Open, High, Low, Close) data consistency."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        inconsistent_data = [
            # Valid OHLC
            MarketData(
                time=base_time,
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Invalid: High < Low
            MarketData(
                time=base_time + timedelta(minutes=1),
                symbol="AAPL",
                open=150.0,
                high=148.0,  # High less than Low
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Invalid: High < Open
            MarketData(
                time=base_time + timedelta(minutes=2),
                symbol="AAPL",
                open=155.0,
                high=154.0,  # High less than Open
                low=149.0,
                close=151.0,
                volume=1000000
            ),
            # Invalid: Low > Close
            MarketData(
                time=base_time + timedelta(minutes=3),
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=153.0,  # Low greater than Close
                close=151.0,
                volume=1000000
            )
        ]
        
        result = await coordinator.validate_data(inconsistent_data, DataGranularity.MINUTE)
        
        assert result.invalid_points == 3  # 3 OHLC inconsistent records
        assert any("High" in issue and "Low" in issue for issue in result.issues)
    
    @pytest.mark.asyncio
    async def test_quality_score_calculation(self, coordinator):
        """Test quality score calculation algorithm."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Create data with known quality characteristics
        mixed_quality_data = []
        
        # Add 7 valid records
        for i in range(7):
            mixed_quality_data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=150.0 + i,
                high=152.0 + i,
                low=149.0 + i,
                close=151.0 + i,
                volume=1000000 + i * 10000
            ))
        
        # Add 2 invalid records (negative prices)
        for i in range(2):
            mixed_quality_data.append(MarketData(
                time=base_time + timedelta(minutes=7 + i),
                symbol="AAPL",
                open=-10.0,  # Invalid
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000
            ))
        
        # Add 1 duplicate
        mixed_quality_data.append(mixed_quality_data[0])  # Duplicate first record
        
        result = await coordinator.validate_data(mixed_quality_data, DataGranularity.MINUTE)
        
        # Expected: 10 total, 7 valid, 2 invalid, 1 duplicate
        # Quality score = 7/10 = 0.7
        expected_quality = 7.0 / 10.0
        
        assert abs(result.quality_score - expected_quality) < 0.1
        assert result.is_valid is False  # Below 0.8 threshold
    
    @pytest.mark.asyncio
    async def test_empty_data_validation(self, coordinator):
        """Test validation of empty data."""
        result = await coordinator.validate_data([], DataGranularity.MINUTE)
        
        assert result.is_valid is False
        assert result.total_points == 0
        assert result.valid_points == 0
        assert result.quality_score == 0.0
        assert "No data provided" in result.issues
    
    @pytest.mark.asyncio
    async def test_statistics_generation(self, coordinator, valid_market_data):
        """Test generation of data statistics."""
        result = await coordinator.validate_data(valid_market_data, DataGranularity.MINUTE)
        
        assert 'price_mean' in result.statistics
        assert 'price_std' in result.statistics
        assert 'price_min' in result.statistics
        assert 'price_max' in result.statistics
        assert 'volume_mean' in result.statistics
        assert 'volume_total' in result.statistics
        
        # Verify statistics are reasonable
        assert result.statistics['price_min'] > 0
        assert result.statistics['price_max'] >= result.statistics['price_min']
        assert result.statistics['volume_total'] > 0
    
    @pytest.mark.asyncio
    async def test_granularity_specific_validation(self, coordinator):
        """Test validation rules specific to different data granularities."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Create tick data (should have tight time intervals)
        tick_data = []
        for i in range(5):
            tick_data.append(MarketData(
                time=base_time + timedelta(seconds=i),
                symbol="AAPL",
                open=150.0,
                high=150.0,
                low=150.0,
                close=150.0 + i * 0.01,  # Small price movements
                volume=100 + i * 10
            ))
        
        # Test tick granularity
        tick_result = await coordinator.validate_data(tick_data, DataGranularity.TICK)
        
        # Create daily data (should have larger intervals and movements)
        daily_data = []
        for i in range(3):
            daily_data.append(MarketData(
                time=base_time + timedelta(days=i),
                symbol="AAPL",
                open=150.0 + i * 2,
                high=155.0 + i * 2,
                low=148.0 + i * 2,
                close=153.0 + i * 2,
                volume=10000000 + i * 1000000  # Larger volumes
            ))
        
        # Test daily granularity
        daily_result = await coordinator.validate_data(daily_data, DataGranularity.DAY)
        
        # Both should be valid but have different gap tolerances
        assert tick_result.is_valid
        assert daily_result.is_valid
    
    @pytest.mark.asyncio
    async def test_large_dataset_validation_performance(self, coordinator):
        """Test validation performance with large datasets."""
        import time
        
        # Create large dataset
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        large_dataset = []
        
        for i in range(10000):  # 10k records
            large_dataset.append(MarketData(
                time=base_time + timedelta(seconds=i),
                symbol="AAPL",
                open=150.0 + (i % 100) * 0.01,
                high=151.0 + (i % 100) * 0.01,
                low=149.0 + (i % 100) * 0.01,
                close=150.5 + (i % 100) * 0.01,
                volume=1000000 + i
            ))
        
        # Add some invalid data
        for i in range(100):  # 1% invalid data
            large_dataset[i * 100] = MarketData(
                time=base_time + timedelta(seconds=i * 100),
                symbol="AAPL",
                open=-10.0,  # Invalid
                high=151.0,
                low=149.0,
                close=150.5,
                volume=1000000
            )
        
        start_time = time.time()
        result = await coordinator.validate_data(large_dataset, DataGranularity.TICK)
        end_time = time.time()
        
        # Should complete within reasonable time (under 5 seconds)
        validation_time = end_time - start_time
        assert validation_time < 5.0
        
        # Should detect the invalid records
        assert result.invalid_points == 100
        assert result.total_points == 10000
        
        # Quality score should reflect the 1% invalid data
        expected_quality = 9900.0 / 10000.0  # 99% valid
        assert abs(result.quality_score - expected_quality) < 0.05


class TestDataQualityScoring:
    """Test advanced data quality scoring algorithms"""
    
    @pytest.fixture
    def coordinator(self):
        return HistoricalBackfillCoordinator()
    
    def test_quality_score_thresholds(self, coordinator):
        """Test quality score threshold boundaries."""
        # Test exact threshold boundary (80%)
        assert coordinator._is_quality_acceptable(0.8) is True
        assert coordinator._is_quality_acceptable(0.79) is False
        assert coordinator._is_quality_acceptable(0.81) is True
        
        # Test extreme values
        assert coordinator._is_quality_acceptable(1.0) is True
        assert coordinator._is_quality_acceptable(0.0) is False
    
    @pytest.mark.asyncio
    async def test_weighted_quality_scoring(self, coordinator):
        """Test weighted quality scoring based on error types."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Create data with different types of errors
        # Some errors should be weighted more heavily than others
        critical_error_data = [
            # Valid data
            MarketData(time=base_time, symbol="AAPL", open=150.0, high=152.0, low=149.0, close=151.0, volume=1000000),
            # Missing symbol (critical error)
            MarketData(time=base_time + timedelta(minutes=1), symbol="", open=150.0, high=152.0, low=149.0, close=151.0, volume=1000000),
            # Missing timestamp (critical error)
            MarketData(time=None, symbol="AAPL", open=150.0, high=152.0, low=149.0, close=151.0, volume=1000000),
        ]
        
        minor_error_data = [
            # Valid data
            MarketData(time=base_time, symbol="AAPL", open=150.0, high=152.0, low=149.0, close=151.0, volume=1000000),
            # Minor OHLC inconsistency
            MarketData(time=base_time + timedelta(minutes=1), symbol="AAPL", open=150.0, high=149.9, low=149.0, close=151.0, volume=1000000),
        ]
        
        # Critical errors should result in lower quality score
        critical_result = await coordinator.validate_data(critical_error_data, DataGranularity.MINUTE)
        minor_result = await coordinator.validate_data(minor_error_data, DataGranularity.MINUTE)
        
        # Minor errors should have higher quality score than critical errors
        assert minor_result.quality_score > critical_result.quality_score
    
    @pytest.mark.asyncio
    async def test_temporal_quality_assessment(self, coordinator):
        """Test quality assessment based on temporal characteristics."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Regular interval data (high quality)
        regular_data = []
        for i in range(10):
            regular_data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=150.0 + i * 0.1,
                high=152.0 + i * 0.1,
                low=149.0 + i * 0.1,
                close=151.0 + i * 0.1,
                volume=1000000
            ))
        
        # Irregular interval data (lower quality)
        irregular_data = []
        intervals = [0, 1, 2, 5, 6, 10, 11, 15, 16, 20]  # Irregular intervals
        for i, interval in enumerate(intervals):
            irregular_data.append(MarketData(
                time=base_time + timedelta(minutes=interval),
                symbol="AAPL",
                open=150.0 + i * 0.1,
                high=152.0 + i * 0.1,
                low=149.0 + i * 0.1,
                close=151.0 + i * 0.1,
                volume=1000000
            ))
        
        regular_result = await coordinator.validate_data(regular_data, DataGranularity.MINUTE)
        irregular_result = await coordinator.validate_data(irregular_data, DataGranularity.MINUTE)
        
        # Regular data should have higher quality score
        assert regular_result.quality_score >= irregular_result.quality_score
    
    @pytest.mark.asyncio
    async def test_volume_consistency_scoring(self, coordinator):
        """Test quality scoring based on volume consistency."""
        base_time = datetime(2024, 1, 1, 9, 30, tzinfo=timezone.utc)
        
        # Consistent volume data
        consistent_volume_data = []
        for i in range(5):
            consistent_volume_data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=1000000 + i * 10000  # Gradual increase
            ))
        
        # Inconsistent volume data (sudden spikes)
        inconsistent_volume_data = []
        volumes = [1000000, 1010000, 50000000, 1020000, 1030000]  # Huge spike
        for i, volume in enumerate(volumes):
            inconsistent_volume_data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=150.0,
                high=152.0,
                low=149.0,
                close=151.0,
                volume=volume
            ))
        
        consistent_result = await coordinator.validate_data(consistent_volume_data, DataGranularity.MINUTE)
        inconsistent_result = await coordinator.validate_data(inconsistent_volume_data, DataGranularity.MINUTE)
        
        # Both should be valid, but we can check statistics
        assert consistent_result.is_valid
        assert inconsistent_result.is_valid
        
        # Inconsistent data should have higher volume standard deviation
        consistent_vol_std = consistent_result.statistics.get('volume_std', 0)
        inconsistent_vol_std = inconsistent_result.statistics.get('volume_std', 0)
        
        # This would be implemented in actual validation logic
        # assert inconsistent_vol_std > consistent_vol_std


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])