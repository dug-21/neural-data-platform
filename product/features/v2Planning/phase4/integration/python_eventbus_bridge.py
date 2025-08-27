"""
Python EventBus Bridge Implementation

This module provides a Python bridge to the Rust EventBus service,
enabling Python services to publish and subscribe to events through
the high-performance EventBus infrastructure.

Features:
- Async/await support for non-blocking operations
- Connection pooling and automatic reconnection
- Circuit breaker pattern for resilience
- Comprehensive error handling and retry logic
- Metrics and monitoring integration
- Feature flag support for gradual rollout
"""

import asyncio
import json
import logging
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Callable, Any, Union
from datetime import datetime, timezone
import aiohttp
import backoff
from contextlib import asynccontextmanager
import weakref
import uuid


class EventBusError(Exception):
    """Base exception for EventBus operations."""
    pass


class ConnectionError(EventBusError):
    """Raised when connection to EventBus fails."""
    pass


class PublishError(EventBusError):
    """Raised when event publishing fails."""
    pass


class SubscriptionError(EventBusError):
    """Raised when subscription operations fail."""
    pass


class CircuitState(Enum):
    """Circuit breaker states."""
    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half_open"


@dataclass
class EventBusConfig:
    """Configuration for EventBus bridge."""
    host: str = "localhost"
    port: int = 8080
    base_path: str = "/api/v1"
    max_retries: int = 3
    retry_backoff_base: float = 2.0
    retry_backoff_max: float = 60.0
    connection_timeout: float = 5.0
    request_timeout: float = 30.0
    max_connections: int = 100
    circuit_breaker_threshold: int = 5
    circuit_breaker_timeout: float = 60.0
    enable_metrics: bool = True
    metrics_prefix: str = "eventbus_bridge"


