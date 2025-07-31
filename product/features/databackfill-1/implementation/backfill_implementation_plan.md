# Historical Data Backfill Implementation Plan

## Overview
This document outlines the implementation strategy for backfilling historical market data from Polygon.io S3 flat files into the neural-trader database.

## Architecture Design

### 1. Python Script Structure

```
data_backfill/
├── __init__.py
├── main.py                 # Entry point and orchestrator
├── config.py              # Configuration management
├── downloader/
│   ├── __init__.py
│   ├── s3_client.py       # S3 operations wrapper
│   ├── concurrent_downloader.py  # Async download manager
│   └── retry_handler.py   # Retry logic for failed downloads
├── processor/
│   ├── __init__.py
│   ├── data_parser.py     # Parse downloaded data files
│   ├── batch_processor.py # Batch processing logic
│   └── validator.py       # Data validation
├── database/
│   ├── __init__.py
│   ├── connection_pool.py # Connection pooling
│   ├── bulk_inserter.py   # Bulk insert operations
│   └── schema.py          # Database schema definitions
├── monitoring/
│   ├── __init__.py
│   ├── progress_tracker.py # Progress tracking
│   ├── checkpoint.py      # Checkpoint system
│   └── metrics.py         # Performance metrics
└── utils/
    ├── __init__.py
    ├── logger.py          # Logging utilities
    └── exceptions.py      # Custom exceptions
```

### 2. Async/Concurrent Download Strategy

```python
# concurrent_downloader.py
import asyncio
import aiohttp
import aioboto3
from typing import List, Dict, AsyncIterator
from dataclasses import dataclass

@dataclass
class DownloadTask:
    symbol: str
    date: str
    s3_key: str
    local_path: str
    retry_count: int = 0
    max_retries: int = 3

class ConcurrentDownloader:
    def __init__(self, max_concurrent: int = 10):
        self.max_concurrent = max_concurrent
        self.semaphore = asyncio.Semaphore(max_concurrent)
        self.session = None
        self.s3_client = None
        
    async def download_batch(self, tasks: List[DownloadTask]) -> Dict[str, Any]:
        """Download multiple files concurrently with rate limiting"""
        async with aioboto3.Session().client('s3') as self.s3_client:
            results = await asyncio.gather(
                *[self._download_with_semaphore(task) for task in tasks],
                return_exceptions=True
            )
        return self._process_results(tasks, results)
    
    async def _download_with_semaphore(self, task: DownloadTask):
        async with self.semaphore:
            return await self._download_file(task)
```

### 3. Batch Processing System

```python
# batch_processor.py
from typing import List, Iterator, Optional
import pandas as pd
from concurrent.futures import ProcessPoolExecutor

class BatchProcessor:
    def __init__(self, batch_size: int = 10000, max_workers: int = 4):
        self.batch_size = batch_size
        self.max_workers = max_workers
        self.executor = ProcessPoolExecutor(max_workers=max_workers)
    
    def process_files(self, file_paths: List[str]) -> Iterator[pd.DataFrame]:
        """Process multiple files in parallel batches"""
        for chunk in self._chunk_files(file_paths, chunk_size=self.max_workers):
            futures = [self.executor.submit(self._process_single_file, fp) 
                      for fp in chunk]
            for future in futures:
                yield from self._batch_dataframe(future.result())
    
    def _batch_dataframe(self, df: pd.DataFrame) -> Iterator[pd.DataFrame]:
        """Yield dataframe in batches"""
        for start in range(0, len(df), self.batch_size):
            yield df.iloc[start:start + self.batch_size]
```

### 4. Database Connection Pooling

