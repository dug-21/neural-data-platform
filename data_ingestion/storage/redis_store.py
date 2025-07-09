"""Redis storage implementation for real-time data."""
import json
import asyncio
from typing import Dict, Any, Optional, List, Set
from datetime import datetime, timedelta
import redis.asyncio as redis
from redis.asyncio.connection import ConnectionPool

from config import get_settings
from utils.logging import get_logger
from utils.metrics import metrics
from utils.retry import with_retry

logger = get_logger(__name__)


class RedisStore:
    """Redis storage for real-time market data and caching."""
    
    def __init__(self):
        self.settings = get_settings()
        self.redis: Optional[redis.Redis] = None
        self.pubsub: Optional[redis.client.PubSub] = None
        self._pool: Optional[ConnectionPool] = None
    
    async def connect(self):
        """Connect to Redis."""
        try:
            self._pool = redis.ConnectionPool.from_url(
                self.settings.redis_url,
                max_connections=50,
                decode_responses=True
            )
            self.redis = redis.Redis(connection_pool=self._pool)
            self.pubsub = self.redis.pubsub()
            
            # Test connection
            await self.redis.ping()
            
            logger.info("Connected to Redis")
            metrics.active_connections.labels(connection_type="redis").inc()
        except Exception as e:
            logger.error("Failed to connect to Redis", error=str(e))
            raise
    
    async def disconnect(self):
        """Disconnect from Redis."""
        if self.pubsub:
            await self.pubsub.close()
        if self.redis:
            await self.redis.close()
        if self._pool:
            await self._pool.disconnect()
        
        metrics.active_connections.labels(connection_type="redis").dec()
        logger.info("Disconnected from Redis")
    
    @metrics.track_storage_operation("redis", "set_price")
    @with_retry(max_attempts=3, exceptions=(redis.RedisError,))
    async def set_latest_price(self, symbol: str, price_data: Dict[str, Any]):
        """Store latest price data for a symbol."""
        key = f"price:latest:{symbol}"
        price_data['timestamp'] = datetime.utcnow().isoformat()
        
        await self.redis.hset(
            key,
            mapping={k: json.dumps(v) if isinstance(v, (dict, list)) else str(v) 
                    for k, v in price_data.items()}
        )
        
        # Set expiration (1 hour)
        await self.redis.expire(key, 3600)
        
        # Publish price update
        await self.publish_price_update(symbol, price_data)
    
    async def get_latest_price(self, symbol: str) -> Optional[Dict[str, Any]]:
        """Get latest price data for a symbol."""
        key = f"price:latest:{symbol}"
        data = await self.redis.hgetall(key)
        
        if data:
            # Parse JSON fields
            result = {}
            for k, v in data.items():
                try:
                    result[k] = json.loads(v)
                except:
                    result[k] = v
            return result
        return None
    
    @metrics.track_storage_operation("redis", "set_tick")
    async def add_tick_data(self, symbol: str, tick: Dict[str, Any]):
        """Add tick data to sorted set."""
        key = f"ticks:{symbol}"
        timestamp = tick.get('timestamp', datetime.utcnow().timestamp())
        
        # Add to sorted set with timestamp as score
        await self.redis.zadd(key, {json.dumps(tick): timestamp})
        
        # Keep only last hour of ticks
        cutoff = datetime.utcnow().timestamp() - 3600
        await self.redis.zremrangebyscore(key, '-inf', cutoff)
        
        # Publish tick update
        await self.publish_tick_update(symbol, tick)
    
    async def get_recent_ticks(
        self, 
        symbol: str, 
        minutes: int = 5
    ) -> List[Dict[str, Any]]:
        """Get recent tick data."""
        key = f"ticks:{symbol}"
        cutoff = datetime.utcnow().timestamp() - (minutes * 60)
        
        # Get ticks from the last N minutes
        ticks = await self.redis.zrangebyscore(
            key, 
            cutoff, 
            '+inf',
            withscores=True
        )
        
        result = []
        for tick_json, score in ticks:
            tick = json.loads(tick_json)
            tick['redis_timestamp'] = score
            result.append(tick)
        
        return result
    
    @metrics.track_storage_operation("redis", "set_orderbook")
    async def set_orderbook(self, symbol: str, orderbook: Dict[str, Any]):
        """Store order book snapshot."""
        key = f"orderbook:{symbol}"
        orderbook['timestamp'] = datetime.utcnow().isoformat()
        
        await self.redis.set(
            key,
            json.dumps(orderbook),
            ex=60  # Expire after 1 minute
        )
        
        # Publish orderbook update
        await self.publish_orderbook_update(symbol, orderbook)
    
    async def get_orderbook(self, symbol: str) -> Optional[Dict[str, Any]]:
        """Get current order book."""
        key = f"orderbook:{symbol}"
        data = await self.redis.get(key)
        
        if data:
            return json.loads(data)
        return None
    
    # Pub/Sub methods
    async def publish(self, channel: str, message: str):
        """Publish message to a channel."""
        await self.redis.publish(channel, message)
    
    async def set(self, key: str, value: str, ttl: Optional[int] = None):
        """Set a key-value pair with optional TTL."""
        if ttl:
            await self.redis.set(key, value, ex=ttl)
        else:
            await self.redis.set(key, value)
    
    async def get(self, key: str) -> Optional[str]:
        """Get value by key."""
        return await self.redis.get(key)
    
    async def publish_price_update(self, symbol: str, price_data: Dict[str, Any]):
        """Publish price update to subscribers."""
        channel = f"price_updates:{symbol}"
        message = json.dumps({
            'type': 'price_update',
            'symbol': symbol,
            'data': price_data,
            'timestamp': datetime.utcnow().isoformat()
        })
        
        await self.redis.publish(channel, message)
    
    async def publish_tick_update(self, symbol: str, tick: Dict[str, Any]):
        """Publish tick update to subscribers."""
        channel = f"tick_updates:{symbol}"
        message = json.dumps({
            'type': 'tick',
            'symbol': symbol,
            'data': tick,
            'timestamp': datetime.utcnow().isoformat()
        })
        
        await self.redis.publish(channel, message)
    
    async def publish_orderbook_update(self, symbol: str, orderbook: Dict[str, Any]):
        """Publish order book update to subscribers."""
        channel = f"orderbook_updates:{symbol}"
        message = json.dumps({
            'type': 'orderbook',
            'symbol': symbol,
            'data': orderbook,
            'timestamp': datetime.utcnow().isoformat()
        })
        
        await self.redis.publish(channel, message)
    
    async def subscribe_to_updates(
        self, 
        symbols: List[str], 
        update_types: List[str] = ['price', 'tick', 'orderbook']
    ):
        """Subscribe to real-time updates for symbols."""
        channels = []
        
        for symbol in symbols:
            if 'price' in update_types:
                channels.append(f"price_updates:{symbol}")
            if 'tick' in update_types:
                channels.append(f"tick_updates:{symbol}")
            if 'orderbook' in update_types:
                channels.append(f"orderbook_updates:{symbol}")
        
        await self.pubsub.subscribe(*channels)
        logger.info(f"Subscribed to {len(channels)} channels")
    
    async def get_updates(self):
        """Get updates from subscribed channels."""
        async for message in self.pubsub.listen():
            if message['type'] == 'message':
                try:
                    data = json.loads(message['data'])
                    yield data
                except json.JSONDecodeError:
                    logger.error("Failed to decode message", message=message)
    
    # Cache methods
    async def cache_set(
        self, 
        key: str, 
        value: Any, 
        ttl: Optional[int] = 3600
    ):
        """Set cached value with TTL."""
        cache_key = f"cache:{key}"
        
        if isinstance(value, (dict, list)):
            value = json.dumps(value)
        
        await self.redis.set(cache_key, value, ex=ttl)
    
    async def cache_get(self, key: str) -> Optional[Any]:
        """Get cached value."""
        cache_key = f"cache:{key}"
        value = await self.redis.get(cache_key)
        
        if value:
            try:
                return json.loads(value)
            except:
                return value
        return None
    
    async def cache_delete(self, key: str):
        """Delete cached value."""
        cache_key = f"cache:{key}"
        await self.redis.delete(cache_key)
    
    # Queue methods for task processing
    async def queue_push(self, queue_name: str, item: Dict[str, Any]):
        """Push item to queue."""
        await self.redis.lpush(f"queue:{queue_name}", json.dumps(item))
        metrics.queue_size.labels(queue_name=queue_name).inc()
    
    async def queue_pop(self, queue_name: str, timeout: int = 1) -> Optional[Dict[str, Any]]:
        """Pop item from queue with blocking."""
        result = await self.redis.brpop(f"queue:{queue_name}", timeout=timeout)
        
        if result:
            metrics.queue_size.labels(queue_name=queue_name).dec()
            _, data = result
            return json.loads(data)
        return None
    
    async def queue_length(self, queue_name: str) -> int:
        """Get queue length."""
        return await self.redis.llen(f"queue:{queue_name}")
    
    # Metrics storage
    async def increment_metric(self, metric_name: str, labels: Dict[str, str] = None):
        """Increment a metric counter."""
        key = f"metrics:{metric_name}"
        if labels:
            key += ":" + ":".join(f"{k}={v}" for k, v in labels.items())
        
        await self.redis.incr(key)
    
    async def get_metric(self, metric_name: str, labels: Dict[str, str] = None) -> int:
        """Get metric value."""
        key = f"metrics:{metric_name}"
        if labels:
            key += ":" + ":".join(f"{k}={v}" for k, v in labels.items())
        
        value = await self.redis.get(key)
        return int(value) if value else 0