"""Enhanced Polygon.io WebSocket implementation with full streaming support."""
import asyncio
import aiohttp
import json
from typing import Dict, List, Any, Optional, Set, Callable, AsyncIterator
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from enum import Enum, auto
from collections import deque
import time
from contextlib import asynccontextmanager

from .base import BaseProvider, MarketData, TickData, OrderBookData
from utils.logging import get_logger
from utils.metrics import metrics


class ConnectionState(Enum):
    """WebSocket connection states."""
    DISCONNECTED = auto()
    CONNECTING = auto()
    AUTHENTICATING = auto()
    CONNECTED = auto()
    RECONNECTING = auto()
    FAILED = auto()


class MessageType(Enum):
    """Polygon WebSocket message types."""
    STATUS = "status"
    TRADE = "T"
    QUOTE = "Q"
    AGGREGATE_SECOND = "A"
    AGGREGATE_MINUTE = "AM"
    

@dataclass
class WebSocketConfig:
    """Configuration for WebSocket connection."""
    url: str = "wss://socket.polygon.io"
    max_reconnect_attempts: int = 10
    initial_reconnect_delay: float = 1.0
    max_reconnect_delay: float = 60.0
    heartbeat_interval: float = 30.0
    message_buffer_size: int = 10000
    subscription_batch_size: int = 100
    connection_timeout: float = 10.0
    

@dataclass
class Subscription:
    """Represents a subscription to market data."""
    symbols: Set[str] = field(default_factory=set)
    channels: Set[str] = field(default_factory=set)
    callbacks: List[Callable] = field(default_factory=list)
    
    def to_polygon_format(self) -> List[str]:
        """Convert to Polygon subscription format."""
        subscriptions = []
        for symbol in self.symbols:
            for channel in self.channels:
                subscriptions.append(f"{channel}.{symbol}")
        return subscriptions


class StreamBuffer:
    """Thread-safe circular buffer for message queuing."""
    
    def __init__(self, max_size: int = 10000):
        self.buffer = deque(maxlen=max_size)
        self.overflow_count = 0
        self.total_messages = 0
        self._lock = asyncio.Lock()
    
    async def push(self, message: Dict[str, Any]) -> bool:
        """Add message to buffer, returns False if buffer full."""
        async with self._lock:
            self.total_messages += 1
            if len(self.buffer) >= self.buffer.maxlen:
                self.overflow_count += 1
                return False
            self.buffer.append(message)
            return True
    
    async def pop(self) -> Optional[Dict[str, Any]]:
        """Remove and return oldest message."""
        async with self._lock:
            return self.buffer.popleft() if self.buffer else None
    
    async def pop_batch(self, max_items: int = 100) -> List[Dict[str, Any]]:
        """Remove and return multiple messages."""
        async with self._lock:
            batch = []
            for _ in range(min(max_items, len(self.buffer))):
                if self.buffer:
                    batch.append(self.buffer.popleft())
            return batch
    
    @property
    def size(self) -> int:
        """Current buffer size."""
        return len(self.buffer)
    
    def get_stats(self) -> Dict[str, int]:
        """Get buffer statistics."""
        return {
            "current_size": self.size,
            "max_size": self.buffer.maxlen,
            "total_messages": self.total_messages,
            "overflow_count": self.overflow_count
        }


