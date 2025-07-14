"""Alpaca Markets data provider implementation using official SDK (stocks only)."""
import asyncio
from typing import List, AsyncIterator, Optional
from datetime import datetime, timedelta

from alpaca.data.historical import StockHistoricalDataClient
from alpaca.data.live import StockDataStream
from alpaca.data.requests import (
    StockBarsRequest, 
    StockTradesRequest,
    StockLatestQuoteRequest
)
from alpaca.data.timeframe import TimeFrame, TimeFrameUnit
from alpaca.data.models import Bar, Trade, Quote
from alpaca.data.enums import DataFeed

from .base import BaseProvider, MarketData, TickData, OrderBookData


class AlpacaProvider(BaseProvider):
    """Alpaca Markets data provider using official SDK for stocks."""
    
    # Map intervals to Alpaca TimeFrame parameters
    INTERVAL_MAP = {
        "1min": (1, "Minute"),
        "5min": (5, "Minute"),
        "15min": (15, "Minute"),
        "30min": (30, "Minute"),
        "1hour": (1, "Hour"),
        "4hour": (4, "Hour"),
        "1day": (1, "Day"),
        "1week": (1, "Week"),
        "1month": (1, "Month")
    }
    
    # Alpaca subscription levels
    SUBSCRIPTION_LIMITS = {
        "basic": {
            "websocket_symbols": 30,
            "historical_calls_per_minute": 200,
            "historical_data_age_limit": timedelta(minutes=15),
            "feed": DataFeed.IEX  # Free accounts only have IEX access
        },
        "unlimited": {
            "websocket_symbols": None,  # Unlimited
            "historical_calls_per_minute": 10000,
            "historical_data_age_limit": None,  # No limit
            "feed": DataFeed.SIP  # Paid accounts get SIP access
        }
    }
    
    def __init__(self):
        super().__init__("alpaca")
        self.api_key = self.settings.alpaca_api_key
        self.api_secret = self.settings.alpaca_api_secret
        self.subscription_level = self.settings.alpaca_subscription_level or "basic"
        
        # Initialize clients
        self.stock_client: Optional[StockHistoricalDataClient] = None
        self.stock_stream: Optional[StockDataStream] = None
        
        self._subscription_limits = self.SUBSCRIPTION_LIMITS[self.subscription_level]
        self._stream_task = None
        self._data_queue = None
    
    async def connect(self):
        """Initialize Alpaca SDK clients."""
        if not self.api_key or not self.api_secret:
            raise ValueError("Alpaca API key and secret not configured")
        
        try:
            # Initialize historical data client with both key and secret
            self.stock_client = StockHistoricalDataClient(
                api_key=self.api_key,
                secret_key=self.api_secret
            )
            
            # Initialize streaming client with both key and secret
            feed_enum = self._subscription_limits["feed"]
            
            try:
                self.stock_stream = StockDataStream(
                    api_key=self.api_key,
                    secret_key=self.api_secret,
                    feed=feed_enum
                )
                self.logger.info(f"StockDataStream initialized with feed={feed_enum}")
            except Exception as e:
                self.logger.error(f"Failed to create StockDataStream: {e}")
                # Try without feed parameter as fallback
                self.stock_stream = StockDataStream(
                    api_key=self.api_key,
                    secret_key=self.api_secret
                )
                self.logger.info("StockDataStream initialized with default feed")
            
            self._connected = True
            self.logger.info(f"Connected to Alpaca Markets SDK ({self.subscription_level} plan, feed={feed_enum})")
        except Exception as e:
            self.logger.error(f"Failed to initialize Alpaca SDK: {str(e)}")
            self.logger.error(f"Error type: {type(e).__name__}")
            import traceback
            self.logger.error(f"Traceback: {traceback.format_exc()}")
            raise
    
    async def disconnect(self):
        """Close all connections."""
        try:
            # Cancel streaming task first
            if self._stream_task and not self._stream_task.done():
                self._stream_task.cancel()
                try:
                    await self._stream_task
                except asyncio.CancelledError:
                    pass
            
            # Then close the stream
            if self.stock_stream:
                try:
                    await self.stock_stream.close()
                except Exception as e:
                    self.logger.error(f"Error closing stream: {e}")
            
            self._connected = False
            self.logger.info("Disconnected from Alpaca Markets")
        except Exception as e:
            self.logger.error(f"Error during disconnect: {e}")
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Fetch market data from Alpaca - uses latest quotes for real-time, historical bars for batched requests."""
        symbols = self._validate_symbols(symbols)
        
        # Determine if this is a real-time request or historical batch request
        now = datetime.now()
        time_diff = (now - end_time).total_seconds()
        
        # If end_time is very recent (within 5 minutes), treat as real-time request
        is_realtime_request = time_diff < 300  # 5 minutes
        
        if is_realtime_request:
            # Use latest quotes for current/real-time pricing
            self.logger.info("Using real-time quotes for current pricing")
            async for data in self._get_current_market_data(symbols):
                yield data
        else:
            # Use historical bars for batch/historical requests
            self.logger.info(f"Using historical bars for batch request ({start_time} to {end_time})")
            async for data in self._get_historical_market_data(symbols, start_time, end_time, interval):
                yield data
    
    async def _get_current_market_data(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """Get current market data using latest quotes."""
        for symbol in symbols:
            try:
                # Get latest quote for current pricing
                feed_enum = self._subscription_limits["feed"]
                request = StockLatestQuoteRequest(
                    symbol_or_symbols=symbol,
                    feed=feed_enum
                )
                
                self.logger.debug(f"Fetching latest quote for {symbol} with feed={feed_enum}")
                quotes = self.stock_client.get_stock_latest_quote(request)
                
                if symbol in quotes:
                    quote = quotes[symbol]
                    # Convert quote to MarketData format using bid/ask as OHLC
                    current_price = (quote.bid_price + quote.ask_price) / 2 if quote.bid_price and quote.ask_price else quote.bid_price or quote.ask_price
                    
                    market_data = MarketData(
                        time=quote.timestamp,  # Use actual quote timestamp
                        symbol=symbol,
                        open=current_price,
                        high=current_price,
                        low=current_price,
                        close=current_price,
                        volume=quote.bid_size + quote.ask_size,  # Combined bid/ask volume
                        provider=self.name,
                        metadata={
                            "bid_price": float(quote.bid_price) if quote.bid_price else None,
                            "ask_price": float(quote.ask_price) if quote.ask_price else None,
                            "bid_size": int(quote.bid_size) if quote.bid_size else None,
                            "ask_size": int(quote.ask_size) if quote.ask_size else None,
                            "spread": float(quote.ask_price - quote.bid_price) if quote.bid_price and quote.ask_price else None,
                            "data_type": "latest_quote"
                        }
                    )
                    
                    yield market_data
                    self.logger.info(f"Retrieved current quote for {symbol}: ${current_price:.2f} (bid: ${quote.bid_price:.2f}, ask: ${quote.ask_price:.2f}, timestamp: {quote.timestamp})")
                else:
                    self.logger.warning(f"No quote data returned for {symbol}")
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch current data for {symbol}", error=str(e))
                continue
    
    async def _get_historical_market_data(self, symbols: List[str], start_time: datetime, end_time: datetime, interval: str) -> AsyncIterator[MarketData]:
        """Get historical market data using bars."""
        # Get timeframe parameters and create TimeFrame object
        tf_params = self.INTERVAL_MAP.get(interval, (1, "Minute"))
        timeframe = TimeFrame(tf_params[0], TimeFrameUnit[tf_params[1]])
        
        # Check subscription limits for historical data
        if self.subscription_level == "basic":
            age_limit = self._subscription_limits["historical_data_age_limit"]
            if age_limit and datetime.now() - start_time > age_limit:
                self.logger.warning(f"Basic plan only allows data from last {age_limit}. Adjusting start_time.")
                start_time = datetime.now() - age_limit
        
        for symbol in symbols:
            try:
                # Fetch stock bars with feed specification
                feed_enum = self._subscription_limits["feed"]
                request = StockBarsRequest(
                    symbol_or_symbols=symbol,
                    timeframe=timeframe,
                    start=start_time,
                    end=end_time,
                    feed=feed_enum
                )
                
                self.logger.debug(f"Fetching historical bars for {symbol} from {start_time} to {end_time} with feed={feed_enum}")
                bars = self.stock_client.get_stock_bars(request)
                
                if symbol in bars and len(bars[symbol]) > 0:
                    bar_count = 0
                    for bar in bars[symbol]:
                        yield self._parse_bar(bar, symbol)
                        bar_count += 1
                    self.logger.info(f"Retrieved {bar_count} historical bars for {symbol}")
                else:
                    self.logger.warning(f"No historical data returned for {symbol}")
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch historical data for {symbol}", error=str(e))
                continue
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data using latest quotes polling."""
        symbols = self._validate_symbols(symbols)
        
        if not symbols:
            self.logger.warning("No valid symbols to stream")
            return
        
        self.logger.info(f"Starting real-time quote streaming for symbols: {symbols}")
        
        # Poll every 5 seconds for current quotes
        poll_count = 0
        while True:
            try:
                poll_count += 1
                self.logger.info(f"Starting poll #{poll_count} for current quotes")
                
                # Get current quotes directly (not historical data)
                data_count = 0
                async for data in self._get_current_market_data(symbols):
                    data_count += 1
                    self.logger.info(f"Poll #{poll_count}: Got data for {data.symbol} = ${data.close:.2f}")
                    yield data
                
                if data_count == 0:
                    self.logger.warning(f"Poll #{poll_count}: No data received")
                
                # Wait before next poll
                self.logger.info(f"Poll #{poll_count} complete, waiting 5 seconds...")
                await asyncio.sleep(5)
                
            except Exception as e:
                self.logger.error(f"Real-time polling error: {str(e)}")
                import traceback
                self.logger.error(f"Traceback: {traceback.format_exc()}")
                await asyncio.sleep(10)  # Wait longer on error
    
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Fetch historical tick/trade data."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                # Fetch stock trades with feed specification
                feed_enum = self._subscription_limits["feed"]
                request = StockTradesRequest(
                    symbol_or_symbols=symbol,
                    start=start_time,
                    end=end_time,
                    feed=feed_enum
                )
                trades = self.stock_client.get_stock_trades(request)
                
                if symbol in trades:
                    for trade in trades[symbol]:
                        yield self._parse_trade(trade, symbol)
                        
            except Exception as e:
                self.logger.error(f"Failed to fetch tick data for {symbol}", error=str(e))
                continue
    
    async def stream_tick_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[TickData]:
        """Stream real-time tick/trade data via WebSocket."""
        symbols = self._validate_symbols(symbols)
        
        if not symbols:
            self.logger.warning("No valid symbols to stream")
            return
        
        # For now, we'll use polling with get_tick_data as a workaround
        self.logger.warning("Alpaca WebSocket tick streaming is currently disabled due to SDK limitations.")
        self.logger.info(f"Using polling mode for tick data: {symbols}")
        
        # Poll every 2 seconds for recent trades
        while True:
            try:
                end_time = datetime.now()
                start_time = end_time - timedelta(seconds=10)
                
                # Get recent trades
                async for tick in self.get_tick_data(symbols, start_time, end_time):
                    yield tick
                
                # Wait before next poll
                await asyncio.sleep(2)
                
            except Exception as e:
                self.logger.error(f"Tick polling error: {str(e)}")
                await asyncio.sleep(5)  # Wait longer on error
    
    async def get_order_book(
        self,
        symbols: List[str]
    ) -> AsyncIterator[OrderBookData]:
        """Get order book snapshots (latest quotes)."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                # Get latest stock quote with feed specification
                feed_enum = self._subscription_limits["feed"]
                request = StockLatestQuoteRequest(
                    symbol_or_symbols=symbol,
                    feed=feed_enum
                )
                quotes = self.stock_client.get_stock_latest_quote(request)
                
                if symbol in quotes:
                    yield self._parse_quote(quotes[symbol], symbol)
                        
            except Exception as e:
                self.logger.error(f"Failed to fetch order book for {symbol}", error=str(e))
                continue
    
    def _parse_bar(self, bar: Bar, symbol: str) -> MarketData:
        """Parse Alpaca SDK Bar object to MarketData."""
        return MarketData(
            time=bar.timestamp,
            symbol=symbol,
            open=float(bar.open),
            high=float(bar.high),
            low=float(bar.low),
            close=float(bar.close),
            volume=int(bar.volume),
            provider=self.name,
            metadata={
                "trade_count": bar.trade_count if hasattr(bar, 'trade_count') else None,
                "vwap": float(bar.vwap) if hasattr(bar, 'vwap') and bar.vwap else None
            }
        )
    
    def _parse_trade(self, trade: Trade, symbol: str) -> TickData:
        """Parse Alpaca SDK Trade object to TickData."""
        return TickData(
            time=trade.timestamp,
            symbol=symbol,
            price=float(trade.price),
            size=int(trade.size),
            exchange=trade.exchange if hasattr(trade, 'exchange') else None,
            conditions=",".join(trade.conditions) if hasattr(trade, 'conditions') and trade.conditions else None,
            provider=self.name
        )
    
    def _parse_quote(self, quote: Quote, symbol: str) -> OrderBookData:
        """Parse Alpaca SDK Quote object to OrderBookData."""
        bid_price = float(quote.bid_price)
        ask_price = float(quote.ask_price)
        
        return OrderBookData(
            time=quote.timestamp,
            symbol=symbol,
            bid_price=bid_price,
            bid_size=int(quote.bid_size),
            ask_price=ask_price,
            ask_size=int(quote.ask_size),
            mid_price=(bid_price + ask_price) / 2 if bid_price and ask_price else 0,
            spread=ask_price - bid_price if bid_price and ask_price else 0,
            provider=self.name
        )