"""Binance cryptocurrency data provider implementation."""
import asyncio
import json
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime, timedelta
import pandas as pd
from decimal import Decimal

from .base import BaseProvider, MarketData, TickData, OrderBookData


class BinanceProvider(BaseProvider):
    """Binance cryptocurrency data provider for historical and real-time data."""
    
    # Binance API endpoints
    BASE_URL = "https://api.binance.com"
    BASE_URL_TESTNET = "https://testnet.binance.vision"
    WS_URL = "wss://stream.binance.com:9443/ws"
    WS_URL_TESTNET = "wss://testnet.binance.vision/ws"
    
    # Map intervals to Binance kline intervals
    INTERVAL_MAP = {
        "1min": "1m",
        "3min": "3m",
        "5min": "5m",
        "15min": "15m",
        "30min": "30m",
        "1hour": "1h",
        "2hour": "2h",
        "4hour": "4h",
        "6hour": "6h",
        "8hour": "8h",
        "12hour": "12h",
        "1day": "1d",
        "3day": "3d",
        "1week": "1w",
        "1month": "1M"
    }
    
    # Binance rate limits (weight per minute)
    RATE_LIMITS = {
        "request_weight": 1200,  # Total weight per minute
        "order_weight": 50,      # Orders per 10 seconds
        "order_daily": 160000    # Orders per day
    }
    
    # Kline/candle data weights
    KLINE_WEIGHTS = {
        100: 1,    # <= 100 candles
        500: 2,    # <= 500 candles
        1000: 5,   # <= 1000 candles
        5000: 10   # > 1000 candles
    }
    
    def __init__(self, api_key: Optional[str] = None, api_secret: Optional[str] = None,
                 testnet: bool = False, **kwargs):
        """Initialize Binance provider.
        
        Args:
            api_key: Binance API key (optional for public data)
            api_secret: Binance API secret (optional for public data)
            testnet: Use testnet endpoints
            **kwargs: Additional provider configuration
        """
        super().__init__(**kwargs)
        self.api_key = api_key
        self.api_secret = api_secret
        self.testnet = testnet
        self.base_url = self.BASE_URL_TESTNET if testnet else self.BASE_URL
        self.ws_url = self.WS_URL_TESTNET if testnet else self.WS_URL
        self._ws_connection = None
        self._ws_subscriptions = set()
        self._request_weight_used = 0
        self._weight_reset_time = datetime.now()
    
    def _check_rate_limit(self, weight: int = 1) -> None:
        """Check and update rate limit tracking."""
        now = datetime.now()
        # Reset counter every minute
        if (now - self._weight_reset_time).total_seconds() >= 60:
            self._request_weight_used = 0
            self._weight_reset_time = now
        
        if self._request_weight_used + weight > self.RATE_LIMITS["request_weight"]:
            sleep_time = 60 - (now - self._weight_reset_time).total_seconds()
            if sleep_time > 0:
                self.logger.warning(f"Rate limit approaching, sleeping for {sleep_time:.1f}s")
                asyncio.create_task(asyncio.sleep(sleep_time))
                self._request_weight_used = 0
                self._weight_reset_time = datetime.now()
        
        self._request_weight_used += weight
    
    def _get_kline_weight(self, limit: int) -> int:
        """Calculate request weight for kline data."""
        for threshold, weight in self.KLINE_WEIGHTS.items():
            if limit <= threshold:
                return weight
        return self.KLINE_WEIGHTS[5000]
    
    async def _make_request(self, endpoint: str, params: Optional[Dict] = None) -> Dict[str, Any]:
        """Make HTTP request to Binance API."""
        url = f"{self.base_url}{endpoint}"
        
        headers = {}
        if self.api_key:
            headers["X-MBX-APIKEY"] = self.api_key
        
        async with aiohttp.ClientSession() as session:
            try:
                async with session.get(url, params=params, headers=headers) as response:
                    if response.status == 200:
                        return await response.json()
                    else:
                        error_data = await response.text()
                        raise Exception(f"Binance API error {response.status}: {error_data}")
            except Exception as e:
                self.logger.error(f"Request failed: {str(e)}")
                raise
    
    async def connect(self) -> None:
        """Connect to Binance (test connectivity)."""
        try:
            # Test connectivity
            await self._make_request("/api/v3/ping")
            
            # Get server time to check time sync
            server_time_resp = await self._make_request("/api/v3/time")
            server_time = datetime.fromtimestamp(server_time_resp["serverTime"] / 1000)
            local_time = datetime.now()
            time_diff = abs((server_time - local_time).total_seconds())
            
            if time_diff > 5:
                self.logger.warning(f"Time sync issue: {time_diff:.1f}s difference with server")
            
            # Get exchange info for symbol validation
            exchange_info = await self._make_request("/api/v3/exchangeInfo")
            self._symbols = {s["symbol"]: s for s in exchange_info["symbols"]}
            
            self.connected = True
            self.logger.info("Connected to Binance successfully")
            
        except Exception as e:
            self.logger.error(f"Failed to connect to Binance: {str(e)}")
            raise
    
    async def disconnect(self) -> None:
        """Disconnect from Binance."""
        if self._ws_connection:
            await self._ws_connection.close()
            self._ws_connection = None
        self._ws_subscriptions.clear()
        self.connected = False
        self.logger.info("Disconnected from Binance")
    
    def _validate_symbol(self, symbol: str) -> str:
        """Validate and format symbol for Binance."""
        # Binance uses uppercase symbols without separators
        # e.g., "BTC/USDT" -> "BTCUSDT"
        formatted = symbol.upper().replace("/", "").replace("-", "")
        
        if hasattr(self, "_symbols") and formatted not in self._symbols:
            # Try common variations
            variations = [
                formatted,
                formatted + "USDT",
                formatted + "BUSD",
                formatted + "BTC",
                formatted + "ETH"
            ]
            for var in variations:
                if var in self._symbols:
                    return var
            raise ValueError(f"Invalid symbol: {symbol}")
        
        return formatted
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_date: datetime,
        end_date: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[MarketData]:
        """Get historical market data from Binance.
        
        Args:
            symbols: List of symbols to fetch
            start_date: Start date for historical data
            end_date: End date for historical data
            interval: Time interval for candles
            
        Yields:
            MarketData objects
        """
        if interval not in self.INTERVAL_MAP:
            raise ValueError(f"Invalid interval: {interval}")
        
        binance_interval = self.INTERVAL_MAP[interval]
        
        for symbol in symbols:
            try:
                formatted_symbol = self._validate_symbol(symbol)
                
                # Binance limits to 1000 candles per request
                # Calculate time chunks based on interval
                interval_ms = self._get_interval_ms(interval)
                chunk_size = 1000 * interval_ms
                
                current_start = int(start_date.timestamp() * 1000)
                end_ts = int(end_date.timestamp() * 1000)
                
                while current_start < end_ts:
                    current_end = min(current_start + chunk_size, end_ts)
                    
                    params = {
                        "symbol": formatted_symbol,
                        "interval": binance_interval,
                        "startTime": current_start,
                        "endTime": current_end,
                        "limit": 1000
                    }
                    
                    # Check rate limit
                    weight = self._get_kline_weight(1000)
                    self._check_rate_limit(weight)
                    
                    try:
                        klines = await self._make_request("/api/v3/klines", params)
                        
                        for kline in klines:
                            # Kline format: [open_time, open, high, low, close, volume, 
                            #                close_time, quote_volume, trades, taker_buy_base,
                            #                taker_buy_quote, ignore]
                            yield MarketData(
                                symbol=symbol,
                                timestamp=datetime.fromtimestamp(kline[0] / 1000),
                                open=float(kline[1]),
                                high=float(kline[2]),
                                low=float(kline[3]),
                                close=float(kline[4]),
                                volume=float(kline[5]),
                                provider="binance",
                                metadata={
                                    "quote_volume": float(kline[7]),
                                    "trades": int(kline[8]),
                                    "taker_buy_base_volume": float(kline[9]),
                                    "taker_buy_quote_volume": float(kline[10])
                                }
                            )
                        
                        # Update metrics
                        self.metrics["total_records"] += len(klines)
                        self.metrics["last_update"] = datetime.now()
                        
                    except Exception as e:
                        self.logger.error(f"Failed to fetch data for {symbol}: {str(e)}")
                        self.metrics["errors"] += 1
                    
                    # Move to next chunk
                    current_start = current_end + 1
                    
                    # Small delay to be nice to the API
                    await asyncio.sleep(0.1)
                    
            except Exception as e:
                self.logger.error(f"Error processing symbol {symbol}: {str(e)}")
                self.metrics["errors"] += 1
    
    async def stream_market_data(
        self,
        symbols: List[str],
        interval: str = "1min",
        data_types: Optional[List[str]] = None
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data from Binance WebSocket.
        
        Args:
            symbols: List of symbols to stream
            interval: Time interval for kline streams
            data_types: Types of data to stream (kline, trade, depth)
            
        Yields:
            MarketData objects
        """
        if not data_types:
            data_types = ["kline"]
        
        # Format symbols and create stream names
        streams = []
        for symbol in symbols:
            formatted_symbol = self._validate_symbol(symbol).lower()
            
            if "kline" in data_types:
                binance_interval = self.INTERVAL_MAP.get(interval, "1m")
                streams.append(f"{formatted_symbol}@kline_{binance_interval}")
            
            if "trade" in data_types:
                streams.append(f"{formatted_symbol}@trade")
            
            if "depth" in data_types:
                streams.append(f"{formatted_symbol}@depth20@100ms")
        
        # Connect to WebSocket
        stream_url = f"{self.ws_url}/stream?streams={'/'.join(streams)}"
        
        try:
            async with aiohttp.ClientSession() as session:
                async with session.ws_connect(stream_url) as ws:
                    self._ws_connection = ws
                    self.logger.info(f"Connected to Binance WebSocket with {len(streams)} streams")
                    
                    async for msg in ws:
                        if msg.type == aiohttp.WSMsgType.TEXT:
                            data = json.loads(msg.data)
                            stream_name = data.get("stream", "")
                            stream_data = data.get("data", {})
                            
                            if "@kline" in stream_name:
                                kline = stream_data["k"]
                                if kline["x"]:  # Only emit closed candles
                                    yield MarketData(
                                        symbol=self._format_symbol_output(kline["s"]),
                                        timestamp=datetime.fromtimestamp(kline["t"] / 1000),
                                        open=float(kline["o"]),
                                        high=float(kline["h"]),
                                        low=float(kline["l"]),
                                        close=float(kline["c"]),
                                        volume=float(kline["v"]),
                                        provider="binance",
                                        metadata={
                                            "quote_volume": float(kline["q"]),
                                            "trades": int(kline["n"])
                                        }
                                    )
                            
                            elif "@trade" in stream_name:
                                # Emit as tick data
                                yield TickData(
                                    symbol=self._format_symbol_output(stream_data["s"]),
                                    timestamp=datetime.fromtimestamp(stream_data["T"] / 1000),
                                    price=float(stream_data["p"]),
                                    size=float(stream_data["q"]),
                                    side="buy" if stream_data["m"] else "sell",
                                    provider="binance",
                                    metadata={
                                        "trade_id": stream_data["t"],
                                        "buyer_maker": stream_data["m"]
                                    }
                                )
                            
                        elif msg.type == aiohttp.WSMsgType.ERROR:
                            self.logger.error(f"WebSocket error: {ws.exception()}")
                            break
                            
        except Exception as e:
            self.logger.error(f"WebSocket connection failed: {str(e)}")
            raise
        finally:
            self._ws_connection = None
    
    async def get_tick_data(
        self,
        symbol: str,
        start_date: datetime,
        end_date: datetime
    ) -> AsyncIterator[TickData]:
        """Get historical tick data (trades) from Binance.
        
        Args:
            symbol: Symbol to fetch
            start_date: Start date
            end_date: End date
            
        Yields:
            TickData objects
        """
        formatted_symbol = self._validate_symbol(symbol)
        
        # Binance provides aggregated trades (similar to tick data)
        # Limited to 1000 trades per request
        current_start = int(start_date.timestamp() * 1000)
        end_ts = int(end_date.timestamp() * 1000)
        from_id = None
        
        while current_start < end_ts:
            params = {
                "symbol": formatted_symbol,
                "limit": 1000
            }
            
            if from_id:
                params["fromId"] = from_id
            else:
                params["startTime"] = current_start
                params["endTime"] = end_ts
            
            # Aggregated trades have weight of 1
            self._check_rate_limit(1)
            
            try:
                trades = await self._make_request("/api/v3/aggTrades", params)
                
                if not trades:
                    break
                
                for trade in trades:
                    trade_time = datetime.fromtimestamp(trade["T"] / 1000)
                    
                    if trade_time > end_date:
                        return
                    
                    yield TickData(
                        symbol=symbol,
                        timestamp=trade_time,
                        price=float(trade["p"]),
                        size=float(trade["q"]),
                        side="buy" if not trade["m"] else "sell",
                        provider="binance",
                        metadata={
                            "agg_trade_id": trade["a"],
                            "first_trade_id": trade["f"],
                            "last_trade_id": trade["l"],
                            "buyer_maker": trade["m"]
                        }
                    )
                
                # Get the last trade ID for pagination
                if trades:
                    from_id = trades[-1]["a"] + 1
                    current_start = trades[-1]["T"]
                else:
                    break
                
                self.metrics["total_ticks"] += len(trades)
                
                # Rate limit courtesy delay
                await asyncio.sleep(0.1)
                
            except Exception as e:
                self.logger.error(f"Failed to fetch tick data: {str(e)}")
                self.metrics["errors"] += 1
                break
    
    async def get_order_book(
        self,
        symbol: str,
        depth: int = 20
    ) -> OrderBookData:
        """Get current order book snapshot from Binance.
        
        Args:
            symbol: Symbol to fetch
            depth: Order book depth (5, 10, 20, 50, 100, 500, 1000, 5000)
            
        Returns:
            OrderBookData object
        """
        formatted_symbol = self._validate_symbol(symbol)
        
        # Validate depth
        valid_depths = [5, 10, 20, 50, 100, 500, 1000, 5000]
        if depth not in valid_depths:
            depth = min(valid_depths, key=lambda x: abs(x - depth))
        
        # Weight depends on depth
        weight = 1 if depth <= 100 else (5 if depth <= 500 else (10 if depth <= 1000 else 50))
        self._check_rate_limit(weight)
        
        params = {
            "symbol": formatted_symbol,
            "limit": depth
        }
        
        try:
            order_book = await self._make_request("/api/v3/depth", params)
            
            return OrderBookData(
                symbol=symbol,
                timestamp=datetime.now(),  # Binance doesn't provide timestamp in depth
                bids=[(float(price), float(qty)) for price, qty in order_book["bids"]],
                asks=[(float(price), float(qty)) for price, qty in order_book["asks"]],
                provider="binance",
                metadata={
                    "last_update_id": order_book["lastUpdateId"]
                }
            )
            
        except Exception as e:
            self.logger.error(f"Failed to fetch order book: {str(e)}")
            raise
    
    def _get_interval_ms(self, interval: str) -> int:
        """Convert interval string to milliseconds."""
        interval_map = {
            "1min": 60 * 1000,
            "3min": 3 * 60 * 1000,
            "5min": 5 * 60 * 1000,
            "15min": 15 * 60 * 1000,
            "30min": 30 * 60 * 1000,
            "1hour": 60 * 60 * 1000,
            "2hour": 2 * 60 * 60 * 1000,
            "4hour": 4 * 60 * 60 * 1000,
            "6hour": 6 * 60 * 60 * 1000,
            "8hour": 8 * 60 * 60 * 1000,
            "12hour": 12 * 60 * 60 * 1000,
            "1day": 24 * 60 * 60 * 1000,
            "3day": 3 * 24 * 60 * 60 * 1000,
            "1week": 7 * 24 * 60 * 60 * 1000,
            "1month": 30 * 24 * 60 * 60 * 1000
        }
        return interval_map.get(interval, 24 * 60 * 60 * 1000)
    
    def _format_symbol_output(self, binance_symbol: str) -> str:
        """Format Binance symbol back to standard format."""
        # This is a simplified version - in production you'd want to
        # properly parse based on the exchange info
        symbol = binance_symbol.upper()
        
        # Common patterns
        for quote in ["USDT", "BUSD", "BTC", "ETH", "BNB", "USDC", "TUSD", "PAX", "USDP"]:
            if symbol.endswith(quote):
                base = symbol[:-len(quote)]
                return f"{base}/{quote}"
        
        return symbol