class WebSocketManager:
    """Manages WebSocket connection lifecycle."""
    
    def __init__(self, config: WebSocketConfig, api_key: str):
        self.config = config
        self.api_key = api_key
        self.logger = get_logger(self.__class__.__name__)
        
        self._session: Optional[aiohttp.ClientSession] = None
        self._connection: Optional[aiohttp.ClientWebSocketResponse] = None
        self._state = ConnectionState.DISCONNECTED
        self._reconnect_attempts = 0
        self._last_heartbeat = time.time()
        self._connection_id: Optional[str] = None
        
        # Tasks
        self._heartbeat_task: Optional[asyncio.Task] = None
        self._monitor_task: Optional[asyncio.Task] = None
    
    @property
    def state(self) -> ConnectionState:
        """Current connection state."""
        return self._state
    
    @property
    def is_connected(self) -> bool:
        """Check if fully connected and authenticated."""
        return self._state == ConnectionState.CONNECTED
    
    async def connect(self) -> bool:
        """Establish WebSocket connection."""
        if self._state in (ConnectionState.CONNECTING, ConnectionState.CONNECTED):
            return self.is_connected
        
        self._state = ConnectionState.CONNECTING
        
        try:
            # Create session if needed
            if not self._session:
                self._session = aiohttp.ClientSession()
            
            # Connect with timeout
            self.logger.info(f"Connecting to {self.config.url}/stocks")
            self._connection = await asyncio.wait_for(
                self._session.ws_connect(f"{self.config.url}/stocks"),
                timeout=self.config.connection_timeout
            )
            
            # Authenticate
            self._state = ConnectionState.AUTHENTICATING
            await self._authenticate()
            
            # Start background tasks
            self._start_background_tasks()
            
            self._state = ConnectionState.CONNECTED
            self._reconnect_attempts = 0
            self.logger.info("WebSocket connected and authenticated")
            
            # Update metrics
            metrics.websocket_connections.labels(provider="polygon", status="connected").inc()
            
            return True
            
        except asyncio.TimeoutError:
            self.logger.error("Connection timeout")
            self._state = ConnectionState.FAILED
            return False
        except Exception as e:
            self.logger.error(f"Connection failed: {e}")
            self._state = ConnectionState.FAILED
            return False
    
    async def disconnect(self):
        """Close WebSocket connection."""
        self.logger.info("Disconnecting WebSocket")
        
        # Cancel background tasks
        if self._heartbeat_task:
            self._heartbeat_task.cancel()
        if self._monitor_task:
            self._monitor_task.cancel()
        
        # Close connection
        if self._connection:
            await self._connection.close()
            self._connection = None
        
        # Close session
        if self._session:
            await self._session.close()
            self._session = None
        
        self._state = ConnectionState.DISCONNECTED
        metrics.websocket_connections.labels(provider="polygon", status="disconnected").inc()
    
    async def reconnect(self) -> bool:
        """Reconnect with exponential backoff."""
        if self._state == ConnectionState.RECONNECTING:
            return False
        
        self._state = ConnectionState.RECONNECTING
        
        while self._reconnect_attempts < self.config.max_reconnect_attempts:
            self._reconnect_attempts += 1
            
            # Calculate backoff delay
            delay = min(
                self.config.initial_reconnect_delay * (2 ** (self._reconnect_attempts - 1)),
                self.config.max_reconnect_delay
            )
            
            self.logger.info(f"Reconnection attempt {self._reconnect_attempts}/{self.config.max_reconnect_attempts} in {delay}s")
            await asyncio.sleep(delay)
            
            # Try to reconnect
            await self.disconnect()
            if await self.connect():
                return True
        
        self.logger.error("Max reconnection attempts reached")
        self._state = ConnectionState.FAILED
        return False
    
    async def send(self, message: Dict[str, Any]):
        """Send message to WebSocket."""
        if not self._connection or self._connection.closed:
            raise ConnectionError("WebSocket not connected")
        
        await self._connection.send_json(message)
        metrics.websocket_messages_sent.labels(provider="polygon").inc()
    
    async def receive(self) -> AsyncIterator[Dict[str, Any]]:
        """Receive messages from WebSocket."""
        if not self._connection:
            raise ConnectionError("WebSocket not connected")
        
        async for msg in self._connection:
            if msg.type == aiohttp.WSMsgType.TEXT:
                try:
                    data = json.loads(msg.data)
                    metrics.websocket_messages_received.labels(provider="polygon").inc()
                    
                    # Handle array of messages
                    if isinstance(data, list):
                        for item in data:
                            yield item
                    else:
                        yield data
                        
                except json.JSONDecodeError as e:
                    self.logger.error(f"Failed to parse message: {e}")
                    
            elif msg.type == aiohttp.WSMsgType.ERROR:
                self.logger.error(f"WebSocket error: {msg.data}")
                metrics.websocket_errors.labels(provider="polygon", error_type="message").inc()
                
            elif msg.type == aiohttp.WSMsgType.CLOSED:
                self.logger.warning("WebSocket connection closed")
                break
    
    async def _authenticate(self):
        """Authenticate with Polygon."""
        auth_message = {
            "action": "auth",
            "params": self.api_key
        }
        
        await self.send(auth_message)
        
        # Wait for auth response
        auth_timeout = 5.0
        start_time = time.time()
        
        async for message in self.receive():
            if isinstance(message, dict) and message.get("status") == "auth_success":
                self._connection_id = message.get("message")
                self.logger.info("Authentication successful")
                return
            
            if time.time() - start_time > auth_timeout:
                raise TimeoutError("Authentication timeout")
        
        raise ConnectionError("Authentication failed")
    
    def _start_background_tasks(self):
        """Start heartbeat and monitoring tasks."""
        self._heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        self._monitor_task = asyncio.create_task(self._monitor_loop())
    
    async def _heartbeat_loop(self):
        """Send periodic heartbeats."""
        while self.is_connected:
            try:
                await asyncio.sleep(self.config.heartbeat_interval)
                
                # Send ping
                if self._connection and not self._connection.closed:
                    await self._connection.ping()
                    self._last_heartbeat = time.time()
                    
            except Exception as e:
                self.logger.error(f"Heartbeat failed: {e}")
                break
    
    async def _monitor_loop(self):
        """Monitor connection health."""
        while self._state != ConnectionState.DISCONNECTED:
            try:
                await asyncio.sleep(5.0)
                
                # Check heartbeat timeout
                if time.time() - self._last_heartbeat > self.config.heartbeat_interval * 2:
                    self.logger.warning("Heartbeat timeout detected")
                    await self.reconnect()
                    break
                
                # Check connection state
                if self._connection and self._connection.closed:
                    self.logger.warning("Connection closed unexpectedly")
                    await self.reconnect()
                    break
                    
            except Exception as e:
                self.logger.error(f"Monitor error: {e}")


