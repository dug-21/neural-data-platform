"""TimescaleDB storage implementation."""
import asyncio
import asyncpg
from typing import List, Dict, Any, Optional
from datetime import datetime
import pandas as pd
from contextlib import asynccontextmanager

from config import get_settings
from utils.logging import get_logger
from utils.metrics import metrics
from utils.retry import with_retry

logger = get_logger(__name__)


class TimescaleDB:
    """TimescaleDB storage manager for time-series data."""
    
    def __init__(self):
        self.settings = get_settings()
        self.pool: Optional[asyncpg.Pool] = None
    
    @with_retry(max_attempts=10, exceptions=(asyncpg.PostgresError, ConnectionRefusedError, OSError))
    async def connect(self):
        """Create connection pool to TimescaleDB with retry logic."""
        try:
            logger.info("Attempting to connect to TimescaleDB...")
            self.pool = await asyncpg.create_pool(
                self.settings.timescale_url,
                min_size=1,
                max_size=10,
                command_timeout=60,
                ssl=False  # Disable SSL for internal Docker network
            )
            await self._initialize_schema()
            logger.info("Connected to TimescaleDB successfully")
            metrics.active_connections.labels(connection_type="timescale").inc()
            
            # Update connection pool metrics
            pool_stats = self.pool.get_size()
            metrics.update_db_connection_pool(
                "timescale",
                active=pool_stats - self.pool.get_idle_size(),
                idle=self.pool.get_idle_size(),
                total=pool_stats
            )
        except Exception as e:
            logger.error("Failed to connect to TimescaleDB", error=str(e))
            raise
    
    async def disconnect(self):
        """Close connection pool."""
        if self.pool:
            await self.pool.close()
            metrics.active_connections.labels(connection_type="timescale").dec()
            logger.info("Disconnected from TimescaleDB")
    
    @asynccontextmanager
    async def acquire(self):
        """Acquire a connection from the pool."""
        async with self.pool.acquire() as connection:
            yield connection
    
    async def _initialize_schema(self):
        """Initialize database schema and hypertables."""
        async with self.acquire() as conn:
            # Create main tables with light constraints
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS market_data (
                    time TIMESTAMPTZ NOT NULL,
                    symbol VARCHAR(10) NOT NULL,
                    open DECIMAL(10, 4) NOT NULL CHECK (open > 0),
                    high DECIMAL(10, 4) NOT NULL CHECK (high > 0),
                    low DECIMAL(10, 4) NOT NULL CHECK (low > 0),
                    close DECIMAL(10, 4) NOT NULL CHECK (close > 0),
                    volume BIGINT NOT NULL CHECK (volume >= 0),
                    provider VARCHAR(50) NOT NULL,
                    metadata JSONB,
                    -- Ensure OHLC consistency
                    CONSTRAINT check_high_low CHECK (high >= low),
                    CONSTRAINT check_ohlc_range CHECK (
                        high >= open AND high >= close AND
                        low <= open AND low <= close
                    ),
                    -- Composite primary key to prevent duplicates
                    PRIMARY KEY (time, symbol, provider)
                );
                
                -- Convert to hypertable if not already
                SELECT create_hypertable('market_data', 'time', if_not_exists => TRUE);
                
                -- Create index for symbol queries
                CREATE INDEX IF NOT EXISTS idx_market_data_symbol_time 
                ON market_data (symbol, time DESC);
                
                -- Create index for provider queries
                CREATE INDEX IF NOT EXISTS idx_market_data_provider 
                ON market_data (provider, time DESC);
            """)
            
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS tick_data (
                    time TIMESTAMPTZ NOT NULL,
                    symbol VARCHAR(10) NOT NULL,
                    price DECIMAL(10, 4) NOT NULL CHECK (price > 0),
                    size BIGINT NOT NULL CHECK (size > 0),
                    exchange VARCHAR(10),
                    conditions TEXT,
                    provider VARCHAR(50) NOT NULL,
                    PRIMARY KEY (time, symbol, provider)
                );
                
                SELECT create_hypertable('tick_data', 'time', if_not_exists => TRUE);
                
                CREATE INDEX IF NOT EXISTS idx_tick_data_symbol_time 
                ON tick_data (symbol, time DESC);
            """)
            
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS order_book (
                    time TIMESTAMPTZ NOT NULL,
                    symbol VARCHAR(10) NOT NULL,
                    bid_price DECIMAL(10, 4) NOT NULL CHECK (bid_price > 0),
                    bid_size BIGINT NOT NULL CHECK (bid_size >= 0),
                    ask_price DECIMAL(10, 4) NOT NULL CHECK (ask_price > 0),
                    ask_size BIGINT NOT NULL CHECK (ask_size >= 0),
                    mid_price DECIMAL(10, 4) NOT NULL CHECK (mid_price > 0),
                    spread DECIMAL(10, 4) NOT NULL CHECK (spread >= 0),
                    provider VARCHAR(50) NOT NULL,
                    -- Ensure bid < ask
                    CONSTRAINT check_bid_ask CHECK (bid_price < ask_price),
                    PRIMARY KEY (time, symbol, provider)
                );
                
                SELECT create_hypertable('order_book', 'time', if_not_exists => TRUE);
                
                CREATE INDEX IF NOT EXISTS idx_order_book_symbol_time 
                ON order_book (symbol, time DESC);
            """)
            
            await conn.execute("""
                CREATE TABLE IF NOT EXISTS technical_indicators (
                    time TIMESTAMPTZ NOT NULL,
                    symbol VARCHAR(10) NOT NULL,
                    indicator VARCHAR(50) NOT NULL,
                    value DECIMAL(20, 6),
                    timeframe VARCHAR(10),
                    parameters JSONB
                );
                
                SELECT create_hypertable('technical_indicators', 'time', if_not_exists => TRUE);
                
                CREATE INDEX IF NOT EXISTS idx_technical_indicators_symbol_indicator 
                ON technical_indicators (symbol, indicator, time DESC);
            """)
            
            logger.info("Database schema initialized")
    
    @metrics.track_db_write("market_data", "insert")
    @metrics.track_storage_operation("timescale", "insert_market_data")
    @with_retry(max_attempts=3, exceptions=(asyncpg.PostgresError,))
    async def insert_market_data(self, data: List[Dict[str, Any]]):
        """Insert market data into TimescaleDB."""
        if not data:
            return
        
        async with self.acquire() as conn:
            # Prepare data for insertion
            records = [
                (
                    d['time'],
                    d['symbol'],
                    d.get('open'),
                    d.get('high'),
                    d.get('low'),
                    d.get('close'),
                    d.get('volume'),
                    d.get('provider', 'unknown')
                )
                for d in data
            ]
            
            # Bulk insert with ON CONFLICT handling
            await conn.executemany("""
                INSERT INTO market_data (time, symbol, open, high, low, close, volume, provider)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (time, symbol, provider) DO UPDATE
                SET open = EXCLUDED.open,
                    high = EXCLUDED.high,
                    low = EXCLUDED.low,
                    close = EXCLUDED.close,
                    volume = EXCLUDED.volume
            """, records)
            
            metrics.data_points_processed.labels(
                provider=data[0].get('provider', 'unknown'),
                data_type='market_data'
            ).inc(len(records))
            
            logger.info(f"Inserted {len(records)} market data records")
    
    @metrics.track_db_write("tick_data", "insert")
    @metrics.track_storage_operation("timescale", "insert_tick_data")
    @with_retry(max_attempts=3, exceptions=(asyncpg.PostgresError,))
    async def insert_tick_data(self, data: List[Dict[str, Any]]):
        """Insert tick data into TimescaleDB."""
        if not data:
            return
        
        async with self.acquire() as conn:
            records = [
                (
                    d['time'],
                    d['symbol'],
                    d['price'],
                    d.get('size'),
                    d.get('exchange'),
                    d.get('conditions'),
                    d.get('provider', 'unknown')
                )
                for d in data
            ]
            
            await conn.executemany("""
                INSERT INTO tick_data (time, symbol, price, size, exchange, conditions, provider)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            """, records)
            
            metrics.data_points_processed.labels(
                provider=data[0].get('provider', 'unknown'),
                data_type='tick_data'
            ).inc(len(records))
            
            logger.info(f"Inserted {len(records)} tick data records")
    
    @metrics.track_storage_operation("timescale", "query")
    async def query_market_data(
        self,
        symbol: str,
        start_time: datetime,
        end_time: datetime,
        provider: Optional[str] = None
    ) -> pd.DataFrame:
        """Query market data from TimescaleDB."""
        async with self.acquire() as conn:
            query = """
                SELECT time, symbol, open, high, low, close, volume, provider
                FROM market_data
                WHERE symbol = $1 AND time >= $2 AND time <= $3
            """
            params = [symbol, start_time, end_time]
            
            if provider:
                query += " AND provider = $4"
                params.append(provider)
            
            query += " ORDER BY time"
            
            rows = await conn.fetch(query, *params)
            
            # Convert to pandas DataFrame
            df = pd.DataFrame(rows)
            if not df.empty:
                df['time'] = pd.to_datetime(df['time'])
                df.set_index('time', inplace=True)
            
            return df
    
    async def get_latest_price(self, symbol: str) -> Optional[Dict[str, Any]]:
        """Get the latest price for a symbol."""
        async with self.acquire() as conn:
            row = await conn.fetchrow("""
                SELECT time, close as price, volume
                FROM market_data
                WHERE symbol = $1
                ORDER BY time DESC
                LIMIT 1
            """, symbol)
            
            if row:
                return dict(row)
            return None
    
    async def create_compression_policy(self, table: str, interval: str = '7 days'):
        """Create compression policy for old data."""
        async with self.acquire() as conn:
            await conn.execute(f"""
                SELECT add_compression_policy('{table}', INTERVAL '{interval}');
            """)
            logger.info(f"Created compression policy for {table}")
    
    async def create_retention_policy(self, table: str, interval: str = '1 year'):
        """Create data retention policy."""
        async with self.acquire() as conn:
            await conn.execute(f"""
                SELECT add_retention_policy('{table}', INTERVAL '{interval}');
            """)
            logger.info(f"Created retention policy for {table}")