"""
Data Ingestion Service Integration

This module provides integration between the data-ingestion service and the EventBus,
with backwards compatibility for Redis publishing during the migration period.

Features:
- Dual publishing to both Redis and EventBus during migration
- Feature flags for gradual rollout
- Comprehensive monitoring and observability
- Graceful fallback handling
- Performance benchmarking
"""

import asyncio
import json
import logging
import os
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Any, Callable, Union
from datetime import datetime, timezone
import redis.asyncio as redis
from contextlib import asynccontextmanager
import uuid

from python_eventbus_bridge import (
    EventBusBridge, Event, EventBusConfig, EventBusError,
    create_eventbus_bridge, MetricsCollector
)


class PublishStrategy(Enum):
    """Publishing strategies for migration."""
    REDIS_ONLY = "redis_only"
    EVENTBUS_ONLY = "eventbus_only"
    DUAL_PUBLISH = "dual_publish"
    EVENTBUS_PRIMARY = "eventbus_primary"  # EventBus with Redis fallback
    REDIS_PRIMARY = "redis_primary"  # Redis with EventBus fallback


@dataclass
class FeatureFlags:
    """Feature flags for gradual rollout."""
    enable_eventbus: bool = False
    enable_dual_publish: bool = False
    eventbus_percentage: float = 0.0  # 0.0 to 1.0
    enable_benchmarking: bool = True
    enable_detailed_logging: bool = False
    fallback_to_redis: bool = True
    max_eventbus_failures: int = 3
    circuit_breaker_timeout: float = 60.0

    @classmethod
    def from_env(cls) -> 'FeatureFlags':
        """Load feature flags from environment variables."""
        return cls(
            enable_eventbus=os.getenv("ENABLE_EVENTBUS", "false").lower() == "true",
            enable_dual_publish=os.getenv("ENABLE_DUAL_PUBLISH", "false").lower() == "true",
            eventbus_percentage=float(os.getenv("EVENTBUS_PERCENTAGE", "0.0")),
            enable_benchmarking=os.getenv("ENABLE_BENCHMARKING", "true").lower() == "true",
            enable_detailed_logging=os.getenv("ENABLE_DETAILED_LOGGING", "false").lower() == "true",
            fallback_to_redis=os.getenv("FALLBACK_TO_REDIS", "true").lower() == "true",
            max_eventbus_failures=int(os.getenv("MAX_EVENTBUS_FAILURES", "3")),
            circuit_breaker_timeout=float(os.getenv("CIRCUIT_BREAKER_TIMEOUT", "60.0"))
        )


@dataclass
class RedisConfig:
    """Redis configuration."""
    host: str = "localhost"
    port: int = 6379
    db: int = 0
    password: Optional[str] = None
    max_connections: int = 50
    retry_on_timeout: bool = True
    socket_timeout: float = 5.0
    socket_connect_timeout: float = 5.0

    @classmethod
    def from_env(cls) -> 'RedisConfig':
        """Load Redis config from environment."""
        return cls(
            host=os.getenv("REDIS_HOST", "localhost"),
            port=int(os.getenv("REDIS_PORT", "6379")),
            db=int(os.getenv("REDIS_DB", "0")),
            password=os.getenv("REDIS_PASSWORD"),
            max_connections=int(os.getenv("REDIS_MAX_CONNECTIONS", "50"))
        )


@dataclass
class PublishResult:
    """Result of a publish operation."""
    success: bool
    redis_success: bool = False
    eventbus_success: bool = False
    redis_error: Optional[Exception] = None
    eventbus_error: Optional[Exception] = None
    publish_time: Optional[float] = None
    strategy_used: Optional[PublishStrategy] = None


