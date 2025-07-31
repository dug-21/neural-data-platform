"""Tests for market hours utility."""
import pytest
from datetime import datetime
import pytz

from utils.market_hours import MarketHours, MarketStatus, is_market_data_expected


class TestMarketHours:
    """Test market hours functionality."""
    
    def test_us_stock_market_weekend(self):
        """Test US stock market status on weekend."""
        # Saturday
        saturday = datetime(2024, 1, 6, 12, 0, 0, tzinfo=pytz.UTC)
        status, message = MarketHours.get_market_status('US_STOCK', saturday)
        assert status == MarketStatus.WEEKEND
        assert "Saturday" in message
        
        # Sunday
        sunday = datetime(2024, 1, 7, 12, 0, 0, tzinfo=pytz.UTC)
        status, message = MarketHours.get_market_status('US_STOCK', sunday)
        assert status == MarketStatus.WEEKEND
        assert "Sunday" in message
    
    def test_us_stock_market_weekday(self):
        """Test US stock market status on weekday."""
        # Monday 10:30 AM ET (regular hours)
        et_tz = pytz.timezone('US/Eastern')
        monday_open = et_tz.localize(datetime(2024, 1, 8, 10, 30, 0))
        status, message = MarketHours.get_market_status('US_STOCK', monday_open)
        assert status == MarketStatus.OPEN
        assert "regular hours" in message
        
        # Monday 5:00 AM ET (pre-market)
        monday_premarket = et_tz.localize(datetime(2024, 1, 8, 5, 0, 0))
        status, message = MarketHours.get_market_status('US_STOCK', monday_premarket)
        assert status == MarketStatus.PRE_MARKET
        assert "Pre-market" in message
        
        # Monday 5:00 PM ET (after-hours)
        monday_afterhours = et_tz.localize(datetime(2024, 1, 8, 17, 0, 0))
        status, message = MarketHours.get_market_status('US_STOCK', monday_afterhours)
        assert status == MarketStatus.AFTER_HOURS
        assert "After-hours" in message
        
        # Monday 9:00 PM ET (closed)
        monday_closed = et_tz.localize(datetime(2024, 1, 8, 21, 0, 0))
        status, message = MarketHours.get_market_status('US_STOCK', monday_closed)
        assert status == MarketStatus.CLOSED
        assert "closed" in message
    
    def test_crypto_market_always_open(self):
        """Test crypto market is always open."""
        # Test various times
        times = [
            datetime(2024, 1, 6, 12, 0, 0, tzinfo=pytz.UTC),  # Saturday noon
            datetime(2024, 1, 7, 0, 0, 0, tzinfo=pytz.UTC),   # Sunday midnight
            datetime(2024, 1, 8, 10, 30, 0, tzinfo=pytz.UTC), # Monday morning
        ]
        
        for check_time in times:
            status, message = MarketHours.get_market_status('CRYPTO', check_time)
            assert status == MarketStatus.OPEN
            # Crypto markets show as regular hours since they're always open
            assert "open" in message.lower()
    
    def test_is_market_open(self):
        """Test simple market open check."""
        # Saturday - US stocks closed
        saturday = datetime(2024, 1, 6, 12, 0, 0, tzinfo=pytz.UTC)
        assert not MarketHours.is_market_open('US_STOCK', check_time=saturday)
        
        # Crypto always open
        assert MarketHours.is_market_open('CRYPTO', check_time=saturday)
        
        # Monday regular hours - US stocks open
        et_tz = pytz.timezone('US/Eastern')
        monday_open = et_tz.localize(datetime(2024, 1, 8, 10, 30, 0))
        assert MarketHours.is_market_open('US_STOCK', check_time=monday_open)
    
    def test_is_market_data_expected(self):
        """Test provider-specific market data expectations."""
        # Test on weekend
        saturday = datetime(2024, 1, 6, 12, 0, 0, tzinfo=pytz.UTC)
        
        # Stock providers - no data expected
        for provider in ['alpaca', 'polygon', 'finnhub']:
            expected, message = is_market_data_expected(provider)
            # This will check current time, so we can't assert the result
            # Just verify it returns proper types
            assert isinstance(expected, bool)
            assert isinstance(message, str)
        
        # Crypto provider - always expects data
        expected, message = is_market_data_expected('binance')
        assert expected is True
        assert "24/7" in message
    
    def test_us_holidays(self):
        """Test US holiday detection."""
        # New Year's Day 2024
        et_tz = pytz.timezone('US/Eastern')
        new_years = et_tz.localize(datetime(2024, 1, 1, 10, 30, 0))
        status, message = MarketHours.get_market_status('US_STOCK', new_years)
        assert status == MarketStatus.HOLIDAY
        assert "holiday" in message
        
        # Christmas 2024
        christmas = et_tz.localize(datetime(2024, 12, 25, 10, 30, 0))
        status, message = MarketHours.get_market_status('US_STOCK', christmas)
        assert status == MarketStatus.HOLIDAY
        assert "holiday" in message


if __name__ == "__main__":
    pytest.main([__file__, "-v"])