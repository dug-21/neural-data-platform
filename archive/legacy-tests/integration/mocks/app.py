#!/usr/bin/env python3
"""
Mock Config-Store Service for Integration Testing

This service provides a mock implementation of the config-store API
for testing config-store integration with data-ingestion components.
"""

import asyncio
import json
import logging
import os
import time
from contextlib import asynccontextmanager
from typing import Any, Dict, List, Optional

import asyncpg
import redis.asyncio as redis
import structlog
from cryptography.fernet import Fernet
from fastapi import FastAPI, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field


# Configure structured logging
structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.processors.UnicodeDecoder(),
        structlog.processors.JSONRenderer()
    ],
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    wrapper_class=structlog.stdlib.BoundLogger,
    cache_logger_on_first_use=True,
)

logger = structlog.get_logger()

# Configuration Models
class ConfigItem(BaseModel):
    key: str
    value: Any
    encrypted: bool = False
    created_at: float = Field(default_factory=time.time)
    updated_at: float = Field(default_factory=time.time)
    version: int = 1
    metadata: Dict[str, Any] = Field(default_factory=dict)

class ConfigUpdate(BaseModel):
    value: Any
    encrypted: bool = False
    metadata: Dict[str, Any] = Field(default_factory=dict)

class HealthStatus(BaseModel):
    status: str
    timestamp: float
    version: str
    dependencies: Dict[str, str]

# Global state
app_state = {
    "redis_pool": None,
    "postgres_pool": None,
    "encryption_key": None,
    "startup_time": time.time()
}

class ConfigStore:
    """Mock Config Store implementation."""
    
    def __init__(self, redis_pool, postgres_pool, encryption_key):
        self.redis = redis_pool
        self.postgres = postgres_pool
        self.cipher_suite = Fernet(encryption_key) if encryption_key else None
        
    async def get(self, key: str) -> Optional[ConfigItem]:
        """Get configuration item by key."""
        logger.info("Getting configuration", key=key)
        
        # Try Redis cache first
        cached = await self.redis.get(f"config:{key}")
        if cached:
            data = json.loads(cached)
            logger.info("Configuration found in cache", key=key)
            return ConfigItem(**data)
        
        # Fall back to PostgreSQL
        async with self.postgres.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT key, value, encrypted, created_at, updated_at, version, metadata "
                "FROM configurations WHERE key = $1", key
            )
            
            if not row:
                logger.warning("Configuration not found", key=key)
                return None
            
            # Decrypt value if encrypted
            value = row['value']
            if row['encrypted'] and self.cipher_suite:
                try:
                    decrypted = self.cipher_suite.decrypt(value.encode()).decode()
                    value = json.loads(decrypted)
                except Exception as e:
                    logger.error("Failed to decrypt configuration", key=key, error=str(e))
                    raise HTTPException(status_code=500, detail="Decryption failed")
            
            config_item = ConfigItem(
                key=row['key'],
                value=value,
                encrypted=row['encrypted'],
                created_at=row['created_at'].timestamp() if row['created_at'] else time.time(),
                updated_at=row['updated_at'].timestamp() if row['updated_at'] else time.time(),
                version=row['version'],
                metadata=row['metadata'] or {}
            )
            
            # Cache in Redis for future requests
            await self.redis.setex(
                f"config:{key}", 
                300,  # 5 minutes TTL
                json.dumps(config_item.dict())
            )
            
            logger.info("Configuration found in database", key=key)
            return config_item
    
    async def set(self, key: str, update: ConfigUpdate) -> ConfigItem:
        """Set configuration item."""
        logger.info("Setting configuration", key=key, encrypted=update.encrypted)
        
        # Encrypt value if requested
        value = update.value
        if update.encrypted and self.cipher_suite:
            encrypted_value = self.cipher_suite.encrypt(
                json.dumps(value).encode()
            ).decode()
            value = encrypted_value
        
        current_time = time.time()
        
        async with self.postgres.acquire() as conn:
            # Upsert configuration
            row = await conn.fetchrow("""
                INSERT INTO configurations (key, value, encrypted, created_at, updated_at, version, metadata)
                VALUES ($1, $2, $3, $4, $4, 1, $5)
                ON CONFLICT (key) DO UPDATE SET
                    value = EXCLUDED.value,
                    encrypted = EXCLUDED.encrypted,
                    updated_at = EXCLUDED.updated_at,
                    version = configurations.version + 1,
                    metadata = EXCLUDED.metadata
                RETURNING key, value, encrypted, created_at, updated_at, version, metadata
            """, key, json.dumps(value), update.encrypted, 
                 current_time, update.metadata)
            
            config_item = ConfigItem(
                key=row['key'],
                value=update.value,  # Return original unencrypted value
                encrypted=row['encrypted'],
                created_at=row['created_at'],
                updated_at=row['updated_at'],
                version=row['version'],
                metadata=row['metadata'] or {}
            )
            
            # Update Redis cache
            await self.redis.setex(
                f"config:{key}", 
                300,
                json.dumps(config_item.dict())
            )
            
            # Publish change notification
            await self.redis.publish(
                "config_changes",
                json.dumps({
                    "key": key,
                    "action": "set",
                    "timestamp": current_time
                })
            )
            
            logger.info("Configuration updated", key=key, version=config_item.version)
            return config_item
    
    async def delete(self, key: str) -> bool:
        """Delete configuration item."""
        logger.info("Deleting configuration", key=key)
        
        async with self.postgres.acquire() as conn:
            result = await conn.execute("DELETE FROM configurations WHERE key = $1", key)
            deleted = result == "DELETE 1"
            
            if deleted:
                # Remove from Redis cache
                await self.redis.delete(f"config:{key}")
                
                # Publish change notification
                await self.redis.publish(
                    "config_changes",
                    json.dumps({
                        "key": key,
                        "action": "delete",
                        "timestamp": time.time()
                    })
                )
                
                logger.info("Configuration deleted", key=key)
            else:
                logger.warning("Configuration not found for deletion", key=key)
            
            return deleted
    
    async def list_keys(self, prefix: str = None) -> List[str]:
        """List configuration keys, optionally filtered by prefix."""
        logger.info("Listing configuration keys", prefix=prefix)
        
        query = "SELECT key FROM configurations"
        params = []
        
        if prefix:
            query += " WHERE key LIKE $1"
            params.append(f"{prefix}%")
        
        query += " ORDER BY key"
        
        async with self.postgres.acquire() as conn:
            rows = await conn.fetch(query, *params)
            keys = [row['key'] for row in rows]
            
        logger.info("Listed configuration keys", count=len(keys), prefix=prefix)
        return keys
    
    async def get_multiple(self, keys: List[str]) -> Dict[str, Optional[ConfigItem]]:
        """Get multiple configuration items."""
        logger.info("Getting multiple configurations", keys=keys)
        
        result = {}
        for key in keys:
            result[key] = await self.get(key)
        
        return result

