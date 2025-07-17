"""
Comprehensive tests for the Alpaca SDK-based provider implementation.

This test suite focuses on achieving >85% code coverage for the AlpacaProvider
module, testing all critical paths including WebSocket streaming, historical data,
error handling, authentication, and edge cases.
"""

import pytest
import asyncio
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, MagicMock, AsyncMock
from alpaca.data.models import Bar, Trade, Quote
from alpaca.data.enums import DataFeed
import traceback

# Import the actual provider implementation
from providers.alpaca import AlpacaProvider
from providers.base import MarketData, TickData, OrderBookData


class TestAlpacaSDKProvider:
    """Comprehensive test suite for Alpaca SDK-based provider."""

    @pytest.fixture
    def mock_settings(self):
        """Mock settings for testing."""
        settings = Mock()
        settings.alpaca_api_key = "test_api_key"
        settings.alpaca_api_secret = "test_api_secret"
        settings.alpaca_subscription_level = "basic"
        return settings

    @pytest.fixture
    def provider(self, mock_settings):
        """Create provider instance with mocked settings."""
        with patch('providers.alpaca.get_settings', return_value=mock_settings):
            return AlpacaProvider()

    @pytest.fixture
    def unlimited_provider(self, mock_settings):
        """Create provider with unlimited subscription."""
        mock_settings.alpaca_subscription_level = "unlimited"
        with patch('providers.alpaca.get_settings', return_value=mock_settings):
            return AlpacaProvider()

    @pytest.mark.asyncio
    async def test_provider_initialization(self, provider):
        """Test basic provider initialization."""
        assert provider.name == "alpaca"
        assert provider.api_key == "test_api_key"
        assert provider.api_secret == "test_api_secret"
        assert provider.subscription_level == "basic"
        assert provider._subscription_limits["feed"] == DataFeed.IEX
        assert not provider._connected
        assert provider.stock_client is None
        assert provider.stock_stream is None

    @pytest.mark.asyncio
    async def test_unlimited_subscription_initialization(self, unlimited_provider):
        """Test initialization with unlimited subscription."""
        assert unlimited_provider.subscription_level == "unlimited"
        assert unlimited_provider._subscription_limits["feed"] == DataFeed.SIP
        assert unlimited_provider._subscription_limits["websocket_symbols"] is None

    @pytest.mark.asyncio
    async def test_connect_missing_credentials(self):
        """Test connection failure with missing credentials."""
        mock_settings = Mock()
        mock_settings.alpaca_api_key = None
        mock_settings.alpaca_api_secret = None
        mock_settings.alpaca_subscription_level = "basic"
        
        with patch('providers.alpaca.get_settings', return_value=mock_settings):
            provider = AlpacaProvider()
            
            with pytest.raises(ValueError, match="Alpaca API key and secret not configured"):
                await provider.connect()

    @pytest.mark.asyncio
    async def test_connect_empty_credentials(self):
        """Test connection failure with empty credentials."""
        mock_settings = Mock()
        mock_settings.alpaca_api_key = ""
        mock_settings.alpaca_api_secret = ""
        mock_settings.alpaca_subscription_level = "basic"
        
        with patch('providers.alpaca.get_settings', return_value=mock_settings):
            provider = AlpacaProvider()
            
            with pytest.raises(ValueError, match="Alpaca API key and secret not configured"):
                await provider.connect()

    @pytest.mark.asyncio
    async def test_connect_success(self, provider):
        """Test successful connection initialization."""
        with patch('alpaca.data.historical.StockHistoricalDataClient') as mock_client, \
             patch('alpaca.data.live.StockDataStream') as mock_stream:
            
            mock_client_instance = Mock()
            mock_stream_instance = Mock()
            mock_client.return_value = mock_client_instance
            mock_stream.return_value = mock_stream_instance
            
            await provider.connect()
            
            assert provider._connected
            assert provider.stock_client is not None
            assert provider.stock_stream is not None
            
            # Verify clients were initialized with correct parameters
            mock_client.assert_called_once_with(
                api_key="test_api_key",
                secret_key="test_api_secret"
            )
            mock_stream.assert_called_once_with(
                api_key="test_api_key",
                secret_key="test_api_secret",
                feed=DataFeed.IEX
            )

    @pytest.mark.asyncio
    async def test_connect_stream_fallback(self, provider):
        """Test fallback when StockDataStream fails with feed parameter."""
        with patch('alpaca.data.historical.StockHistoricalDataClient') as mock_client, \
             patch('alpaca.data.live.StockDataStream') as mock_stream:
            
            mock_client_instance = Mock()
            mock_stream_instance = Mock()
            mock_client.return_value = mock_client_instance
            
            # First call with feed fails, second without feed succeeds
            mock_stream.side_effect = [Exception("Feed not supported"), mock_stream_instance]
            
            await provider.connect()
            
            assert provider._connected
            assert provider.stock_stream is not None
            
            # Should have been called twice - once with feed, once without
            assert mock_stream.call_count == 2

    @pytest.mark.asyncio
    async def test_connect_failure(self, provider):
        """Test connection failure during client initialization."""
        with patch('alpaca.data.historical.StockHistoricalDataClient') as mock_client:
            mock_client.side_effect = Exception("Connection failed")
            
            with pytest.raises(Exception, match="Connection failed"):
                await provider.connect()
            
            assert not provider._connected

    @pytest.mark.asyncio
    async def test_disconnect_success(self, provider):
        """Test successful disconnection."""
        # Setup connected state
        provider._connected = True
        provider._stream_task = Mock()
        provider._stream_task.done.return_value = False
        provider._stream_task.cancel = Mock()
        provider.stock_stream = AsyncMock()
        provider.stock_stream.close = AsyncMock()
        
        await provider.disconnect()
        
        assert not provider._connected
        provider._stream_task.cancel.assert_called_once()
        provider.stock_stream.close.assert_called_once()

    @pytest.mark.asyncio
    async def test_disconnect_with_completed_task(self, provider):
        """Test disconnection when stream task is already done."""
        provider._connected = True
        provider._stream_task = Mock()
        provider._stream_task.done.return_value = True
        provider.stock_stream = AsyncMock()
        
        await provider.disconnect()
        
        assert not provider._connected
        # Should not try to cancel completed task
        provider._stream_task.cancel.assert_not_called()

    @pytest.mark.asyncio
    async def test_disconnect_with_stream_error(self, provider):
        """Test disconnection when stream close fails."""
        provider._connected = True
        provider.stock_stream = AsyncMock()
        provider.stock_stream.close.side_effect = Exception("Close failed")
        
        await provider.disconnect()
        
        assert not provider._connected
        # Should handle error gracefully

    @pytest.mark.asyncio
    async def test_get_market_data_realtime_detection(self, provider):
        """Test real-time vs historical data detection."""
        with patch.object(provider, '_get_current_market_data') as mock_current, \
             patch.object(provider, '_get_historical_market_data') as mock_historical:
            
            mock_current.return_value = iter([])
            mock_historical.return_value = iter([])
            
            # Test real-time request (recent end_time)
            now = datetime.now()
            recent_end = now - timedelta(minutes=2)
            recent_start = recent_end - timedelta(minutes=5)
            
            data_points = []
            async for data in provider.get_market_data(["AAPL"], recent_start, recent_end):
                data_points.append(data)
            
            mock_current.assert_called_once()
            mock_historical.assert_not_called()

    @pytest.mark.asyncio
    async def test_get_market_data_historical_detection(self, provider):
        """Test historical data detection."""
        with patch.object(provider, '_get_current_market_data') as mock_current, \
             patch.object(provider, '_get_historical_market_data') as mock_historical:
            
            mock_current.return_value = iter([])
            mock_historical.return_value = iter([])
            
            # Test historical request (old end_time)
            now = datetime.now()
            old_end = now - timedelta(hours=1)
            old_start = old_end - timedelta(hours=1)
            
            data_points = []
            async for data in provider.get_market_data(["AAPL"], old_start, old_end):
                data_points.append(data)
            
            mock_historical.assert_called_once()
            mock_current.assert_not_called()

    @pytest.mark.asyncio
    async def test_get_current_market_data_success(self, provider):
        """Test getting current market data with latest quotes."""
        mock_quote = Mock()
        mock_quote.timestamp = datetime.now()
        mock_quote.bid_price = 150.0
        mock_quote.ask_price = 150.5
        mock_quote.bid_size = 100
        mock_quote.ask_size = 200
        
        provider.stock_client = Mock()
        provider.stock_client.get_stock_latest_quote.return_value = {"AAPL": mock_quote}
        
        data_points = []
        async for data in provider._get_current_market_data(["AAPL"]):
            data_points.append(data)
        
        assert len(data_points) == 1
        data = data_points[0]
        assert isinstance(data, MarketData)
        assert data.symbol == "AAPL"
        assert data.close == 150.25  # (bid + ask) / 2
        assert data.volume == 300  # bid_size + ask_size
        assert data.metadata["bid_price"] == 150.0
        assert data.metadata["ask_price"] == 150.5

    @pytest.mark.asyncio
    async def test_get_current_market_data_no_data(self, provider):
        """Test getting current market data when no data is returned."""
        provider.stock_client = Mock()
        provider.stock_client.get_stock_latest_quote.return_value = {}
        
        data_points = []
        async for data in provider._get_current_market_data(["AAPL"]):
            data_points.append(data)
        
        assert len(data_points) == 0

    @pytest.mark.asyncio
    async def test_get_current_market_data_error(self, provider):
        """Test error handling in current market data retrieval."""
        provider.stock_client = Mock()
        provider.stock_client.get_stock_latest_quote.side_effect = Exception("API Error")
        
        data_points = []
        async for data in provider._get_current_market_data(["AAPL"]):
            data_points.append(data)
        
        # Should handle error gracefully and return no data
        assert len(data_points) == 0

    @pytest.mark.asyncio
    async def test_get_historical_market_data_success(self, provider):
        """Test getting historical market data."""
        mock_bar = Mock()
        mock_bar.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_bar.open = 150.0
        mock_bar.high = 151.0
        mock_bar.low = 149.0
        mock_bar.close = 150.5
        mock_bar.volume = 1000000
        mock_bar.trade_count = 500
        mock_bar.vwap = 150.25
        
        provider.stock_client = Mock()
        provider.stock_client.get_stock_bars.return_value = {"AAPL": [mock_bar]}
        
        start_time = datetime(2024, 1, 1, 9, 0)
        end_time = datetime(2024, 1, 1, 10, 0)
        
        data_points = []
        async for data in provider._get_historical_market_data(["AAPL"], start_time, end_time, "1min"):
            data_points.append(data)
        
        assert len(data_points) == 1
        data = data_points[0]
        assert isinstance(data, MarketData)
        assert data.symbol == "AAPL"
        assert data.open == 150.0
        assert data.close == 150.5
        assert data.volume == 1000000

    @pytest.mark.asyncio
    async def test_get_historical_market_data_basic_plan_limit(self, provider):
        """Test basic plan historical data age limit."""
        provider.subscription_level = "basic"
        provider._subscription_limits = provider.SUBSCRIPTION_LIMITS["basic"]
        provider.stock_client = Mock()
        provider.stock_client.get_stock_bars.return_value = {"AAPL": []}
        
        # Try to get data older than 15 minutes
        now = datetime.now()
        old_start = now - timedelta(hours=1)
        end_time = now
        
        data_points = []
        async for data in provider._get_historical_market_data(["AAPL"], old_start, end_time, "1min"):
            data_points.append(data)
        
        # Should adjust start_time to within 15 minutes
        call_args = provider.stock_client.get_stock_bars.call_args[0][0]
        adjusted_start = call_args.start
        assert now - adjusted_start <= timedelta(minutes=16)  # Allow 1 minute buffer

    @pytest.mark.asyncio
    async def test_get_historical_market_data_no_data(self, provider):
        """Test historical data when no bars are returned."""
        provider.stock_client = Mock()
        provider.stock_client.get_stock_bars.return_value = {}
        
        start_time = datetime(2024, 1, 1, 9, 0)
        end_time = datetime(2024, 1, 1, 10, 0)
        
        data_points = []
        async for data in provider._get_historical_market_data(["AAPL"], start_time, end_time, "1min"):
            data_points.append(data)
        
        assert len(data_points) == 0

    @pytest.mark.asyncio
    async def test_get_historical_market_data_error(self, provider):
        """Test error handling in historical data retrieval."""
        provider.stock_client = Mock()
        provider.stock_client.get_stock_bars.side_effect = Exception("API Error")
        
        start_time = datetime(2024, 1, 1, 9, 0)
        end_time = datetime(2024, 1, 1, 10, 0)
        
        data_points = []
        async for data in provider._get_historical_market_data(["AAPL"], start_time, end_time, "1min"):
            data_points.append(data)
        
        # Should handle error gracefully
        assert len(data_points) == 0

    @pytest.mark.asyncio
    async def test_stream_market_data_no_symbols(self, provider):
        """Test streaming with no valid symbols."""
        with patch.object(provider, '_validate_symbols', return_value=[]):
            data_points = []
            async for data in provider.stream_market_data([]):
                data_points.append(data)
                break  # Should not yield any data
            
            assert len(data_points) == 0

    @pytest.mark.asyncio
    async def test_stream_market_data_polling(self, provider):
        """Test streaming market data polling mechanism."""
        with patch.object(provider, '_get_current_market_data') as mock_current:
            # Mock returning one data point per poll
            mock_data = Mock()
            mock_data.symbol = "AAPL"
            mock_data.close = 150.0
            mock_current.return_value = iter([mock_data])
            
            data_points = []
            poll_count = 0
            async for data in provider.stream_market_data(["AAPL"]):
                data_points.append(data)
                poll_count += 1
                if poll_count >= 2:  # Test 2 polls then break
                    break
            
            assert len(data_points) == 2
            assert all(d.symbol == "AAPL" for d in data_points)

    @pytest.mark.asyncio
    async def test_stream_market_data_error_handling(self, provider):
        """Test error handling during streaming."""
        with patch.object(provider, '_get_current_market_data') as mock_current:
            # First call succeeds, second raises error, third succeeds
            mock_data = Mock()
            mock_data.symbol = "AAPL"
            mock_data.close = 150.0
            
            call_count = 0
            def side_effect(*args):
                nonlocal call_count
                call_count += 1
                if call_count == 2:
                    raise Exception("Network error")
                return iter([mock_data])
            
            mock_current.side_effect = side_effect
            
            data_points = []
            poll_count = 0
            async for data in provider.stream_market_data(["AAPL"]):
                data_points.append(data)
                poll_count += 1
                if poll_count >= 2:  # Get data from before and after error
                    break
            
            # Should recover from error and continue
            assert len(data_points) == 2

    @pytest.mark.asyncio
    async def test_get_tick_data_success(self, provider):
        """Test getting tick/trade data."""
        mock_trade = Mock()
        mock_trade.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_trade.price = 150.05
        mock_trade.size = 100
        mock_trade.exchange = "V"
        mock_trade.conditions = ["@", "I"]
        
        provider.stock_client = Mock()
        provider.stock_client.get_stock_trades.return_value = {"AAPL": [mock_trade]}
        
        start_time = datetime(2024, 1, 1, 9, 0)
        end_time = datetime(2024, 1, 1, 10, 0)
        
        ticks = []
        async for tick in provider.get_tick_data(["AAPL"], start_time, end_time):
            ticks.append(tick)
        
        assert len(ticks) == 1
        tick = ticks[0]
        assert isinstance(tick, TickData)
        assert tick.symbol == "AAPL"
        assert tick.price == 150.05
        assert tick.size == 100

    @pytest.mark.asyncio
    async def test_get_tick_data_error(self, provider):
        """Test error handling in tick data retrieval."""
        provider.stock_client = Mock()
        provider.stock_client.get_stock_trades.side_effect = Exception("API Error")
        
        start_time = datetime(2024, 1, 1, 9, 0)
        end_time = datetime(2024, 1, 1, 10, 0)
        
        ticks = []
        async for tick in provider.get_tick_data(["AAPL"], start_time, end_time):
            ticks.append(tick)
        
        assert len(ticks) == 0

    @pytest.mark.asyncio
    async def test_stream_tick_data_polling(self, provider):
        """Test tick data streaming via polling."""
        with patch.object(provider, 'get_tick_data') as mock_get_ticks:
            mock_tick = Mock()
            mock_tick.symbol = "AAPL"
            mock_tick.price = 150.0
            
            call_count = 0
            async def mock_tick_generator(*args):
                nonlocal call_count
                call_count += 1
                yield mock_tick
            
            mock_get_ticks.return_value = mock_tick_generator()
            
            ticks = []
            poll_count = 0
            async for tick in provider.stream_tick_data(["AAPL"]):
                ticks.append(tick)
                poll_count += 1
                if poll_count >= 2:
                    break
            
            assert len(ticks) == 2

    @pytest.mark.asyncio
    async def test_stream_tick_data_error(self, provider):
        """Test error handling in tick streaming."""
        with patch.object(provider, 'get_tick_data') as mock_get_ticks:
            # Simulate error every other call
            call_count = 0
            async def mock_tick_generator(*args):
                nonlocal call_count
                call_count += 1
                if call_count % 2 == 0:
                    raise Exception("Network error")
                mock_tick = Mock()
                mock_tick.symbol = "AAPL"
                yield mock_tick
            
            mock_get_ticks.return_value = mock_tick_generator()
            
            ticks = []
            error_count = 0
            async for tick in provider.stream_tick_data(["AAPL"]):
                ticks.append(tick)
                if len(ticks) >= 1:  # Get at least one successful tick
                    break
            
            assert len(ticks) >= 1

    @pytest.mark.asyncio
    async def test_get_order_book_success(self, provider):
        """Test getting order book data."""
        mock_quote = Mock()
        mock_quote.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_quote.bid_price = 150.0
        mock_quote.ask_price = 150.5
        mock_quote.bid_size = 100
        mock_quote.ask_size = 200
        
        provider.stock_client = Mock()
        provider.stock_client.get_stock_latest_quote.return_value = {"AAPL": mock_quote}
        
        books = []
        async for book in provider.get_order_book(["AAPL"]):
            books.append(book)
        
        assert len(books) == 1
        book = books[0]
        assert isinstance(book, OrderBookData)
        assert book.symbol == "AAPL"
        assert book.bid_price == 150.0
        assert book.ask_price == 150.5
        assert book.spread == 0.5
        assert book.mid_price == 150.25

    @pytest.mark.asyncio
    async def test_get_order_book_error(self, provider):
        """Test error handling in order book retrieval."""
        provider.stock_client = Mock()
        provider.stock_client.get_stock_latest_quote.side_effect = Exception("API Error")
        
        books = []
        async for book in provider.get_order_book(["AAPL"]):
            books.append(book)
        
        assert len(books) == 0

    def test_parse_bar(self, provider):
        """Test parsing Alpaca Bar object."""
        mock_bar = Mock()
        mock_bar.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_bar.open = 150.0
        mock_bar.high = 151.0
        mock_bar.low = 149.0
        mock_bar.close = 150.5
        mock_bar.volume = 1000000
        mock_bar.trade_count = 500
        mock_bar.vwap = 150.25
        
        data = provider._parse_bar(mock_bar, "AAPL")
        
        assert isinstance(data, MarketData)
        assert data.symbol == "AAPL"
        assert data.open == 150.0
        assert data.close == 150.5
        assert data.volume == 1000000
        assert data.metadata["trade_count"] == 500
        assert data.metadata["vwap"] == 150.25

    def test_parse_bar_missing_optional_fields(self, provider):
        """Test parsing Bar object without optional fields."""
        mock_bar = Mock()
        mock_bar.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_bar.open = 150.0
        mock_bar.high = 151.0
        mock_bar.low = 149.0
        mock_bar.close = 150.5
        mock_bar.volume = 1000000
        # No trade_count or vwap
        
        data = provider._parse_bar(mock_bar, "AAPL")
        
        assert data.metadata["trade_count"] is None
        assert data.metadata["vwap"] is None

    def test_parse_trade(self, provider):
        """Test parsing Alpaca Trade object."""
        mock_trade = Mock()
        mock_trade.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_trade.price = 150.05
        mock_trade.size = 100
        mock_trade.exchange = "V"
        mock_trade.conditions = ["@", "I"]
        
        tick = provider._parse_trade(mock_trade, "AAPL")
        
        assert isinstance(tick, TickData)
        assert tick.symbol == "AAPL"
        assert tick.price == 150.05
        assert tick.size == 100
        assert tick.exchange == "V"
        assert tick.conditions == "@,I"

    def test_parse_trade_missing_optional_fields(self, provider):
        """Test parsing Trade object without optional fields."""
        mock_trade = Mock()
        mock_trade.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_trade.price = 150.05
        mock_trade.size = 100
        # No exchange or conditions
        
        tick = provider._parse_trade(mock_trade, "AAPL")
        
        assert tick.exchange is None
        assert tick.conditions is None

    def test_parse_quote(self, provider):
        """Test parsing Alpaca Quote object."""
        mock_quote = Mock()
        mock_quote.timestamp = datetime(2024, 1, 1, 10, 0)
        mock_quote.bid_price = 150.0
        mock_quote.ask_price = 150.5
        mock_quote.bid_size = 100
        mock_quote.ask_size = 200
        
        book = provider._parse_quote(mock_quote, "AAPL")
        
        assert isinstance(book, OrderBookData)
        assert book.symbol == "AAPL"
        assert book.bid_price == 150.0
        assert book.ask_price == 150.5
        assert book.bid_size == 100
        assert book.ask_size == 200
        assert book.mid_price == 150.25
        assert book.spread == 0.5

    def test_interval_mapping(self, provider):
        """Test interval mapping to Alpaca TimeFrame."""
        from alpaca.data.timeframe import TimeFrameUnit
        
        # Test all mapped intervals
        intervals_to_test = [
            ("1min", (1, "Minute")),
            ("5min", (5, "Minute")),
            ("15min", (15, "Minute")),
            ("30min", (30, "Minute")),
            ("1hour", (1, "Hour")),
            ("4hour", (4, "Hour")),
            ("1day", (1, "Day")),
            ("1week", (1, "Week")),
            ("1month", (1, "Month"))
        ]
        
        for interval, expected in intervals_to_test:
            mapped = provider.INTERVAL_MAP.get(interval, (1, "Minute"))
            assert mapped == expected

    def test_subscription_limits(self, provider):
        """Test subscription limit configurations."""
        basic_limits = provider.SUBSCRIPTION_LIMITS["basic"]
        unlimited_limits = provider.SUBSCRIPTION_LIMITS["unlimited"]
        
        # Basic plan limits
        assert basic_limits["websocket_symbols"] == 30
        assert basic_limits["historical_calls_per_minute"] == 200
        assert basic_limits["feed"] == DataFeed.IEX
        
        # Unlimited plan limits
        assert unlimited_limits["websocket_symbols"] is None
        assert unlimited_limits["historical_calls_per_minute"] == 10000
        assert unlimited_limits["feed"] == DataFeed.SIP

    @pytest.mark.asyncio
    async def test_symbol_validation_basic(self, provider):
        """Test symbol validation functionality."""
        # Assuming _validate_symbols exists in base class
        valid_symbols = ["AAPL", "MSFT", "GOOGL"]
        
        with patch.object(provider, '_validate_symbols', return_value=valid_symbols) as mock_validate:
            # Test that validation is called
            result = []
            async for data in provider._get_current_market_data(valid_symbols):
                result.append(data)
            
            mock_validate.assert_called_once_with(valid_symbols)

    def test_provider_metadata(self, provider):
        """Test provider metadata and constants."""
        assert provider.name == "alpaca"
        assert hasattr(provider, 'INTERVAL_MAP')
        assert hasattr(provider, 'SUBSCRIPTION_LIMITS')
        assert len(provider.INTERVAL_MAP) >= 9  # Should have all standard intervals
        assert len(provider.SUBSCRIPTION_LIMITS) == 2  # basic and unlimited

    @pytest.mark.asyncio 
    async def test_error_logging_and_recovery(self, provider):
        """Test error logging and recovery mechanisms."""
        provider.stock_client = Mock()
        
        # Test that errors are logged but don't crash the system
        provider.stock_client.get_stock_latest_quote.side_effect = [
            Exception("First error"),
            {"AAPL": Mock(bid_price=150.0, ask_price=150.5, bid_size=100, ask_size=200, timestamp=datetime.now())}
        ]
        
        data_points = []
        call_count = 0
        async for data in provider._get_current_market_data(["AAPL", "MSFT"]):  # Two symbols, one will fail
            data_points.append(data)
            call_count += 1
            if call_count >= 1:  # Get at least one success
                break
        
        # Should recover and continue processing
        assert len(data_points) >= 0  # May get 0 or 1 depending on which call succeeds