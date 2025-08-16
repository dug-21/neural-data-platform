#!/usr/bin/env python3
"""Test Redis publishing functionality"""
import asyncio
import json
from storage import RedisStore
from config import get_settings
from utils.logging import get_logger

logger = get_logger(__name__)

async def test_redis_publishing():
    """Test basic Redis publishing to verify it's working"""
    settings = get_settings()
    redis = RedisStore()
    
    try:
        # Connect to Redis
        await redis.connect()
        logger.info("Connected to Redis successfully")
        
        # Test data
        test_data = {
            'symbol': 'AAPL',
            'price': 195.50,
            'volume': 1000000,
            'timestamp': '2025-01-08T12:00:00Z'
        }
        
        # Test publishing to different channels
        channels = [
            'market:AAPL',  # Phase 2 format
            'market:updates',  # Legacy format
            'test:channel'  # Test channel
        ]
        
        for channel in channels:
            try:
                message = json.dumps(test_data, default=str)
                result = await redis.publish(channel, message)
                logger.info(f"✅ Published to {channel}: {result} subscribers received")
            except Exception as e:
                logger.error(f"❌ Failed to publish to {channel}: {e}")
        
        # Test getting keys to verify Redis connection
        try:
            # Try to set a test key
            await redis.set('test:connection', 'working', ttl=60)
            value = await redis.get('test:connection')
            if value == 'working':
                logger.info("✅ Redis SET/GET operations working")
            else:
                logger.error(f"❌ Redis GET returned unexpected value: {value}")
        except Exception as e:
            logger.error(f"❌ Redis SET/GET operations failed: {e}")
        
        # Disconnect
        await redis.disconnect()
        logger.info("Disconnected from Redis")
        
    except Exception as e:
        logger.error(f"Test failed: {e}")
        raise

if __name__ == "__main__":
    asyncio.run(test_redis_publishing())