# Startup and shutdown
@asynccontextmanager
async def lifespan(app: FastAPI):
    """Manage application lifespan."""
    logger.info("Starting mock config-store service")
    
    # Initialize Redis connection
    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
    app_state["redis_pool"] = redis.from_url(redis_url, decode_responses=True)
    
    # Initialize PostgreSQL connection
    postgres_url = os.getenv("POSTGRES_URL", "postgresql://postgres:postgres@localhost/neural_trader_test")
    app_state["postgres_pool"] = await asyncpg.create_pool(postgres_url)
    
    # Initialize encryption
    encryption_key = os.getenv("ENCRYPTION_KEY", "test_encryption_key_32_chars_long")
    if len(encryption_key) != 32:
        encryption_key = encryption_key.ljust(32)[:32]
    app_state["encryption_key"] = Fernet.generate_key() if encryption_key == "generate" else encryption_key.encode()
    
    # Test connections
    try:
        await app_state["redis_pool"].ping()
        logger.info("Redis connection successful")
    except Exception as e:
        logger.error("Redis connection failed", error=str(e))
        raise
    
    try:
        async with app_state["postgres_pool"].acquire() as conn:
            await conn.fetchval("SELECT 1")
        logger.info("PostgreSQL connection successful")
    except Exception as e:
        logger.error("PostgreSQL connection failed", error=str(e))
        raise
    
    logger.info("Mock config-store service started successfully")
    
    yield
    
    # Cleanup
    logger.info("Shutting down mock config-store service")
    
    if app_state["redis_pool"]:
        await app_state["redis_pool"].close()
    
    if app_state["postgres_pool"]:
        await app_state["postgres_pool"].close()
    
    logger.info("Mock config-store service shutdown complete")