class DataIngestionPublisher:
    """
    Publisher for data ingestion events with migration support.
    
    Supports gradual migration from Redis to EventBus with comprehensive
    monitoring, fallback handling, and performance benchmarking.
    """
    
    def __init__(
        self,
        eventbus_config: Optional[EventBusConfig] = None,
        redis_config: Optional[RedisConfig] = None,
        feature_flags: Optional[FeatureFlags] = None
    ):
        self.eventbus_config = eventbus_config or EventBusConfig()
        self.redis_config = redis_config or RedisConfig.from_env()
        self.feature_flags = feature_flags or FeatureFlags.from_env()
        
        self.logger = logging.getLogger(__name__)
        self.eventbus: Optional[EventBusBridge] = None
        self.redis_client: Optional[redis.Redis] = None
        
        self.metrics = MetricsCollector("data_ingestion") if self.feature_flags.enable_benchmarking else None
        self.eventbus_failures = 0
        self.last_eventbus_failure = 0
        self._closed = False

    async def __aenter__(self):
        """Async context manager entry."""
        await self.initialize()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()

    async def initialize(self):
        """Initialize connections."""
        if self._closed:
            raise RuntimeError("Publisher is closed")

        # Initialize Redis connection
        try:
            self.redis_client = redis.Redis(
                host=self.redis_config.host,
                port=self.redis_config.port,
                db=self.redis_config.db,
                password=self.redis_config.password,
                max_connections=self.redis_config.max_connections,
                retry_on_timeout=self.redis_config.retry_on_timeout,
                socket_timeout=self.redis_config.socket_timeout,
                socket_connect_timeout=self.redis_config.socket_connect_timeout
            )
            
            # Test Redis connection
            await self.redis_client.ping()
            self.logger.info("Connected to Redis")
            
            if self.metrics:
                self.metrics.increment("redis.connections.established")
                
        except Exception as e:
            self.logger.error(f"Failed to connect to Redis: {e}")
            if not self.feature_flags.enable_eventbus:
                raise

        # Initialize EventBus connection if enabled
        if self.feature_flags.enable_eventbus:
            try:
                self.eventbus = EventBusBridge(self.eventbus_config)
                await self.eventbus.connect()
                self.logger.info("Connected to EventBus")
                
                if self.metrics:
                    self.metrics.increment("eventbus.connections.established")
                    
            except Exception as e:
                self.logger.error(f"Failed to connect to EventBus: {e}")
                if not self.redis_client and not self.feature_flags.fallback_to_redis:
                    raise

    async def close(self):
        """Close all connections."""
        if self._closed:
            return
            
        self._closed = True
        
        if self.eventbus:
            try:
                await self.eventbus.close()
                self.logger.info("Closed EventBus connection")
            except Exception as e:
                self.logger.warning(f"Error closing EventBus: {e}")

        if self.redis_client:
            try:
                await self.redis_client.close()
                self.logger.info("Closed Redis connection")
            except Exception as e:
                self.logger.warning(f"Error closing Redis: {e}")

    def _should_use_eventbus(self, event_data: Dict[str, Any]) -> bool:
        """Determine if EventBus should be used for this event."""
        if not self.feature_flags.enable_eventbus:
            return False
            
        if not self.eventbus:
            return False
            
        # Check circuit breaker
        if (self.eventbus_failures >= self.feature_flags.max_eventbus_failures and
            time.time() - self.last_eventbus_failure < self.feature_flags.circuit_breaker_timeout):
            return False
            
        # Check percentage rollout
        if self.feature_flags.eventbus_percentage <= 0:
            return False
        elif self.feature_flags.eventbus_percentage >= 1.0:
            return True
        else:
            # Use hash of correlation_id or generate one for consistent routing
            correlation_id = event_data.get("correlation_id", str(uuid.uuid4()))
            hash_value = hash(correlation_id) % 100
            return (hash_value / 100.0) < self.feature_flags.eventbus_percentage

    def _determine_strategy(self, event_data: Dict[str, Any]) -> PublishStrategy:
        """Determine the publishing strategy for this event."""
        if not self.feature_flags.enable_eventbus:
            return PublishStrategy.REDIS_ONLY
            
        if not self.redis_client:
            return PublishStrategy.EVENTBUS_ONLY
            
        if not self.eventbus:
            return PublishStrategy.REDIS_ONLY
            
        if self.feature_flags.enable_dual_publish:
            return PublishStrategy.DUAL_PUBLISH
            
        if self._should_use_eventbus(event_data):
            return (PublishStrategy.EVENTBUS_PRIMARY 
                   if self.feature_flags.fallback_to_redis 
                   else PublishStrategy.EVENTBUS_ONLY)
        else:
            return PublishStrategy.REDIS_PRIMARY

    async def publish_market_data(
        self, 
        symbol: str, 
        price: float, 
        volume: int, 
        timestamp: Optional[datetime] = None,
        **kwargs
    ) -> PublishResult:
        """
        Publish market data event.
        
        Args:
            symbol: Stock symbol
            price: Current price
            volume: Trade volume
            timestamp: Event timestamp
            **kwargs: Additional metadata
            
        Returns:
            PublishResult with operation details
        """
        event_data = {
            "symbol": symbol,
            "price": price,
            "volume": volume,
            "timestamp": (timestamp or datetime.now(timezone.utc)).isoformat(),
            "correlation_id": str(uuid.uuid4()),
            "source": "data-ingestion",
            "event_type": "market_tick",
            **kwargs
        }
        
        return await self.publish_event("market.data", event_data)

    async def publish_trade_execution(
        self,
        order_id: str,
        symbol: str,
        side: str,
        quantity: int,
        price: float,
        timestamp: Optional[datetime] = None,
        **kwargs
    ) -> PublishResult:
        """
        Publish trade execution event.
        
        Args:
            order_id: Order identifier
            symbol: Stock symbol
            side: Buy/sell side
            quantity: Trade quantity
            price: Execution price
            timestamp: Event timestamp
            **kwargs: Additional metadata
            
        Returns:
            PublishResult with operation details
        """
        event_data = {
            "order_id": order_id,
            "symbol": symbol,
            "side": side,
            "quantity": quantity,
            "price": price,
            "timestamp": (timestamp or datetime.now(timezone.utc)).isoformat(),
            "correlation_id": str(uuid.uuid4()),
            "source": "data-ingestion",
            "event_type": "trade_execution",
            **kwargs
        }
        
        return await self.publish_event("trading.executions", event_data)

    async def publish_event(self, topic: str, event_data: Dict[str, Any]) -> PublishResult:
        """
        Publish event using the appropriate strategy.
        
        Args:
            topic: Event topic
            event_data: Event payload
            
        Returns:
            PublishResult with operation details
        """
        if self._closed:
            raise RuntimeError("Publisher is closed")

        start_time = time.time()
        strategy = self._determine_strategy(event_data)
        
        if self.feature_flags.enable_detailed_logging:
            self.logger.debug(f"Publishing to topic '{topic}' using strategy '{strategy.value}'")

        result = PublishResult(success=False, strategy_used=strategy)

        try:
            if strategy == PublishStrategy.REDIS_ONLY:
                result = await self._publish_redis_only(topic, event_data, result)
            elif strategy == PublishStrategy.EVENTBUS_ONLY:
                result = await self._publish_eventbus_only(topic, event_data, result)
            elif strategy == PublishStrategy.DUAL_PUBLISH:
                result = await self._publish_dual(topic, event_data, result)
            elif strategy == PublishStrategy.EVENTBUS_PRIMARY:
                result = await self._publish_eventbus_primary(topic, event_data, result)
            elif strategy == PublishStrategy.REDIS_PRIMARY:
                result = await self._publish_redis_primary(topic, event_data, result)

            result.publish_time = time.time() - start_time
            
            if self.metrics:
                self._record_metrics(topic, result, strategy)
                
            return result
            
        except Exception as e:
            result.publish_time = time.time() - start_time
            self.logger.error(f"Unexpected error publishing event: {e}")
            if self.metrics:
                self.metrics.increment("publish.unexpected_errors", tags={"topic": topic})
            raise

    async def _publish_redis_only(self, topic: str, event_data: Dict[str, Any], result: PublishResult) -> PublishResult:
        """Publish to Redis only."""
        try:
            await self._publish_to_redis(topic, event_data)
            result.redis_success = True
            result.success = True
        except Exception as e:
            result.redis_error = e
            result.success = False
        return result

    async def _publish_eventbus_only(self, topic: str, event_data: Dict[str, Any], result: PublishResult) -> PublishResult:
        """Publish to EventBus only."""
        try:
            await self._publish_to_eventbus(topic, event_data)
            result.eventbus_success = True
            result.success = True
            self._reset_eventbus_failures()
        except Exception as e:
            result.eventbus_error = e
            result.success = False
            self._record_eventbus_failure()
        return result

    async def _publish_dual(self, topic: str, event_data: Dict[str, Any], result: PublishResult) -> PublishResult:
        """Publish to both Redis and EventBus."""
        # Publish to both in parallel
        redis_task = asyncio.create_task(self._safe_publish_to_redis(topic, event_data))
        eventbus_task = asyncio.create_task(self._safe_publish_to_eventbus(topic, event_data))
        
        redis_success, redis_error = await redis_task
        eventbus_success, eventbus_error = await eventbus_task
        
        result.redis_success = redis_success
        result.eventbus_success = eventbus_success
        result.redis_error = redis_error
        result.eventbus_error = eventbus_error
        result.success = redis_success or eventbus_success  # Success if at least one succeeds
        
        if eventbus_success:
            self._reset_eventbus_failures()
        elif eventbus_error:
            self._record_eventbus_failure()
            
        return result

    async def _publish_eventbus_primary(self, topic: str, event_data: Dict[str, Any], result: PublishResult) -> PublishResult:
        """Publish to EventBus with Redis fallback."""
        try:
            await self._publish_to_eventbus(topic, event_data)
            result.eventbus_success = True
            result.success = True
            self._reset_eventbus_failures()
        except Exception as e:
            result.eventbus_error = e
            self._record_eventbus_failure()
            
            # Fallback to Redis
            if self.feature_flags.fallback_to_redis and self.redis_client:
                try:
                    await self._publish_to_redis(topic, event_data)
                    result.redis_success = True
                    result.success = True
                    self.logger.warning(f"EventBus failed, successfully fell back to Redis: {e}")
                except Exception as redis_e:
                    result.redis_error = redis_e
                    result.success = False
                    self.logger.error(f"Both EventBus and Redis failed: EventBus={e}, Redis={redis_e}")
            else:
                result.success = False
                
        return result

    async def _publish_redis_primary(self, topic: str, event_data: Dict[str, Any], result: PublishResult) -> PublishResult:
        """Publish to Redis with EventBus fallback."""
        try:
            await self._publish_to_redis(topic, event_data)
            result.redis_success = True
            result.success = True
        except Exception as e:
            result.redis_error = e
            
            # Fallback to EventBus
            if self.eventbus:
                try:
                    await self._publish_to_eventbus(topic, event_data)
                    result.eventbus_success = True
                    result.success = True
                    self._reset_eventbus_failures()
                    self.logger.warning(f"Redis failed, successfully fell back to EventBus: {e}")
                except Exception as eventbus_e:
                    result.eventbus_error = eventbus_e
                    result.success = False
                    self._record_eventbus_failure()
                    self.logger.error(f"Both Redis and EventBus failed: Redis={e}, EventBus={eventbus_e}")
            else:
                result.success = False
                
        return result

    async def _publish_to_redis(self, topic: str, event_data: Dict[str, Any]):
        """Publish event to Redis."""
        if not self.redis_client:
            raise RuntimeError("Redis client not initialized")
            
        # Convert topic to Redis channel format
        redis_channel = topic.replace('.', ':')
        message = json.dumps(event_data)
        
        await self.redis_client.publish(redis_channel, message)

    async def _publish_to_eventbus(self, topic: str, event_data: Dict[str, Any]):
        """Publish event to EventBus."""
        if not self.eventbus:
            raise RuntimeError("EventBus not initialized")
            
        event = Event(
            topic=topic,
            payload=event_data,
            timestamp=datetime.fromisoformat(event_data.get("timestamp", datetime.now(timezone.utc).isoformat())),
            correlation_id=event_data.get("correlation_id"),
            source=event_data.get("source"),
            event_type=event_data.get("event_type")
        )
        
        await self.eventbus.publish(event)

    async def _safe_publish_to_redis(self, topic: str, event_data: Dict[str, Any]) -> tuple[bool, Optional[Exception]]:
        """Safely publish to Redis, returning success status and error."""
        try:
            await self._publish_to_redis(topic, event_data)
            return True, None
        except Exception as e:
            return False, e

    async def _safe_publish_to_eventbus(self, topic: str, event_data: Dict[str, Any]) -> tuple[bool, Optional[Exception]]:
        """Safely publish to EventBus, returning success status and error."""
        try:
            await self._publish_to_eventbus(topic, event_data)
            return True, None
        except Exception as e:
            return False, e

    def _record_eventbus_failure(self):
        """Record EventBus failure for circuit breaker."""
        self.eventbus_failures += 1
        self.last_eventbus_failure = time.time()

    def _reset_eventbus_failures(self):
        """Reset EventBus failure count."""
        self.eventbus_failures = 0
        self.last_eventbus_failure = 0

    def _record_metrics(self, topic: str, result: PublishResult, strategy: PublishStrategy):
        """Record metrics for the publish operation."""
        if not self.metrics:
            return
            
        tags = {"topic": topic, "strategy": strategy.value}
        
        self.metrics.increment("publish.total", tags=tags)
        
        if result.success:
            self.metrics.increment("publish.success", tags=tags)
        else:
            self.metrics.increment("publish.failed", tags=tags)
            
        if result.redis_success:
            self.metrics.increment("publish.redis_success", tags=tags)
        elif result.redis_error:
            self.metrics.increment("publish.redis_failed", tags=tags)
            
        if result.eventbus_success:
            self.metrics.increment("publish.eventbus_success", tags=tags)
        elif result.eventbus_error:
            self.metrics.increment("publish.eventbus_failed", tags=tags)
            
        if result.publish_time:
            self.metrics.histogram("publish.duration", result.publish_time, tags=tags)

    async def get_metrics(self) -> Optional[Dict[str, Any]]:
        """Get publisher metrics."""
        if not self.metrics:
            return None
            
        metrics = self.metrics.get_metrics()
        
        # Add custom metrics
        metrics["custom"] = {
            "eventbus_failures": self.eventbus_failures,
            "last_eventbus_failure": self.last_eventbus_failure,
            "circuit_breaker_active": (
                self.eventbus_failures >= self.feature_flags.max_eventbus_failures and
                time.time() - self.last_eventbus_failure < self.feature_flags.circuit_breaker_timeout
            )
        }
        
        return metrics

    async def health_check(self) -> Dict[str, Any]:
        """Perform health check on all connections."""
        health = {
            "status": "healthy",
            "redis": {"status": "unknown"},
            "eventbus": {"status": "unknown"},
            "timestamp": datetime.now(timezone.utc).isoformat()
        }
        
        # Check Redis
        if self.redis_client:
            try:
                await self.redis_client.ping()
                health["redis"]["status"] = "healthy"
            except Exception as e:
                health["redis"]["status"] = "unhealthy"
                health["redis"]["error"] = str(e)
                health["status"] = "degraded"
        else:
            health["redis"]["status"] = "not_configured"
            
        # Check EventBus
        if self.eventbus:
            try:
                await self.eventbus._health_check()
                health["eventbus"]["status"] = "healthy"
            except Exception as e:
                health["eventbus"]["status"] = "unhealthy"
                health["eventbus"]["error"] = str(e)
                health["status"] = "degraded"
        else:
            health["eventbus"]["status"] = "not_configured"
            
        return health


