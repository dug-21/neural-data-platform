# Database Bulk Operations Design

## Overview
High-performance database operations optimized for bulk historical data insertion.

## PostgreSQL Optimizations

### 1. Connection Pool Manager
```python
import asyncpg
from typing import Optional, Dict, Any, List
import asyncio
from contextlib import asynccontextmanager
import logging

class OptimizedDatabasePool:
    """
    High-performance connection pool with optimizations for bulk operations
    """
    def __init__(
        self,
        dsn: str,
        min_connections: int = 20,
        max_connections: int = 50,
        command_timeout: int = 300,  # 5 minutes for bulk ops
        setup_queries: Optional[List[str]] = None
    ):
        self.dsn = dsn
        self.min_connections = min_connections
        self.max_connections = max_connections
        self.command_timeout = command_timeout
        self.setup_queries = setup_queries or [
            # Optimize for bulk inserts
            "SET synchronous_commit = OFF",
            "SET work_mem = '256MB'",
            "SET maintenance_work_mem = '1GB'",
            "SET temp_buffers = '512MB'",
            "SET checkpoint_completion_target = 0.9",
            "SET wal_buffers = '16MB'",
            "SET shared_buffers = '2GB'",
            "SET effective_cache_size = '8GB'"
        ]
        self._pool: Optional[asyncpg.Pool] = None
        self.logger = logging.getLogger(__name__)
        
    async def initialize(self):
        """Initialize connection pool with optimizations"""
        self._pool = await asyncpg.create_pool(
            self.dsn,
            min_size=self.min_connections,
            max_size=self.max_connections,
            command_timeout=self.command_timeout,
            init=self._init_connection
        )
        
        # Pre-warm connections
        await self._prewarm_pool()
        
    async def _init_connection(self, conn):
        """Initialize each connection with optimization settings"""
        for query in self.setup_queries:
            try:
                await conn.execute(query)
            except Exception as e:
                self.logger.warning(f"Failed to execute setup query '{query}': {e}")
                
    async def _prewarm_pool(self):
        """Pre-warm all connections for immediate availability"""
        tasks = []
        for _ in range(self.min_connections):
            tasks.append(self._prewarm_single())
        await asyncio.gather(*tasks)
        
    async def _prewarm_single(self):
        async with self._pool.acquire() as conn:
            await conn.fetchval("SELECT 1")
```

### 2. Bulk Insert Engine
```python
import asyncpg
from typing import List, Dict, Any, Optional, TypeVar, Generic
from datetime import datetime
import numpy as np
from dataclasses import dataclass, fields

T = TypeVar('T')

@dataclass
class BulkInsertConfig:
    table_name: str
    columns: List[str]
    conflict_columns: List[str]
    update_columns: Optional[List[str]] = None
    batch_size: int = 100_000
    use_copy: bool = True  # Use COPY for maximum performance
    
class BulkInsertEngine(Generic[T]):
    """
    Ultra-fast bulk insert engine using PostgreSQL COPY protocol
    """
    def __init__(
        self,
        pool: OptimizedDatabasePool,
        config: BulkInsertConfig
    ):
        self.pool = pool
        self.config = config
        self.buffer: List[T] = []
        self.total_inserted = 0
        
    async def insert_records(self, records: List[T]) -> Dict[str, Any]:
        """Insert records using optimal method"""
        if self.config.use_copy and len(records) > 1000:
            return await self._copy_insert(records)
        else:
            return await self._batch_insert(records)
            
    async def _copy_insert(self, records: List[T]) -> Dict[str, Any]:
        """Use COPY protocol for maximum speed"""
        start_time = datetime.utcnow()
        
        async with self.pool._pool.acquire() as conn:
            # Create temporary table for conflict handling
            temp_table = f"temp_{self.config.table_name}_{int(datetime.now().timestamp())}"
            
            try:
                # Create temp table with same structure
                await conn.execute(f"""
                    CREATE TEMP TABLE {temp_table} 
                    (LIKE {self.config.table_name} INCLUDING ALL)
                    ON COMMIT DROP
                """)
                
                # Copy data to temp table (fastest method)
                result = await conn.copy_records_to_table(
                    temp_table,
                    records=self._records_to_tuples(records),
                    columns=self.config.columns
                )
                
                # Merge from temp table with conflict handling
                merge_query = self._build_merge_query(temp_table)
                merge_result = await conn.execute(merge_query)
                
                # Extract insert count
                inserted = int(merge_result.split()[-1])
                self.total_inserted += inserted
                
                duration = (datetime.utcnow() - start_time).total_seconds()
                
                return {
                    'method': 'COPY',
                    'records_processed': len(records),
                    'records_inserted': inserted,
                    'duration_seconds': duration,
                    'records_per_second': len(records) / duration if duration > 0 else 0,
                    'total_inserted': self.total_inserted
                }
                
            except Exception as e:
                self.logger.error(f"COPY insert failed: {e}")
                # Fallback to batch insert
                return await self._batch_insert(records)
                
    async def _batch_insert(self, records: List[T]) -> Dict[str, Any]:
        """Fallback batch insert method"""
        start_time = datetime.utcnow()
        inserted = 0
        
        # Process in smaller batches
        for i in range(0, len(records), self.config.batch_size):
            batch = records[i:i + self.config.batch_size]
            
            query = self._build_insert_query()
            values = [self._record_to_values(r) for r in batch]
            
            async with self.pool._pool.acquire() as conn:
                result = await conn.executemany(query, values)
                # Count actual inserts (not updates)
                batch_inserted = sum(1 for r in result if 'INSERT' in r)
                inserted += batch_inserted
                
        self.total_inserted += inserted
        duration = (datetime.utcnow() - start_time).total_seconds()
        
        return {
            'method': 'BATCH',
            'records_processed': len(records),
            'records_inserted': inserted,
            'duration_seconds': duration,
            'records_per_second': len(records) / duration if duration > 0 else 0,
            'total_inserted': self.total_inserted
        }
        
    def _build_merge_query(self, temp_table: str) -> str:
        """Build efficient MERGE query"""
        conflict_cols = ', '.join(self.config.conflict_columns)
        
        if self.config.update_columns:
            # Update specific columns on conflict
            update_set = ', '.join([
                f"{col} = EXCLUDED.{col}" 
                for col in self.config.update_columns
            ])
        else:
            # Update all non-conflict columns
            update_set = ', '.join([
                f"{col} = EXCLUDED.{col}" 
                for col in self.config.columns 
                if col not in self.config.conflict_columns
            ])
            
        return f"""
            INSERT INTO {self.config.table_name} ({', '.join(self.config.columns)})
            SELECT {', '.join(self.config.columns)} FROM {temp_table}
            ON CONFLICT ({conflict_cols}) 
            DO UPDATE SET {update_set}
        """
```

