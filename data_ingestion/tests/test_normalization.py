"""Tests for data normalization utilities."""
import pytest
from datetime import datetime, timezone
from decimal import Decimal
import pandas as pd

from utils.normalization import DataNormalizer
from providers.base import MarketData, TickData, OrderBookData


class TestDataNormalizer:
    """Test data normalization functions."""
    
    def test_normalize_timestamp_string(self):
        """Test timestamp normalization from string."""
        # ISO format
        ts = DataNormalizer.normalize_timestamp("2024-07-03T14:30:00Z")
        assert ts.tzinfo == timezone.utc
        assert ts.hour == 14
        assert ts.minute == 30
        assert ts.second == 0
        assert ts.microsecond == 0
    
    def test_normalize_timestamp_daily(self):
        """Test timestamp normalization for daily data."""
        # Daily data should be normalized to midnight UTC
        ts = DataNormalizer.normalize_timestamp(
            "2024-07-03T14:30:00Z", 
            interval="1day"
        )
        assert ts.hour == 0
        assert ts.minute == 0
        assert ts.second == 0
        assert ts.microsecond == 0
    
    def test_normalize_timestamp_intraday(self):
        """Test timestamp normalization for intraday data."""
        # 1min data should remove seconds/microseconds
        ts = DataNormalizer.normalize_timestamp(
            "2024-07-03T14:30:45.123456Z",
            interval="1min"
        )
        assert ts.minute == 30
        assert ts.second == 0
        assert ts.microsecond == 0
    
    def test_normalize_price(self):
        """Test price normalization."""
        # Test various price formats
        assert DataNormalizer.normalize_price(123.456) == 123.46
        assert DataNormalizer.normalize_price("123.456") == 123.46
        assert DataNormalizer.normalize_price(Decimal("123.455")) == 123.46
        assert DataNormalizer.normalize_price(100) == 100.00
        assert DataNormalizer.normalize_price(0.001) == 0.00
    
    def test_normalize_volume(self):
        """Test volume normalization."""
        assert DataNormalizer.normalize_volume(1000) == 1000
        assert DataNormalizer.normalize_volume(1000.5) == 1000
        assert DataNormalizer.normalize_volume("1000") == 1000
        assert DataNormalizer.normalize_volume("1,000,000") == 1000000
    
    def test_normalize_symbol(self):
        """Test symbol normalization."""
        assert DataNormalizer.normalize_symbol("aapl") == "AAPL"
        assert DataNormalizer.normalize_symbol(" MSFT ") == "MSFT"
        assert DataNormalizer.normalize_symbol("btc-usd") == "BTC-USD"
        assert DataNormalizer.normalize_symbol("SPY.NYSE") == "SPY.NYSE"
        
        # Test invalid symbols
        with pytest.raises(ValueError):
            DataNormalizer.normalize_symbol("")
        
        with pytest.raises(ValueError):
            DataNormalizer.normalize_symbol("VERYLONGSYMBOL")
        
        with pytest.raises(ValueError):
            DataNormalizer.normalize_symbol("BAD@SYMBOL")
    
    def test_validate_ohlc_consistency(self):
        """Test OHLC consistency validation."""
        # Valid OHLC
        assert DataNormalizer.validate_ohlc_consistency(100, 110, 95, 105) == True
        
        # High < Low
        assert DataNormalizer.validate_ohlc_consistency(100, 95, 110, 105) == False
        
        # Open > High
        assert DataNormalizer.validate_ohlc_consistency(115, 110, 95, 105) == False
        
        # Close < Low
        assert DataNormalizer.validate_ohlc_consistency(100, 110, 95, 90) == False
        
        # Negative prices
        assert DataNormalizer.validate_ohlc_consistency(-100, 110, 95, 105) == False
    
    def test_normalize_provider_name(self):
        """Test provider name normalization."""
        assert DataNormalizer.normalize_provider_name("Yahoo Finance") == "yahoo_finance"
        assert DataNormalizer.normalize_provider_name(" ALPACA ") == "alpaca"
        assert DataNormalizer.normalize_provider_name("IEX Cloud") == "iex_cloud"


class TestMarketDataNormalization:
    """Test MarketData normalization via __post_init__."""
    
    def test_market_data_normalization(self):
        """Test full MarketData normalization."""
        data = MarketData(
            time="2024-07-03T14:30:45.123456Z",
            symbol="aapl",
            open=123.456,
            high=124.789,
            low=122.123,
            close=123.789,
            volume=1000000.5,
            provider="Yahoo Finance"
        )
        
        # Check normalization happened
        assert data.symbol == "AAPL"
        assert data.open == 123.46
        assert data.high == 124.79
        assert data.low == 122.12
        assert data.close == 123.79
        assert data.volume == 1000000
        assert data.provider == "yahoo_finance"
        assert data.time.second == 0
        assert data.time.microsecond == 0
    
    def test_market_data_invalid_ohlc(self):
        """Test MarketData with invalid OHLC."""
        with pytest.raises(ValueError, match="Invalid OHLC data"):
            MarketData(
                time=datetime.now(timezone.utc),
                symbol="AAPL",
                open=100,
                high=95,  # High < Low
                low=105,
                close=100,
                volume=1000,
                provider="test"
            )
    
    def test_tick_data_normalization(self):
        """Test TickData normalization."""
        data = TickData(
            time="2024-07-03T14:30:45.123456Z",
            symbol="msft",
            price=123.456,
            size=100.5,
            provider="Alpaca"
        )
        
        assert data.symbol == "MSFT"
        assert data.price == 123.46
        assert data.size == 100
        assert data.provider == "alpaca"
    
    def test_order_book_normalization(self):
        """Test OrderBookData normalization."""
        data = OrderBookData(
            time=datetime.now(),
            symbol="googl",
            bid_price=100.123,
            bid_size=1000,
            ask_price=100.456,
            ask_size=2000,
            mid_price=100.2895,
            spread=0.333,
            provider="IEX Cloud"
        )
        
        assert data.symbol == "GOOGL"
        assert data.bid_price == 100.12
        assert data.ask_price == 100.46
        assert data.mid_price == 100.29
        assert data.spread == 0.33
        assert data.provider == "iex_cloud"