class SubscriptionManager:
    """Manages symbol subscriptions and channels."""
    
    def __init__(self, ws_manager: WebSocketManager):
        self.ws_manager = ws_manager
        self.logger = get_logger(self.__class__.__name__)
        
        self._subscriptions: Dict[str, Subscription] = {
            "trades": Subscription(channels={"T"}),
            "quotes": Subscription(channels={"Q"}),
            "aggregates": Subscription(channels={"A", "AM"})
        }
        
        self._pending_subscriptions: List[str] = []
        self._active_subscriptions: Set[str] = set()
    
    async def subscribe_trades(self, symbols: List[str]):
        """Subscribe to trade data."""
        await self._subscribe("trades", symbols)
    
    async def subscribe_quotes(self, symbols: List[str]):
        """Subscribe to quote data."""
        await self._subscribe("quotes", symbols)
    
    async def subscribe_aggregates(self, symbols: List[str]):
        """Subscribe to aggregate bars."""
        await self._subscribe("aggregates", symbols)
    
    async def unsubscribe(self, symbols: List[str], channels: Optional[List[str]] = None):
        """Unsubscribe from symbols."""
        if not self.ws_manager.is_connected:
            self.logger.warning("Cannot unsubscribe: not connected")
            return
        
        # Build unsubscription list
        unsubs = []
        for symbol in symbols:
            if channels:
                for channel in channels:
                    sub_key = f"{channel}.{symbol}"
                    if sub_key in self._active_subscriptions:
                        unsubs.append(sub_key)
                        self._active_subscriptions.remove(sub_key)
            else:
                # Unsubscribe from all channels
                for sub_type, subscription in self._subscriptions.items():
                    if symbol in subscription.symbols:
                        subscription.symbols.remove(symbol)
                        for channel in subscription.channels:
                            sub_key = f"{channel}.{symbol}"
                            if sub_key in self._active_subscriptions:
                                unsubs.append(sub_key)
                                self._active_subscriptions.remove(sub_key)
        
        if unsubs:
            # Send in batches
            for i in range(0, len(unsubs), self.ws_manager.config.subscription_batch_size):
                batch = unsubs[i:i + self.ws_manager.config.subscription_batch_size]
                await self._send_unsubscribe(batch)
    
    async def resubscribe_all(self):
        """Resubscribe to all active subscriptions after reconnect."""
        self.logger.info("Resubscribing to all active subscriptions")
        
        all_subs = []
        for sub_type, subscription in self._subscriptions.items():
            all_subs.extend(subscription.to_polygon_format())
        
        if all_subs:
            # Clear active set as we're starting fresh
            self._active_subscriptions.clear()
            
            # Subscribe in batches
            for i in range(0, len(all_subs), self.ws_manager.config.subscription_batch_size):
                batch = all_subs[i:i + self.ws_manager.config.subscription_batch_size]
                await self._send_subscribe(batch)
                self._active_subscriptions.update(batch)
    
    async def _subscribe(self, sub_type: str, symbols: List[str]):
        """Internal subscription method."""
        if sub_type not in self._subscriptions:
            raise ValueError(f"Unknown subscription type: {sub_type}")
        
        subscription = self._subscriptions[sub_type]
        
        # Add symbols
        new_symbols = set(symbols) - subscription.symbols
        if not new_symbols:
            return  # Already subscribed
        
        subscription.symbols.update(new_symbols)
        
        # Build subscription list
        new_subs = []
        for symbol in new_symbols:
            for channel in subscription.channels:
                sub_key = f"{channel}.{symbol}"
                if sub_key not in self._active_subscriptions:
                    new_subs.append(sub_key)
        
        if not new_subs:
            return
        
        # If not connected, add to pending
        if not self.ws_manager.is_connected:
            self._pending_subscriptions.extend(new_subs)
            self.logger.info(f"Added {len(new_subs)} subscriptions to pending queue")
            return
        
        # Subscribe in batches
        for i in range(0, len(new_subs), self.ws_manager.config.subscription_batch_size):
            batch = new_subs[i:i + self.ws_manager.config.subscription_batch_size]
            await self._send_subscribe(batch)
            self._active_subscriptions.update(batch)
    
    async def _send_subscribe(self, subscriptions: List[str]):
        """Send subscription message."""
        message = {
            "action": "subscribe",
            "params": ",".join(subscriptions)
        }
        
        try:
            await self.ws_manager.send(message)
            self.logger.info(f"Subscribed to {len(subscriptions)} channels")
            metrics.websocket_subscriptions.labels(provider="polygon", action="subscribe").inc(len(subscriptions))
        except Exception as e:
            self.logger.error(f"Subscription failed: {e}")
            raise
    
    async def _send_unsubscribe(self, subscriptions: List[str]):
        """Send unsubscription message."""
        message = {
            "action": "unsubscribe",
            "params": ",".join(subscriptions)
        }
        
        try:
            await self.ws_manager.send(message)
            self.logger.info(f"Unsubscribed from {len(subscriptions)} channels")
            metrics.websocket_subscriptions.labels(provider="polygon", action="unsubscribe").inc(len(subscriptions))
        except Exception as e:
            self.logger.error(f"Unsubscription failed: {e}")


