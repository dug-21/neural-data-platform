"""
Unit tests for data validation functionality
"""
import pytest
from datetime import datetime, timedelta
from typing import List
import numpy as np

from data_ingestion.providers.historical_backfill import (
    HistoricalBackfillCoordinator, DataValidationResult, DataGranularity
)
from data_ingestion.providers.base import MarketData


class TestDataValidation:
    """Test suite for data validation logic"""
    
    @pytest.fixture
    def coordinator(self):
        """Create coordinator instance for testing"""
        return HistoricalBackfillCoordinator()
    
    @pytest.fixture
    def sample_valid_data(self) -> List[MarketData]:
        """Generate valid OHLCV data"""
        data = []
        base_time = datetime(2023, 1, 2, 9, 30)  # Market open
        
        for i in range(100):
            # Generate realistic OHLCV data
            open_price = 100 + np.random.normal(0, 2)
            close_price = open_price + np.random.normal(0, 1)
            high_price = max(open_price, close_price) + abs(np.random.normal(0, 0.5))
            low_price = min(open_price, close_price) - abs(np.random.normal(0, 0.5))
            volume = int(1000000 + np.random.normal(0, 100000))
            
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=round(open_price, 2),
                high=round(high_price, 2),
                low=round(low_price, 2),
                close=round(close_price, 2),
                volume=max(0, volume)
            ))
        
        return data
    
    @pytest.mark.asyncio
    async def test_ohlc_consistency_validation(self, coordinator, sample_valid_data):
        """Test OHLC data consistency checks"""
        # Test valid data
        result = await coordinator.validate_data(sample_valid_data, DataGranularity.MINUTE)
        assert result.is_valid
        assert result.invalid_points == 0
        
        # Introduce OHLC inconsistencies
        invalid_data = sample_valid_data.copy()
        
        # High < Low
        invalid_data[10].high = 95
        invalid_data[10].low = 105
        
        # High < Open
        invalid_data[20].high = 98
        invalid_data[20].open = 102
        invalid_data[20].close = 101
        
        # Low > Close
        invalid_data[30].low = 105
        invalid_data[30].close = 100
        
        result = await coordinator.validate_data(invalid_data, DataGranularity.MINUTE)
        assert not result.is_valid
        assert result.invalid_points >= 3
        assert any("High" in issue and "Low" in issue for issue in result.issues)
        assert any("highest price" in issue for issue in result.issues)
        assert any("lowest price" in issue for issue in result.issues)
    
    @pytest.mark.asyncio
    async def test_duplicate_detection(self, coordinator):
        """Test duplicate data point detection"""
        # Create data with duplicates
        base_time = datetime(2023, 1, 2, 9, 30)
        data = []
        
        for i in range(10):
            point = MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=100,
                high=101,
                low=99,
                close=100.5,
                volume=1000000
            )
            data.append(point)
            
            # Add duplicate at indices 3 and 7
            if i in [3, 7]:
                data.append(point)  # Exact duplicate
        
        result = await coordinator.validate_data(data, DataGranularity.MINUTE)
        
        assert result.total_points == 12
        assert result.duplicate_points == 2
        assert result.valid_points == 10
        assert len([issue for issue in result.issues if "Duplicate" in issue]) == 2
    
    @pytest.mark.asyncio
    async def test_gap_detection(self, coordinator):
        """Test missing data gap identification"""
        # Create data with gaps
        data = []
        base_time = datetime(2023, 1, 2, 9, 30)
        
        # Add data for minutes 0-10
        for i in range(11):
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=100,
                high=101,
                low=99,
                close=100.5,
                volume=1000000
            ))
        
        # Skip minutes 11-20 (gap of 10 minutes)
        
        # Add data for minutes 21-30
        for i in range(21, 31):
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=100,
                high=101,
                low=99,
                close=100.5,
                volume=1000000
            ))
        
        result = await coordinator.validate_data(data, DataGranularity.MINUTE)
        
        assert result.missing_points > 0
        assert any("Gap" in issue for issue in result.issues)
        # Quality score should be reduced due to missing data
        assert result.quality_score < 1.0
    
    @pytest.mark.asyncio
    async def test_quality_score_calculation(self, coordinator):
        """Test data quality scoring algorithm"""
        # Perfect data
        perfect_data = []
        base_time = datetime(2023, 1, 2, 9, 30)
        
        for i in range(100):
            perfect_data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=100,
                high=102,
                low=98,
                close=101,
                volume=1000000
            ))
        
        result = await coordinator.validate_data(perfect_data, DataGranularity.MINUTE)
        assert result.quality_score > 0.95  # Should be very high
        
        # Data with issues
        poor_data = perfect_data.copy()
        
        # Add duplicates (10%)
        for i in range(0, 100, 10):
            poor_data.insert(i, poor_data[i])
        
        # Add invalid points (5%)
        for i in range(0, len(poor_data), 20):
            poor_data[i].high = 90  # Below low
            poor_data[i].low = 110   # Above high
        
        result = await coordinator.validate_data(poor_data, DataGranularity.MINUTE)
        assert result.quality_score < 0.8  # Should fail quality threshold
        assert not result.is_valid
    
    @pytest.mark.asyncio
    async def test_timestamp_validation(self, coordinator):
        """Test timestamp format and sequence validation"""
        # Test out-of-order timestamps
        data = []
        base_time = datetime(2023, 1, 2, 9, 30)
        
        # Add data in wrong order
        for i in [0, 2, 1, 4, 3, 6, 5]:  # Out of sequence
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=100,
                high=101,
                low=99,
                close=100.5,
                volume=1000000
            ))
        
        result = await coordinator.validate_data(data, DataGranularity.MINUTE)
        
        # Validator should sort data internally
        assert result.is_valid  # Should still be valid after sorting
        assert result.valid_points == 7
        
        # Test missing timestamps
        data_missing_time = data.copy()
        data_missing_time[3].time = None
        
        result = await coordinator.validate_data(data_missing_time, DataGranularity.MINUTE)
        assert result.invalid_points >= 1
        assert any("Missing timestamp" in issue for issue in result.issues)
    
    @pytest.mark.asyncio
    async def test_price_data_validation(self, coordinator):
        """Test price data sanity checks"""
        data = []
        base_time = datetime(2023, 1, 2, 9, 30)
        
        # Add data with invalid prices
        invalid_scenarios = [
            {"open": -10, "high": 101, "low": 99, "close": 100},  # Negative price
            {"open": 0, "high": 101, "low": 99, "close": 100},    # Zero price
            {"open": 100, "high": 101, "low": 99, "close": -5},   # Negative close
            {"open": 100, "high": 0, "low": 99, "close": 100},    # Zero high
        ]
        
        for i, scenario in enumerate(invalid_scenarios):
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                **scenario,
                volume=1000000
            ))
        
        result = await coordinator.validate_data(data, DataGranularity.MINUTE)
        
        assert not result.is_valid
        assert result.invalid_points == len(invalid_scenarios)
        assert any("Invalid price data" in issue for issue in result.issues)
    
    @pytest.mark.asyncio
    async def test_volume_validation(self, coordinator):
        """Test volume data consistency"""
        data = []
        base_time = datetime(2023, 1, 2, 9, 30)
        
        # Mix of valid and invalid volumes
        volumes = [1000000, 500000, -100, 2000000, 0, -50000, 1500000]
        
        for i, volume in enumerate(volumes):
            data.append(MarketData(
                time=base_time + timedelta(minutes=i),
                symbol="AAPL",
                open=100,
                high=101,
                low=99,
                close=100.5,
                volume=volume
            ))
        
        result = await coordinator.validate_data(data, DataGranularity.MINUTE)
        
        # Negative volumes should be flagged
        negative_volume_count = len([v for v in volumes if v < 0])
        assert result.invalid_points >= negative_volume_count
        assert any("Negative volume" in issue for issue in result.issues)
        
        # Zero volume is valid (no trading)
        assert result.valid_points >= len([v for v in volumes if v >= 0])
    
    @pytest.mark.asyncio
    async def test_statistics_calculation(self, coordinator, sample_valid_data):
        """Test statistical metrics calculation"""
        result = await coordinator.validate_data(sample_valid_data, DataGranularity.MINUTE)
        
        assert result.is_valid
        assert 'price_mean' in result.statistics
        assert 'price_std' in result.statistics
        assert 'price_min' in result.statistics
        assert 'price_max' in result.statistics
        assert 'volume_mean' in result.statistics
        assert 'volume_total' in result.statistics
        
        # Verify statistics are reasonable
        assert 90 < result.statistics['price_mean'] < 110  # Around 100
        assert result.statistics['price_std'] > 0  # Some variation
        assert result.statistics['price_min'] < result.statistics['price_max']
        assert result.statistics['volume_total'] > 0
    
    @pytest.mark.asyncio
    async def test_empty_data_validation(self, coordinator):
        """Test validation of empty data set"""
        result = await coordinator.validate_data([], DataGranularity.MINUTE)
        
        assert not result.is_valid
        assert result.total_points == 0
        assert result.quality_score == 0.0
        assert "No data provided" in result.issues
    
    @pytest.mark.asyncio
    async def test_granularity_specific_validation(self, coordinator):
        """Test validation rules specific to data granularity"""
        # Test tick data - should allow sub-second intervals
        tick_data = []
        base_time = datetime(2023, 1, 2, 9, 30, 0)
        
        for i in range(10):
            tick_data.append(MarketData(
                time=base_time + timedelta(milliseconds=i * 100),  # 100ms intervals
                symbol="AAPL",
                price=100.05 + (i * 0.01),
                size=100
            ))
        
        result = await coordinator.validate_data(tick_data, DataGranularity.TICK)
        assert result.is_valid
        
        # Test daily data - larger gaps acceptable
        daily_data = []
        base_date = datetime(2023, 1, 2)
        
        # Skip weekends
        for i in range(10):
            date = base_date + timedelta(days=i)
            if date.weekday() < 5:  # Monday = 0, Friday = 4
                daily_data.append(MarketData(
                    time=date,
                    symbol="AAPL",
                    open=100,
                    high=102,
                    low=98,
                    close=101,
                    volume=10000000
                ))
        
        result = await coordinator.validate_data(daily_data, DataGranularity.DAY)
        assert result.is_valid
        # Should not flag weekend gaps as issues


if __name__ == "__main__":
    pytest.main([__file__, "-v"])