```python
# connection_pool.py
import asyncpg
from contextlib import asynccontextmanager
from typing import Optional

class DatabasePool:
    def __init__(self, dsn: str, min_size: int = 10, max_size: int = 20):
        self.dsn = dsn
        self.min_size = min_size
        self.max_size = max_size
        self._pool: Optional[asyncpg.Pool] = None
    
    async def init(self):
        """Initialize connection pool"""
        self._pool = await asyncpg.create_pool(
            self.dsn,
            min_size=self.min_size,
            max_size=self.max_size,
            command_timeout=60,
            server_settings={
                'application_name': 'neural-trader-backfill'
            }
        )
    
    @asynccontextmanager
    async def acquire(self):
        """Acquire connection from pool"""
        async with self._pool.acquire() as conn:
            yield conn
    
    async def execute_batch(self, query: str, data: List[tuple]):
        """Execute batch insert with COPY protocol"""
        async with self.acquire() as conn:
            await conn.copy_records_to_table(
                'market_data',
                records=data,
                columns=['symbol', 'timestamp', 'open', 'high', 'low', 
                        'close', 'volume', 'vwap', 'transactions']
            )
```

### 5. Bulk Insert Operations

```python
# bulk_inserter.py
import asyncio
from typing import List, Dict, Any
import asyncpg

class BulkInserter:
    def __init__(self, pool: DatabasePool, batch_size: int = 50000):
        self.pool = pool
        self.batch_size = batch_size
        self.buffer = []
        self.lock = asyncio.Lock()
    
    async def insert_batch(self, records: List[Dict[str, Any]]):
        """Insert batch of records using COPY protocol for performance"""
        # Convert records to tuples for COPY
        data = [
            (r['symbol'], r['timestamp'], r['open'], r['high'], 
             r['low'], r['close'], r['volume'], r['vwap'], r['transactions'])
            for r in records
        ]
        
        try:
            await self.pool.execute_batch(
                """
                INSERT INTO market_data 
                (symbol, timestamp, open, high, low, close, volume, vwap, transactions)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (symbol, timestamp) DO UPDATE SET
                    open = EXCLUDED.open,
                    high = EXCLUDED.high,
                    low = EXCLUDED.low,
                    close = EXCLUDED.close,
                    volume = EXCLUDED.volume,
                    vwap = EXCLUDED.vwap,
                    transactions = EXCLUDED.transactions
                """,
                data
            )
        except asyncpg.exceptions.UniqueViolationError:
            # Handle duplicates gracefully
            await self._handle_duplicates(data)
```

### 6. Progress Tracking and Checkpoints

```python
# progress_tracker.py
import json
from datetime import datetime
from typing import Dict, List, Optional
import aiofiles

class ProgressTracker:
    def __init__(self, checkpoint_file: str = 'backfill_checkpoint.json'):
        self.checkpoint_file = checkpoint_file
        self.progress = {
            'started_at': None,
            'last_updated': None,
            'symbols_completed': [],
            'symbols_in_progress': {},
            'total_records': 0,
            'failed_downloads': [],
            'metrics': {
                'download_speed_mbps': 0,
                'insert_rate_per_sec': 0,
                'error_rate': 0
            }
        }
    
    async def load_checkpoint(self) -> Dict:
        """Load progress from checkpoint file"""
        try:
            async with aiofiles.open(self.checkpoint_file, 'r') as f:
                content = await f.read()
                self.progress = json.loads(content)
        except FileNotFoundError:
            self.progress['started_at'] = datetime.utcnow().isoformat()
        return self.progress
    
    async def save_checkpoint(self):
        """Save current progress"""
        self.progress['last_updated'] = datetime.utcnow().isoformat()
        async with aiofiles.open(self.checkpoint_file, 'w') as f:
            await f.write(json.dumps(self.progress, indent=2))
    
    def mark_symbol_complete(self, symbol: str, records: int):
        """Mark a symbol as completed"""
        self.progress['symbols_completed'].append(symbol)
        self.progress['total_records'] += records
        if symbol in self.progress['symbols_in_progress']:
            del self.progress['symbols_in_progress'][symbol]
```

### 7. Error Handling and Retry Mechanisms