class MessageProcessor:
    """Processes incoming WebSocket messages."""
    
    def __init__(self, provider_name: str):
        self.provider_name = provider_name
        self.logger = get_logger(self.__class__.__name__)
        
        # Message handlers
        self._handlers = {
            MessageType.TRADE.value: self._process_trade,
            MessageType.QUOTE.value: self._process_quote,
            MessageType.AGGREGATE_SECOND.value: self._process_aggregate,
            MessageType.AGGREGATE_MINUTE.value: self._process_aggregate,
            MessageType.STATUS.value: self._process_status
        }
    
    async def process(self, message: Dict[str, Any]) -> Optional[Any]:
        """Process a single message."""
        msg_type = message.get("ev")
        
        if not msg_type:
            self.logger.warning(f"Message missing event type: {message}")
            return None
        
        handler = self._handlers.get(msg_type)
        if not handler:
            self.logger.debug(f"Unknown message type: {msg_type}")
            return None
        
        try:
            return await handler(message)
        except Exception as e:
            self.logger.error(f"Error processing {msg_type} message: {e}", extra={"message": message})
            metrics.websocket_errors.labels(provider="polygon", error_type="processing").inc()
            return None
    
    async def _process_trade(self, message: Dict[str, Any]) -> TickData:
        """Process trade message."""
        return TickData(
            time=datetime.fromtimestamp(message.get("t", 0) / 1e9),  # nanoseconds to datetime
            symbol=message.get("sym", ""),
            price=float(message.get("p", 0)),
            size=int(message.get("s", 0)),
            exchange=str(message.get("x", "")),
            conditions=",".join(map(str, message.get("c", []))),
            provider=self.provider_name
        )
    
    async def _process_quote(self, message: Dict[str, Any]) -> OrderBookData:
        """Process quote message."""
        bid_price = float(message.get("bp", 0))
        ask_price = float(message.get("ap", 0))
        
        return OrderBookData(
            time=datetime.fromtimestamp(message.get("t", 0) / 1e9),
            symbol=message.get("sym", ""),
            bid_price=bid_price,
            bid_size=int(message.get("bs", 0)),
            ask_price=ask_price,
            ask_size=int(message.get("as", 0)),
            mid_price=(bid_price + ask_price) / 2 if bid_price and ask_price else 0,
            spread=ask_price - bid_price if bid_price and ask_price else 0,
            provider=self.provider_name
        )
    
    async def _process_aggregate(self, message: Dict[str, Any]) -> MarketData:
        """Process aggregate bar message."""
        return MarketData(
            time=datetime.fromtimestamp(message.get("s", 0) / 1000),  # milliseconds to datetime
            symbol=message.get("sym", ""),
            open=float(message.get("o", 0)),
            high=float(message.get("h", 0)),
            low=float(message.get("l", 0)),
            close=float(message.get("c", 0)),
            volume=int(message.get("v", 0)),
            provider=self.provider_name,
            metadata={
                "vwap": message.get("vw"),
                "average_size": message.get("av"),
                "aggregate_type": message.get("ev")
            }
        )
    
    async def _process_status(self, message: Dict[str, Any]) -> Dict[str, Any]:
        """Process status message."""
        status = message.get("status")
        msg = message.get("message", "")
        
        if status == "connected":
            self.logger.info(f"Status: {msg}")
        elif status == "auth_success":
            self.logger.info("Authentication successful")
        elif status == "auth_failed":
            self.logger.error(f"Authentication failed: {msg}")
        else:
            self.logger.info(f"Status update: {status} - {msg}")
        
        return message