### 3. Partitioned Table Handler
```python
class PartitionedTableManager:
    """
    Handle partitioned tables for better performance with time-series data
    """
    def __init__(self, pool: OptimizedDatabasePool):
        self.pool = pool
        
    async def ensure_partition_exists(
        self,
        base_table: str,
        partition_date: datetime
    ) -> str:
        """Create partition if it doesn't exist"""
        partition_name = f"{base_table}_{partition_date.strftime('%Y_%m')}"
        
        async with self.pool._pool.acquire() as conn:
            # Check if partition exists
            exists = await conn.fetchval(f"""
                SELECT EXISTS (
                    SELECT 1 FROM pg_tables 
                    WHERE tablename = $1
                )
            """, partition_name)
            
            if not exists:
                # Create partition
                start_date = partition_date.replace(day=1)
                end_date = (start_date + timedelta(days=32)).replace(day=1)
                
                await conn.execute(f"""
                    CREATE TABLE {partition_name} 
                    PARTITION OF {base_table}
                    FOR VALUES FROM ('{start_date}') TO ('{end_date}')
                """)
                
                # Create indexes on partition
                await self._create_partition_indexes(conn, partition_name)
                
        return partition_name
        
    async def _create_partition_indexes(self, conn, partition_name: str):
        """Create optimized indexes for partition"""
        indexes = [
            f"CREATE INDEX idx_{partition_name}_symbol_ts ON {partition_name} (symbol, timestamp)",
            f"CREATE INDEX idx_{partition_name}_ts ON {partition_name} (timestamp)",
            f"CREATE INDEX idx_{partition_name}_symbol ON {partition_name} (symbol) WHERE volume > 0"
        ]
        
        for idx_query in indexes:
            try:
                await conn.execute(idx_query)
            except asyncpg.exceptions.DuplicateTableError:
                pass  # Index already exists
```