```python
# retry_handler.py
import asyncio
from typing import Callable, Any, Optional
from functools import wraps
import logging

class RetryHandler:
    def __init__(self, max_retries: int = 3, backoff_factor: float = 2.0):
        self.max_retries = max_retries
        self.backoff_factor = backoff_factor
        self.logger = logging.getLogger(__name__)
    
    def with_retry(self, exceptions=(Exception,)):
        """Decorator for retry logic"""
        def decorator(func: Callable) -> Callable:
            @wraps(func)
            async def wrapper(*args, **kwargs) -> Any:
                last_exception = None
                for attempt in range(self.max_retries):
                    try:
                        return await func(*args, **kwargs)
                    except exceptions as e:
                        last_exception = e
                        wait_time = self.backoff_factor ** attempt
                        self.logger.warning(
                            f"Attempt {attempt + 1} failed: {e}. "
                            f"Retrying in {wait_time}s..."
                        )
                        await asyncio.sleep(wait_time)
                
                self.logger.error(f"All retries exhausted. Last error: {last_exception}")
                raise last_exception
            return wrapper
        return decorator
```

## API Interfaces

### Main Entry Point
```python
# main.py
async def backfill_historical_data(
    symbols: List[str],
    start_date: str,
    end_date: str,
    concurrent_downloads: int = 10,
    batch_size: int = 50000,
    checkpoint_enabled: bool = True
) -> Dict[str, Any]:
    """
    Main entry point for historical data backfill
    
    Args:
        symbols: List of stock symbols to backfill
        start_date: Start date in YYYY-MM-DD format
        end_date: End date in YYYY-MM-DD format
        concurrent_downloads: Max concurrent S3 downloads
        batch_size: Records per database batch insert
        checkpoint_enabled: Enable checkpoint/resume functionality
    
    Returns:
        Dictionary with backfill statistics and results
    """
```

### Data Models

```python
# schema.py
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal

@dataclass
class MarketDataRecord:
    symbol: str
    timestamp: datetime
    open: Decimal
    high: Decimal
    low: Decimal
    close: Decimal
    volume: int
    vwap: Decimal
    transactions: int
    
@dataclass
class BackfillJob:
    job_id: str
    symbols: List[str]
    start_date: datetime
    end_date: datetime
    status: str  # 'pending', 'running', 'completed', 'failed'
    created_at: datetime
    updated_at: datetime
    progress_pct: float
    records_processed: int
    errors: List[Dict[str, Any]]
```

## Performance Optimizations

1. **Concurrent Downloads**: Use asyncio with semaphore to limit concurrent S3 downloads
2. **Batch Processing**: Process files in chunks to optimize memory usage
3. **Connection Pooling**: Maintain persistent database connections
4. **Bulk Inserts**: Use PostgreSQL COPY protocol for fastest inserts
5. **Parallel Processing**: Use multiprocessing for CPU-intensive parsing
6. **Memory Management**: Stream large files instead of loading entirely
7. **Checkpoint System**: Enable resume capability for long-running jobs

## Error Recovery

1. **Automatic Retries**: Exponential backoff for transient failures
2. **Checkpoint Resume**: Continue from last successful position
3. **Failed Item Tracking**: Log and retry failed downloads/inserts
4. **Circuit Breaker**: Prevent cascading failures
5. **Dead Letter Queue**: Store persistently failing items

## Monitoring and Metrics

1. **Progress Dashboard**: Real-time progress visualization
2. **Performance Metrics**: Download speed, insert rate, error rate
3. **Resource Monitoring**: CPU, memory, network, disk I/O
4. **Alert System**: Notify on failures or performance degradation
5. **Audit Trail**: Complete log of all operations

## Next Steps

1. Implement core modules following this design
2. Create unit tests for each component
3. Build integration tests for end-to-end flow
4. Create deployment scripts and configuration
5. Document usage and maintenance procedures