class PolygonWebSocketProvider(BaseProvider):
    """Enhanced Polygon.io provider with full WebSocket support."""
    
    def __init__(self, config: Optional[WebSocketConfig] = None):
        super().__init__("polygon_ws")
        
        self.config = config or WebSocketConfig()
        self.api_key = self.settings.polygon_api_key
        
        # Components
        self.ws_manager = WebSocketManager(self.config, self.api_key)
        self.subscription_manager = SubscriptionManager(self.ws_manager)
        self.message_processor = MessageProcessor(self.name)
        self.stream_buffer = StreamBuffer(self.config.message_buffer_size)
        
        # State
        self._streaming = False
        self._stream_task: Optional[asyncio.Task] = None
    
    async def connect(self):
        """Initialize WebSocket connection."""
        await super().connect()
        
        if not self.api_key:
            raise ValueError("Polygon API key not configured")
        
        # Connect WebSocket
        connected = await self.ws_manager.connect()
        if not connected:
            raise ConnectionError("Failed to establish WebSocket connection")
        
        # Start message streaming
        self._streaming = True
        self._stream_task = asyncio.create_task(self._stream_loop())
        
        self.logger.info("Polygon WebSocket provider connected")
    
    async def disconnect(self):
        """Close WebSocket connection."""
        self._streaming = False
        
        # Cancel stream task
        if self._stream_task:
            self._stream_task.cancel()
            try:
                await self._stream_task
            except asyncio.CancelledError:
                pass
        
        # Disconnect WebSocket
        await self.ws_manager.disconnect()
        
        await super().disconnect()
        self.logger.info("Polygon WebSocket provider disconnected")
    
    async def stream_market_data(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """Stream real-time market data."""
        symbols = self._validate_symbols(symbols)
        
        # Subscribe to aggregate bars
        await self.subscription_manager.subscribe_aggregates(symbols)
        
        # Stream from buffer
        async for data in self._consume_buffer():
            if isinstance(data, MarketData) and data.symbol in symbols:
                yield data
    
    async def stream_tick_data(self, symbols: List[str]) -> AsyncIterator[TickData]:
        """Stream real-time tick data."""
        symbols = self._validate_symbols(symbols)
        
        # Subscribe to trades
        await self.subscription_manager.subscribe_trades(symbols)
        
        # Stream from buffer
        async for data in self._consume_buffer():
            if isinstance(data, TickData) and data.symbol in symbols:
                yield data
    
    async def stream_quotes(self, symbols: List[str]) -> AsyncIterator[OrderBookData]:
        """Stream real-time quote data."""
        symbols = self._validate_symbols(symbols)
        
        # Subscribe to quotes
        await self.subscription_manager.subscribe_quotes(symbols)
        
        # Stream from buffer
        async for data in self._consume_buffer():
            if isinstance(data, OrderBookData) and data.symbol in symbols:
                yield data
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Stream historical data via WebSocket replay."""
        # For historical data, we could implement WebSocket replay
        # For now, this would fall back to HTTP
        raise NotImplementedError("Historical data via WebSocket not yet implemented")
    
    async def _stream_loop(self):
        """Main streaming loop."""
        reconnect_count = 0
        
        while self._streaming:
            try:
                # Check connection
                if not self.ws_manager.is_connected:
                    if reconnect_count > 0:
                        # Resubscribe after reconnect
                        await self.subscription_manager.resubscribe_all()
                    reconnect_count += 1
                
                # Stream messages
                async for message in self.ws_manager.receive():
                    if not self._streaming:
                        break
                    
                    # Process message
                    data = await self.message_processor.process(message)
                    if data:
                        # Add to buffer
                        buffered = await self.stream_buffer.push(data)
                        if not buffered:
                            self.logger.warning("Stream buffer overflow")
                            metrics.websocket_errors.labels(provider="polygon", error_type="buffer_overflow").inc()
                
                # Connection closed, attempt reconnect
                if self._streaming and not self.ws_manager.is_connected:
                    self.logger.warning("Connection lost, attempting reconnect")
                    await self.ws_manager.reconnect()
                    
            except asyncio.CancelledError:
                break
            except Exception as e:
                self.logger.error(f"Stream loop error: {e}")
                await asyncio.sleep(1)  # Brief pause before retry
    
    async def _consume_buffer(self) -> AsyncIterator[Any]:
        """Consume messages from buffer."""
        while self._streaming or self.stream_buffer.size > 0:
            # Get batch of messages
            batch = await self.stream_buffer.pop_batch(100)
            
            if batch:
                for data in batch:
                    yield data
            else:
                # No messages, wait briefly
                await asyncio.sleep(0.01)
    
    def get_statistics(self) -> Dict[str, Any]:
        """Get provider statistics."""
        return {
            "connection_state": self.ws_manager.state.name,
            "is_connected": self.ws_manager.is_connected,
            "buffer_stats": self.stream_buffer.get_stats(),
            "active_subscriptions": len(self.subscription_manager._active_subscriptions),
            "pending_subscriptions": len(self.subscription_manager._pending_subscriptions)
        }


# Convenience function for creating provider
def create_polygon_websocket_provider(
    config: Optional[WebSocketConfig] = None
) -> PolygonWebSocketProvider:
    """Create a configured Polygon WebSocket provider."""
    return PolygonWebSocketProvider(config)