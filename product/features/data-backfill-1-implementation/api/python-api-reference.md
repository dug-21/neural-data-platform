# Python API Reference

## Overview

This document provides a comprehensive reference for the Python API used in the data backfill implementation.

## Core Classes

### FileProvider

The main class for reading market data from files.

```python
from data_ingestion.providers.file_provider import FileProvider

# Initialize provider
provider = FileProvider(
    base_path="/mnt/external/polygon_data",
    checkpoint_dir="/var/lib/neural_trader/checkpoints"
)
```

#### Methods

##### `__init__(base_path: str, checkpoint_dir: Optional[str] = None)`

Initialize the file provider.

**Parameters:**
- `base_path` (str): Base directory path for data files
- `checkpoint_dir` (str, optional): Directory to store checkpoint files

**Example:**
```python
provider = FileProvider(
    base_path="/mnt/data",
    checkpoint_dir="/tmp/checkpoints"
)
```

##### `async connect()`

Initialize provider connection and load checkpoints.

**Example:**
```python
await provider.connect()
```

##### `async disconnect()`

Clean up provider connection and save checkpoints.

**Example:**
```python
await provider.disconnect()
```

##### `async get_market_data(symbols: List[str], start_time: datetime, end_time: datetime, interval: str = "1min") -> AsyncIterator[MarketData]`

Fetch historical market data from files.

**Parameters:**
- `symbols` (List[str]): List of symbols to fetch
- `start_time` (datetime): Start time for data
- `end_time` (datetime): End time for data  
- `interval` (str): Data interval (default: "1min")

**Returns:**
- AsyncIterator[MarketData]: Stream of market data objects

**Example:**
```python
async for data in provider.get_market_data(
    symbols=["AAPL", "MSFT"],
    start_time=datetime(2023, 1, 1),
    end_time=datetime(2023, 12, 31)
):
    print(f"{data.symbol}: {data.close}")
```

##### `get_checkpoint_status() -> Dict[str, Any]`

Get current checkpoint status for monitoring.

**Returns:**
- Dict containing checkpoint statistics

**Example:**
```python
status = provider.get_checkpoint_status()
print(f"Active files: {status['active_files']}")
print(f"Completed: {status['completed_files']}")
```

##### `async clear_checkpoints(file_patterns: Optional[List[str]] = None)`

Clear checkpoints for specific files or all files.

**Parameters:**
- `file_patterns` (List[str], optional): File patterns to clear

**Example:**
```python
# Clear specific pattern
await provider.clear_checkpoints(["2023-01-*.csv"])

# Clear all
await provider.clear_checkpoints()
```

### FileBackfillHandler

Handler for backfilling historical data from files.

```python
from data_ingestion.utils.file_backfill import FileBackfillHandler

handler = FileBackfillHandler(
    path=Path("/mnt/external/data"),
    format='csv',
    symbols=['AAPL', 'MSFT'],
    start_date=datetime(2023, 1, 1),
    end_date=datetime(2023, 12, 31)
)
```

#### Methods

##### `__init__(path: Path, format: str = 'csv', symbols: Optional[List[str]] = None, ...)`

Initialize the file backfill handler.

**Parameters:**
- `path` (Path): Path to file or directory
- `format` (str): File format (csv, json, parquet)
- `symbols` (List[str], optional): Symbols to filter
- `start_date` (datetime, optional): Start date filter
- `end_date` (datetime, optional): End date filter
- `batch_size` (int): Records per batch (default: 1000)
- `use_checkpoint` (bool): Enable checkpointing (default: True)
- `dry_run` (bool): Preview mode (default: False)

##### `async run()`

Run the backfill process.

**Example:**
```python
handler = FileBackfillHandler(
    path=Path("/data"),
    format='csv',
    symbols=['AAPL']
)
await handler.run()
```

### PolygonS3Downloader

Handles downloading Polygon data from S3.

```python
from scripts.download_polygon_s3 import PolygonS3Downloader

downloader = PolygonS3Downloader(
    aws_profile="polygon-s3",
    external_drive_path="/mnt/external"
)
```

#### Methods

##### `download_batch(prefix: str, start_date: Optional[datetime] = None, ...)`

Download a batch of files from S3.

**Parameters:**
- `prefix` (str): S3 prefix to download from
- `start_date` (datetime, optional): Start date filter
- `end_date` (datetime, optional): End date filter
- `file_pattern` (str, optional): File pattern to match
- `max_files` (int, optional): Maximum files to download

**Example:**
```python
downloader.download_batch(
    prefix="us_stocks_sip/day_aggs_v1/",
    start_date=datetime(2023, 1, 1),
    end_date=datetime(2023, 12, 31),
    max_files=100
)
```