@dataclass
class Event:
    """Event data structure."""
    topic: str
    payload: Dict[str, Any]
    timestamp: Optional[datetime] = None
    correlation_id: Optional[str] = None
    source: Optional[str] = None
    event_type: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def __post_init__(self):
        if self.timestamp is None:
            self.timestamp = datetime.now(timezone.utc)
        if self.correlation_id is None:
            self.correlation_id = str(uuid.uuid4())

    def to_dict(self) -> Dict[str, Any]:
        """Convert event to dictionary for serialization."""
        return {
            "topic": self.topic,
            "payload": self.payload,
            "timestamp": self.timestamp.isoformat() if self.timestamp else None,
            "correlation_id": self.correlation_id,
            "source": self.source,
            "event_type": self.event_type,
            "metadata": self.metadata
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'Event':
        """Create event from dictionary."""
        timestamp = None
        if data.get("timestamp"):
            timestamp = datetime.fromisoformat(data["timestamp"].replace('Z', '+00:00'))
        
        return cls(
            topic=data["topic"],
            payload=data["payload"],
            timestamp=timestamp,
            correlation_id=data.get("correlation_id"),
            source=data.get("source"),
            event_type=data.get("event_type"),
            metadata=data.get("metadata", {})
        )


class CircuitBreaker:
    """Circuit breaker implementation for resilience."""
    
    def __init__(self, threshold: int = 5, timeout: float = 60.0):
        self.threshold = threshold
        self.timeout = timeout
        self.failure_count = 0
        self.last_failure_time = 0
        self.state = CircuitState.CLOSED
        self._lock = asyncio.Lock()

    async def call(self, func, *args, **kwargs):
        """Execute function with circuit breaker protection."""
        async with self._lock:
            if self.state == CircuitState.OPEN:
                if time.time() - self.last_failure_time > self.timeout:
                    self.state = CircuitState.HALF_OPEN
                else:
                    raise EventBusError("Circuit breaker is open")

        try:
            result = await func(*args, **kwargs)
            async with self._lock:
                if self.state == CircuitState.HALF_OPEN:
                    self.state = CircuitState.CLOSED
                    self.failure_count = 0
            return result
        except Exception as e:
            async with self._lock:
                self.failure_count += 1
                self.last_failure_time = time.time()
                if self.failure_count >= self.threshold:
                    self.state = CircuitState.OPEN
            raise e


class MetricsCollector:
    """Collects and reports metrics for EventBus operations."""
    
    def __init__(self, prefix: str = "eventbus_bridge"):
        self.prefix = prefix
        self.counters: Dict[str, int] = {}
        self.gauges: Dict[str, float] = {}
        self.histograms: Dict[str, List[float]] = {}
        
    def increment(self, metric: str, value: int = 1, tags: Optional[Dict[str, str]] = None):
        """Increment a counter metric."""
        key = f"{self.prefix}.{metric}"
        if tags:
            key += "." + ".".join(f"{k}_{v}" for k, v in tags.items())
        self.counters[key] = self.counters.get(key, 0) + value
        
    def gauge(self, metric: str, value: float, tags: Optional[Dict[str, str]] = None):
        """Set a gauge metric."""
        key = f"{self.prefix}.{metric}"
        if tags:
            key += "." + ".".join(f"{k}_{v}" for k, v in tags.items())
        self.gauges[key] = value
        
    def histogram(self, metric: str, value: float, tags: Optional[Dict[str, str]] = None):
        """Record a histogram value."""
        key = f"{self.prefix}.{metric}"
        if tags:
            key += "." + ".".join(f"{k}_{v}" for k, v in tags.items())
        if key not in self.histograms:
            self.histograms[key] = []
        self.histograms[key].append(value)
        
    def get_metrics(self) -> Dict[str, Any]:
        """Get all collected metrics."""
        return {
            "counters": self.counters.copy(),
            "gauges": self.gauges.copy(),
            "histograms": {k: {
                "count": len(v),
                "mean": sum(v) / len(v) if v else 0,
                "min": min(v) if v else 0,
                "max": max(v) if v else 0
            } for k, v in self.histograms.items()}
        }


class EventBusBridge:
    """
    Python bridge to Rust EventBus service.
    
    Provides async/await interface for publishing and subscribing to events
    with comprehensive error handling, retries, and monitoring.
    """
    
    def __init__(self, config: Optional[EventBusConfig] = None):
        self.config = config or EventBusConfig()
        self.logger = logging.getLogger(__name__)
        self.session: Optional[aiohttp.ClientSession] = None
        self.base_url = f"http://{self.config.host}:{self.config.port}{self.config.base_path}"
        self.circuit_breaker = CircuitBreaker(
            threshold=self.config.circuit_breaker_threshold,
            timeout=self.config.circuit_breaker_timeout
        )
        self.metrics = MetricsCollector(self.config.metrics_prefix) if self.config.enable_metrics else None
        self.subscribers: weakref.WeakSet = weakref.WeakSet()
        self._closed = False

    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()

    async def connect(self):
        """Establish connection to EventBus service."""
        if self.session and not self.session.closed:
            return

        connector = aiohttp.TCPConnector(
            limit=self.config.max_connections,
            limit_per_host=self.config.max_connections
        )
        
        timeout = aiohttp.ClientTimeout(
            total=self.config.request_timeout,
            connect=self.config.connection_timeout
        )
        
        self.session = aiohttp.ClientSession(
            connector=connector,
            timeout=timeout,
            headers={"Content-Type": "application/json"}
        )
        
        # Test connection
        try:
            await self._health_check()
            self.logger.info("Connected to EventBus service")
            if self.metrics:
                self.metrics.increment("connections.established")
        except Exception as e:
            await self.close()
            raise ConnectionError(f"Failed to connect to EventBus: {e}")

    async def close(self):
        """Close connection and cleanup resources."""
        if self._closed:
            return
            
        self._closed = True
        
        # Close all subscribers
        for subscriber in list(self.subscribers):
            if hasattr(subscriber, 'close'):
                try:
                    await subscriber.close()
                except Exception as e:
                    self.logger.warning(f"Error closing subscriber: {e}")
        
        if self.session and not self.session.closed:
            await self.session.close()
            self.logger.info("Disconnected from EventBus service")
            if self.metrics:
                self.metrics.increment("connections.closed")

    @backoff.on_exception(
        backoff.expo,
        (aiohttp.ClientError, asyncio.TimeoutError),
        max_tries=3,
        base=2,
        max_value=60
    )
    async def _health_check(self):
        """Check EventBus service health."""
        if not self.session:
            raise ConnectionError("Not connected")
            
        async with self.session.get(f"{self.base_url}/health") as response:
            if response.status != 200:
                raise ConnectionError(f"Health check failed: {response.status}")
            return await response.json()

    async def publish(self, event: Event, retries: Optional[int] = None) -> bool:
        """
        Publish an event to the EventBus.
        
        Args:
            event: Event to publish
            retries: Number of retries (overrides config)
            
        Returns:
            True if published successfully
            
        Raises:
            PublishError: If publishing fails after all retries
        """
        if self._closed:
            raise EventBusError("Bridge is closed")
            
        start_time = time.time()
        max_retries = retries if retries is not None else self.config.max_retries
        
        @backoff.on_exception(
            backoff.expo,
            (aiohttp.ClientError, asyncio.TimeoutError, EventBusError),
            max_tries=max_retries + 1,
            base=self.config.retry_backoff_base,
            max_value=self.config.retry_backoff_max
        )
        async def _publish():
            return await self.circuit_breaker.call(self._do_publish, event)

        try:
            result = await _publish()
            
            if self.metrics:
                duration = time.time() - start_time
                self.metrics.increment("events.published", tags={"topic": event.topic})
                self.metrics.histogram("publish.duration", duration, tags={"topic": event.topic})
                
            self.logger.debug(f"Published event to topic '{event.topic}' with correlation_id '{event.correlation_id}'")
            return result
            
        except Exception as e:
            if self.metrics:
                self.metrics.increment("events.publish_failed", tags={"topic": event.topic})
            self.logger.error(f"Failed to publish event to topic '{event.topic}': {e}")
            raise PublishError(f"Failed to publish event: {e}")

    async def _do_publish(self, event: Event) -> bool:
        """Internal method to publish event."""
        if not self.session:
            await self.connect()
            
        url = f"{self.base_url}/events/publish"
        data = event.to_dict()
        
        async with self.session.post(url, json=data) as response:
            if response.status == 200:
                return True
            elif response.status == 429:  # Rate limited
                raise EventBusError("Rate limited")
            else:
                error_text = await response.text()
                raise EventBusError(f"Publish failed: {response.status} - {error_text}")

    async def subscribe(
        self, 
        topic: str, 
        handler: Callable[[Event], Any],
        error_handler: Optional[Callable[[Exception], Any]] = None
    ) -> 'EventSubscriber':
        """
        Subscribe to events on a topic.
        
        Args:
            topic: Topic to subscribe to
            handler: Async function to handle events
            error_handler: Optional error handler
            
        Returns:
            EventSubscriber instance
        """
        if self._closed:
            raise EventBusError("Bridge is closed")
            
        subscriber = EventSubscriber(
            bridge=self,
            topic=topic,
            handler=handler,
            error_handler=error_handler
        )
        
        self.subscribers.add(subscriber)
        await subscriber.start()
        
        if self.metrics:
            self.metrics.increment("subscriptions.created", tags={"topic": topic})
            
        return subscriber

    async def get_metrics(self) -> Optional[Dict[str, Any]]:
        """Get bridge metrics."""
        if not self.metrics:
            return None
        return self.metrics.get_metrics()


class EventSubscriber:
    """Handles event subscription and message processing."""
    
    def __init__(
        self, 
        bridge: EventBusBridge,
        topic: str,
        handler: Callable[[Event], Any],
        error_handler: Optional[Callable[[Exception], Any]] = None
    ):
        self.bridge = bridge
        self.topic = topic
        self.handler = handler
        self.error_handler = error_handler
        self.logger = logging.getLogger(f"{__name__}.{topic}")
        self.running = False
        self.task: Optional[asyncio.Task] = None

    async def start(self):
        """Start the subscription."""
        if self.running:
            return
            
        self.running = True
        self.task = asyncio.create_task(self._subscription_loop())
        self.logger.info(f"Started subscription for topic '{self.topic}'")

    async def stop(self):
        """Stop the subscription."""
        if not self.running:
            return
            
        self.running = False
        if self.task and not self.task.done():
            self.task.cancel()
            try:
                await self.task
            except asyncio.CancelledError:
                pass
                
        self.logger.info(f"Stopped subscription for topic '{self.topic}'")

    async def close(self):
        """Close the subscription."""
        await self.stop()

    async def _subscription_loop(self):
        """Main subscription loop."""
        url = f"{self.bridge.base_url}/events/subscribe/{self.topic}"
        
        while self.running:
            try:
                if not self.bridge.session:
                    await self.bridge.connect()
                    
                async with self.bridge.session.ws_connect(url) as ws:
                    self.logger.info(f"Connected to WebSocket for topic '{self.topic}'")
                    
                    async for msg in ws:
                        if msg.type == aiohttp.WSMsgType.TEXT:
                            try:
                                data = json.loads(msg.data)
                                event = Event.from_dict(data)
                                await self._handle_event(event)
                            except Exception as e:
                                await self._handle_error(e)
                        elif msg.type == aiohttp.WSMsgType.ERROR:
                            raise SubscriptionError(f"WebSocket error: {ws.exception()}")
                            
            except asyncio.CancelledError:
                break
            except Exception as e:
                await self._handle_error(e)
                if self.running:
                    await asyncio.sleep(5)  # Wait before reconnecting

    async def _handle_event(self, event: Event):
        """Handle received event."""
        try:
            if asyncio.iscoroutinefunction(self.handler):
                await self.handler(event)
            else:
                self.handler(event)
                
            if self.bridge.metrics:
                self.bridge.metrics.increment("events.processed", tags={"topic": self.topic})
                
        except Exception as e:
            self.logger.error(f"Error handling event: {e}")
            await self._handle_error(e)

    async def _handle_error(self, error: Exception):
        """Handle errors in subscription."""
        if self.bridge.metrics:
            self.bridge.metrics.increment("subscription.errors", tags={"topic": self.topic})
            
        if self.error_handler:
            try:
                if asyncio.iscoroutinefunction(self.error_handler):
                    await self.error_handler(error)
                else:
                    self.error_handler(error)
            except Exception as e:
                self.logger.error(f"Error in error handler: {e}")


@asynccontextmanager
async def create_eventbus_bridge(config: Optional[EventBusConfig] = None):
    """
    Create and manage EventBus bridge as async context manager.
    
    Usage:
        async with create_eventbus_bridge() as bridge:
            await bridge.publish(event)
    """
    bridge = EventBusBridge(config)
    try:
        await bridge.connect()
        yield bridge
    finally:
        await bridge.close()


# Convenience functions
async def publish_event(
    topic: str,
    payload: Dict[str, Any],
    config: Optional[EventBusConfig] = None,
    **kwargs
) -> bool:
    """
    Convenience function to publish a single event.
    
    Args:
        topic: Event topic
        payload: Event payload
        config: Optional EventBus configuration
        **kwargs: Additional event properties
        
    Returns:
        True if published successfully
    """
    event = Event(topic=topic, payload=payload, **kwargs)
    async with create_eventbus_bridge(config) as bridge:
        return await bridge.publish(event)


# Example usage
async def example_usage():
    """Example usage of EventBus bridge."""
    logging.basicConfig(level=logging.INFO)
    
    # Configuration
    config = EventBusConfig(
        host="localhost",
        port=8080,
        enable_metrics=True
    )
    
    # Create bridge
    async with create_eventbus_bridge(config) as bridge:
        # Publish an event
        event = Event(
            topic="market.data",
            payload={"symbol": "AAPL", "price": 150.25, "volume": 1000000},
            source="data-ingestion",
            event_type="market_tick"
        )
        
        await bridge.publish(event)
        
        # Subscribe to events
        async def handle_market_data(event: Event):
            print(f"Received market data: {event.payload}")
            
        async def handle_error(error: Exception):
            print(f"Subscription error: {error}")
            
        subscriber = await bridge.subscribe(
            topic="market.data",
            handler=handle_market_data,
            error_handler=handle_error
        )
        
        # Keep running
        await asyncio.sleep(10)
        
        # Get metrics
        metrics = await bridge.get_metrics()
        if metrics:
            print(f"Bridge metrics: {metrics}")


if __name__ == "__main__":
    asyncio.run(example_usage())