# Create FastAPI app
app = FastAPI(
    title="Mock Config-Store Service",
    description="Mock implementation of config-store for integration testing",
    version="1.0.0",
    lifespan=lifespan
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# API Endpoints
@app.get("/health")
async def health_check():
    """Health check endpoint."""
    dependencies = {}
    
    # Check Redis
    try:
        await app_state["redis_pool"].ping()
        dependencies["redis"] = "healthy"
    except Exception as e:
        dependencies["redis"] = f"unhealthy: {str(e)}"
    
    # Check PostgreSQL
    try:
        async with app_state["postgres_pool"].acquire() as conn:
            await conn.fetchval("SELECT 1")
        dependencies["postgres"] = "healthy"
    except Exception as e:
        dependencies["postgres"] = f"unhealthy: {str(e)}"
    
    # Overall status
    all_healthy = all(status == "healthy" for status in dependencies.values())
    status = "healthy" if all_healthy else "degraded"
    
    return HealthStatus(
        status=status,
        timestamp=time.time(),
        version="1.0.0",
        dependencies=dependencies
    )

@app.get("/config/{key}")
async def get_configuration(key: str):
    """Get configuration by key."""
    store = ConfigStore(
        app_state["redis_pool"], 
        app_state["postgres_pool"], 
        app_state["encryption_key"]
    )
    
    config_item = await store.get(key)
    if not config_item:
        raise HTTPException(status_code=404, detail=f"Configuration '{key}' not found")
    
    return config_item

@app.put("/config/{key}")
async def set_configuration(key: str, update: ConfigUpdate):
    """Set configuration."""
    store = ConfigStore(
        app_state["redis_pool"], 
        app_state["postgres_pool"], 
        app_state["encryption_key"]
    )
    
    return await store.set(key, update)

@app.delete("/config/{key}")
async def delete_configuration(key: str):
    """Delete configuration."""
    store = ConfigStore(
        app_state["redis_pool"], 
        app_state["postgres_pool"], 
        app_state["encryption_key"]
    )
    
    deleted = await store.delete(key)
    if not deleted:
        raise HTTPException(status_code=404, detail=f"Configuration '{key}' not found")
    
    return {"deleted": True}

@app.get("/config")
async def list_configurations(prefix: str = None):
    """List configuration keys."""
    store = ConfigStore(
        app_state["redis_pool"], 
        app_state["postgres_pool"], 
        app_state["encryption_key"]
    )
    
    keys = await store.list_keys(prefix)
    return {"keys": keys}

@app.post("/config/bulk")
async def get_multiple_configurations(keys: List[str]):
    """Get multiple configurations."""
    store = ConfigStore(
        app_state["redis_pool"], 
        app_state["postgres_pool"], 
        app_state["encryption_key"]
    )
    
    return await store.get_multiple(keys)

@app.get("/metrics")
async def get_metrics():
    """Get service metrics."""
    uptime = time.time() - app_state["startup_time"]
    
    # Get Redis info
    redis_info = {}
    try:
        info = await app_state["redis_pool"].info()
        redis_info = {
            "connected_clients": info.get("connected_clients", 0),
            "used_memory": info.get("used_memory", 0),
            "keyspace_hits": info.get("keyspace_hits", 0),
            "keyspace_misses": info.get("keyspace_misses", 0)
        }
    except Exception as e:
        redis_info = {"error": str(e)}
    
    # Get PostgreSQL stats
    postgres_stats = {}
    try:
        async with app_state["postgres_pool"].acquire() as conn:
            config_count = await conn.fetchval("SELECT COUNT(*) FROM configurations")
            postgres_stats = {
                "configuration_count": config_count,
                "pool_size": len(app_state["postgres_pool"]._holders)
            }
    except Exception as e:
        postgres_stats = {"error": str(e)}
    
    return {
        "uptime_seconds": uptime,
        "redis": redis_info,
        "postgres": postgres_stats,
        "timestamp": time.time()
    }

@app.websocket("/config/stream")
async def configuration_stream(websocket):
    """WebSocket endpoint for real-time configuration updates."""
    await websocket.accept()
    
    # Subscribe to Redis pub/sub
    pubsub = app_state["redis_pool"].pubsub()
    await pubsub.subscribe("config_changes")
    
    try:
        while True:
            message = await pubsub.get_message()
            if message and message['type'] == 'message':
                await websocket.send_text(message['data'])
    except Exception as e:
        logger.error("WebSocket connection error", error=str(e))
    finally:
        await pubsub.unsubscribe("config_changes")
        await pubsub.close()

if __name__ == "__main__":
    import uvicorn
    
    # Configure logging
    log_level = os.getenv("LOG_LEVEL", "INFO").upper()
    
    uvicorn.run(
        "app:app",
        host="0.0.0.0",
        port=8080,
        log_level=log_level.lower(),
        access_log=True,
        use_colors=True
    )