## Data Models

### MarketData

Market data model from `providers.base`.

```python
@dataclass
class MarketData:
    time: datetime
    symbol: str
    open: float
    high: float
    low: float
    close: float
    volume: int
    provider: str
    metadata: Optional[Dict[str, Any]] = None
```

**Validation:**
- OHLC consistency is automatically validated
- Timestamps must be timezone-aware

**Example:**
```python
data = MarketData(
    time=datetime.now(timezone.utc),
    symbol="AAPL",
    open=150.0,
    high=152.0,
    low=149.5,
    close=151.5,
    volume=1000000,
    provider="file_provider"
)
```

### FileCheckpoint

Checkpoint data for resuming file processing.

```python
@dataclass
class FileCheckpoint:
    file_path: str
    processed_lines: int
    last_timestamp: Optional[datetime]
    bad_records: int
    total_records: int
```

## Utility Functions

### Validation

```python
from data_ingestion.validation.data_quality import DataQualityValidator

validator = DataQualityValidator()

# Validate OHLC data
is_valid = validator.validate_ohlc(
    open=100.0,
    high=105.0,
    low=99.0,
    close=104.0
)

# Validate batch
results = validator.validate_batch(records)
print(f"Valid: {results['valid_count']}")
print(f"Invalid: {results['invalid_count']}")
```

### Rate Limiting

```python
from data_ingestion.utils.rate_limiter import RateLimiter

# Create rate limiter
limiter = RateLimiter(
    max_requests=100,
    time_window=60  # seconds
)

# Use with async functions
@limiter.limit
async def download_file(url):
    # Rate-limited function
    pass
```

### Retry Logic

```python
from data_ingestion.utils.retry import with_retry

@with_retry(
    max_attempts=3,
    backoff_factor=2,
    exceptions=(ConnectionError, TimeoutError)
)
async def unreliable_operation():
    # Operation that might fail
    pass
```

### Metrics

```python
from data_ingestion.utils.metrics import metrics

# Increment counter
metrics.data_points_processed.labels(
    provider='file_provider',
    data_type='market_data'
).inc()

# Record duration
with metrics.processing_duration.time():
    # Timed operation
    pass

# Set gauge
metrics.active_connections.set(10)
```

## Integration Examples

### Basic File Import

```python
import asyncio
from datetime import datetime
from data_ingestion.providers.file_provider import FileProvider

async def import_files():
    # Initialize provider
    provider = FileProvider(
        base_path="/mnt/external/polygon_data"
    )
    
    try:
        # Connect
        await provider.connect()
        
        # Import data
        count = 0
        async for data in provider.get_market_data(
            symbols=["AAPL"],
            start_time=datetime(2023, 1, 1),
            end_time=datetime(2023, 1, 31)
        ):
            count += 1
            if count % 1000 == 0:
                print(f"Processed {count} records")
        
        print(f"Total records: {count}")
        
    finally:
        # Disconnect
        await provider.disconnect()

# Run
asyncio.run(import_files())
```

### S3 Download and Import

```python
import asyncio
from pathlib import Path
from scripts.download_polygon_s3 import PolygonS3Downloader
from data_ingestion.utils.file_backfill import FileBackfillHandler

async def download_and_import():
    # Step 1: Download from S3
    downloader = PolygonS3Downloader(
        aws_profile="polygon-s3",
        external_drive_path="/mnt/external"
    )
    
    downloader.download_batch(
        prefix="us_stocks_sip/day_aggs_v1/2023/",
        max_files=10
    )
    
    # Step 2: Import downloaded files
    handler = FileBackfillHandler(
        path=Path("/mnt/external/polygon_data"),
        format='csv',
        batch_size=10000
    )
    
    await handler.run()

# Run
asyncio.run(download_and_import())
```

### Custom Processing Pipeline

```python
import asyncio
from datetime import datetime
from data_ingestion.providers.file_provider import FileProvider
from data_ingestion.processors.validator import DataValidator
from data_ingestion.storage.timescale import TimescaleDB

async def custom_pipeline():
    provider = FileProvider("/mnt/data")
    validator = DataValidator()
    storage = TimescaleDB()
    
    try:
        # Initialize components
        await provider.connect()
        await storage.connect()
        
        # Process with validation
        batch = []
        async for data in provider.get_market_data(
            symbols=["AAPL", "MSFT"],
            start_time=datetime(2023, 1, 1),
            end_time=datetime(2023, 12, 31)
        ):
            # Validate
            if validator.validate_market_data(data):
                batch.append(data)
            
            # Store in batches
            if len(batch) >= 1000:
                await storage.insert_market_data(batch)
                batch = []
        
        # Store remaining
        if batch:
            await storage.insert_market_data(batch)
            
    finally:
        await provider.disconnect()
        await storage.disconnect()

# Run
asyncio.run(custom_pipeline())
```