### 4. Performance Monitor
```python
class DatabasePerformanceMonitor:
    """Monitor and optimize database performance during bulk operations"""
    
    def __init__(self, pool: OptimizedDatabasePool):
        self.pool = pool
        self.metrics: Dict[str, List[float]] = {
            'insert_rate': [],
            'connection_wait_time': [],
            'query_time': [],
            'table_size_gb': [],
            'index_size_gb': []
        }
        
    async def analyze_performance(self) -> Dict[str, Any]:
        """Analyze current performance metrics"""
        async with self.pool._pool.acquire() as conn:
            # Table statistics
            table_stats = await conn.fetch("""
                SELECT 
                    schemaname,
                    tablename,
                    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as total_size,
                    n_tup_ins as inserts,
                    n_tup_upd as updates,
                    n_tup_del as deletes,
                    n_live_tup as live_tuples,
                    n_dead_tup as dead_tuples,
                    last_vacuum,
                    last_autovacuum
                FROM pg_stat_user_tables
                WHERE schemaname = 'public'
                ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
            """)
            
            # Connection pool stats
            pool_stats = self.pool._pool.get_stats()
            
            # Slow queries
            slow_queries = await conn.fetch("""
                SELECT 
                    query,
                    calls,
                    total_time,
                    mean_time,
                    max_time
                FROM pg_stat_statements
                WHERE query LIKE '%market_data%'
                ORDER BY mean_time DESC
                LIMIT 10
            """)
            
            return {
                'table_stats': [dict(row) for row in table_stats],
                'pool_stats': pool_stats,
                'slow_queries': [dict(row) for row in slow_queries],
                'recommendations': self._generate_recommendations(table_stats, pool_stats)
            }
            
    def _generate_recommendations(
        self,
        table_stats: List[asyncpg.Record],
        pool_stats: Dict
    ) -> List[str]:
        """Generate performance recommendations"""
        recommendations = []
        
        # Check for bloat
        for table in table_stats:
            dead_ratio = table['n_dead_tup'] / (table['n_live_tup'] + 1)
            if dead_ratio > 0.2:
                recommendations.append(
                    f"Table {table['tablename']} has high bloat ({dead_ratio:.1%}). "
                    f"Consider running VACUUM ANALYZE."
                )
                
        # Check connection pool
        if pool_stats['free_size'] < 5:
            recommendations.append(
                "Connection pool is near capacity. Consider increasing max_connections."
            )
            
        return recommendations
```

### 5. Transaction Manager
```python
class BulkTransactionManager:
    """Manage transactions for bulk operations with safety and performance"""
    
    def __init__(self, pool: OptimizedDatabasePool):
        self.pool = pool
        self.savepoints: List[str] = []
        
    @asynccontextmanager
    async def bulk_transaction(
        self,
        isolation_level: str = 'READ COMMITTED',
        deferrable: bool = True
    ):
        """Context manager for bulk transactions"""
        async with self.pool._pool.acquire() as conn:
            async with conn.transaction(
                isolation=isolation_level,
                deferrable=deferrable
            ):
                try:
                    # Disable triggers for bulk insert (if safe)
                    await conn.execute("SET session_replication_role = 'replica'")
                    
                    yield conn
                    
                    # Re-enable triggers
                    await conn.execute("SET session_replication_role = 'origin'")
                    
                except Exception as e:
                    # Automatic rollback happens here
                    raise
                    
    async def create_savepoint(self, name: str) -> str:
        """Create savepoint for partial rollback capability"""
        async with self.pool._pool.acquire() as conn:
            await conn.execute(f"SAVEPOINT {name}")
            self.savepoints.append(name)
            return name
            
    async def rollback_to_savepoint(self, name: str):
        """Rollback to specific savepoint"""
        async with self.pool._pool.acquire() as conn:
            await conn.execute(f"ROLLBACK TO SAVEPOINT {name}")
            # Remove rolled back savepoints
            idx = self.savepoints.index(name)
            self.savepoints = self.savepoints[:idx]
```

## Integration Example

```python
# Complete bulk insert pipeline
async def perform_bulk_insert(data: List[MarketDataRecord]):
    # Initialize components
    pool = OptimizedDatabasePool(DATABASE_URL)
    await pool.initialize()
    
    # Configure bulk insert
    config = BulkInsertConfig(
        table_name='market_data',
        columns=['symbol', 'timestamp', 'open', 'high', 'low', 'close', 'volume', 'vwap'],
        conflict_columns=['symbol', 'timestamp'],
        update_columns=['open', 'high', 'low', 'close', 'volume', 'vwap'],
        batch_size=100_000,
        use_copy=True
    )
    
    # Create insert engine
    engine = BulkInsertEngine[MarketDataRecord](pool, config)
    
    # Monitor performance
    monitor = DatabasePerformanceMonitor(pool)
    
    # Perform insert
    async with BulkTransactionManager(pool).bulk_transaction():
        result = await engine.insert_records(data)
        
        # Analyze performance
        perf_stats = await monitor.analyze_performance()
        
        return {
            'insert_result': result,
            'performance': perf_stats
        }
```

## Performance Tips

1. **Disable Indexes**: Drop indexes before bulk insert, recreate after
2. **Partition Tables**: Use time-based partitioning for historical data
3. **Parallel Insert**: Use multiple connections for different symbol groups
4. **COPY Protocol**: Always prefer COPY over INSERT for large datasets
5. **Checkpoint Tuning**: Increase checkpoint intervals during bulk operations
6. **WAL Archiving**: Consider disabling during initial load
7. **Autovacuum**: Tune or disable during bulk operations
8. **Memory Settings**: Increase work_mem and maintenance_work_mem