@asynccontextmanager
async def create_data_ingestion_publisher(
    eventbus_config: Optional[EventBusConfig] = None,
    redis_config: Optional[RedisConfig] = None,
    feature_flags: Optional[FeatureFlags] = None
):
    """
    Create and manage data ingestion publisher as async context manager.
    
    Usage:
        async with create_data_ingestion_publisher() as publisher:
            result = await publisher.publish_market_data("AAPL", 150.25, 1000000)
    """
    publisher = DataIngestionPublisher(eventbus_config, redis_config, feature_flags)
    try:
        await publisher.initialize()
        yield publisher
    finally:
        await publisher.close()


# Example usage and testing
async def example_usage():
    """Example usage of the data ingestion publisher."""
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Configure feature flags for testing
    feature_flags = FeatureFlags(
        enable_eventbus=True,
        enable_dual_publish=True,
        eventbus_percentage=0.5,  # 50% of events to EventBus
        enable_benchmarking=True,
        enable_detailed_logging=True
    )
    
    # Create publisher
    async with create_data_ingestion_publisher(feature_flags=feature_flags) as publisher:
        # Test health check
        health = await publisher.health_check()
        print(f"Health check: {health}")
        
        # Publish market data events
        for i in range(10):
            symbol = f"STOCK{i}"
            price = 100.0 + i * 0.25
            volume = 1000000 + i * 10000
            
            result = await publisher.publish_market_data(
                symbol=symbol,
                price=price,
                volume=volume
            )
            
            print(f"Published {symbol}: success={result.success}, "
                  f"redis={result.redis_success}, eventbus={result.eventbus_success}, "
                  f"strategy={result.strategy_used}, time={result.publish_time:.3f}s")
            
            if not result.success:
                if result.redis_error:
                    print(f"  Redis error: {result.redis_error}")
                if result.eventbus_error:
                    print(f"  EventBus error: {result.eventbus_error}")
        
        # Publish trade execution events
        for i in range(5):
            result = await publisher.publish_trade_execution(
                order_id=f"ORDER_{i}",
                symbol=f"STOCK{i}",
                side="buy" if i % 2 == 0 else "sell",
                quantity=100 * (i + 1),
                price=100.0 + i * 0.5
            )
            
            print(f"Published trade execution: success={result.success}")
        
        # Get metrics
        metrics = await publisher.get_metrics()
        if metrics:
            print(f"\nMetrics summary:")
            print(f"  Total publishes: {metrics.get('counters', {}).get('data_ingestion.publish.total', 0)}")
            print(f"  Successful publishes: {metrics.get('counters', {}).get('data_ingestion.publish.success', 0)}")
            print(f"  Failed publishes: {metrics.get('counters', {}).get('data_ingestion.publish.failed', 0)}")
            print(f"  EventBus failures: {metrics.get('custom', {}).get('eventbus_failures', 0)}")