## Error Handling

### Custom Exceptions

```python
from data_ingestion.exceptions import (
    BackfillError,
    ValidationError,
    CheckpointError,
    RateLimitError
)

try:
    # Backfill operation
    await handler.run()
except ValidationError as e:
    print(f"Data validation failed: {e}")
except CheckpointError as e:
    print(f"Checkpoint error: {e}")
except BackfillError as e:
    print(f"Backfill failed: {e}")
```

### Error Recovery

```python
async def resilient_import():
    max_retries = 3
    
    for attempt in range(max_retries):
        try:
            await run_import()
            break
        except Exception as e:
            if attempt < max_retries - 1:
                print(f"Attempt {attempt + 1} failed: {e}")
                await asyncio.sleep(2 ** attempt)
            else:
                raise
```

## Testing

### Unit Testing

```python
import pytest
from unittest.mock import Mock, patch
from data_ingestion.providers.file_provider import FileProvider

@pytest.mark.asyncio
async def test_file_provider():
    # Mock file system
    with patch('pathlib.Path.exists', return_value=True):
        provider = FileProvider("/test/path")
        
        # Test initialization
        assert provider.base_path == Path("/test/path")
        
        # Test connection
        await provider.connect()
        assert provider.connected

@pytest.mark.asyncio
async def test_market_data_stream():
    provider = FileProvider("/test/path")
    
    # Mock file reading
    with patch.object(provider, '_process_file') as mock_process:
        mock_process.return_value = AsyncIterator([
            MarketData(...),
            MarketData(...)
        ])
        
        # Test streaming
        count = 0
        async for data in provider.get_market_data(
            symbols=["TEST"],
            start_time=datetime.now(),
            end_time=datetime.now()
        ):
            count += 1
        
        assert count == 2
```

### Integration Testing

```python
import pytest
from testcontainers.postgres import PostgresContainer
from testcontainers.redis import RedisContainer

@pytest.fixture
async def test_environment():
    # Start test containers
    with PostgresContainer() as postgres:
        with RedisContainer() as redis:
            yield {
                'postgres_url': postgres.get_connection_url(),
                'redis_url': f"redis://{redis.get_host()}:{redis.get_port()}"
            }

@pytest.mark.asyncio
async def test_full_pipeline(test_environment):
    # Run complete backfill test
    handler = FileBackfillHandler(
        path=Path("test/fixtures/data"),
        format='csv'
    )
    
    # Override connections with test containers
    handler.storage.connection_string = test_environment['postgres_url']
    handler.redis_store.url = test_environment['redis_url']
    
    # Run backfill
    await handler.run()
    
    # Verify results
    assert handler.stats['processed_files'] > 0
    assert handler.stats['errors'] == 0
```

## Performance Considerations

### Memory Management

```python
# Use streaming for large files
async def stream_large_file(file_path):
    async with aiofiles.open(file_path, 'r') as f:
        async for line in f:
            yield process_line(line)

# Clear memory periodically
import gc

async def process_with_gc():
    count = 0
    async for data in data_stream:
        process(data)
        count += 1
        
        # Force garbage collection every 100k records
        if count % 100000 == 0:
            gc.collect()
```

### Concurrent Processing

```python
import asyncio
from concurrent.futures import ProcessPoolExecutor

async def parallel_processing(files):
    # Use process pool for CPU-intensive work
    with ProcessPoolExecutor() as executor:
        loop = asyncio.get_event_loop()
        
        tasks = [
            loop.run_in_executor(executor, process_file, file)
            for file in files
        ]
        
        results = await asyncio.gather(*tasks)
        return results
```

## API Versioning

The API follows semantic versioning:

- **v1.0.0**: Initial release
- **v1.1.0**: Added checkpoint support
- **v1.2.0**: Added Parquet format support
- **v2.0.0**: Breaking changes in FileProvider interface

Check version:
```python
from data_ingestion import __version__
print(f"API Version: {__version__}")
```

## Deprecation Policy

Deprecated features are marked and maintained for 2 minor versions:

```python
import warnings

# Deprecated method
@deprecated("Use get_market_data instead")
def fetch_data(self, symbols):
    warnings.warn(
        "fetch_data is deprecated, use get_market_data",
        DeprecationWarning,
        stacklevel=2
    )
    return self.get_market_data(symbols)
```