# Performance benchmark
async def benchmark_performance():
    """Benchmark performance of different publishing strategies."""
    logging.basicConfig(level=logging.WARNING)  # Reduce noise
    
    strategies = [
        ("Redis Only", FeatureFlags(enable_eventbus=False)),
        ("EventBus Only", FeatureFlags(enable_eventbus=True, eventbus_percentage=1.0)),
        ("Dual Publish", FeatureFlags(enable_eventbus=True, enable_dual_publish=True))
    ]
    
    num_events = 1000
    results = {}
    
    for strategy_name, flags in strategies:
        print(f"\nBenchmarking {strategy_name}...")
        
        async with create_data_ingestion_publisher(feature_flags=flags) as publisher:
            start_time = time.time()
            successful_publishes = 0
            
            # Publish events
            tasks = []
            for i in range(num_events):
                task = publisher.publish_market_data(
                    symbol=f"BENCH{i % 100}",  # 100 unique symbols
                    price=100.0 + (i % 1000) * 0.01,
                    volume=1000000 + i
                )
                tasks.append(task)
            
            # Wait for all publishes
            publish_results = await asyncio.gather(*tasks, return_exceptions=True)
            
            # Count successes
            for result in publish_results:
                if isinstance(result, PublishResult) and result.success:
                    successful_publishes += 1
            
            total_time = time.time() - start_time
            
            results[strategy_name] = {
                "total_events": num_events,
                "successful_events": successful_publishes,
                "total_time": total_time,
                "events_per_second": successful_publishes / total_time,
                "average_latency": total_time / num_events
            }
            
            print(f"  Published {successful_publishes}/{num_events} events in {total_time:.2f}s")
            print(f"  Throughput: {results[strategy_name]['events_per_second']:.1f} events/sec")
            print(f"  Average latency: {results[strategy_name]['average_latency']*1000:.1f}ms")
    
    print("\n=== Benchmark Summary ===")
    for strategy_name, result in results.items():
        print(f"{strategy_name}:")
        print(f"  Throughput: {result['events_per_second']:.1f} events/sec")
        print(f"  Success rate: {result['successful_events']/result['total_events']*100:.1f}%")


if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "benchmark":
        asyncio.run(benchmark_performance())
    else:
        asyncio.run(example_usage())